#include "DatasetLoader.h"

#include <osg/BoundingSphere>
#include <osg/NodeVisitor>
#include <osg/Geode>
#include <osg/Group>
#include <osg/PagedLOD>
#include <osgDB/ReadFile>
#include <osgDB/Registry>

#include <cstdlib>
#include <iostream>

namespace {

struct StatsVisitor : public osg::NodeVisitor {
  StatsVisitor() : osg::NodeVisitor(TRAVERSE_ALL_CHILDREN) {}
  int nodes = 0;
  int geodes = 0;
  int drawables = 0;
  int pagedLods = 0;
  void apply(osg::Node& node) override {
    ++nodes;
    traverse(node);
  }
  void apply(osg::Geode& geode) override {
    ++nodes;
    ++geodes;
    drawables += static_cast<int>(geode.getNumDrawables());
    traverse(geode);
  }
  void apply(osg::PagedLOD& plod) override {
    ++nodes;
    ++pagedLods;
    traverse(plod);
  }
};

}  // namespace

int main(int argc, char* argv[]) {
#ifdef OSGB_VIEWER_CONDA_PREFIX
  setenv("OSG_LIBRARY_PATH", OSGB_VIEWER_CONDA_PREFIX "/lib/osgPlugins-3.6.5", 0);
#endif

  if (argc < 2) {
    std::cerr << "Usage: osgb_headless_load <osgb-dataset-dir-or-file.osgb>\n";
    return 2;
  }

  const std::filesystem::path input = argv[1];
  DatasetLoadResult result;

  if (std::filesystem::is_regular_file(input) && input.extension() == ".osgb") {
    osg::ref_ptr<osg::Node> node = osgDB::readNodeFile(input.string());
    result.datasetPath = input;
    result.estimatedTileFileCount = 1;
    result.rootTiles.push_back(input);
    if (!node) {
      result.errors.push_back("Failed to read " + input.string());
    } else {
      result.root = new osg::Group;
      result.root->addChild(node);
    }
  } else {
    result = DatasetLoader::loadDirectory(input);
  }

  std::cout << "dataset: " << result.datasetPath << "\n";
  std::cout << "metadata: " << result.metadataSummary << "\n";
  std::cout << "root tiles: " << result.rootTiles.size() << "\n";
  std::cout << "estimated .osgb files: " << result.estimatedTileFileCount << "\n";
  for (const auto& e : result.errors) {
    std::cerr << "error: " << e << "\n";
  }

  if (!result.root) {
    return 1;
  }

  const osg::BoundingSphere bs = result.root->getBound();
  std::cout << "bounding sphere center: (" << bs.center().x() << ", "
            << bs.center().y() << ", " << bs.center().z() << ")\n";
  std::cout << "bounding sphere radius: " << bs.radius() << "\n";

  StatsVisitor stats;
  result.root->accept(stats);
  std::cout << "nodes: " << stats.nodes
            << " geodes: " << stats.geodes
            << " drawables: " << stats.drawables
            << " PagedLOD: " << stats.pagedLods << "\n";
  std::cout << "OK\n";
  return result.errors.empty() ? 0 : 1;
}
