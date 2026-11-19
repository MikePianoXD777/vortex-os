// El arranque de vortex-camera-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente, callar a ffmpeg). Los QObject `Puente` y `Colores` son de Rust (src/puente.rs,
// src/colores.rs).
#include "arranque.h"

#include <QtCore/QDeadlineTimer>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <cstdio>
#include <dlfcn.h>
#include <functional>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-camera-qt/src/colores.cxxqt.h"
#include "vortex-camera-qt/src/puente.cxxqt.h"

QVariant json_a_variante(rust::Str texto)
{
    const QByteArray cuerpo = "[" + QByteArray(texto.data(), static_cast<qsizetype>(texto.size())) + "]";
    const auto doc = QJsonDocument::fromJson(cuerpo);
    if (!doc.isArray() || doc.array().isEmpty())
        return {};
    return doc.array().at(0).toVariant();
}

rust::String variante_a_json(const QVariant &valor)
{
    const QByteArray j = QJsonDocument(QJsonArray{QJsonValue::fromVariant(valor)}).toJson(QJsonDocument::Compact);
    const QByteArray sin = j.mid(1, j.size() - 2); // «[valor]» → «valor»
    return rust::String(sin.constData(), static_cast<std::size_t>(sin.size()));
}

// Qt Multimedia usa el ffmpeg del sistema, que en nivel «info» escribe la URL de cada cámara (con su código
// de seguridad) en la terminal y en el registro. Se deja solo en errores (AV_LOG_ERROR = 16). Se abre la
// misma libavutil que ya cargó Qt (misma soname = misma copia), así que el nivel le llega a ella.
static void callar_ffmpeg()
{
    for (int v = 62; v >= 56; --v) {
        const QByteArray nombre = "libavutil.so." + QByteArray::number(v);
        if (void *lib = dlopen(nombre.constData(), RTLD_NOW | RTLD_NOLOAD)) {
            if (auto f = reinterpret_cast<void (*)(int)>(dlsym(lib, "av_log_set_level")))
                f(16);
            dlclose(lib);
            return;
        }
    }
    for (int v = 62; v >= 56; --v) {
        const QByteArray nombre = "libavutil.so." + QByteArray::number(v);
        if (void *lib = dlopen(nombre.constData(), RTLD_NOW)) {
            if (auto f = reinterpret_cast<void (*)(int)>(dlsym(lib, "av_log_set_level")))
                f(16);
            return; // se queda abierta: es la que usará Qt Multimedia
        }
    }
}

// --- modo de pruebas (lo de pruebas/test_interfaz.py) ---

static void todos(QQuickItem *item, QList<QQuickItem *> &lista)
{
    lista << item;
    for (auto *hijo : item->childItems())
        todos(hijo, lista);
}

static QList<QQuickItem *> objetos(QQuickWindow *v, const char *prop)
{
    QList<QQuickItem *> l, r;
    todos(v->contentItem(), l);
    for (auto *o : l)
        if (o->metaObject()->indexOfProperty(prop) >= 0)
            r << o;
    return r;
}

static bool esperar(const std::function<bool()> &condicion, int ms = 15000)
{
    QElapsedTimer t;
    t.start();
    while (t.elapsed() < ms) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
        if (condicion())
            return true;
        QThread::msleep(20);
    }
    return false;
}

static QString agregar(QObject *cam, const QVariantMap &datos)
{
    QString r;
    QMetaObject::invokeMethod(cam, "agregarCamara", Qt::DirectConnection, Q_RETURN_ARG(QString, r),
                              Q_ARG(QVariant, QVariant(datos)));
    return r;
}

