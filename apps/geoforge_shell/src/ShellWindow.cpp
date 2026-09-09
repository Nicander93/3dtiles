#include "ShellWindow.h"
#include "DatasetLoader.h"

#include <QDesktopServices>
#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QPushButton>
#include <QUrl>
#include <QFrame>
#include <QStatusBar>
#include <QFont>

#ifdef GEOFORGE_HAS_WEBENGINE
#  include <QWebEngineView>
#endif

namespace {
constexpr const char* kSampleOsgb = "/workspace/data/OSGBny/OSGBny";
}

ShellWindow::ShellWindow(QWidget* parent) : QMainWindow(parent) {
  setWindowTitle(QStringLiteral("GeoForge 3D · 桌面壳"));
  resize(1360, 860);
  buildUi();
  setStatus(QStringLiteral("就绪 · UI %1 · 浏览器回退模式（WebEngine 未启用）· OSGB 可内嵌")
                .arg(webBase_));
}

void ShellWindow::buildUi() {
  auto* central = new QWidget(this);
  auto* root = new QHBoxLayout(central);
  root->setContentsMargins(0, 0, 0, 0);
  root->setSpacing(0);

  // --- Left nav ---
  auto* side = new QFrame(central);
  side->setObjectName(QStringLiteral("sideNav"));
  side->setFixedWidth(200);
  side->setStyleSheet(QStringLiteral(
      "#sideNav { background: #ffffff; border-right: 1px solid #e3e8f2; }"
      "QListWidget { border: none; background: transparent; outline: none; padding: 8px; }"
      "QListWidget::item { padding: 10px 12px; border-radius: 8px; margin: 2px 4px; color: #5a6578; }"
      "QListWidget::item:selected { background: rgba(47,107,255,0.12); color: #2f6bff; font-weight: 600; }"
      "QListWidget::item:hover { background: #f0f3f9; }"));

  auto* sideLay = new QVBoxLayout(side);
  sideLay->setContentsMargins(8, 12, 8, 12);
  sideLay->setSpacing(8);

  auto* brand = new QLabel(QStringLiteral("<b>GeoForge 3D</b><br>"
                                          "<span style='color:#8a94a8;font-size:11px'>本地 · 高效 · 开放</span>"),
                           side);
  brand->setTextFormat(Qt::RichText);
  sideLay->addWidget(brand);

  nav_ = new QListWidget(side);
  nav_->addItem(QStringLiteral("工作区"));
  nav_->addItem(QStringLiteral("转换"));
  nav_->addItem(QStringLiteral("任务"));
  nav_->addItem(QStringLiteral("OSGB预览"));
  nav_->addItem(QStringLiteral("Tiles预览"));
  nav_->setCurrentRow(0);
  sideLay->addWidget(nav_, 1);

  auto* openBtn = new QPushButton(QStringLiteral("在浏览器打开 UI"), side);
  connect(openBtn, &QPushButton::clicked, this, &ShellWindow::openInBrowser);
  sideLay->addWidget(openBtn);

  // --- Center stack ---
  stack_ = new QStackedWidget(central);

#ifdef GEOFORGE_HAS_WEBENGINE
  webView_ = new QWebEngineView(stack_);
  webView_->load(QUrl(webBase_ + QStringLiteral("/")));
  webPageIndex_ = stack_->addWidget(webView_);
#else
  // Placeholders for web routes (open system browser)
  stack_->addWidget(makeWebPlaceholder(QStringLiteral("工作区"), QStringLiteral("/")));
  stack_->addWidget(makeWebPlaceholder(QStringLiteral("OSGB 转换"), QStringLiteral("/osgb/convert")));
  stack_->addWidget(makeWebPlaceholder(QStringLiteral("正在处理"), QStringLiteral("/processing")));
#endif

  osgbPageIndex_ = stack_->addWidget(makeOsgbPage());

#ifndef GEOFORGE_HAS_WEBENGINE
  stack_->addWidget(makeWebPlaceholder(QStringLiteral("3D Tiles 预览"), QStringLiteral("/preview/tiles")));
#endif

  root->addWidget(side);
  root->addWidget(stack_, 1);
  setCentralWidget(central);

  statusLabel_ = new QLabel(this);
  statusLabel_->setTextInteractionFlags(Qt::TextSelectableByMouse);
  statusBar()->addWidget(statusLabel_, 1);

  connect(nav_, &QListWidget::currentRowChanged, this, &ShellWindow::onNavChanged);
  onNavChanged(0);
}

QWidget* ShellWindow::makeWebPlaceholder(const QString& title, const QString& path) {
  auto* page = new QWidget;
  auto* lay = new QVBoxLayout(page);
  lay->setContentsMargins(32, 32, 32, 32);
  lay->setSpacing(16);

  auto* h = new QLabel(title, page);
  QFont f = h->font();
  f.setPointSize(16);
  f.setBold(true);
  h->setFont(f);

  auto* banner = new QLabel(
      QStringLiteral(
          "<div style='background:#fff4e5;border:1px solid #f0c36d;border-radius:10px;"
          "padding:12px 14px;color:#7a4e00;'>"
          "<b>模式：系统浏览器回退</b>（本构建未链接 Qt WebEngine）<br>"
          "Web 页（工作区 / 转换 / 任务 / Tiles预览）请用下方按钮或侧栏「在浏览器打开 UI」。"
          "左侧「OSGB预览」仍为内嵌原生 OsgbWidget。"
          "</div>"),
      page);
  banner->setTextFormat(Qt::RichText);
  banner->setWordWrap(true);

  auto* desc = new QLabel(
      QStringLiteral(
          "请先启动 GeoForge API：<code>bash scripts/run_geoforge.sh</code>，"
          "确认 UI 在 http://127.0.0.1:8787/ 可访问，再打开下方地址。"),
      page);
  desc->setWordWrap(true);
  desc->setTextFormat(Qt::RichText);
  desc->setStyleSheet(QStringLiteral("color:#5a6578;"));

  auto* url = new QLabel(webBase_ + path, page);
  url->setTextInteractionFlags(Qt::TextSelectableByMouse);
  url->setStyleSheet(QStringLiteral(
      "background:#f0f3f9;border:1px solid #e3e8f2;border-radius:8px;padding:10px 12px;"));

  auto* btn = new QPushButton(QStringLiteral("在浏览器中打开"), page);
  btn->setProperty("path", path);
  connect(btn, &QPushButton::clicked, this, [this, path]() {
    currentWebPath_ = path;
    openInBrowser();
  });

  lay->addWidget(h);
  lay->addWidget(banner);
  lay->addWidget(desc);
  lay->addWidget(url);
  lay->addWidget(btn, 0, Qt::AlignLeft);
  lay->addStretch(1);
  page->setProperty("webPath", path);
  return page;
}

