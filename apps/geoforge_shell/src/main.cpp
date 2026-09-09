#include "ShellWindow.h"

#include <QApplication>
#include <QCoreApplication>
#include <QSurfaceFormat>

int main(int argc, char* argv[]) {
#ifdef GEOFORGE_HAS_WEBENGINE
  // Required before QApplication when linking Qt WebEngine.
  QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
#endif
  QSurfaceFormat fmt;
  fmt.setVersion(2, 1);
  fmt.setProfile(QSurfaceFormat::CompatibilityProfile);
  fmt.setDepthBufferSize(24);
  fmt.setStencilBufferSize(8);
  fmt.setSamples(4);
  QSurfaceFormat::setDefaultFormat(fmt);

  QApplication app(argc, argv);
  QApplication::setApplicationName(QStringLiteral("GeoForge 3D"));
  QApplication::setOrganizationName(QStringLiteral("GeoForge"));

  ShellWindow win;
  win.show();
  return app.exec();
}
