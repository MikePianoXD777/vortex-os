// El arranque de vortex-shop-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente, el ícono). Los QObject `Puente` («tienda») y `Colores` son de Rust.
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
#include "vortex-shop-qt/src/colores.cxxqt.h"
#include "vortex-shop-qt/src/puente.cxxqt.h"

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

// El recorrido de `pruebas/ui_capturas.py` y `ui_android.py`: Inicio, una categoría, la búsqueda, el
// detalle, Instaladas, Actualizaciones, «Quitar» y «Juegos de Android» (sin preparar, preparando y lista).
// Mide lo que ve y guarda capturas si se pide (VORTEX_SHOP_CAPTURAS=/carpeta).
void recorrer(QGuiApplication &app, QObject *ventana, Puente *tienda, const QStringList &errores)
{
    auto datos = std::make_shared<QJsonObject>();
    const QString capturas = qEnvironmentVariable("VORTEX_SHOP_CAPTURAS");
    auto capturar = [ventana, capturas](const QString &nombre) {
        if (auto *w = qobject_cast<QQuickWindow *>(ventana); w && !capturas.isEmpty())
            w->grabWindow().save(QDir(capturas).filePath(nombre + QStringLiteral(".png")));
    };
    auto llamar = [ventana](const char *metodo, const QStringList &args) {
        switch (args.size()) {
        case 0: QMetaObject::invokeMethod(ventana, metodo); break;
        case 1: QMetaObject::invokeMethod(ventana, metodo, Q_ARG(QVariant, args[0])); break;
        default: QMetaObject::invokeMethod(ventana, metodo, Q_ARG(QVariant, args[0]), Q_ARG(QVariant, args[1])); break;
        }
    };
    auto n = [](const QVariant &v) { return v.toList().size(); };

    std::vector<std::function<void()>> pasos;
    pasos.push_back([=] {
        (*datos)[QStringLiteral("inicio_estado")] = tienda->property("inicioEstado").toString();
        (*datos)[QStringLiteral("recomendadas")] = n(tienda->property("recomendadas"));
        (*datos)[QStringLiteral("categorias")] = n(tienda->property("categorias"));
        (*datos)[QStringLiteral("instaladas")] = n(tienda->property("instaladas"));
        capturar(QStringLiteral("01-inicio"));
        llamar("navegar", {QStringLiteral("categoria"), QStringLiteral("Game")});
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("categoria_estado")] = tienda->property("categoriaEstado").toString();
        (*datos)[QStringLiteral("categoria_apps")] = n(tienda->property("categoriaApps"));
        capturar(QStringLiteral("02-categoria"));
        llamar("escribirBusqueda", {QStringLiteral("video")});
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("busqueda_estado")] = tienda->property("busquedaEstado").toString();
        (*datos)[QStringLiteral("busqueda_apps")] = n(tienda->property("busquedaApps"));
        capturar(QStringLiteral("03-busqueda"));
        llamar("verDetalle", {QStringLiteral("com.valvesoftware.Steam")});
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("detalle_estado")] = tienda->property("detalleEstado").toString();
        capturar(QStringLiteral("04-detalle"));
        llamar("navegar", {QStringLiteral("instaladas"), QString()});
    });
    pasos.push_back([=] {
        capturar(QStringLiteral("05-instaladas"));
        llamar("navegar", {QStringLiteral("actualizaciones"), QString()});
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("actualizaciones")] = tienda->property("numActualizaciones").toInt();
        capturar(QStringLiteral("06-actualizaciones"));
        llamar("pedirQuitar", {QStringLiteral("org.videolan.VLC"), QStringLiteral("VLC")});
    });
    pasos.push_back([=] {
        capturar(QStringLiteral("07-quitar-confirmar"));
        llamar("cerrarDialogos", {});
        llamar("navegar", {QStringLiteral("android"), QString()});
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("android_cargado")] = tienda->property("androidCargado").toBool();
        capturar(QStringLiteral("08-android-sin-preparar"));
        QMetaObject::invokeMethod(tienda, "prepararAndroid");
    });
    pasos.push_back([=] {
        (*datos)[QStringLiteral("android_fase")] = tienda->property("androidFase").toString();
        capturar(QStringLiteral("09-android-preparando"));
    });
    pasos.push_back([=] {
        capturar(QStringLiteral("10-android-lista"));
        (*datos)[QStringLiteral("android_apps")] = n(tienda->property("androidApps"));
        (*datos)[QStringLiteral("android_preparado")] =
            tienda->property("androidEstado").toMap().value(QStringLiteral("preparado")).toBool();
    });
    pasos.push_back([=, &app] {
        // Un aviso de error, como en la de Python.
        QMetaObject::invokeMethod(tienda, "aviso", Q_ARG(QString, QStringLiteral("No se pudo instalar Blender: se acabó el espacio en disco.")));
    });
    pasos.push_back([=, &app] {
        capturar(QStringLiteral("11-aviso-error"));
        (*datos)[QStringLiteral("errores")] = QJsonArray::fromStringList(errores);
        (*datos)[QStringLiteral("vmhwm")] = memoria("VmHWM");
        (*datos)[QStringLiteral("rss")] = memoria("VmRSS");
        (*datos)[QStringLiteral("rss_anon")] = memoria("RssAnon");
        std::printf("DATOS%s\n", QJsonDocument(*datos).toJson(QJsonDocument::Compact).constData());
        std::fflush(stdout);
        app.quit();
    });
    const int espera_base = qEnvironmentVariableIntValue("VORTEX_SHOP_PAUSA");
    const int paso = espera_base > 0 ? espera_base : 1300;
    int espera = 1500;
    for (auto &p : pasos) {
        QTimer::singleShot(espera, &app, p);
        espera += paso;
    }
}

} // namespace

int vortex_shop_correr(rust::Str qml, bool prueba)
{
    static int argc = 1;
    static char nombre[] = "vortex-shop";
    static char *argv[] = {nombre, nullptr};
    QQuickStyle::setStyle(QStringLiteral("Basic")); // sin el aspecto nativo: todo con los colores de la tienda
    QGuiApplication app(argc, argv);
    const QString ruta_qml = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    QGuiApplication::setApplicationName(QStringLiteral("vortexines-shop"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine's Shop"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortexines-shop"));
    QGuiApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("vortexines-shop"),
                                                    QIcon(QFileInfo(ruta_qml).absolutePath() + QStringLiteral("/vortex-os.svg"))));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setFamilies({QStringLiteral("Adwaita Sans"), QStringLiteral("Noto Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    auto *tienda = new Puente();
    tienda->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("tienda"), tienda);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta_qml));
    if (motor.rootObjects().isEmpty())
        return 1;
    QMetaObject::invokeMethod(tienda, "iniciar");
    if (prueba)
        recorrer(app, motor.rootObjects().first(), tienda, errores);
    return app.exec();
}
