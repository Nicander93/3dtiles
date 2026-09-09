#include "DatasetLoader.h"

#include <osgDB/ReadFile>
#include <osgDB/Registry>
#include <osg/PagedLOD>

#include <fstream>
#include <sstream>
#include <algorithm>
#include <iostream>
#include <cstdlib>

namespace fs = std::filesystem;

namespace {

void ensureOsgPluginPath() {
#ifdef OSGB_VIEWER_CONDA_PREFIX
  const char* conda = OSGB_VIEWER_CONDA_PREFIX;
  if (conda && *conda) {
    const std::string pluginDir = std::string(conda) + "/lib/osgPlugins-3.6.5";
    // Prepend so conda plugins win (colon-separated on Unix).
    auto& paths = osgDB::Registry::instance()->getLibraryFilePathList();
    paths.insert(paths.begin(), pluginDir);
  }
#endif
  if (const char* env = std::getenv("OSG_LIBRARY_PATH")) {
    (void)env; // already honored by osgDB
  }
}

bool looksLikeTileDir(const fs::path& p) {
  const auto name = p.filename().string();
  return name.rfind("Tile_", 0) == 0;
}

fs::path findRootOsgb(const fs::path& tileDir) {
  const auto expected = tileDir / (tileDir.filename().string() + ".osgb");
  if (fs::is_regular_file(expected)) {
    return expected;
  }
  // Fallback: first .osgb that does not contain _L (LOD child naming).
  std::vector<fs::path> candidates;
  for (const auto& entry : fs::directory_iterator(tileDir)) {
    if (!entry.is_regular_file()) continue;
    if (entry.path().extension() != ".osgb") continue;
    candidates.push_back(entry.path());
  }
  std::sort(candidates.begin(), candidates.end());
  for (const auto& c : candidates) {
    const auto stem = c.stem().string();
    if (stem.find("_L") == std::string::npos) {
      return c;
    }
  }
  return candidates.empty() ? fs::path{} : candidates.front();
}

}  // namespace

int DatasetLoader::countOsgbFiles(const fs::path& dataDir) {
  if (!fs::exists(dataDir)) return 0;
  int n = 0;
  for (auto it = fs::recursive_directory_iterator(dataDir);
       it != fs::recursive_directory_iterator(); ++it) {
    if (it->is_regular_file() && it->path().extension() == ".osgb") {
      ++n;
    }
  }
  return n;
}

std::string DatasetLoader::readMetadataSummary(const fs::path& metadataXml) {
  if (!fs::is_regular_file(metadataXml)) {
    return "(no metadata.xml)";
  }
  std::ifstream in(metadataXml);
  if (!in) return "(failed to read metadata.xml)";
  std::ostringstream oss;
  oss << in.rdbuf();
  const std::string text = oss.str();

  auto extract = [&](const char* tag) -> std::string {
    const std::string open = std::string("<") + tag + ">";
    const std::string close = std::string("</") + tag + ">";
    auto a = text.find(open);
    auto b = text.find(close);
    if (a == std::string::npos || b == std::string::npos || b <= a) return {};
    a += open.size();
    return text.substr(a, b - a);
  };

  const auto srs = extract("SRS");
  const auto origin = extract("SRSOrigin");
  std::ostringstream summary;
  if (!srs.empty()) summary << "SRS=" << srs;
  if (!origin.empty()) {
    if (!srs.empty()) summary << " ";
    summary << "origin=" << origin;
  }
  if (summary.str().empty()) {
    return "metadata.xml present";
  }
  return summary.str();
}

DatasetLoadResult DatasetLoader::loadDirectory(const fs::path& dir) {
  DatasetLoadResult result;
  result.datasetPath = fs::weakly_canonical(dir);
  ensureOsgPluginPath();

  fs::path dataDir = result.datasetPath / "Data";
  if (!fs::is_directory(dataDir)) {
    // Allow passing Data/ itself or a single Tile_* folder.
    if (looksLikeTileDir(result.datasetPath) && fs::is_directory(result.datasetPath)) {
      dataDir = result.datasetPath.parent_path();
      result.datasetPath = dataDir.parent_path();
    } else if (result.datasetPath.filename() == "Data" && fs::is_directory(result.datasetPath)) {
      dataDir = result.datasetPath;
      result.datasetPath = dataDir.parent_path();
    } else {
      result.errors.push_back("Missing Data/ directory under " + dir.string());
      return result;
    }
  }

  result.dataDir = dataDir;
  result.metadataPath = result.datasetPath / "metadata.xml";
  result.metadataSummary = readMetadataSummary(result.metadataPath);
  result.estimatedTileFileCount = countOsgbFiles(dataDir);

  std::vector<fs::path> tileDirs;
  for (const auto& entry : fs::directory_iterator(dataDir)) {
    if (entry.is_directory() && looksLikeTileDir(entry.path())) {
      tileDirs.push_back(entry.path());
    }
  }
  std::sort(tileDirs.begin(), tileDirs.end());

  if (tileDirs.empty()) {
    result.errors.push_back("No Tile_* directories found under " + dataDir.string());
    return result;
  }

  auto group = new osg::Group;
  group->setName("OSGB_Dataset");

  // Options: keep DatabasePath relative so PagedLOD child pages resolve.
  osg::ref_ptr<osgDB::Options> options = new osgDB::Options;
  options->setObjectCacheHint(osgDB::Options::CACHE_NONE);

  for (const auto& tileDir : tileDirs) {
    const fs::path rootOsgb = findRootOsgb(tileDir);
    if (rootOsgb.empty()) {
      result.errors.push_back("No root .osgb in " + tileDir.string());
      continue;
    }
    result.rootTiles.push_back(rootOsgb);

    osg::ref_ptr<osg::Node> node = osgDB::readNodeFile(rootOsgb.string(), options.get());
    if (!node) {
      result.errors.push_back("osgDB::readNodeFile failed: " + rootOsgb.string());
      continue;
    }
    node->setName(tileDir.filename().string());
    group->addChild(node);
  }

  if (group->getNumChildren() == 0) {
    result.errors.push_back("Failed to load any root tiles");
    return result;
  }

  result.root = group;
  return result;
}
