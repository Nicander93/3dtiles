#pragma once

#include <QMainWindow>
#include <QListWidget>
#include <QStackedWidget>
#include <QLabel>
#include <QString>

#include "OsgWidget.h"

#ifdef GEOFORGE_HAS_WEBENGINE
class QWebEngineView;
#endif

class ShellWindow : public QMainWindow {
  Q_OBJECT
public:
  explicit ShellWindow(QWidget* parent = nullptr);

private slots:
  void onNavChanged(int row);
  void openInBrowser();
  void reloadOsgbSample();
  void resetOsgbView();

private:
  void buildUi();
  QWidget* makeWebPlaceholder(const QString& title, const QString& path);
  QWidget* makeOsgbPage();
  void setStatus(const QString& msg);

  QListWidget* nav_ = nullptr;
  QStackedWidget* stack_ = nullptr;
  OsgWidget* osgWidget_ = nullptr;
  QLabel* statusLabel_ = nullptr;
  QString webBase_ = QStringLiteral("http://127.0.0.1:8787");
  QString currentWebPath_ = QStringLiteral("/");
  int osgbPageIndex_ = -1;

#ifdef GEOFORGE_HAS_WEBENGINE
  QWebEngineView* webView_ = nullptr;
  int webPageIndex_ = -1;
#endif
};
