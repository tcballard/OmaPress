#include "Bridge.h"
#include <QGuiApplication>
#include <QTemporaryDir>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QSaveFile>
#include <QElapsedTimer>
#include <QThread>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickItem>
#include <QQuickWindow>
#include <QDebug>
#include <functional>

static void require(bool ok, const char *message) {
    if (!ok) qFatal("%s", message);
}
static void writePalette(const QString &path, const QByteArray &text) {
    require(QDir().mkpath(QFileInfo(path).absolutePath()), "create palette directory");
    QSaveFile file(path);
    require(file.open(QIODevice::WriteOnly), "open palette");
    require(file.write(text) == text.size() && file.commit(), "atomic palette write");
}
static void until(const std::function<bool()> &condition) {
    QElapsedTimer timer; timer.start();
    while (!condition() && timer.elapsed() < 3000) {
        QCoreApplication::processEvents(); QThread::msleep(10);
    }
    require(condition(), "theme did not update");
}
int main(int argc, char **argv) {
    QTemporaryDir temp;
    require(temp.isValid(), "temporary config");
    const auto state = temp.path()+"/state";
    const auto config = temp.path()+"/config";
    QDir().mkpath(state); QDir().mkpath(config);
    qputenv("XDG_STATE_HOME", state.toUtf8());
    qputenv("XDG_CONFIG_HOME", config.toUtf8());
    QGuiApplication app(argc, argv);
    app.setApplicationName("OmaPress"); app.setOrganizationName("OmaPress");
    const QByteArray familiar = "mode = \"light\"\naccent = \"#0067b8\"\nbackground = \"#f5f6f8\"\nforeground = \"#202124\"\n";
    const QByteArray dark = "accent = \"#8ab4f8\"\nbackground = \"#181c19\"\nforeground = \"#edf0e8\"\n";
    const auto legacy = config+"/omarchy/current/theme/colors.toml";
    const auto current = state+"/omarchy/current";
    const auto palette = current+"/theme/colors.toml";
    writePalette(legacy, familiar);
    Bridge bridge;
    require(bridge.background() == QColor("#f5f6f8"), "legacy theme");
    writePalette(legacy, dark);
    until([&]{return bridge.background() == QColor("#181c19");});
    writePalette(palette, familiar);
    until([&]{return bridge.accent() == QColor("#0067b8");});
    require(bridge.background() == QColor("#f5f6f8") && bridge.foreground() == QColor("#202124"), "Familiar colours");
    writePalette(palette, "accent = \"#ff0000\"\nbackground = \"invalid\"\n");
    QElapsedTimer pause; pause.start();
    while (pause.elapsed() < 150) { QCoreApplication::processEvents(); QThread::msleep(10); }
    require(bridge.accent() == QColor("#0067b8"), "retain complete palette on malformed input");
    require(QDir(current+"/theme").removeRecursively(), "remove old theme");
    QCoreApplication::processEvents();
    writePalette(current+"/next-theme/colors.toml", dark);
    require(QDir().rename(current+"/next-theme",current+"/theme"), "replace theme directory");
    until([&]{return bridge.background() == QColor("#181c19");});
    writePalette(palette, familiar);
    until([&]{return bridge.background() == QColor("#f5f6f8");});

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("backend", &bridge);
    engine.load(QUrl("qrc:/qml/Main.qml"));
    require(!engine.rootObjects().isEmpty(), "load themed QML");
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    require(window, "application window");
    auto *toolbar = window->findChild<QQuickItem *>("workspaceToolbar");
    require(toolbar, "toolbar");
    auto *content = qvariant_cast<QQuickItem *>(toolbar->property("contentItem"));
    until([&]{return content && content->x() >= 16 && toolbar->height() >= 68;});
    require(window->color() == QColor("#f5f6f8"), "light window colour");
    require(window->property("border").value<QColor>().isValid(), "typed foreground channels");
    for (int width : {900,1440}) {
        window->setWidth(width); QCoreApplication::processEvents();
        require(content->x()+content->width() <= toolbar->width()-16, "right corner inset");
    }
    writePalette(palette, dark);
    until([&]{return window->color() == QColor("#181c19");});
    qInfo("PASS: legacy, Quattro, Familiar, malformed palette, atomic replacement, live colours and toolbar insets");
}