QWidget* ShellWindow::makeOsgbPage() {
  auto* page = new QWidget;
  auto* lay = new QVBoxLayout(page);
  lay->setContentsMargins(0, 0, 0, 0);
  lay->setSpacing(0);

  auto* bar = new QWidget(page);
  auto* barLay = new QHBoxLayout(bar);
  barLay->setContentsMargins(12, 8, 12, 8);
  auto* title = new QLabel(QStringLiteral("OSGB 原生预览 · OSGBny"), bar);
  QFont tf = title->font();
  tf.setBold(true);
  title->setFont(tf);
  auto* reload = new QPushButton(QStringLiteral("加载样例"), bar);
  auto* reset = new QPushButton(QStringLiteral("复位视角"), bar);
  auto* browser = new QPushButton(QStringLiteral("Web 预览页"), bar);
  connect(reload, &QPushButton::clicked, this, &ShellWindow::reloadOsgbSample);
  connect(reset, &QPushButton::clicked, this, &ShellWindow::resetOsgbView);
  connect(browser, &QPushButton::clicked, this, [this]() {
    currentWebPath_ = QStringLiteral("/preview/osgb");
    openInBrowser();
  });
  barLay->addWidget(title);
  barLay->addStretch(1);
  barLay->addWidget(reload);
  barLay->addWidget(reset);
  barLay->addWidget(browser);

  osgWidget_ = new OsgWidget(page);
  lay->addWidget(bar);
  lay->addWidget(osgWidget_, 1);
  return page;
}

void ShellWindow::onNavChanged(int row) {
  // Nav: 0 工作区, 1 转换, 2 任务, 3 OSGB预览, 4 Tiles预览
#ifdef GEOFORGE_HAS_WEBENGINE
  if (row == 3) {
    stack_->setCurrentIndex(osgbPageIndex_);
    reloadOsgbSample();
    setStatus(QStringLiteral("OSGB 原生预览（OsgbWidget）"));
    return;
  }
  QString path = QStringLiteral("/");
  if (row == 1) path = QStringLiteral("/osgb/convert");
  else if (row == 2) path = QStringLiteral("/processing");
  else if (row == 4) path = QStringLiteral("/preview/tiles");
  currentWebPath_ = path;
  if (webView_) {
    webView_->load(QUrl(webBase_ + path));
    stack_->setCurrentIndex(webPageIndex_);
  }
  setStatus(QStringLiteral("WebEngine · %1%2").arg(webBase_, path));
#else
  if (row == 3) {
    stack_->setCurrentIndex(osgbPageIndex_);
    reloadOsgbSample();
    setStatus(QStringLiteral("OSGB 原生预览（OsgbWidget · %1）").arg(QLatin1String(kSampleOsgb)));
    return;
  }
  // Map: 0->0, 1->1, 2->2, 4->4 (after osgb at 3)
  int idx = row;
  if (row == 4) idx = 4;  // tiles placeholder after osgb
  // stack order: 0 workspace, 1 convert, 2 processing, 3 osgb, 4 tiles
  stack_->setCurrentIndex(idx);
  auto* w = stack_->widget(idx);
  currentWebPath_ = w ? w->property("webPath").toString() : QStringLiteral("/");
  setStatus(QStringLiteral("⚠ 浏览器回退 · 请打开 %1%2（本构建无 Qt WebEngine）")
                .arg(webBase_, currentWebPath_));
#endif
}

void ShellWindow::openInBrowser() {
  const QUrl url(webBase_ + currentWebPath_);
  QDesktopServices::openUrl(url);
  setStatus(QStringLiteral("已请求系统浏览器打开 %1").arg(url.toString()));
}

void ShellWindow::reloadOsgbSample() {
  if (!osgWidget_) return;
  const auto result = DatasetLoader::loadDirectory(kSampleOsgb);
  if (!result.root) {
    QString err = QStringLiteral("加载失败");
    for (const auto& e : result.errors) {
      err += QStringLiteral("; ") + QString::fromStdString(e);
    }
    setStatus(err);
    return;
  }
  osgWidget_->setScene(result.root.get());
  setStatus(QStringLiteral("已加载 OSGBny · roots=%1 · ~%2 .osgb · %3")
                .arg(result.rootTiles.size())
                .arg(result.estimatedTileFileCount)
                .arg(QString::fromStdString(result.metadataSummary)));
}

void ShellWindow::resetOsgbView() {
  if (osgWidget_) osgWidget_->resetView();
  setStatus(QStringLiteral("已复位视角"));
}

void ShellWindow::setStatus(const QString& msg) {
  if (statusLabel_) statusLabel_->setText(msg);
}
