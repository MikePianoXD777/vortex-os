// El arranque de miku-rs: lo de `app.py` que cxx-qt-lib no trae (el app_id de Wayland, setContextProperty, el
// estilo, la fuente, SIGTERM del lanzador) y el modo de pruebas (lo de `pruebas/panel.py` y `pagina.py`).
#include "arranque.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QSocketNotifier>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QKeyEvent>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <cstdio>
#include <functional>
#include <signal.h>
#include <sys/socket.h>
#include <unistd.h>

// Encabezado que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-miku-qt/src/puente.cxxqt.h"

QVariant json_a_variante(rust::Str texto)
{
    const QByteArray cuerpo = "[" + QByteArray(texto.data(), static_cast<qsizetype>(texto.size())) + "]";
    const auto doc = QJsonDocument::fromJson(cuerpo);
    if (!doc.isArray() || doc.array().isEmpty())
        return {};
    return doc.array().at(0).toVariant();
}

// SIGTERM / SIGINT (el lanzador cierra así): un cierre limpio por el bucle de Qt, sin hacer nada dentro del manejador.
static int tuberia[2];
static void al_senal(int)
{
    const char x = 1;
    [[maybe_unused]] auto r = ::write(tuberia[1], &x, 1);
}

// --- modo de pruebas ---

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

static QQuickWindow *ventana(const QString &nombre)
{
    for (auto *w : QGuiApplication::allWindows())
        if (w->objectName() == nombre)
            return qobject_cast<QQuickWindow *>(w);
    return nullptr;
}

static void simular(QObject *mk, const QJsonArray &senal)
{
    QMetaObject::invokeMethod(mk, "simular", Qt::DirectConnection,
                              Q_ARG(QString, QString::fromUtf8(QJsonDocument(senal).toJson(QJsonDocument::Compact))));
    QCoreApplication::processEvents();
}

// Una lista de botones: [[id, texto], ...] (un QJsonArray con una sola lista se confundiría con una copia).
static QJsonArray pares(const QList<QPair<QString, QString>> &l)
{
    QJsonArray r;
    for (const auto &p : l) {
        QJsonArray a;
        a.append(p.first);
        a.append(p.second);
        r.append(a);
    }
    return r;
}

static QString registro(QObject *mk)
{
    QString r;
    QMetaObject::invokeMethod(mk, "registro", Qt::DirectConnection, Q_RETURN_ARG(QString, r));
    return r;
}

// Por nombre, en el árbol visual (lo que crea un Repeater no cuelga del ventana en el árbol de QObject).
static QObject *buscar(QQuickItem *item, const QString &nombre)
{
    if (item->objectName() == nombre)
        return item;
    for (auto *h : item->childItems())
        if (auto *r = buscar(h, nombre))
            return r;
    return nullptr;
}

static QObject *hijo(QQuickWindow *v, const char *nombre)
{
    return buscar(v->contentItem(), QString::fromUtf8(nombre));
}

static QString texto(QQuickWindow *v, const char *nombre)
{
    auto *o = hijo(v, nombre);
    return o ? o->property("text").toString() : QString();
}

static bool visible(QQuickWindow *v, const char *nombre)
{
    auto *o = hijo(v, nombre);
    return o && o->property("visible").toBool();
}

static QImage captura(QQuickWindow *v, const char *archivo)
{
    const QString dir = qEnvironmentVariable("VORTEX_MIKU_CAPTURA_DIR");
    const QImage img = v->grabWindow();
    if (!dir.isEmpty())
        img.save(dir + QStringLiteral("/") + QString::fromUtf8(archivo));
    return img;
}

static void tecla(QQuickWindow *v, Qt::Key k)
{
    QKeyEvent p(QEvent::KeyPress, k, Qt::NoModifier);
    QKeyEvent s(QEvent::KeyRelease, k, Qt::NoModifier);
    QCoreApplication::sendEvent(v, &p);
    QCoreApplication::sendEvent(v, &s);
    QCoreApplication::processEvents();
}

