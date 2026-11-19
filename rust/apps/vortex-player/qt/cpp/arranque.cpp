// El arranque de vortex-player-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente) y lo poco de Qt que usa el puente (JSON ↔ QVariant, el portapapeles, el color de una
// portada, los datos medidos). Los QObject `Puente`, `Rep` y `Colores` son de Rust (src/*.rs).
#include "arranque.h"

#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QColor>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QImage>
#include <QtNetwork/QNetworkInformation>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <algorithm>
#include <cstdio>
#include <functional>
#include <utility>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-player-qt/src/colores.cxxqt.h"
#include "vortex-player-qt/src/puente.cxxqt.h"
#include "vortex-player-qt/src/rep.cxxqt.h"

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

void copiar_texto(rust::Str texto)
{
    if (auto *p = QGuiApplication::clipboard())
        p->setText(QString::fromUtf8(texto.data(), static_cast<qsizetype>(texto.size())));
}

// El promedio de los pixeles con más color de la portada, oscurecido para que se lea el texto (colorDe de
// puente.py, igual).
rust::String color_de_imagen(rust::Str archivo)
{
    QImage img(QString::fromUtf8(archivo.data(), static_cast<qsizetype>(archivo.size())));
    if (img.isNull())
        return rust::String();
    img = img.scaled(24, 24);
    double r = 0, g = 0, b = 0, peso = 0;
    for (int x = 0; x < img.width(); ++x)
        for (int y = 0; y < img.height(); ++y) {
            const QColor c = img.pixelColor(x, y);
            const double w = 0.05 + c.hsvSaturationF() * c.hsvSaturationF() * c.valueF();
            r += c.redF() * w;
            g += c.greenF() * w;
            b += c.blueF() * w;
            peso += w;
        }
    const QColor c = QColor::fromRgbF(float(r / peso), float(g / peso), float(b / peso));
    const float h = c.hsvHueF(), sat = std::min(0.75f, c.hsvSaturationF() * 1.2f),
                v = std::min(0.45f, std::max(0.28f, c.valueF()));
    const QByteArray nombre = QColor::fromHsvF(std::max(0.0f, h), sat, v).name().toUtf8();
    return rust::String(nombre.constData(), static_cast<std::size_t>(nombre.size()));
}

bool datos_medidos()
{
    static bool cargado = QNetworkInformation::loadDefaultBackend();
    (void)cargado;
    auto *i = QNetworkInformation::instance();
    return i && i->supports(QNetworkInformation::Feature::Metered) && i->isMetered();
}

void salir_de_la_app()
{
    QMetaObject::invokeMethod(qApp, "quit", Qt::QueuedConnection);
}

// --- modo de pruebas (lo de pruebas/test_interfaz.py) ---

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
    return condicion();
}

static void dormir(int ms)
{
    esperar([] { return false; }, ms);
}

static QJsonArray titulos(QObject *vp)
{
    QJsonArray a;
    for (const auto &c : vp->property("canciones").toList())
        a.append(c.toMap().value("titulo").toString());
    return a;
}

template <typename... Args> static void llamar(QObject *o, const char *metodo, Args &&...args)
{
    QMetaObject::invokeMethod(o, metodo, Qt::DirectConnection, std::forward<Args>(args)...);
}

