// El arranque de vortex-connect-rs: lo de `ventana.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente, el ícono). Los QObject `Puente` («vc») y `Colores` son de Rust (src/puente.rs, src/colores.rs).
#include "arranque.h"

#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
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
#include <memory>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-connect-qt/src/colores.cxxqt.h"
#include "vortex-connect-qt/src/puente.cxxqt.h"

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

// Lo que recorre `pruebas/test_interfaz.py`: pide las condiciones antes de cifrar, protege, quita la
// protección y prende el modo viaje; mide lo que ve y guarda capturas si se pide.
void recorrer(QGuiApplication &app, QObject *ventana, Puente *vc, const QStringList &errores)
{
    auto datos = std::make_shared<QJsonObject>();
    auto hijo = [ventana](const char *nombre) { return ventana->findChild<QObject *>(QString::fromLatin1(nombre)); };
    const QString capturas = qEnvironmentVariable("VORTEX_CONNECT_CAPTURAS");
    auto capturar = [ventana, capturas](const QString &nombre) {
        if (auto *w = qobject_cast<QQuickWindow *>(ventana); w && !capturas.isEmpty())
            w->grabWindow().save(QDir(capturas).filePath(nombre + QStringLiteral(".png")));
    };
    auto texto = [hijo](const char *nombre) {
        auto *o = hijo(nombre);
        return o ? o->property("text").toString() : QString();
    };
    auto clic = [hijo](const char *nombre) {
        if (auto *o = hijo(nombre))
            QMetaObject::invokeMethod(o, "clicked");
    };
    auto jarray = [](const QVariantList &l) { return QJsonArray::fromVariantList(l); };

    // paso 1 (1,5 s)
    QTimer::singleShot(1500, &app, [=] {
        (*datos)[QStringLiteral("inicio")] = jarray({vc->property("estado"), vc->property("redActual"),
                                                     vc->property("redAbierta"), texto("tituloEstado")});
        QVariantList ssids;
        for (const auto &r : vc->property("redes").toList())
            ssids << r.toMap().value(QStringLiteral("ssid"));
        (*datos)[QStringLiteral("redes")] = jarray(ssids);
        capturar(QStringLiteral("inicio"));
        ventana->setProperty("pidiendoCondiciones", false);
        clic("botonProteger");
        auto *velo = hijo("veloCondiciones");
        (*datos)[QStringLiteral("velo")] = velo && velo->property("visible").toBool();
        (*datos)[QStringLiteral("sin_aceptar")] = vc->property("condiciones").toBool();
        capturar(QStringLiteral("condiciones"));
        clic("botonAceptar");
        (*datos)[QStringLiteral("aceptadas")] = vc->property("condiciones").toBool();
        (*datos)[QStringLiteral("velo_tras")] = velo && velo->property("visible").toBool();
    });
    // paso 2 (+2,5 s): ya cifró
    QTimer::singleShot(4000, &app, [=] {
        (*datos)[QStringLiteral("protegido")] = jarray({vc->property("estado"), texto("tituloEstado"),
                                                        vc->property("ocupado"), vc->property("mensaje")});
        capturar(QStringLiteral("protegido"));
        QMetaObject::invokeMethod(vc, "quitarProteccion");
    });
    // paso 3 (+1,5 s): sin protección y modo viaje
    QTimer::singleShot(5500, &app, [=, &app] {
        (*datos)[QStringLiteral("sin_proteger")] = jarray({vc->property("estado"), vc->property("ocupado")});
        capturar(QStringLiteral("sin-proteger"));
        if (auto *interruptor = hijo("interruptorViaje")) {
            interruptor->setProperty("checked", true); // lo que hace un clic de verdad antes de avisar
            QMetaObject::invokeMethod(interruptor, "clicked");
        }
        (*datos)[QStringLiteral("viaje")] = vc->property("modoViaje").toBool();
        (*datos)[QStringLiteral("errores")] = QJsonArray::fromStringList(errores);
        (*datos)[QStringLiteral("vmhwm")] = memoria("VmHWM");
        (*datos)[QStringLiteral("rss")] = memoria("VmRSS");
        (*datos)[QStringLiteral("rss_anon")] = memoria("RssAnon");
        std::printf("DATOS%s\n", QJsonDocument(*datos).toJson(QJsonDocument::Compact).constData());
        std::fflush(stdout);
        app.quit();
    });
}

} // namespace

int vortex_connect_correr(rust::Str qml, bool recorrer_ventana, bool humo)
{
    static int argc = 1;
    static char nombre[] = "vortex-connect";
    static char *argv[] = {nombre, nullptr};
    QGuiApplication app(argc, argv);
    const QString ruta_qml = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    QGuiApplication::setApplicationName(QStringLiteral("vortex-connect"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine Connect"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-connect"));
    QGuiApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("vortex-connect"),
                                                    QIcon(QFileInfo(ruta_qml).absolutePath() + QStringLiteral("/logo.svg"))));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setFamilies({QStringLiteral("Adwaita Sans"), QStringLiteral("Segoe UI"), QStringLiteral("Noto Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);
    QQuickStyle::setStyle(QStringLiteral("Basic"));

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    auto *vc = new Puente();
    vc->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("vc"), vc);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta_qml));
    if (motor.rootObjects().isEmpty())
        return 1;
    if (recorrer_ventana)
        recorrer(app, motor.rootObjects().first(), vc, errores);
    else if (humo)
        QTimer::singleShot(3000, &app, [&app] { app.quit(); });
    return app.exec();
}
