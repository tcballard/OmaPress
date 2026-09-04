#include "Bridge.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
#include <QFileInfo>
#include <QClipboard>
#include <QMimeData>
#include <QJsonDocument>
#include <QJsonObject>
#include <cstdio>
int main(int argc,char **argv){
    QGuiApplication app(argc,argv);app.setApplicationName("OmaPress");app.setOrganizationName("OmaPress");app.setApplicationVersion("0.1.0-rc.1");app.setDesktopFileName("omapress");
    QQuickStyle::setStyle("Material");Bridge bridge;
    const auto args=app.arguments();
    if(args.contains("--clipboard-smoke")){bridge.copyArticle("<h1>Test</h1><p><a href=\"https://example.com\">Source</a></p>","Test\n\nSource (https://example.com)");auto mime=QGuiApplication::clipboard()->mimeData();return mime&&mime->hasHtml()&&mime->text().contains("https://example.com")?0:1;}
    QQmlApplicationEngine engine;engine.rootContext()->setContextProperty("backend",&bridge);
    QObject::connect(&engine,&QQmlApplicationEngine::objectCreationFailed,&app,[]{QCoreApplication::exit(1);},Qt::QueuedConnection);
    engine.load(QUrl("qrc:/qml/Main.qml"));if(engine.rootObjects().isEmpty())return 1;
    if(args.contains("--smoke"))QTimer::singleShot(1500,&app,&QCoreApplication::quit);
    const int screenshotIndex=args.indexOf("--screenshot");
    if(screenshotIndex>=0&&screenshotIndex+1<args.size())QTimer::singleShot(2000,&app,[&]{auto window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());if(window)window->grabWindow().save(args.at(screenshotIndex+1));app.quit();});
    return app.exec();
}
