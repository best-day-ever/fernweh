#include <QDir>
#include <QGuiApplication>
#include <QImage>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
#include <QtQml>

// Render the shipped QML with documentation fixtures, never a real workstation,
// account, QSettings store, network connection or streaming session.
int main(int argc, char** argv)
{
    QGuiApplication app(argc, argv);
    if (argc != 4) {
        qCritical("Usage: fernweh-doc-capture <fixture-dir> <page:0|1> <output.png>");
        return 2;
    }
    const QDir fixtures(QString::fromLocal8Bit(argv[1]));
    const QString page = QString::fromLocal8Bit(argv[2]);
    if (page != "0" && page != "1") return 2;
    const QString output = QString::fromLocal8Bit(argv[3]);
    QQuickStyle::setStyle("Material");
    qmlRegisterSingletonType(QUrl::fromLocalFile(fixtures.filePath("DisplayFixture.qml")),
                             "DisplaySetup", 1, 0, "DisplaySetup");
    QQmlApplicationEngine engine;
    bool warnings = false;
    QObject::connect(&engine, &QQmlEngine::warnings, &app,
                     [&warnings](const QList<QQmlError>&) { warnings = true; });
    engine.rootContext()->setContextProperty("capturePage", page.toInt());
    engine.load(QUrl::fromLocalFile(fixtures.filePath("Capture.qml")));
    if (engine.rootObjects().size() != 1) return 1;
    auto* window = qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    if (!window) return 1;
    QTimer::singleShot(800, &app, [&] {
        const QImage screenshot = window->grabWindow();
        if (warnings || screenshot.isNull() || !screenshot.save(output)) {
            qCritical("Documentation capture failed or produced QML warnings");
            app.exit(1);
            return;
        }
        qInfo("Captured page %s: %d x %d", qPrintable(page), screenshot.width(), screenshot.height());
        app.exit(0);
    });
    return app.exec();
}