static void pico_de_memoria(QJsonObject &d)
{
    QFile estado(QStringLiteral("/proc/self/status"));
    if (estado.open(QIODevice::ReadOnly))
        for (const auto &l : QString::fromUtf8(estado.readAll()).split(u'\n'))
            if (l.startsWith(QStringLiteral("VmHWM:")) || l.startsWith(QStringLiteral("VmRSS:")) || l.startsWith(QStringLiteral("RssAnon:")))
                d[l.section(u':', 0, 0).toLower()] = l.section(u':', 1).trimmed();
}

// ¿Hay un QtWebEngineProcess hijo nuestro? Sin avatar no debe haber ninguno (ahorra cientos de MB de RAM).
static bool web_engine_cargado()
{
    const QString yo = QString::number(QCoreApplication::applicationPid());
    const QDir proc(QStringLiteral("/proc"));
    for (const QString &p : proc.entryList(QDir::Dirs | QDir::NoDotAndDotDot)) {
        bool ok = false;
        p.toInt(&ok);
        if (!ok)
            continue;
        QFile estado(QStringLiteral("/proc/") + p + QStringLiteral("/status"));
        if (!estado.open(QIODevice::ReadOnly))
            continue;
        const QByteArray t = estado.readAll();
        if (t.contains("PPid:\t" + yo.toUtf8() + "\n") && t.startsWith("Name:\tQtWebEngineProc"))
            return true;
    }
    return false;
}

