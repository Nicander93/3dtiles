#pragma once

#include <QOpenGLWidget>
#include <QTimer>

#include <osg/ref_ptr>
#include <osg/Node>
#include <osgViewer/Viewer>
#include <osgViewer/GraphicsWindow>

class OsgWidget : public QOpenGLWidget {
  Q_OBJECT
public:
  explicit OsgWidget(QWidget* parent = nullptr);
  ~OsgWidget() override;

  void setScene(osg::Node* node);
  void resetView();
  osgViewer::Viewer* viewer() { return viewer_.get(); }

signals:
  void frameStats(QString text);

protected:
  void initializeGL() override;
  void resizeGL(int w, int h) override;
  void paintGL() override;

  void mousePressEvent(QMouseEvent* event) override;
  void mouseReleaseEvent(QMouseEvent* event) override;
  void mouseMoveEvent(QMouseEvent* event) override;
  void wheelEvent(QWheelEvent* event) override;

private:
  void ensureViewer();
  void ensureGraphicsWindow();
  int qtToOsgButton(Qt::MouseButton button) const;

  osg::ref_ptr<osgViewer::Viewer> viewer_;
  osg::ref_ptr<osgViewer::GraphicsWindowEmbedded> graphicsWindow_;
  QTimer frameTimer_;
  bool initialized_ = false;
};
