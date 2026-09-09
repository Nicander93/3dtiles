cmake_minimum_required(VERSION 3.22)
project(geoforge_shell LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

set(OSGB_VIEWER_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../osgb_viewer")

find_package(Qt6 REQUIRED COMPONENTS Widgets OpenGLWidgets OpenGL)
find_package(OpenSceneGraph REQUIRED COMPONENTS osg osgDB osgGA osgUtil osgViewer osgText)

# Optional WebEngine. Package may install but fail to link (conda libudev vs sysroot glibc).
option(GEOFORGE_ENABLE_WEBENGINE "Try Qt6::WebEngineWidgets embed" ON)
set(GEOFORGE_HAS_WEBENGINE OFF)
if(GEOFORGE_ENABLE_WEBENGINE)
  find_package(Qt6 COMPONENTS WebEngineWidgets QUIET)
  if(TARGET Qt6::WebEngineWidgets)
    set(GEOFORGE_HAS_WEBENGINE ON)
    message(STATUS "GeoForge shell: Qt6 WebEngineWidgets FOUND — embedding http://127.0.0.1:8787/")
  else()
    message(STATUS "GeoForge shell: Qt6 WebEngineWidgets NOT found — browser link + OSGB embed fallback")
  endif()
else()
  message(STATUS "GeoForge shell: WebEngine disabled (GEOFORGE_ENABLE_WEBENGINE=OFF) — browser fallback")
endif()

add_executable(geoforge_shell
  src/main.cpp
  src/ShellWindow.cpp
  include/ShellWindow.h
  # Reuse native OSGB preview pieces from osgb_viewer
  ${OSGB_VIEWER_DIR}/src/OsgWidget.cpp
  ${OSGB_VIEWER_DIR}/src/DatasetLoader.cpp
  ${OSGB_VIEWER_DIR}/include/OsgWidget.h
  ${OSGB_VIEWER_DIR}/include/DatasetLoader.h
)

target_include_directories(geoforge_shell PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/include
  ${OSGB_VIEWER_DIR}/include
  ${OPENSCENEGRAPH_INCLUDE_DIRS}
)

target_link_libraries(geoforge_shell PRIVATE
  Qt6::Widgets
  Qt6::OpenGLWidgets
  Qt6::OpenGL
  ${OPENSCENEGRAPH_LIBRARIES}
)

if(GEOFORGE_HAS_WEBENGINE)
  target_link_libraries(geoforge_shell PRIVATE Qt6::WebEngineWidgets)
  target_compile_definitions(geoforge_shell PRIVATE GEOFORGE_HAS_WEBENGINE=1)
endif()

if(UNIX AND NOT APPLE)
  target_link_libraries(geoforge_shell PRIVATE GL)
endif()

if(DEFINED ENV{CONDA_PREFIX})
  target_compile_definitions(geoforge_shell PRIVATE
    OSGB_VIEWER_CONDA_PREFIX="$ENV{CONDA_PREFIX}"
  )
endif()

install(TARGETS geoforge_shell RUNTIME DESTINATION bin)
