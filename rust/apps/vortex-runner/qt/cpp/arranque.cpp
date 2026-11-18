// El arranque de vortex-runner-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente, el ícono). Los QObject `Puente` y `Colores` son de Rust (src/puente.rs, src/colores.rs).
#include "arranque.h"

#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <cstdio>
#include <functional>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-runner-qt/src/colores.cxxqt.h"
#include "vortex-runner-qt/src/puente.cxxqt.h"

namespace {

QString memoria(const char *clave)
{
    QFile estado(QStringLiteral("/proc/self/status"));
    if (estado.open(QIODevice::ReadOnly))
        for (const auto &l : QString::fromUtf8(estado.readAll()).split(u'\n'))
            if (l.startsWith(QString::fromLatin1(clave) + u':'))
                return l.mid(static_cast<qsizetype>(qstrlen(clave)) + 1).trimmed();
    return {};
}

// Lo que recorre `pruebas/test_interfaz.py`: la biblioteca, la búsqueda, «Configurar», «Quitar» y la
// pregunta de un .exe abierto desde fuera; mide lo que ve y guarda capturas si se pide.
void recorrer(QGuiApplication &app, QObject *ventana, Puente *runner, const QStringList &errores)
{
    auto datos = std::make_shared<QJsonObject>();
    auto hijo = [ventana](const char *nombre) { return ventana->findChild<QObject *>(QString::fromLatin1(nombre)); };
    const QString capturas = qEnvironmentVariable("VORTEX_RUNNER_CAPTURAS");
    auto capturar = [ventana, capturas](const QString &nombre) {
        if (auto *w = qobject_cast<QQuickWindow *>(ventana); w && !capturas.isEmpty())
            w->grabWindow().save(QDir(capturas).filePath(nombre + QStringLiteral(".png")));
    };
    auto visible = [hijo](const char *nombre) {
        auto *o = hijo(nombre);
        return o && o->property("visible").toBool();
    };
    auto *busqueda = hijo("busqueda");
    auto *rejilla = hijo("rejilla");

    std::vector<std::function<void()>> pasos;
    pasos.push_back([=] {
        (*datos)[QStringLiteral("programas")] = runner->property("programas").toList().size();
        (*datos)[QStringLiteral("falta")] = QJsonArray::fromStringList(runner->property("faltaMotor").toStringList());
        (*datos)[QStringLiteral("celdas")] = rejilla ? rejilla->property("count").toInt() : -1;
        capturar(QStringLiteral("biblioteca"));
        if (busqueda)
            busqueda->setProperty("text", QStringLiteral("otro"));
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("buscada")] = rejilla ? rejilla->property("count").toInt() : -1;
        capturar(QStringLiteral("buscada"));
        if (busqueda)
            busqueda->setProperty("text", QString());
        if (!runner->property("programas").toList().isEmpty())
            QMetaObject::invokeMethod(ventana, "configurar", Q_ARG(QVariant, QVariant(0)));
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("config_abierto")] = visible("dialogoPrograma");
        capturar(QStringLiteral("configurar"));
        hijo("dialogoPrograma")->setProperty("visible", false);
        if (!runner->property("programas").toList().isEmpty())
            QMetaObject::invokeMethod(ventana, "pedirQuitar", Q_ARG(QVariant, QVariant(0)));
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("quitar_abierto")] = visible("dialogoQuitar");
        capturar(QStringLiteral("quitar"));
        hijo("dialogoQuitar")->setProperty("visible", false);
        const QString setup = qEnvironmentVariable("VORTEX_RUNNER_PRUEBA_ARCHIVO");
        if (!setup.isEmpty())
            QMetaObject::invokeMethod(runner, "abrirArchivo", Q_ARG(QString, setup));
    });
    pasos.push_back([=] {
        if (auto *d = hijo("dialogoArchivo")) {
            (*datos)[QStringLiteral("archivo_abierto")] = visible("dialogoArchivo");
            (*datos)[QStringLiteral("archivo_instalador")] = d->property("instalador").toBool();
            (*datos)[QStringLiteral("archivo_nombre")] = d->property("nombre").toString();
        }
        capturar(QStringLiteral("pregunta-archivo"));
        QMetaObject::invokeMethod(runner, "aviso", Q_ARG(QString, QStringLiteral("Listo: lo agregué a tu biblioteca.")),
                                  Q_ARG(QString, QStringLiteral("exito")));
    });
    pasos.push_back([=, &app] {
        if (auto *a = hijo("aviso"))
            (*datos)[QStringLiteral("aviso")] = a->property("texto").toString();
        (*datos)[QStringLiteral("errores")] = QJsonArray::fromStringList(errores);
        (*datos)[QStringLiteral("vmhwm")] = memoria("VmHWM");
        (*datos)[QStringLiteral("rss")] = memoria("VmRSS");
        (*datos)[QStringLiteral("rss_anon")] = memoria("RssAnon");
        std::printf("DATOS%s\n", QJsonDocument(*datos).toJson(QJsonDocument::Compact).constData());
        std::fflush(stdout);
        app.quit();
    });
    int espera = 500;
    for (auto &p : pasos) {
        QTimer::singleShot(espera, &app, p);
        espera += 400;
    }
}

} // namespace

int vortex_runner_correr(rust::Str qml, rust::Str archivo, bool prueba)
{
    static int argc = 1;
    static char nombre[] = "vortex-runner";
    static char *argv[] = {nombre, nullptr};
    QGuiApplication app(argc, argv);
    const QString ruta_qml = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    QGuiApplication::setApplicationName(QStringLiteral("vortex-runner"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine's Runner"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-runner"));
    QGuiApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("vortex-runner"),
                                                    QIcon(QFileInfo(ruta_qml).absolutePath() + QStringLiteral("/logo.svg"))));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setFamilies({QStringLiteral("Adwaita Sans"), QStringLiteral("Noto Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    auto *runner = new Puente();
    runner->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("runner"), runner);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta_qml));
    if (motor.rootObjects().isEmpty())
        return 1;
    if (prueba)
        recorrer(app, motor.rootObjects().first(), runner, errores);
    else if (archivo.size() > 0)
        QMetaObject::invokeMethod(runner, "abrirArchivo",
                                  Q_ARG(QString, QString::fromUtf8(archivo.data(), static_cast<qsizetype>(archivo.size()))));
    return app.exec();
}