static void prueba(QGuiApplication &app, QQmlApplicationEngine &motor, QObject *cam, const QStringList &errores)
{
    auto *v = qobject_cast<QQuickWindow *>(motor.rootObjects().first());
    const QString codigo = qEnvironmentVariable("VORTEX_CAMERA_PRUEBA_CODIGO");
    const int puerto = qEnvironmentVariableIntValue("VORTEX_CAMERA_PRUEBA_PUERTO");
    QJsonObject d;
    d["vacio"] = agregar(cam, {{"nombre", ""}, {"codigo", "x"}, {"ip", "1.2.3.4"}});
    d["sin_codigo"] = agregar(cam, {{"nombre", "A"}, {"ip", "1.2.3.4"}});
    d["ip_mala"] = agregar(cam, {{"nombre", "A"}, {"codigo", "x"}, {"ip", "no es ip!"}});
    d["agregar"] = agregar(cam, {{"nombre", "Patio"}, {"codigo", codigo}, {"ip", "127.0.0.1"}, {"puerto", puerto}});
    agregar(cam, {{"nombre", "Calle"}, {"codigo", "MALO"}, {"ip", "127.0.0.1"}, {"puerto", puerto}});
    const QVariantList camaras = cam->property("camaras").toList();
    const QString patio = camaras.value(0).toMap().value("id").toString();
    const QString calle = camaras.value(1).toMap().value("id").toString();
    d["codigo_en_qml"] = QJsonDocument::fromVariant(camaras).toJson().contains(codigo.toUtf8());

    auto celdas = [&] {
        QList<QQuickItem *> r;
        for (auto *o : objetos(v, "camId"))
            if (o->metaObject()->indexOfProperty("pos") >= 0)
                r << o;
        return r;
    };
    auto celda = [&](const QString &id) -> QQuickItem * {
        for (auto *c : celdas())
            if (c->property("camId").toString() == id)
                return c;
        return nullptr;
    };
    auto estados = [&] { return cam->property("estados").toMap(); };

    d["mosaico_video"] = esperar([&] { auto *c = celda(patio); return c && c->property("conVideo").toBool(); });
    d["estados"] = esperar([&] {
        return estados().value(patio) == "en_linea" && estados().value(calle) == "codigo";
    });
    d["estados_valor"] = QJsonObject::fromVariantMap(estados());
    d["calle_sin_video"] = celda(calle) && !celda(calle)->property("conVideo").toBool();

    // vista grande: el mosaico suelta su video y la grande recibe el principal
    v->setProperty("grande", patio);
    QQuickItem *grande = nullptr;
    esperar([&] {
        for (auto *o : objetos(v, "audio"))
            if (o->metaObject()->indexOfProperty("camId") >= 0 && o->metaObject()->indexOfProperty("pos") < 0
                && o->property("url").toString().contains("subtype=0"))
                grande = o;
        return grande != nullptr;
    }, 5000);
    d["grande_url_principal"] = grande && grande->property("url").toString().contains("subtype=0");
    d["grande_video"] = grande && esperar([&] { return grande->property("conVideo").toBool(); });
    d["mosaico_suelta"] = celda(patio) && celda(patio)->property("url").toString().isEmpty();
    v->setProperty("grande", QString());

    // división y ventanas
    QMetaObject::invokeMethod(cam, "ponerDivision", Qt::DirectConnection, Q_ARG(int, 0), Q_ARG(int, 9));
    d["nueve"] = esperar([&] { return celdas().size() == 9; }, 5000);
    int i = -1;
    QMetaObject::invokeMethod(cam, "nuevaVentana", Qt::DirectConnection, Q_RETURN_ARG(int, i));
    QMetaObject::invokeMethod(cam, "renombrarVentana", Qt::DirectConnection, Q_ARG(int, i), Q_ARG(QString, "Calle"));
    QJsonArray nombres;
    for (const auto &w : cam->property("ventanas").toList())
        nombres.append(w.toMap().value("nombre").toString());
    d["ventanas"] = nombres;

    // grabaciones de la microSD
    v->setProperty("pagina", "grabaciones");
    QQuickItem *pag = nullptr;
    esperar([&] {
        for (auto *o : objetos(v, "fecha"))
            if (o->metaObject()->indexOfProperty("hora") >= 0)
                pag = o;
        return pag != nullptr;
    }, 5000);
    if (pag) {
        QMetaObject::invokeMethod(pag, "elegir", Qt::DirectConnection, Q_ARG(QVariant, QVariant(patio)));
        d["grabado_url"] = pag->property("url").toString().contains("/cam/playback?");
        d["grabado_video"] = esperar([&] { return pag->property("conVideo").toBool(); });
    }

    // El pico de memoria, para comparar con la de Python.
    QFile estado(QStringLiteral("/proc/self/status"));
    if (estado.open(QIODevice::ReadOnly))
        for (const auto &l : QString::fromUtf8(estado.readAll()).split(u'\n'))
            if (l.startsWith(QStringLiteral("VmHWM:")))
                d["vmhwm"] = l.mid(6).trimmed();
    const QString captura = qEnvironmentVariable("VORTEX_CAMERA_CAPTURA");
    if (!captura.isEmpty())
        v->grabWindow().save(captura);
    d["errores"] = QJsonArray::fromStringList(errores);
    QMetaObject::invokeMethod(cam, "cerrar", Qt::DirectConnection);
    std::printf("DATOS%s\n", QJsonDocument(d).toJson(QJsonDocument::Compact).constData());
    std::fflush(stdout);
    app.quit();
}

int vortex_camera_correr(rust::Str qml, bool modo_prueba)
{
    static int argc = 1;
    static char nombre[] = "vortex-camera";
    static char *argv[] = {nombre, nullptr};
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("vortex-camera"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine's Camera"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-camera"));
    const QString ruta = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    const QString carpeta = ruta.left(ruta.lastIndexOf(u'/'));
    QGuiApplication::setWindowIcon(
        QIcon::fromTheme(QStringLiteral("vortex-camera"), QIcon(carpeta + QStringLiteral("/logo.svg"))));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setFamilies({QStringLiteral("Adwaita Sans"), QStringLiteral("Noto Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);
    callar_ffmpeg();

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    auto *cam = new Puente();
    cam->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("cam"), cam);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta));
    callar_ffmpeg(); // por si el plugin de Qt Multimedia lo cambió al cargarse
    if (motor.rootObjects().isEmpty())
        return 1;
    // Al cerrar la ventana: las grabaciones se cierran bien (ffmpeg recibe «q») antes de salir.
    QObject::connect(&app, &QCoreApplication::aboutToQuit, cam, [cam] {
        QMetaObject::invokeMethod(cam, "cerrar", Qt::DirectConnection);
    });
    if (modo_prueba)
        QTimer::singleShot(300, &app, [&] { prueba(app, motor, cam, errores); });
    return app.exec();
}
