// El arranque de vortex-local-ai-rs: lo de `__main__.py` que cxx-qt-lib no trae (setContextProperty, el
// estilo, la fuente) y las listas de los QML. Los QObject `General` (ia), `Modelos`, `Chat`, `Boveda` y
// `Colores` son de Rust (src/*.rs).
#include "arranque.h"

#include "modelo.h"
#include "prueba.h"

#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtQml/QJSEngine>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <cstdio>
#include <unistd.h>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-local-ai-qt/src/boveda.cxxqt.h"
#include "vortex-local-ai-qt/src/chat.cxxqt.h"
#include "vortex-local-ai-qt/src/colores.cxxqt.h"
#include "vortex-local-ai-qt/src/general.cxxqt.h"
#include "vortex-local-ai-qt/src/modelos.cxxqt.h"

QVariant json_a_variante(rust::Str texto)
{
    const QByteArray cuerpo = "[" + QByteArray(texto.data(), static_cast<qsizetype>(texto.size())) + "]";
    const auto doc = QJsonDocument::fromJson(cuerpo);
    if (!doc.isArray() || doc.array().isEmpty())
        return {};
    return doc.array().at(0).toVariant();
}

// --- las listas ---

static QByteArray bytes(rust::Str s)
{
    return QByteArray(s.data(), static_cast<qsizetype>(s.size()));
}

static ModeloLista *lista(std::size_t m)
{
    return reinterpret_cast<ModeloLista *>(m);
}

std::size_t modelo_nuevo(rust::Str roles, rust::Str defectos)
{
    QStringList r;
    for (const auto &v : QJsonDocument::fromJson(bytes(roles)).array())
        r << v.toString();
    const QVariantMap d = QJsonDocument::fromJson(bytes(defectos)).object().toVariantMap();
    return reinterpret_cast<std::size_t>(new ModeloLista(r, d, QCoreApplication::instance()));
}

void modelo_poner(std::size_t m, rust::Str filas)
{
    lista(m)->poner(QJsonDocument::fromJson(bytes(filas)).array().toVariantList());
}

void modelo_agregar(std::size_t m, rust::Str fila)
{
    lista(m)->agregar(QJsonDocument::fromJson(bytes(fila)).object().toVariantMap());
}

void modelo_quitar_desde(std::size_t m, int i)
{
    lista(m)->quitarDesde(i);
}

void modelo_cambiar(std::size_t m, int i, rust::Str campos)
{
    lista(m)->cambiar(i, QJsonDocument::fromJson(bytes(campos)).object().toVariantMap());
}

rust::String modelo_fila(std::size_t m, int i)
{
    const QByteArray j = QJsonDocument(QJsonObject::fromVariantMap(lista(m)->obtener(i))).toJson(QJsonDocument::Compact);
    return rust::String(j.constData(), static_cast<std::size_t>(j.size()));
}

int modelo_cuenta(std::size_t m)
{
    return lista(m)->count();
}

QVariant modelo_a_variante(std::size_t m)
{
    return QVariant::fromValue(static_cast<QObject *>(lista(m)));
}

void copiar_al_portapapeles(rust::Str texto)
{
    if (auto *p = QGuiApplication::clipboard())
        p->setText(QString::fromUtf8(texto.data(), static_cast<qsizetype>(texto.size())));
}

rust::String url_a_local(rust::Str url)
{
    const QUrl q(QString::fromUtf8(url.data(), static_cast<qsizetype>(url.size())));
    const QByteArray r = (q.isLocalFile() ? q.toLocalFile() : QString::fromUtf8(url.data(), static_cast<qsizetype>(url.size()))).toUtf8();
    return rust::String(r.constData(), static_cast<std::size_t>(r.size()));
}

// --- el arranque ---

int vortex_local_ai_correr(rust::Str qml, rust::Str guion)
{
    static int argc = 1;
    static char nombre[] = "vortex-local-ai";
    static char *argv[] = {nombre, nullptr};
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("vortex-local-ai"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine's Local AI"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-local-ai"));
    const QString ruta = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    const QString carpeta = ruta.left(ruta.lastIndexOf(u'/'));
    QGuiApplication::setWindowIcon(
        QIcon::fromTheme(QStringLiteral("vortex-local-ai"), QIcon(carpeta + QStringLiteral("/logo.svg"))));
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
    auto *ia = new General();
    ia->setParent(&app);
    auto *modelos = new Modelos();
    modelos->setParent(&app);
    auto *chat = new Chat();
    chat->setParent(&app);
    auto *boveda = new Boveda();
    boveda->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    auto *contexto = motor.rootContext();
    contexto->setContextProperty(QStringLiteral("ia"), ia);
    contexto->setContextProperty(QStringLiteral("chat"), chat);
    contexto->setContextProperty(QStringLiteral("modelos"), modelos);
    contexto->setContextProperty(QStringLiteral("boveda"), boveda);
    contexto->setContextProperty(QStringLiteral("colores"), colores);
    // Como Puente.arrancar(): la bóveda, los modelos, el chat y lo general, en ese orden.
    QMetaObject::invokeMethod(ia, "iniciar", Qt::DirectConnection);
    motor.load(QUrl::fromLocalFile(ruta));
    if (motor.rootObjects().isEmpty())
        return 1;
    // Al salir: se corta lo que esté pasando (chat, memoria, descargas) y se para el Ollama que arrancamos.
    QObject::connect(&app, &QCoreApplication::aboutToQuit, ia, [=] {
        QMetaObject::invokeMethod(chat, "detener", Qt::DirectConnection);
        QMetaObject::invokeMethod(boveda, "cancelarMemoria", Qt::DirectConnection);
        QMetaObject::invokeMethod(modelos, "cancelarTodo", Qt::DirectConnection);
        QMetaObject::invokeMethod(ia, "cerrar", Qt::DirectConnection);
    });

    int codigo = 0;
    if (guion.size() > 0) {
        auto *ventana = qobject_cast<QQuickWindow *>(motor.rootObjects().first());
        auto *prueba = new Prueba(ventana, errores, &app);
        auto global = motor.globalObject();
        global.setProperty(QStringLiteral("ia"), motor.newQObject(ia));
        global.setProperty(QStringLiteral("chat"), motor.newQObject(chat));
        global.setProperty(QStringLiteral("modelos"), motor.newQObject(modelos));
        global.setProperty(QStringLiteral("boveda"), motor.newQObject(boveda));
        global.setProperty(QStringLiteral("ventana"), motor.newQObject(ventana));
        global.setProperty(QStringLiteral("prueba"), motor.newQObject(prueba));
        const QString archivo = QString::fromUtf8(guion.data(), static_cast<qsizetype>(guion.size()));
        QFile f(archivo);
        if (!f.open(QIODevice::ReadOnly)) {
            std::fprintf(stderr, "No pude leer el guion %s\n", qPrintable(archivo));
            return 2;
        }
        const QJSValue r = motor.evaluate(QString::fromUtf8(f.readAll()), archivo);
        if (r.isError()) {
            std::fprintf(stderr, "ERROR en el guion: %s (línea %d)\n", qPrintable(r.toString()),
                         r.property("lineNumber").toInt());
            return 2;
        }
    }
    codigo = app.exec();
    // Como `os._exit` de la de Python: sin desarmar la QML (se quejaría de los objetos que ya se borraron).
    std::fflush(stdout);
    std::fflush(stderr);
    _exit(codigo);
}
