#include "OsgWidget.h"

#include <QMouseEvent>
#include <QWheelEvent>

#include <osg/Camera>
#include <osgGA/TrackballManipulator>
#include <osgViewer/ViewerEventHandlers>
#include <osg/DisplaySettings>
#include <osg/GL>
#include <osg/StateSet>

OsgWidget::OsgWidget(QWidget* parent)
    : QOpenGLWidget(parent) {
  setFocusPolicy(Qt::StrongFocus);
  setMouseTracking(true);
  // Important: do not start the frame timer until initializeGL — early update()
  // races QOpenGLWidget FBO setup and can segfault.
  connect(&frameTimer_, &QTimer::timeout, this, [this]() {
    if (initialized_) {
      update();
    }
  });
}

OsgWidget::~OsgWidget() {
  frameTimer_.stop();
  makeCurrent();
  if (viewer_) {
    viewer_->setSceneData(nullptr);
    viewer_->setCameraManipulator(nullptr);
    viewer_->getCamera()->setGraphicsContext(nullptr);
    graphicsWindow_ = nullptr;
    viewer_ = nullptr;
  }
  doneCurrent();
}

void OsgWidget::ensureViewer() {
  if (viewer_.valid()) return;

  viewer_ = new osgViewer::Viewer;
  viewer_->setThreadingModel(osgViewer::Viewer::SingleThreaded);
  viewer_->setKeyEventSetsDone(0);

  auto* manip = new osgGA::TrackballManipulator;
  manip->setAllowThrow(false);
  viewer_->setCameraManipulator(manip);
  viewer_->addEventHandler(new osgViewer::StatsHandler);
}

void OsgWidget::ensureGraphicsWindow() {
  ensureViewer();
  if (graphicsWindow_.valid()) return;

  const int w = std::max(1, width() * static_cast<int>(devicePixelRatioF()));
  const int h = std::max(1, height() * static_cast<int>(devicePixelRatioF()));

  graphicsWindow_ = viewer_->setUpViewerAsEmbeddedInWindow(0, 0, w, h);

  osg::Camera* camera = viewer_->getCamera();
  camera->setClearColor(osg::Vec4(0.12f, 0.14f, 0.18f, 1.0f));
  camera->setNearFarRatio(0.0001);
  // Prefer textured appearance: force lighting off at camera level so
  // embedded albedo textures show without requiring scene lights.
  auto* ss = camera->getOrCreateStateSet();
  ss->setMode(GL_LIGHTING, osg::StateAttribute::OFF | osg::StateAttribute::OVERRIDE);
  ss->setMode(GL_BLEND, osg::StateAttribute::OFF);
  const double aspect = static_cast<double>(w) / static_cast<double>(std::max(1, h));
  camera->setProjectionMatrixAsPerspective(30.0, aspect, 1.0, 1e7);
}

void OsgWidget::initializeGL() {
  ensureGraphicsWindow();
  initialized_ = true;
  if (!frameTimer_.isActive()) {
    frameTimer_.start(16);
  }
}

void OsgWidget::resizeGL(int w, int h) {
  if (!initialized_) return;
  const qreal dpr = devicePixelRatioF();
  const int pw = std::max(1, static_cast<int>(w * dpr));
  const int ph = std::max(1, static_cast<int>(h * dpr));

  if (graphicsWindow_.valid()) {
    graphicsWindow_->resized(0, 0, pw, ph);
    graphicsWindow_->getEventQueue()->windowResize(0, 0, pw, ph);
  }
  if (viewer_.valid()) {
    auto* camera = viewer_->getCamera();
    camera->setViewport(0, 0, pw, ph);
    const double aspect = static_cast<double>(pw) / static_cast<double>(ph);
    camera->setProjectionMatrixAsPerspective(30.0, aspect, 1.0, 1e7);
  }
}

void OsgWidget::paintGL() {
  if (!initialized_ || !viewer_.valid()) return;
  // QOpenGLWidget already made the context current — do not call makeCurrent/doneCurrent.
  viewer_->frame();
}

void OsgWidget::setScene(osg::Node* node) {
  ensureViewer();
  viewer_->setSceneData(node);
  if (initialized_) {
    resetView();
  }
}

void OsgWidget::resetView() {
  if (!viewer_.valid()) return;
  if (auto* manip = dynamic_cast<osgGA::TrackballManipulator*>(
          viewer_->getCameraManipulator())) {
    // Fit to scene bounds, then pull in slightly so sparse multi-tile
    // roots (large sphere, small footprints) fill more of the viewport.
    manip->setAutoComputeHomePosition(true);
    manip->home(0.0);
    manip->setDistance(manip->getDistance() * 0.55);
  } else if (viewer_->getCameraManipulator()) {
    viewer_->getCameraManipulator()->home(0.0);
  }
  if (initialized_) {
    update();
  }
}

int OsgWidget::qtToOsgButton(Qt::MouseButton button) const {
  switch (button) {
    case Qt::LeftButton: return 1;
    case Qt::MiddleButton: return 2;
    case Qt::RightButton: return 3;
    default: return 0;
  }
}

void OsgWidget::mousePressEvent(QMouseEvent* event) {
  if (!graphicsWindow_.valid()) return;
  graphicsWindow_->getEventQueue()->mouseButtonPress(
      static_cast<float>(event->position().x() * devicePixelRatioF()),
      static_cast<float>(event->position().y() * devicePixelRatioF()),
      qtToOsgButton(event->button()));
}

void OsgWidget::mouseReleaseEvent(QMouseEvent* event) {
  if (!graphicsWindow_.valid()) return;
  graphicsWindow_->getEventQueue()->mouseButtonRelease(
      static_cast<float>(event->position().x() * devicePixelRatioF()),
      static_cast<float>(event->position().y() * devicePixelRatioF()),
      qtToOsgButton(event->button()));
}

void OsgWidget::mouseMoveEvent(QMouseEvent* event) {
  if (!graphicsWindow_.valid()) return;
  graphicsWindow_->getEventQueue()->mouseMotion(
      static_cast<float>(event->position().x() * devicePixelRatioF()),
      static_cast<float>(event->position().y() * devicePixelRatioF()));
}

void OsgWidget::wheelEvent(QWheelEvent* event) {
  if (!graphicsWindow_.valid()) return;
  const QPoint numDegrees = event->angleDelta() / 8;
  if (numDegrees.y() > 0) {
    graphicsWindow_->getEventQueue()->mouseScroll(osgGA::GUIEventAdapter::SCROLL_UP);
  } else if (numDegrees.y() < 0) {
    graphicsWindow_->getEventQueue()->mouseScroll(osgGA::GUIEventAdapter::SCROLL_DOWN);
  }
}
