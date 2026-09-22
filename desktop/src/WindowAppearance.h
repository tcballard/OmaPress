#pragma once
#include <QGuiApplication>
#include <QProcess>
#include <QQuickWindow>
#include <QStandardPaths>
#include <QTimer>

// Qt paints an opaque surface; Omarchy also applies compositor-side opacity.
// Register one session-only rule, restricted to our window identity. A config
// reload clears the Lua global; the next activation restores the rule.
inline void keepWritingSurfaceOpaque(QQuickWindow *window) {
    if (!window || qEnvironmentVariableIsEmpty("HYPRLAND_INSTANCE_SIGNATURE") ||
        qEnvironmentVariableIntValue("OMAPRESS_KEEP_COMPOSITOR_OPACITY") == 1)
        return;
    const auto hyprctl = QStandardPaths::findExecutable("hyprctl");
    if (hyprctl.isEmpty()) return;
    auto *process = new QProcess(window);
    auto *timeout = new QTimer(process);
    timeout->setSingleShot(true);
    QObject::connect(timeout, &QTimer::timeout, process, &QProcess::kill);
    QObject::connect(process, &QProcess::finished, timeout, &QTimer::stop);
    QObject::connect(process, &QProcess::errorOccurred, timeout, &QTimer::stop);
    const auto apply = [process, timeout, hyprctl] {
        if (process->state() != QProcess::NotRunning) return;
        process->start(hyprctl, {"eval",
            "if not _G.omapressOpaqueRule then "
            "_G.omapressOpaqueRule = hl.window_rule({name = 'omapress-writing-surface', "
            "match = {class = '^(omapress|OmaPress)$'}, "
            "opacity = '1 override 1 override 1 override', opaque = true}) end"});
        timeout->start(1000);
    };
    QObject::connect(window, &QWindow::activeChanged, process, [window, apply] {
        if (window->isActive()) apply();
    });
    QTimer::singleShot(0, process, apply);
}
