#include "Bridge.h"
#include <QGuiApplication>
#include <QClipboard>
#include <QMimeData>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <QDir>
#include <QColor>
#include <QStandardPaths>
#include <QSettings>
#include <QDesktopServices>
#include <QRegularExpression>
#include <QCoreApplication>

Bridge::Bridge(QObject *parent):QObject(parent) {
    m_deadline.setSingleShot(true);
    connect(&m_deadline,&QTimer::timeout,this,[this]{ if(m_active){emit failed(m_current.tag,"The operation timed out. Saved drafts remain intact. Recheck any pending deployment before retrying.");m_active->kill();} });
    connect(&m_themeWatcher,&QFileSystemWatcher::fileChanged,this,[this]{loadTheme();});
    connect(&m_themeWatcher,&QFileSystemWatcher::directoryChanged,this,[this]{loadTheme();});
    loadTheme();
}
Bridge::~Bridge(){if(m_active){m_active->kill();m_active->waitForFinished(1000);}stopPreview();}
void Bridge::setPublicationPath(const QString &path){if(path==m_path)return;stopPreview();m_path=path;emit publicationPathChanged();}
QString Bridge::localPath(const QUrl &url)const{return url.toLocalFile();}
QString Bridge::lastPublication()const{return QSettings().value("lastPublication").toString();}
QString Bridge::cliPath()const{auto override=qEnvironmentVariable("OMAPRESS_CLI");if(!override.isEmpty())return override;const QString sibling=QCoreApplication::applicationDirPath()+"/omapress";if(QFile::exists(sibling))return sibling;return QStandardPaths::findExecutable("omapress");}
void Bridge::request(const QString &command,const QVariantMap &args,const QString &tag){
    const QString actualTag=tag.isEmpty()?command:tag;
    if(command=="render-document"||command=="recovery-document")for(int i=m_queue.size()-1;i>=0;--i)if(m_queue[i].tag==actualTag)m_queue.removeAt(i);
    if(m_queue.size()>30){emit failed(actualTag,"Too many operations are queued. Wait for the current operation to finish.");return;}
    m_queue.enqueue({command,m_path,actualTag,args});emit busyChanged();next();
}
void Bridge::next(){
    if(m_active||m_queue.isEmpty())return;
    m_current=m_queue.dequeue();m_stdout.clear();m_stderr.clear();
    if(cliPath().isEmpty()){emit failed(m_current.tag,"The omapress engine was not found. Install the release bundle with both binaries.");emit busyChanged();QTimer::singleShot(0,this,&Bridge::next);return;}
    m_active=new QProcess(this);m_active->setProgram(cliPath());m_active->setArguments({"rpc"});
    connect(m_active,&QProcess::readyReadStandardOutput,this,[this]{m_stdout+=m_active->readAllStandardOutput();if(m_stdout.size()>32*1024*1024)m_active->kill();});
    connect(m_active,&QProcess::readyReadStandardError,this,[this]{m_stderr+=m_active->readAllStandardError();if(m_stderr.size()>1024*1024)m_active->kill();});
    connect(m_active,&QProcess::errorOccurred,this,[this](QProcess::ProcessError e){if(e==QProcess::FailedToStart){emit failed(m_current.tag,m_active->errorString());m_active->deleteLater();m_active=nullptr;m_deadline.stop();emit busyChanged();QTimer::singleShot(0,this,&Bridge::next);}});
    connect(m_active,qOverload<int,QProcess::ExitStatus>(&QProcess::finished),this,[this]{finish();});
    connect(m_active,&QProcess::started,this,[this]{QJsonObject input{{"schema",1},{"command",m_current.command},{"path",m_current.path},{"args",QJsonObject::fromVariantMap(m_current.args)}};m_active->write(QJsonDocument(input).toJson(QJsonDocument::Compact));m_active->closeWriteChannel();});
    const bool network=QStringList{"publish","rollback","recheck","publish-plan","setup","repositories","github-status"}.contains(m_current.command);
    m_deadline.start(network?600000:15000);m_active->start();emit busyChanged();
}
void Bridge::finish(){
    if(!m_active) return;
    m_deadline.stop();m_stdout+=m_active->readAllStandardOutput();m_stderr+=m_active->readAllStandardError();
    QJsonParseError error;auto doc=QJsonDocument::fromJson(m_stdout,&error);auto obj=doc.object();
    if(error.error!=QJsonParseError::NoError||obj["schema"].toInt()!=1)emit failed(m_current.tag,"The engine returned an invalid response. "+QString::fromUtf8(m_stderr.left(2000)));
    else if(!obj["ok"].toBool())emit failed(m_current.tag,obj["error"].toString());
    else{if(m_current.command=="inspect"||m_current.command=="init")QSettings().setValue("lastPublication",m_current.path);if(obj["result"].isArray())emit arrayResult(m_current.tag,obj["result"].toArray().toVariantList());else emit result(m_current.tag,obj["result"].toObject().toVariantMap());}
    m_active->deleteLater();m_active=nullptr;emit busyChanged();QTimer::singleShot(0,this,&Bridge::next);
}
void Bridge::copyArticle(const QString &html,const QString &text){auto *mime=new QMimeData;mime->setHtml(html);mime->setText(text);QGuiApplication::clipboard()->setMimeData(mime);}
void Bridge::copyText(const QString &text){QGuiApplication::clipboard()->setText(text);}
void Bridge::openUrl(const QString &text){const QUrl url(text);if(url.isValid()&&(url.scheme()=="https"||url.scheme()=="http"||url.scheme()=="mailto"))QDesktopServices::openUrl(url);}
void Bridge::startPreview(bool drafts){
    stopPreview();m_previewOutput.clear();m_preview=new QProcess(this);m_preview->setProgram(cliPath());QStringList args{"preview",m_path,"--json"};if(drafts)args<<"--drafts";m_preview->setArguments(args);
    connect(m_preview,&QProcess::readyReadStandardOutput,this,[this]{m_previewOutput+=m_preview->readAllStandardOutput();const auto newline=m_previewOutput.indexOf('\n');if(newline<0)return;auto obj=QJsonDocument::fromJson(m_previewOutput.left(newline)).object();m_previewOutput.remove(0,newline+1);if(obj["event"].toString()=="preview_ready"){m_previewUrl=obj["url"].toString();emit previewChanged();openUrl(m_previewUrl);}else if(obj["ok"].isBool()&&!obj["ok"].toBool())emit failed("preview",obj["error"].toString());});
    connect(m_preview,&QProcess::errorOccurred,this,[this]{if(m_preview)emit failed("preview",m_preview->errorString());});
    m_preview->start();
}
void Bridge::stopPreview(){if(m_preview){m_preview->terminate();if(!m_preview->waitForFinished(500)){m_preview->kill();m_preview->waitForFinished(500);}delete m_preview;m_preview=nullptr;}m_previewUrl.clear();emit previewChanged();}
QVariantMap Bridge::colorsFromFile(const QString &path){
    QVariantMap values;QFile file(path);if(!file.open(QIODevice::ReadOnly))return values;
    const auto text=QString::fromUtf8(file.read(65536));QRegularExpression re("^\\s*(accent|background|foreground)\\s*=\\s*[\"'](#[0-9a-fA-F]{6})[\"']",QRegularExpression::MultilineOption);
    auto matches=re.globalMatch(text);while(matches.hasNext()){auto m=matches.next();if(QColor(m.captured(2)).isValid())values[m.captured(1)]=m.captured(2);}return values;
}
void Bridge::loadTheme(){
    const auto current=QStandardPaths::writableLocation(QStandardPaths::ConfigLocation)+"/omarchy/current";const auto theme=current+"/theme/colors.toml";const auto colors=colorsFromFile(theme);
    m_accent=colors.value("accent","#a7c58b").toString();m_background=colors.value("background","#181c19").toString();m_foreground=colors.value("foreground","#edf0e8").toString();
    const auto watched=m_themeWatcher.files()+m_themeWatcher.directories();if(!watched.isEmpty())m_themeWatcher.removePaths(watched);
    for(const auto &p:QStringList{current,current+"/theme",theme}) {
        if(QFile::exists(p)) m_themeWatcher.addPath(p);
    }
    emit themeChanged();
}
void Bridge::selectText(QObject *editor,int start,int end){QMetaObject::invokeMethod(editor,"select",Q_ARG(int,start),Q_ARG(int,end));}
