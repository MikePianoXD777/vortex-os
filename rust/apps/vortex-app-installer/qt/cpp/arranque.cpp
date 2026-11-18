// El arranque de vortex-app-installer-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente, el ícono, el portapapeles) y los dos recorridos de pruebas (`pruebas/test_interfaz.py` y
// `pruebas/capturas.py`). Los QObject `Puente` y `Colores` son de Rust (src/puente.rs, src/colores.rs).
#include "arranque.h"

#include <QtCore/QElapsedTimer>
#include <QtCore/QDir>
#include <QtCore/QEventLoop>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <cstdio>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-app-installer-qt/src/colores.cxxqt.h"
#include "vortex-app-installer-qt/src/puente.cxxqt.h"

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
    // «[valor]» → «valor»
    const QByteArray sin = j.mid(1, j.size() - 2);
    return rust::String(sin.constData(), static_cast<std::size_t>(sin.size()));
}

void copiar_texto(rust::Str texto)
{
    if (auto *c = QGuiApplication::clipboard())
        c->setText(QString::fromUtf8(texto.data(), static_cast<qsizetype>(texto.size())));
}

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

void esperar(int ms)
{
    QEventLoop bucle;
    QTimer::singleShot(ms, &bucle, &QEventLoop::quit);
    bucle.exec();
}

QString estado_de(QObject *instalador) { return instalador->property("estado").toString(); }

// Espera a que el estado sea uno de `estados` (o se acabe el tiempo, en segundos).
void esperar_estado(QObject *instalador, const QStringList &estados, int limite)
{
    QElapsedTimer t;
    t.start();
    while (!estados.contains(estado_de(instalador)) && t.elapsed() < limite * 1000)
        esperar(50);
}

void abrir(QObject *instalador, const QString &ruta)
{
    QMetaObject::invokeMethod(instalador, "abrir", Q_ARG(QString, ruta));
}

QVariantList acciones(QObject *instalador)
{
    return instalador->property("archivo").toMap().value(QStringLiteral("acciones")).toList();
}

void imprimir_datos(const QJsonObject &datos)
{
    std::printf("DATOS%s\n", QJsonDocument(datos).toJson(QJsonDocument::Compact).constData());
    std::fflush(stdout);
}

// Lo que recorre `pruebas/test_interfaz.py`: abre varios archivos, comprueba que cada uno llega a «listo» con
// botones, descomprime el primero, mira la pestaña «Instalado» y cuenta lo que QML avisó.
void recorrer(QGuiApplication &app, QObject *ventana, QObject *instalador, const QStringList &errores)
{
    QTimer::singleShot(300, &app, [&app, ventana, instalador, &errores] {
        const QStringList archivos = QString::fromUtf8(qgetenv("VORTEX_APP_INSTALLER_PRUEBA_ARCHIVOS")).split(u'\n', Qt::SkipEmptyParts);
        QJsonObject datos;
        QJsonArray listos;
        for (const auto &a : archivos) {
            abrir(instalador, a);
            esperar_estado(instalador, {QStringLiteral("listo")}, 20);
            QJsonObject r;
            r[QStringLiteral("archivo")] = a;
            r[QStringLiteral("estado")] = estado_de(instalador);
            r[QStringLiteral("acciones")] = static_cast<int>(acciones(instalador).size());
            listos.append(r);
            esperar(150);
        }
        datos[QStringLiteral("abiertos")] = listos;
        if (!archivos.isEmpty()) {
            abrir(instalador, archivos.first());
            esperar_estado(instalador, {QStringLiteral("listo")}, 20);
            QMetaObject::invokeMethod(instalador, "ejecutar", Q_ARG(QString, QStringLiteral("descomprimir")),
                                      Q_ARG(QVariant, QVariant(QVariantMap{})));
            esperar_estado(instalador, {QStringLiteral("hecho"), QStringLiteral("error")}, 20);
        }
        datos[QStringLiteral("estado_final")] = estado_de(instalador);
        datos[QStringLiteral("instalados")] = instalador->property("instalados").toList().size();
        ventana->setProperty("pestana", QStringLiteral("instalado"));
        esperar(200);
        datos[QStringLiteral("errores")] = QJsonArray::fromStringList(errores);
        datos[QStringLiteral("vmhwm")] = memoria("VmHWM");
        datos[QStringLiteral("rss")] = memoria("VmRSS");
        datos[QStringLiteral("rss_anon")] = memoria("RssAnon");
        imprimir_datos(datos);
        app.quit();
    });
}

