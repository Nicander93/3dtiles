#pragma once

#include <QMainWindow>
#include <QLabel>
#include <QString>

#include "OsgWidget.h"
#include "DatasetLoader.h"

class MainWindow : public QMainWindow {
  Q_OBJECT
public:
  explicit MainWindow(QWidget* parent = nullptr);

  /// Programmatic open (e.g. CLI arg). Returns false on failure.
  bool openDataset(const QString& path);

private slots:
  void onOpenDirectory();
  void onResetView();
  void onAbout();

private:
  void updateStatus(const DatasetLoadResult& result);
  void setStatusMessage(const QString& msg);

  OsgWidget* osgWidget_ = nullptr;
  QLabel* statusLabel_ = nullptr;
  QString currentDataset_;
};
