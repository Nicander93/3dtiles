#include "MainWindow.h"

#include <QAction>
#include <QFileDialog>
#include <QMenuBar>
#include <QMessageBox>
#include <QStatusBar>
#include <QToolBar>
#include <QVBoxLayout>
#include <QWidget>
#include <QApplication>

MainWindow::MainWindow(QWidget* parent)
    : QMainWindow(parent) {
  setWindowTitle(QStringLiteral("GeoForge OSGB Preview (native)"));
  resize(1280, 800);

  auto* central = new QWidget(this);
  auto* layout = new QVBoxLayout(central);
  layout->setContentsMargins(0, 0, 0, 0);

  osgWidget_ = new OsgWidget(central);
  layout->addWidget(osgWidget_);
  setCentralWidget(central);

  statusLabel_ = new QLabel(QStringLiteral("Open an OSGB dataset directory (Data/ + metadata.xml)"), this);
  statusLabel_->setTextInteractionFlags(Qt::TextSelectableByMouse);
  statusBar()->addWidget(statusLabel_, 1);

  auto* openAct = new QAction(QStringLiteral("Open Directory…"), this);
  openAct->setShortcut(QKeySequence::Open);
  connect(openAct, &QAction::triggered, this, &MainWindow::onOpenDirectory);

  auto* resetAct = new QAction(QStringLiteral("Reset View"), this);
  resetAct->setShortcut(QKeySequence(Qt::Key_R));
  connect(resetAct, &QAction::triggered, this, &MainWindow::onResetView);

  auto* quitAct = new QAction(QStringLiteral("Quit"), this);
  quitAct->setShortcut(QKeySequence::Quit);
  connect(quitAct, &QAction::triggered, qApp, &QApplication::quit);

  auto* aboutAct = new QAction(QStringLiteral("About"), this);
  connect(aboutAct, &QAction::triggered, this, &MainWindow::onAbout);

  auto* fileMenu = menuBar()->addMenu(QStringLiteral("&File"));
  fileMenu->addAction(openAct);
  fileMenu->addSeparator();
  fileMenu->addAction(quitAct);

  auto* viewMenu = menuBar()->addMenu(QStringLiteral("&View"));
  viewMenu->addAction(resetAct);

  auto* helpMenu = menuBar()->addMenu(QStringLiteral("&Help"));
  helpMenu->addAction(aboutAct);

  auto* tb = addToolBar(QStringLiteral("Main"));
  tb->addAction(openAct);
  tb->addAction(resetAct);
}

bool MainWindow::openDataset(const QString& path) {
  const auto result = DatasetLoader::loadDirectory(path.toStdString());
  if (!result.root) {
    QString err = QStringLiteral("Failed to load dataset:\n");
    for (const auto& e : result.errors) {
      err += QString::fromStdString(e) + QLatin1Char('\n');
    }
    QMessageBox::warning(this, QStringLiteral("Load error"), err);
    updateStatus(result);
    return false;
  }

  osgWidget_->setScene(result.root.get());
  currentDataset_ = QString::fromStdString(result.datasetPath.string());
  updateStatus(result);

  if (!result.errors.empty()) {
    QString warn = QStringLiteral("Loaded with warnings:\n");
    for (const auto& e : result.errors) {
      warn += QString::fromStdString(e) + QLatin1Char('\n');
    }
    statusBar()->showMessage(warn.simplified(), 8000);
  }
  return result.errors.empty();
}

void MainWindow::onOpenDirectory() {
  const QString dir = QFileDialog::getExistingDirectory(
      this,
      QStringLiteral("Open OSGB dataset directory"),
      currentDataset_.isEmpty() ? QStringLiteral("/workspace/data") : currentDataset_);
  if (dir.isEmpty()) return;
  openDataset(dir);
}

void MainWindow::onResetView() {
  osgWidget_->resetView();
}

void MainWindow::onAbout() {
  QMessageBox::about(
      this,
      QStringLiteral("About GeoForge OSGB Preview"),
      QStringLiteral(
          "Native OSGB preview prototype (Qt6 + OpenSceneGraph).\n"
          "Loads Data/Tile_*/Tile_*.osgb with PagedLOD paging honored.\n"
          "Product direction: native OSGB preview (not GLB/Web)."));
}

void MainWindow::updateStatus(const DatasetLoadResult& result) {
  QString msg = QStringLiteral("path=%1 | roots=%2 | ~%3 .osgb | %4")
                    .arg(QString::fromStdString(result.datasetPath.string()))
                    .arg(result.rootTiles.size())
                    .arg(result.estimatedTileFileCount)
                    .arg(QString::fromStdString(result.metadataSummary));
  if (!result.errors.empty()) {
    msg += QStringLiteral(" | errors=%1").arg(result.errors.size());
    for (const auto& e : result.errors) {
      msg += QStringLiteral("; ") + QString::fromStdString(e);
    }
  }
  setStatusMessage(msg);
}

void MainWindow::setStatusMessage(const QString& msg) {
  statusLabel_->setText(msg);
}