// Lo que hace `pruebas/capturas.py`: por cada archivo, lo abre, captura; si tiene acción la ejecuta y captura el
// progreso y el resultado; al final, la pestaña «Instalado».
void capturar(QGuiApplication &app, QObject *ventana, QObject *instalador, const QStringList &errores,
              const QString &carpeta, const QString &plan, const QString &contrasena)
{
    QTimer::singleShot(300, &app, [&app, ventana, instalador, &errores, carpeta, plan, contrasena] {
        QDir().mkpath(carpeta);
        auto *w = qobject_cast<QQuickWindow *>(ventana);
        auto guardar = [w, &carpeta](const QString &nombre) {
            if (w)
                w->grabWindow().save(QDir(carpeta).filePath(nombre));
            std::printf("captura: %s\n", qPrintable(QDir(carpeta).filePath(nombre)));
        };
        auto captura = [&](const QString &nombre) {
            esperar(450); // animaciones de entrada
            guardar(nombre);
        };
        const QString sufijo = qEnvironmentVariable("VORTEX_APP_INSTALLER_IDIOMA") == QLatin1String("en")
                                   ? QStringLiteral("-en") : QString();
        captura(QStringLiteral("00-vacio%1.png").arg(sufijo));
        int n = 0;
        for (const auto &linea : plan.split(u'\n', Qt::SkipEmptyParts)) {
            ++n;
            const QString archivo = linea.section(u'\t', 0, 0);
            const QString id_accion = linea.section(u'\t', 1, 1);
            const QString base = QStringLiteral("%1-%2%3").arg(n, 2, 10, QLatin1Char('0')).arg(QFileInfo(archivo).fileName(), sufijo);
            abrir(instalador, archivo);
            esperar_estado(instalador, {QStringLiteral("listo"), QStringLiteral("error")}, 120);
            captura(base + QStringLiteral("-a.png"));
            if (id_accion.isEmpty())
                continue;
            QVariantMap elegida;
            bool hay = false;
            for (const auto &a : acciones(instalador)) {
                const auto m = a.toMap();
                if (m.value(QStringLiteral("id")).toString() == id_accion) {
                    elegida = m;
                    hay = true;
                    break;
                }
            }
            if (!hay) {
                std::printf("sin la acción %s para %s\n", qPrintable(id_accion), qPrintable(archivo));
                continue;
            }
            QMetaObject::invokeMethod(instalador, "ejecutar", Q_ARG(QString, id_accion),
                                      Q_ARG(QVariant, elegida.value(QStringLiteral("datos"), QVariantMap{})));
            QElapsedTimer t;
            t.start();
            while (estado_de(instalador) == QLatin1String("trabajando") && instalador->property("progreso").toDouble() < 0.15
                   && t.elapsed() < 5000)
                esperar(30);
            if (estado_de(instalador) == QLatin1String("trabajando")) {
                esperar(250);
                guardar(base + QStringLiteral("-b-trabajando.png"));
            }
            const QStringList fin{QStringLiteral("hecho"), QStringLiteral("error"), QStringLiteral("listo")};
            esperar_estado(instalador, fin, 120);
            if (!instalador->property("contrasena").toMap().value(QStringLiteral("accion")).toString().isEmpty()
                && !contrasena.isEmpty()) {
                captura(base + QStringLiteral("-c-contrasena.png"));
                QMetaObject::invokeMethod(instalador, "enviarContrasena", Q_ARG(QString, contrasena));
                esperar(200);
                esperar_estado(instalador, fin, 120);
            }
            captura(base + QStringLiteral("-d-resultado.png"));
            QMetaObject::invokeMethod(instalador, "cerrarArchivo");
        }
        ventana->setProperty("pestana", QStringLiteral("instalado"));
        captura(QStringLiteral("99-instalado%1.png").arg(sufijo));
        if (!errores.isEmpty())
            std::printf("AVISOS DE QML:\n  %s\n", qPrintable(errores.join(QStringLiteral("\n  "))));
        std::printf("RssAnon: %s  Rss: %s\n", qPrintable(memoria("RssAnon")), qPrintable(memoria("VmRSS")));
        std::fflush(stdout);
        app.exit(errores.isEmpty() ? 0 : 1);
    });
}

} // namespace

int vortex_app_installer_correr(rust::Str qml, rust::Str archivo, bool prueba, rust::Str capturas, rust::Str plan,
                                int ancho, int alto, rust::Str contrasena)
{
    static int argc = 1;
    static char nombre[] = "vortex-app-installer";
    static char *argv[] = {nombre, nullptr};
    auto texto = [](rust::Str s) { return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size())); };
    QQuickStyle::setStyle(QStringLiteral("Basic")); // sin el aspecto nativo: todo con los colores de Vortexine's
    QGuiApplication app(argc, argv);
    const QString ruta_qml = texto(qml);
    QGuiApplication::setApplicationName(QStringLiteral("vortex-app-installer"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortex App Installer"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-app-installer")); // en Wayland: el app_id
    QGuiApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("vortex-app-installer"),
                                                    QIcon(QFileInfo(ruta_qml).absolutePath() + QStringLiteral("/logo.svg"))));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setFamilies({QStringLiteral("Adwaita Sans"), QStringLiteral("Noto Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);

    QQmlApplicationEngine motor; // primero el motor QML: al cerrar se va antes que el puente
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            if (!w.toString().contains(QLatin1String("propagateSizeHints")))
                errores << w.toString();
    });
    auto *instalador = new Puente();
    instalador->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("instalador"), instalador);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta_qml));
    if (motor.rootObjects().isEmpty())
        return 1;
    QObject *ventana = motor.rootObjects().first();
    const bool de_prueba = prueba || capturas.size() > 0;
    if (!de_prueba)
        QMetaObject::invokeMethod(instalador, "escuchar"); // una sola ventana por persona
    if (prueba) {
        recorrer(app, ventana, instalador, errores);
    } else if (capturas.size() > 0) {
        ventana->setProperty("width", ancho);
        ventana->setProperty("height", alto);
        capturar(app, ventana, instalador, errores, texto(capturas), texto(plan), texto(contrasena));
    } else if (archivo.size() > 0) {
        abrir(instalador, texto(archivo));
    }
    const int codigo = app.exec();
    return codigo;
}