static void prueba_panel(QGuiApplication &app, QObject *mk, const QStringList &errores)
{
    auto *v = ventana(QStringLiteral("ventanaPanel"));
    QJsonObject d;
    if (!v) {
        d["sin_ventana"] = true;
        d["errores"] = QJsonArray::fromStringList(errores);
        std::printf("DATOS%s\n", QJsonDocument(d).toJson(QJsonDocument::Compact).constData());
        app.quit();
        return;
    }
    QCoreApplication::processEvents();
    d["app_id"] = QGuiApplication::desktopFileName();
    d["titulo"] = v->title();
    d["ancho"] = v->width();
    d["alto"] = v->height();
    d["fondo"] = mk->property("paleta").toMap().value("fondo").toString();
    d["web_engine_proceso"] = web_engine_cargado();

    // Primera vez: la descarga de los modelos se ve con su barra.
    simular(mk, {"progreso", "Preparando la voz de Miku (una sola vez, ~560 MB)…", "Bajando el oído 90 MB de 590 MB", 0.15});
    d["progreso_visible"] = visible(v, "preparando");
    d["progreso_titulo"] = texto(v, "progresoTitulo");
    d["progreso_fraccion"] = hijo(v, "progresoBarra") ? hijo(v, "progresoBarra")->property("fraccion").toDouble() : -1;
    captura(v, "panel-preparando.png");
    simular(mk, {"progreso", "", "", -1.0});
    d["progreso_oculto"] = !visible(v, "preparando");

    // Claude apagado: el globo dice qué hacer y trae su botón.
    simular(mk, {"respuesta", "Para hablar conmigo necesito tu cuenta de Claude.", true});
    simular(mk, {"acciones", pares({{"activar", "Activar Claude"}})});
    esperar([&] { return hijo(v, "boton0") != nullptr; }, 3000);
    d["boton_texto"] = hijo(v, "boton0") ? hijo(v, "boton0")->property("texto").toString() : QString();
    d["boton_visible"] = visible(v, "boton0") && visible(v, "botones");
    if (auto *b = hijo(v, "boton0"))
        QMetaObject::invokeMethod(b, "clicked", Qt::DirectConnection);
    simular(mk, {"acciones", pares({{"sesion", "Iniciar sesión"}})});
    esperar([&] { return hijo(v, "boton0") && hijo(v, "boton0")->property("texto").toString() == "Iniciar sesión"; }, 3000);
    d["boton2_texto"] = hijo(v, "boton0") ? hijo(v, "boton0")->property("texto").toString() : QString();
    simular(mk, {"acciones", pares({})});
    esperar([&] { return !hijo(v, "boton0"); }, 3000);
    d["sin_botones"] = !visible(v, "botones");

    simular(mk, {"estado", "escuchando", "Escuchando..."});
    simular(mk, {"nivel", 0.7});
    d["escuchando_texto"] = texto(v, "estadoTexto");
    d["mic_activo"] = hijo(v, "mic") && hijo(v, "mic")->property("activo").toBool();
    d["punto_activo"] = hijo(v, "punto") && hijo(v, "punto")->property("activo").toBool();
    captura(v, "panel-escuchando.png");

    simular(mk, {"usuario", "¿Qué hora es en Tokio?"});
    simular(mk, {"estado", "pensando", "Pensando..."});
    simular(mk, {"respuesta", "", false});
    d["pensando_puntos"] = texto(v, "respuesta");
    simular(mk, {"estado", "hablando", ""});
    simular(mk, {"respuesta", "En Tokio son las nueve y cuarto de la mañana. Si quieres, te ayudo a poner un reloj con esa zona horaria en el escritorio.", true});
    d["hablando_texto"] = texto(v, "estadoTexto");
    d["respuesta"] = texto(v, "respuesta");
    d["tu"] = texto(v, "tu");
    captura(v, "panel-respuesta.png");

    simular(mk, {"aviso", "No tengo internet, así que no puedo pensar ahora mismo."});
    d["aviso"] = texto(v, "respuesta");

    simular(mk, {"camara", true});
    d["camara_visible"] = visible(v, "camara");
    simular(mk, {"camara", false});
    d["camara_oculta"] = !visible(v, "camara");

    // Escribir y Enter.
    if (auto *e = hijo(v, "entrada")) {
        e->setProperty("text", "abre la terminal");
        QMetaObject::invokeMethod(e, "accepted", Qt::DirectConnection);
        QCoreApplication::processEvents();
        d["entrada_vacia"] = e->property("text").toString().isEmpty();
    }
    // El micrófono: un clic escucha; si ya escucha, corta.
    simular(mk, {"estado", "reposo", ""});
    QMetaObject::invokeMethod(hijo(v, "mic"), "clicked", Qt::DirectConnection);
    simular(mk, {"estado", "escuchando", "Escuchando..."});
    QMetaObject::invokeMethod(hijo(v, "mic"), "clicked", Qt::DirectConnection);
    // Esc cierra todo.
    if (auto *t = hijo(v, "tarjeta"))
        QMetaObject::invokeMethod(t, "forceActiveFocus", Qt::DirectConnection);
    tecla(v, Qt::Key_Escape);
    d["registro"] = registro(mk);

    pico_de_memoria(d);
    d["errores"] = QJsonArray::fromStringList(errores);
    std::printf("DATOS%s\n", QJsonDocument(d).toJson(QJsonDocument::Compact).constData());
    std::fflush(stdout);
    app.quit();
}

