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
#include <QSettings>
int main(int argc,char **argv){
    QGuiApplication app(argc,argv);app.setApplicationName("OmaPress");app.setOrganizationName("OmaPress");app.setApplicationVersion("0.0.2");app.setDesktopFileName("omapress");
    QSettings settings;
    if (!settings.value("migration/pressroomImported", false).toBool()) {
        QSettings legacy("Pressroom", "Pressroom");
        for (const auto &key : legacy.allKeys())
            if (!settings.contains(key)) settings.setValue(key, legacy.value(key));
        settings.setValue("migration/pressroomImported", true);
        settings.sync();
    }
    QQuickStyle::setStyle("Material");Bridge bridge;
    const auto args=app.arguments();
    if(args.contains("--clipboard-smoke")){bridge.copyArticle("<h1>Test</h1><p><a href=\"https://example.com\">Source</a></p>","Test\n\nSource (https://example.com)");auto mime=QGuiApplication::clipboard()->mimeData();return mime&&mime->hasHtml()&&mime->text().contains("https://example.com")?0:1;}
    QQmlApplicationEngine engine;engine.rootContext()->setContextProperty("backend",&bridge);
    QObject::connect(&engine,&QQmlApplicationEngine::objectCreationFailed,&app,[]{QCoreApplication::exit(1);},Qt::QueuedConnection);
    engine.load(QUrl("qrc:/qml/Main.qml"));if(engine.rootObjects().isEmpty())return 1;
    if(args.contains("--smoke"))QTimer::singleShot(1500,&app,&QCoreApplication::quit);
    if(args.contains("--dialogs-smoke")) {
        auto root=engine.rootObjects().first();
        QTimer::singleShot(200,&app,[root]{QMetaObject::invokeMethod(root,"smokeConnections");});
        QTimer::singleShot(600,&app,[root,&app]{auto dialog=root->findChild<QObject*>("connectionsDialog");if(!dialog||!dialog->property("visible").toBool()){app.exit(1);return;}QMetaObject::invokeMethod(root,"smokeDistribution");});
        QTimer::singleShot(1000,&app,[root,&app]{auto dialog=root->findChild<QObject*>("distributionDialog");if(!dialog||!dialog->property("visible").toBool()){app.exit(1);return;}QMetaObject::invokeMethod(root,"smokeQueue");});
        QTimer::singleShot(1400,&app,[root,&app]{auto dialog=root->findChild<QObject*>("queueDialog");auto intake=root->findChild<QObject*>("intakeDialog");auto gateway=root->findChild<QObject*>("gatewayConfigDialog");app.exit(gateway&&gateway->property("visible").toBool()&&dialog&&intake&&dialog->property("visible").toBool()&&intake->property("visible").toBool()?0:1);});
    }
    const int screenshotIndex=args.indexOf("--screenshot");
    if(screenshotIndex>=0 && args.contains("--screenshot-preview")) engine.rootObjects().first()->setProperty("showPreview", true);
    if(screenshotIndex>=0 && args.contains("--screenshot-compact")) engine.rootObjects().first()->setProperty("width", 900);
    if(screenshotIndex>=0&&screenshotIndex+1<args.size())QTimer::singleShot(2000,&app,[&]{auto window=qobject_cast<QQuickWindow*>(engine.rootObjects().first());if(window)window->grabWindow().save(args.at(screenshotIndex+1));app.quit();});
    return app.exec();
}
