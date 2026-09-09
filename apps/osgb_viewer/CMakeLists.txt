cmake_minimum_required(VERSION 3.22)
project(osgb_viewer LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

option(OSGB_VIEWER_BUILD_HEADLESS_CLI "Build headless OSGB load smoke CLI" ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets OpenGLWidgets OpenGL)
find_package(OpenSceneGraph REQUIRED COMPONENTS osg osgDB osgGA osgUtil osgViewer osgText)

add_executable(osgb_viewer
  src/main.cpp
  src/MainWindow.cpp
  src/OsgWidget.cpp
  src/DatasetLoader.cpp
  include/MainWindow.h
  include/OsgWidget.h
  include/DatasetLoader.h
)

target_include_directories(osgb_viewer PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/include
  ${OPENSCENEGRAPH_INCLUDE_DIRS}
)

target_link_libraries(osgb_viewer PRIVATE
  Qt6::Widgets
  Qt6::OpenGLWidgets
  Qt6::OpenGL
  ${OPENSCENEGRAPH_LIBRARIES}
)

if(UNIX AND NOT APPLE)
  target_link_libraries(osgb_viewer PRIVATE GL)
endif()

# Help runtime find OSG plugins when installed into a conda prefix.
if(DEFINED ENV{CONDA_PREFIX})
  target_compile_definitions(osgb_viewer PRIVATE
    OSGB_VIEWER_CONDA_PREFIX="$ENV{CONDA_PREFIX}"
  )
endif()

if(OSGB_VIEWER_BUILD_HEADLESS_CLI)
  add_executable(osgb_headless_load src/headless_load.cpp src/DatasetLoader.cpp)
  target_include_directories(osgb_headless_load PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${OPENSCENEGRAPH_INCLUDE_DIRS}
  )
  target_link_libraries(osgb_headless_load PRIVATE
    ${OPENSCENEGRAPH_LIBRARIES}
  )
  if(DEFINED ENV{CONDA_PREFIX})
    target_compile_definitions(osgb_headless_load PRIVATE
      OSGB_VIEWER_CONDA_PREFIX="$ENV{CONDA_PREFIX}"
    )
  endif()
endif()

install(TARGETS osgb_viewer RUNTIME DESTINATION bin)
if(TARGET osgb_headless_load)
  install(TARGETS osgb_headless_load RUNTIME DESTINATION bin)
endif()