static void prueba(QGuiApplication &app, QQmlApplicationEngine &motor, QObject *vp, QObject *rep,
                   const QStringList &errores)
{
    auto *v = qobject_cast<QQuickWindow *>(motor.rootObjects().first());
    QJsonObject d;
    d["escaneo"] = esperar([&] { return vp->property("canciones").toList().size() == 3; });
    d["totales"] = QJsonObject::fromVariantMap(vp->property("totales").toMap());
    d["canciones"] = titulos(vp);
    QJsonArray etiquetas;
    for (const auto &c : vp->property("canciones").toList())
        etiquetas.append(c.toMap().value("etiqueta").toString());
    d["etiquetas"] = etiquetas;

    // ❤️
    const QString ruta = vp->property("canciones").toList().value(0).toMap().value("ruta").toString();
    bool antes = true, despues = false;
    QMetaObject::invokeMethod(vp, "leGusta", Qt::DirectConnection, Q_RETURN_ARG(bool, antes), Q_ARG(QString, ruta),
                              Q_ARG(int, 0));
    const int version = vp->property("versionGustos").toInt();
    llamar(vp, "alternarMeGusta", Q_ARG(QString, ruta));
    QMetaObject::invokeMethod(vp, "leGusta", Qt::DirectConnection, Q_RETURN_ARG(bool, despues), Q_ARG(QString, ruta),
                              Q_ARG(int, 0));
    d["me_gusta"] = QJsonArray{antes, despues, vp->property("versionGustos").toInt() > version};

    llamar(vp, "ordenar", Q_ARG(QString, QStringLiteral("calidad")));
    d["por_calidad"] = titulos(vp);

    // tocar, la cola, siguiente, pausa y repetir
    llamar(vp, "tocar", Q_ARG(int, 0));
    esperar([&] { return rep->property("posicion").toDouble() > 0.3; }, 8000);
    d["sonando"] = rep->property("cancion").toMap().value("titulo").toString();
    d["avanza"] = rep->property("posicion").toDouble() > 0.3;
    d["cola"] = QJsonArray{rep->property("colaRestantes").toInt(), rep->property("colaSiguientes").toList().size()};
    v->setProperty("pagina", QStringLiteral("cola"));
    dormir(200);
    llamar(rep, "siguiente");
    esperar([&] { return rep->property("cancion").toMap().value("titulo").toString() != d["sonando"].toString(); }, 5000);
    d["siguiente"] = rep->property("cancion").toMap().value("titulo").toString();
    dormir(500);
    llamar(rep, "alternar");
    esperar([&] { return rep->property("pausado").toBool(); }, 5000);
    d["pausado"] = rep->property("pausado").toBool();
    llamar(rep, "alternarRepetir");
    llamar(rep, "alternarRepetir");
    dormir(100);
    d["repetir"] = rep->property("repetir").toString();

    // playlists
    int pid = -1;
    QMetaObject::invokeMethod(vp, "crearPlaylist", Qt::DirectConnection, Q_RETURN_ARG(int, pid),
                              Q_ARG(QString, QStringLiteral("Para correr")));
    llamar(vp, "agregarAPlaylist", Q_ARG(int, pid),
           Q_ARG(QString, vp->property("canciones").toList().value(2).toMap().value("ruta").toString()));
    llamar(vp, "verPlaylist", Q_ARG(int, pid));
    d["playlist"] = titulos(vp);
    QJsonArray nombres;
    for (const auto &p : vp->property("playlists").toList())
        nombres.append(p.toMap().value("nombre").toString());
    d["playlists"] = nombres;
    llamar(vp, "buscarParaPlaylist", Q_ARG(int, pid), Q_ARG(QString, QString()));
    d["para_agregar"] = vp->property("paraAgregar").toList().size();
    llamar(vp, "verPlaylist", Q_ARG(int, -1));

    // todas las páginas cargan sin errores
    for (const char *p : {"agregar", "ecualizador", "ajustes", "letra", "cola", "canciones"}) {
        v->setProperty("pagina", QString::fromLatin1(p));
        dormir(250);
    }
    QString color;
    QMetaObject::invokeMethod(vp, "colorDe", Qt::DirectConnection, Q_RETURN_ARG(QString, color), Q_ARG(QString, ruta));
    d["eq_bandas"] = vp->property("eq").toMap().value("bandas").toList().size();
    d["perfiles"] = vp->property("perfiles").toList().size();
    d["ajustes_navegadores"] = vp->property("ajustes").toMap().value("navegadores").toList().size();

    QFile estado(QStringLiteral("/proc/self/status"));
    if (estado.open(QIODevice::ReadOnly))
        for (const auto &l : QString::fromUtf8(estado.readAll()).split(u'\n'))
            if (l.startsWith(QStringLiteral("VmHWM:")))
                d["vmhwm"] = l.mid(6).trimmed();
    const QString captura = qEnvironmentVariable("VORTEX_PLAYER_CAPTURA");
    if (!captura.isEmpty())
        v->grabWindow().save(captura);
    d["errores"] = QJsonArray::fromStringList(errores);
    std::printf("DATOS%s\n", QJsonDocument(d).toJson(QJsonDocument::Compact).constData());
    std::fflush(stdout);
    app.quit();
}

int vortex_player_correr(rust::Str qml, bool modo_prueba)
{
    static int argc = 1;
    static char nombre[] = "vortex-player";
    static char *argv[] = {nombre, nullptr};
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("vortex-player"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortex Player"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-player"));
    const QString ruta = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    const QString carpeta = ruta.left(ruta.lastIndexOf(u'/'));
    QGuiApplication::setWindowIcon(
        QIcon::fromTheme(QStringLiteral("vortex-player"), QIcon(carpeta + QStringLiteral("/logo.svg"))));
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
    // El reproductor primero: el puente lo usa al arrancar.
    auto *rep = new Rep();
    rep->setParent(&app);
    auto *vp = new Puente();
    vp->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("vp"), vp);
    motor.rootContext()->setContextProperty(QStringLiteral("rep"), rep);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta));
    if (motor.rootObjects().isEmpty())
        return 1;
    // Al cerrar: la sesión (el segundo exacto), lo que suena a la nube, y mpv.
    QObject::connect(&app, &QCoreApplication::aboutToQuit, vp, [vp] {
        QMetaObject::invokeMethod(vp, "cerrar", Qt::DirectConnection);
    });
    if (modo_prueba)
        QTimer::singleShot(300, &app, [&] { prueba(app, motor, vp, rep, errores); });
    return app.exec();
}