static void prueba_avatar(QGuiApplication &app, QObject *mk, const QStringList &errores)
{
    auto *v = ventana(QStringLiteral("ventanaAvatar"));
    QJsonObject d;
    if (!v) {
        d["sin_ventana"] = true;
        d["panel"] = ventana(QStringLiteral("ventanaPanel")) != nullptr;
    } else {
        d["app_id"] = QGuiApplication::desktopFileName();
        d["titulo"] = v->title();
        d["ancho"] = v->width();
        d["alto"] = v->height();
        auto *web = hijo(v, "web");
        // La página carga y conecta el canal: llama a puente.listo().
        d["pagina_lista"] = esperar([&] { return registro(mk).contains("listo"); }, 90000);
        d["url"] = web ? web->property("url").toString() : QString();
        // Lo del asistente mueve la página (sin errores de JavaScript).
        simular(mk, {"estado", "escuchando", "Escuchando..."});
        simular(mk, {"usuario", "hola Miku"});
        simular(mk, {"respuesta", "Hola, Mike.", true});
        simular(mk, {"emocion", "feliz", 0.8});
        simular(mk, {"sonrojo", 0.5});
        simular(mk, {"afecto", 60, "cariño"});
        simular(mk, {"aviso", "Un aviso."});
        simular(mk, {"nivel", 0.5});
        // La página le habla al asistente por el puente.
        auto js = [&](const char *c) {
            QMetaObject::invokeMethod(mk, "js", Qt::DirectConnection, Q_ARG(QString, QString::fromUtf8(c)));
        };
        js("miku.puente.enviarTexto('hola desde la pagina')");
        js("miku.puente.microfono()");
        js("miku.puente.accion('activar')");
        js("miku.puente.cerrar()");
        d["puente_ok"] = esperar([&] { const auto r = registro(mk); return r.contains("enviar:hola desde la pagina") && r.contains("cerrar") && r.contains("accion:activar"); }, 20000);
        QThread::msleep(1500);
        QCoreApplication::processEvents();
        if (!qEnvironmentVariable("VORTEX_MIKU_CAPTURA_DIR").isEmpty()) {
            const QImage img = captura(v, "avatar.png");
            d["esquina_alfa"] = img.isNull() ? -1 : qAlpha(img.pixel(2, 2));
        }
        d["registro"] = registro(mk);
    }
    pico_de_memoria(d);
    d["errores"] = QJsonArray::fromStringList(errores);
    std::printf("DATOS%s\n", QJsonDocument(d).toJson(QJsonDocument::Compact).constData());
    std::fflush(stdout);
    app.quit();
}

int vortex_miku_correr(rust::Str qml, bool avatar, bool modo_prueba)
{
    static int argc = 1;
    static char nombre[] = "vortex-miku";
    static char *argv[] = {nombre, nullptr};
    // QtWebEngine tiene que iniciarse antes de crear la app; sin avatar ni se toca (~500 MB de RAM que no se gastan).
    if (avatar)
        QtWebEngineQuick::initialize();
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    // El app_id de Wayland: lo que casan las reglas de Hyprland (class = "vortex-miku"). Va ANTES de crear la app.
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-miku"));
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("vortex-miku"));
    QGuiApplication::setQuitOnLastWindowClosed(true);
    QFont fuente;
    fuente.setFamilies({QStringLiteral("Google Sans Flex"), QStringLiteral("Noto Sans"), QStringLiteral("Cantarell"),
                        QStringLiteral("DejaVu Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);

    if (::socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, tuberia) == 0) {
        auto *aviso = new QSocketNotifier(tuberia[0], QSocketNotifier::Read, &app);
        QObject::connect(aviso, &QSocketNotifier::activated, &app, [&app] {
            char x;
            [[maybe_unused]] auto r = ::read(tuberia[0], &x, 1);
            app.quit();
        });
        struct sigaction sa = {};
        sa.sa_handler = al_senal;
        sigemptyset(&sa.sa_mask);
        sigaction(SIGTERM, &sa, nullptr);
        sigaction(SIGINT, &sa, nullptr);
    }

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    auto *mk = new Miku();
    mk->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("mk"), mk);
    motor.load(QUrl::fromLocalFile(QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()))));
    if (motor.rootObjects().isEmpty()) {
        for (const auto &e : errores)
            std::fprintf(stderr, "miku: %s\n", e.toUtf8().constData());
        return 1;
    }
    // Al salir: el asistente cierra la voz, el micrófono y lo que tenga abierto.
    QObject::connect(&app, &QCoreApplication::aboutToQuit, mk, [mk] {
        QMetaObject::invokeMethod(mk, "cerrarTodo", Qt::DirectConnection);
    });
    // El asistente se pone en marcha con la ventana ya puesta.
    QTimer::singleShot(0, mk, [mk] { QMetaObject::invokeMethod(mk, "iniciar", Qt::DirectConnection); });
    // La ventana (en el panel o el avatar) pide cerrarse.
    QObject::connect(mk, &Miku::cerrarPedido, &app, &QCoreApplication::quit);
    if (modo_prueba)
        QTimer::singleShot(400, &app, [&] {
            if (avatar && ventana(QStringLiteral("ventanaAvatar")))
                prueba_avatar(app, mk, errores);
            else
                prueba_panel(app, mk, errores);
        });
    return app.exec();
}
