// El arranque de vortex-shared-rs: lo de `ventana.py` que cxx-qt-lib no trae (setContextProperty, el estilo,
// la fuente, el ícono, el portapapeles). Los QObject `Puente` («vs») y `Colores` son de Rust (src/puente.rs,
// src/colores.rs).
#include "arranque.h"

#include <QtCore/QDir>
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
#include <memory>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-shared-qt/src/colores.cxxqt.h"
#include "vortex-shared-qt/src/puente.cxxqt.h"

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

// Lo que recorre `pruebas/test_interfaz.py`: agrega archivos, quita uno, crea un código, cancela y manda a una
// MAC (que se rechaza); mide lo que ve y guarda capturas si se pide. Las rutas van en VORTEX_SHARED_RECORRER
// separadas por «:» (la última no existe a propósito).
void recorrer(QGuiApplication &app, QObject *ventana, Puente *vs, const QStringList &errores)
{
    auto datos = std::make_shared<QJsonObject>();
    auto hijo = [ventana](const char *nombre) { return ventana->findChild<QObject *>(QString::fromLatin1(nombre)); };
    const QString capturas = qEnvironmentVariable("VORTEX_SHARED_CAPTURAS");
    auto capturar = [ventana, capturas](const QString &nombre) {
        if (auto *w = qobject_cast<QQuickWindow *>(ventana); w && !capturas.isEmpty())
            w->grabWindow().save(QDir(capturas).filePath(nombre + QStringLiteral(".png")));
    };
    auto nombres = [vs] {
        QJsonArray l;
        for (const auto &a : vs->property("archivos").toList())
            l.append(QJsonValue::fromVariant(a.toMap().value(QStringLiteral("nombre"))));
        return l;
    };

    // paso 1 (0,3 s)
    QTimer::singleShot(300, &app, [=] {
        QVariantList urls;
        for (const auto &r : qEnvironmentVariable("VORTEX_SHARED_RECORRER").split(u':', Qt::SkipEmptyParts))
            urls << QUrl::fromLocalFile(r).toString();
        QMetaObject::invokeMethod(vs, "agregar", Q_ARG(QVariantList, urls));
        (*datos)[QStringLiteral("archivos")] = nombres();
        auto *usuario = hijo("miUsuario");
        (*datos)[QStringLiteral("usuario")] = usuario ? usuario->property("text").toString() : QString();
        capturar(QStringLiteral("con-archivos"));
        QMetaObject::invokeMethod(vs, "quitar", Q_ARG(int, 1));
        (*datos)[QStringLiteral("tras_quitar")] = vs->property("archivos").toList().size();
        QMetaObject::invokeMethod(vs, "crearCodigo");
    });
    // paso 2 (+1,2 s): ya hay código
    QTimer::singleShot(1500, &app, [=] {
        (*datos)[QStringLiteral("codigo")] = vs->property("codigo").toString();
        (*datos)[QStringLiteral("ocupado")] = vs->property("ocupado").toBool();
        capturar(QStringLiteral("codigo"));
        QMetaObject::invokeMethod(vs, "cancelar");
        (*datos)[QStringLiteral("tras_cancelar")] =
            QJsonArray{vs->property("codigo").toString(), vs->property("ocupado").toBool()};
        QMetaObject::invokeMethod(vs, "mandar", Q_ARG(QString, QStringLiteral("04:42:1a:08:6b:62")),
                                  Q_ARG(QString, QStringLiteral("mike")));
    });
    // paso 3 (+0,8 s): la MAC se explicó
    QTimer::singleShot(2300, &app, [=, &app] {
        (*datos)[QStringLiteral("mac")] = vs->property("estado").toString();
        capturar(QStringLiteral("mac"));
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

void vortex_shared_copiar(rust::Str texto)
{
    if (auto *c = QGuiApplication::clipboard())
        c->setText(QString::fromUtf8(texto.data(), static_cast<qsizetype>(texto.size())));
}

int vortex_shared_correr(rust::Str qml, bool recorrer_ventana, bool humo)
{
    static int argc = 1;
    static char nombre[] = "vortex-shared";
    static char *argv[] = {nombre, nullptr};
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QGuiApplication app(argc, argv);
    const QString ruta_qml = QString::fromUtf8(qml.data(), static_cast<qsizetype>(qml.size()));
    QGuiApplication::setApplicationName(QStringLiteral("vortex-shared"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine Shared"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-shared"));
    QGuiApplication::setWindowIcon(QIcon::fromTheme(QStringLiteral("vortex-shared"),
                                                    QIcon(QFileInfo(ruta_qml).absolutePath() + QStringLiteral("/logo.svg"))));
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
    auto *vs = new Puente();
    vs->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.rootContext()->setContextProperty(QStringLiteral("vs"), vs);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta_qml));
    if (motor.rootObjects().isEmpty())
        return 1;
    if (recorrer_ventana)
        recorrer(app, motor.rootObjects().first(), vs, errores);
    else if (humo)
        QTimer::singleShot(3000, &app, [&app] { app.quit(); });
    return app.exec();
}
