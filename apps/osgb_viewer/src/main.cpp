#include "MainWindow.h"

#include <QApplication>
#include <QSurfaceFormat>
#include <QOffscreenSurface>
#include <QOpenGLContext>
#include <QCommandLineParser>
#include <QTimer>

#include <osgDB/Registry>
#include <iostream>

static void configureDefaultSurfaceFormat() {
  QSurfaceFormat fmt;
  fmt.setVersion(2, 1);
  fmt.setProfile(QSurfaceFormat::CompatibilityProfile);
  fmt.setDepthBufferSize(24);
  fmt.setStencilBufferSize(8);
  fmt.setSamples(0);
  fmt.setSwapBehavior(QSurfaceFormat::DoubleBuffer);
  QSurfaceFormat::setDefaultFormat(fmt);
}

int main(int argc, char* argv[]) {
#ifdef OSGB_VIEWER_CONDA_PREFIX
  // Ensure plugin path before any osgDB use.
  qputenv("OSG_LIBRARY_PATH",
          QByteArray(OSGB_VIEWER_CONDA_PREFIX) + "/lib/osgPlugins-3.6.5");
#endif

  // Soft software GL fallback on boxes without GPU.
  if (qEnvironmentVariableIsEmpty("LIBGL_ALWAYS_SOFTWARE") &&
      qEnvironmentVariableIsEmpty("OSGB_VIEWER_FORCE_HW")) {
    // Prefer real GL when available; uncomment to force software:
    // qputenv("LIBGL_ALWAYS_SOFTWARE", "1");
  }

  configureDefaultSurfaceFormat();

  QApplication app(argc, argv);
  QApplication::setApplicationName(QStringLiteral("osgb_viewer"));
  QApplication::setOrganizationName(QStringLiteral("GeoForge"));
  QApplication::setApplicationVersion(QStringLiteral("0.1.0"));

  QCommandLineParser parser;
  parser.setApplicationDescription(
      QStringLiteral("GeoForge native OSGB preview (Qt6 + OpenSceneGraph)"));
  parser.addHelpOption();
  parser.addVersionOption();
  parser.addPositionalArgument(
      QStringLiteral("dataset"),
      QStringLiteral("OSGB dataset directory (contains Data/ and metadata.xml)"),
      QStringLiteral("[dataset]"));
  QCommandLineOption smokeOption(
      QStringList{QStringLiteral("smoke")},
      QStringLiteral("Load dataset, show window briefly, then quit (CI smoke)"));
  parser.addOption(smokeOption);
  parser.process(app);

  MainWindow window;
  window.show();

  const QStringList pos = parser.positionalArguments();
  if (!pos.isEmpty()) {
    if (!window.openDataset(pos.front())) {
      std::cerr << "Failed to open dataset: " << pos.front().toStdString() << "\n";
      // Still show UI so user can pick another path.
    }
  }

  if (parser.isSet(smokeOption)) {
    QTimer::singleShot(2500, &app, &QApplication::quit);
  }

  return app.exec();
}
