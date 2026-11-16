// El arranque de vortex-video-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente). Los QObject `Puente` y `Colores` son de Rust (src/puente.rs, src/colores.rs).
#include "arranque.h"

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuickControls2/QQuickStyle>
#include <cstdio>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»; revisarla al compilar).
#include "vortex-video-qt/src/colores.cxxqt.h"
#include "vortex-video-qt/src/puente.cxxqt.h"

int vortex_video_correr(rust::Str qml, bool prueba)
{
    // Un solo hilo de dibujo gasta menos memoria con NVIDIA (~45 MB), como el lanzador de Python.
    if (qEnvironmentVariableIsEmpty("QSG_RENDER_LOOP"))
        qputenv("QSG_RENDER_LOOP", "basic");

    static int argc = 1;
    static char nombre[] = "vortex-video";
    static char *argv[] = {nombre, nullptr};
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("Vortexine Video"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-video"));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    auto *vv = new Puente(&app);
    auto *colores = new Colores(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("vv"), vv);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()))));
    if (motor.rootObjects().isEmpty())
        return 1;

    if (prueba) {
        // Lo mismo que mide pruebas/test_interfaz.py.
        QObject *ventana = motor.rootObjects().first();
        QTimer::singleShot(800, &app, [&, ventana, vv] {
            QJsonObject datos;
            auto *mensaje = ventana->findChild<QObject *>(QStringLiteral("mensajeSinCamara"));
            auto *disparador = ventana->findChild<QObject *>(QStringLiteral("disparador"));
            datos[QStringLiteral("mensaje")] = mensaje ? mensaje->property("text").toString() : QString();
            datos[QStringLiteral("disparador")] = disparador ? disparador->property("enabled").toBool() : true;
            datos[QStringLiteral("ultima")] = vv->property("ultima").toString();
            datos[QStringLiteral("miniatura")] = vv->property("miniatura").toString();
            datos[QStringLiteral("errores")] = QJsonArray::fromStringList(errores);
            QMetaObject::invokeMethod(vv, "guardar", Q_ARG(QString, QStringLiteral("modo")),
                                      Q_ARG(QVariant, QVariant(QStringLiteral("video"))));
            std::printf("DATOS%s\n", QJsonDocument(datos).toJson(QJsonDocument::Compact).constData());
            std::fflush(stdout);
            app.quit();
        });
    }
    return app.exec();
}
