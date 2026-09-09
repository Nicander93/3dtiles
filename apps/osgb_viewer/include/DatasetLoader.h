#pragma once

#include <osg/ref_ptr>
#include <osg/Node>
#include <osg/Group>

#include <filesystem>
#include <string>
#include <vector>

struct DatasetLoadResult {
  osg::ref_ptr<osg::Group> root;
  std::filesystem::path datasetPath;
  std::filesystem::path dataDir;
  std::filesystem::path metadataPath;
  std::string metadataSummary;
  std::vector<std::filesystem::path> rootTiles;
  std::vector<std::string> errors;
  int estimatedTileFileCount = 0;
  bool ok() const { return root.valid() && errors.empty(); }
};

class DatasetLoader {
public:
  /// Open a standard OSGB dataset directory containing Data/ (+ optional metadata.xml).
  /// Loads each Data/Tile_*/Tile_*.osgb root; PagedLOD children remain deferred.
  static DatasetLoadResult loadDirectory(const std::filesystem::path& dir);

  /// Count .osgb files under Data/ (estimate of tile pages).
  static int countOsgbFiles(const std::filesystem::path& dataDir);

  /// Parse a short summary from metadata.xml if present.
  static std::string readMetadataSummary(const std::filesystem::path& metadataXml);
};
