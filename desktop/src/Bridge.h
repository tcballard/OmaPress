#pragma once
#include <QObject>
#include <QColor>
#include <QUrl>
#include <QVariantMap>
#include <QQueue>
#include <QProcess>
#include <QTimer>
#include <QFileSystemWatcher>

class Bridge final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString publicationPath READ publicationPath WRITE setPublicationPath NOTIFY publicationPathChanged)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QColor accent READ accent NOTIFY themeChanged)
    Q_PROPERTY(QColor background READ background NOTIFY themeChanged)
    Q_PROPERTY(QColor foreground READ foreground NOTIFY themeChanged)
    Q_PROPERTY(QString previewUrl READ previewUrl NOTIFY previewChanged)
public:
    explicit Bridge(QObject *parent=nullptr);
    ~Bridge() override;
    QString publicationPath() const { return m_path; }
    void setPublicationPath(const QString &path);
    bool busy() const { return m_active || !m_queue.isEmpty(); }
    QColor accent() const { return m_accent; }
    QColor background() const { return m_background; }
    QColor foreground() const { return m_foreground; }
    QString previewUrl() const { return m_previewUrl; }
    Q_INVOKABLE void request(const QString &command, const QVariantMap &args={}, const QString &tag={});
    Q_INVOKABLE QVariantMap workerSettings() const;
    Q_INVOKABLE void saveWorkerSettings(const QString &host, const QString &path);
    Q_INVOKABLE QString newJobId() const;
    Q_INVOKABLE QUrl bannerUrl(const QString &path) const;
    Q_INVOKABLE QString localPath(const QUrl &url) const;
    Q_INVOKABLE void copyArticle(const QString &html,const QString &text);
    Q_INVOKABLE void copyText(const QString &text);
    Q_INVOKABLE void startPreview(bool drafts=true);
    Q_INVOKABLE void stopPreview();
    Q_INVOKABLE void openUrl(const QString &url);
    Q_INVOKABLE QString lastPublication() const;
    Q_INVOKABLE QString cliPath() const;
    Q_INVOKABLE void selectText(QObject *editor, int start, int end);
    static QVariantMap colorsFromFile(const QString &path);
signals:
    void publicationPathChanged();
    void busyChanged();
    void themeChanged();
    void previewChanged();
    void result(const QString &tag,const QVariantMap &value);
    void arrayResult(const QString &tag,const QVariantList &value);
    void failed(const QString &tag,const QString &error);
private:
    struct Job { QString command,path,tag; QVariantMap args; quint64 generation; };
    void next();
    void finish();
    void loadTheme();
    QString m_path,m_previewUrl;
    QColor m_accent{"#a7c58b"},m_background{"#181c19"},m_foreground{"#edf0e8"};
    quint64 m_generation=0;
    QQueue<Job> m_queue;
    Job m_current;
    QProcess *m_active=nullptr;
    QProcess *m_preview=nullptr;
    QTimer m_deadline;
    QByteArray m_stdout,m_stderr,m_previewOutput;
    QFileSystemWatcher m_themeWatcher;
};
