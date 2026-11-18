// El arranque de vortex-task-manager-rs: lo de `__main__.py` que cxx-qt-lib no trae. Los QObject `Puente`
// («tm») y `Colores` son de Rust (src/puente.rs, src/colores.rs); aquí viven el modelo de la lista de
// procesos (como `modelo.py`, pero las diferencias las calcula el motor) y el proveedor de íconos.
#include "arranque.h"

#include <QtCore/QAbstractListModel>
#include <QtCore/QDir>
#include <QtCore/QFileInfo>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QProcess>
#include <QtCore/QStandardPaths>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QPixmap>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickImageProvider>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>
#include <cstdio>
#include <functional>

// Encabezados que genera cxx-qt-build (ruta «<crate>/src/<archivo>.cxxqt.h»).
#include "vortex-task-manager-qt/src/colores.cxxqt.h"
#include "vortex-task-manager-qt/src/puente.cxxqt.h"

namespace {

// La lista plana de procesos: cada fila es un mapa en el rol «fila» (QML lo lee como model.fila).
class ModeloFilas : public QAbstractListModel
{
public:
    explicit ModeloFilas(QObject *padre) : QAbstractListModel(padre) {}

    int rowCount(const QModelIndex &padre = QModelIndex()) const override
    {
        return padre.isValid() ? 0 : static_cast<int>(m_filas.size());
    }
    QVariant data(const QModelIndex &indice, int rol) const override
    {
        if (!indice.isValid() || indice.row() < 0 || indice.row() >= m_filas.size() || rol != Qt::UserRole + 1)
            return {};
        return m_filas.at(indice.row());
    }
    QHash<int, QByteArray> roleNames() const override { return {{Qt::UserRole + 1, "fila"}}; }

    // Quita y mete solo lo que cambió (de atrás para adelante, para que los índices valgan), y avisa
    // que el contenido de todas puede haber cambiado (cpu, memoria…).
    void actualizar(const QJsonArray &ops, const QJsonArray &filas)
    {
        QList<QVariant> nuevas;
        for (const auto &f : filas)
            nuevas << f.toObject().toVariantMap();
        for (qsizetype k = ops.size() - 1; k >= 0; --k) {
            const auto o = ops.at(k).toArray();
            const int i1 = o.at(0).toInt(), i2 = o.at(1).toInt(), j1 = o.at(2).toInt(), j2 = o.at(3).toInt();
            if (i2 > i1) {
                beginRemoveRows({}, i1, i2 - 1);
                m_filas.remove(i1, i2 - i1);
                endRemoveRows();
            }
            if (j2 > j1) {
                beginInsertRows({}, i1, i1 + (j2 - j1) - 1);
                for (int n = 0; n < j2 - j1; ++n)
                    m_filas.insert(i1 + n, nuevas.at(j1 + n));
                endInsertRows();
            }
        }
        m_filas = nuevas;
        if (!m_filas.isEmpty())
            emit dataChanged(index(0), index(static_cast<int>(m_filas.size()) - 1), {Qt::UserRole + 1});
    }

private:
    QList<QVariant> m_filas;
};

ModeloFilas *g_modelo = nullptr;

// image://icono/<nombre o ruta>: el ícono de la app, del tema de íconos que uses.
class ProveedorIconos : public QQuickImageProvider
{
public:
    ProveedorIconos() : QQuickImageProvider(QQuickImageProvider::Pixmap) {}
    QPixmap requestPixmap(const QString &ident, QSize *, const QSize &pedido) override
    {
        const int lado = pedido.isValid() ? qMax(qMax(pedido.width(), pedido.height()), 32) : 64;
        QIcon icono = ident.startsWith(u'/') ? QIcon(ident) : QIcon::fromTheme(ident);
        if (icono.isNull())
            icono = QIcon::fromTheme(ident.toLower());
        QPixmap pixmap = icono.pixmap(QSize(lado, lado));
        if (pixmap.isNull()) {
            pixmap = QPixmap(lado, lado);
            pixmap.fill(Qt::transparent);
        }
        return pixmap;
    }
};

// Fuera de KDE, Qt no sabe qué tema de íconos usas ni dónde están: se toma el de GTK (gsettings) o Papirus,
// y se buscan en las carpetas de XDG y en las de Flatpak (ahí están los de tus apps Flatpak).
void temaIconos()
{
    const QString home = QDir::homePath();
    QStringList datos;
    const QString dh = qEnvironmentVariable("XDG_DATA_HOME");
    datos << (dh.isEmpty() ? home + QStringLiteral("/.local/share") : dh);
    const QString dd = qEnvironmentVariable("XDG_DATA_DIRS", QStringLiteral("/usr/local/share:/usr/share"));
    datos << dd.split(u':', Qt::SkipEmptyParts);
    datos << home + QStringLiteral("/.local/share/flatpak/exports/share") << QStringLiteral("/var/lib/flatpak/exports/share")
          << QStringLiteral("/usr/share");
    QStringList rutas = QIcon::themeSearchPaths();
    QStringList candidatas{home + QStringLiteral("/.icons")};
    for (const auto &d : datos)
        candidatas << d + QStringLiteral("/icons");
    for (const auto &d : candidatas)
        if (!rutas.contains(d) && QDir(d).exists())
            rutas << d;
    QIcon::setThemeSearchPaths(rutas);
    if (!QIcon::themeName().isEmpty() && QIcon::themeName() != QStringLiteral("hicolor"))
        return;
    QString tema;
    if (!QStandardPaths::findExecutable(QStringLiteral("gsettings")).isEmpty()) {
        QProcess p;
        p.start(QStringLiteral("gsettings"), {QStringLiteral("get"), QStringLiteral("org.gnome.desktop.interface"),
                                              QStringLiteral("icon-theme")});
        if (p.waitForFinished(2000))
            tema = QString::fromUtf8(p.readAllStandardOutput()).trimmed();
        else
            p.kill();
        tema.remove(u'\'');
    }
    const QStringList bases{home + QStringLiteral("/.local/share/icons"), QStringLiteral("/usr/share/icons")};
    for (const auto &c : {tema, QStringLiteral("Papirus-Dark"), QStringLiteral("Papirus"), QStringLiteral("breeze-dark"),
                          QStringLiteral("Adwaita")}) {
        if (c.isEmpty())
            continue;
        bool hay = false;
        for (const auto &b : bases)
            hay = hay || QDir(b + u'/' + c).exists();
        if (hay) {
            QIcon::setThemeName(c);
            break;
        }
    }
    QIcon::setFallbackThemeName(QStringLiteral("hicolor"));
    QStringList fallback = QIcon::fallbackSearchPaths();
    fallback << QStringLiteral("/usr/share/pixmaps");
    QIcon::setFallbackSearchPaths(fallback);
}

QString desdeRust(rust::Str s)
{
    return QString::fromUtf8(s.data(), static_cast<qsizetype>(s.size()));
}

} // namespace

QVariant vortex_tm_json_a_variant(rust::Str json)
{
    QJsonParseError error;
    const auto doc = QJsonDocument::fromJson(QByteArray(json.data(), static_cast<qsizetype>(json.size())), &error);
    if (error.error != QJsonParseError::NoError)
        return {};
    return doc.isArray() ? QVariant(doc.array().toVariantList()) : QVariant(doc.object().toVariantMap());
}

bool vortex_tm_icono_en_tema(rust::Str nombre)
{
    return QIcon::hasThemeIcon(desdeRust(nombre));
}

void vortex_tm_copiar(rust::Str texto)
{
    if (auto *c = QGuiApplication::clipboard())
        c->setText(desdeRust(texto));
}

void vortex_tm_filas_actualizar(rust::Str ops, rust::Str filas)
{
    if (!g_modelo)
        return;
    g_modelo->actualizar(QJsonDocument::fromJson(QByteArray(ops.data(), static_cast<qsizetype>(ops.size()))).array(),
                         QJsonDocument::fromJson(QByteArray(filas.data(), static_cast<qsizetype>(filas.size()))).array());
}

int vortex_tm_correr(rust::Str qml, bool prueba)
{
    // Un solo hilo de dibujo gasta menos memoria con NVIDIA, como el lanzador de Python.
    if (qEnvironmentVariableIsEmpty("QSG_RENDER_LOOP"))
        qputenv("QSG_RENDER_LOOP", "basic");

    QQuickStyle::setStyle(QStringLiteral("Basic"));
    static int argc = 1;
    static char nombre[] = "vortex-task-manager";
    static char *argv[] = {nombre, nullptr};
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("vortex-task-manager"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("Vortexine Task Manager"));
    QGuiApplication::setOrganizationName(QStringLiteral("Vortexine's"));
    QGuiApplication::setDesktopFileName(QStringLiteral("vortex-task-manager"));
    const QString ruta = desdeRust(qml);
    QGuiApplication::setWindowIcon(
        QIcon::fromTheme(QStringLiteral("vortex-task-manager"), QIcon(QFileInfo(ruta).absolutePath() + QStringLiteral("/logo.svg"))));
    QFont fuente(QStringLiteral("Adwaita Sans"));
    fuente.setFamilies({QStringLiteral("Adwaita Sans"), QStringLiteral("Noto Sans")});
    fuente.setPixelSize(14);
    QGuiApplication::setFont(fuente);
    temaIconos();

    QQmlApplicationEngine motor;
    QStringList errores;
    QObject::connect(&motor, &QQmlEngine::warnings, [&](const QList<QQmlError> &ws) {
        for (const auto &w : ws)
            errores << w.toString();
    });
    g_modelo = new ModeloFilas(&app);
    auto *tm = new Puente();
    tm->setParent(&app);
    auto *colores = new Colores();
    colores->setParent(&app);
    motor.addImageProvider(QStringLiteral("icono"), new ProveedorIconos());
    motor.rootContext()->setContextProperty(QStringLiteral("tm"), tm);
    motor.rootContext()->setContextProperty(QStringLiteral("filasModelo"), g_modelo);
    motor.rootContext()->setContextProperty(QStringLiteral("colores"), colores);
    motor.load(QUrl::fromLocalFile(ruta));
    if (motor.rootObjects().isEmpty())
        return 1;

    QObject *ventana = motor.rootObjects().first();
    // Una captura de la ventana (pruebas y comparaciones), a los N ms (VORTEX_TASK_MANAGER_CAPTURA_MS).
    const QString captura = qEnvironmentVariable("VORTEX_TASK_MANAGER_CAPTURA");
    if (!captura.isEmpty()) {
        const QString pagina = qEnvironmentVariable("VORTEX_TASK_MANAGER_PAGINA");
        if (!pagina.isEmpty())
            ventana->setProperty("pagina", pagina);
        const int ms = qEnvironmentVariable("VORTEX_TASK_MANAGER_CAPTURA_MS", QStringLiteral("4000")).toInt();
        QTimer::singleShot(ms, &app, [ventana, captura] {
            if (auto *w = qobject_cast<QQuickWindow *>(ventana))
                w->grabWindow().save(captura);
        });
    }

    if (prueba) {
        // Lo mismo que hace pruebas/test_interfaz.py: lista, filtro, finalizar un «sleep» propio, las páginas.
        auto paso = [tm](int n) { return tm->pruebaPaso(n); };
        auto agenda = [&](int ms, std::function<void()> f) { QTimer::singleShot(ms, &app, std::move(f)); };
        QString resultado;
        agenda(2500, [paso] { paso(0); });
        agenda(2800, [paso] { paso(1); });
        agenda(3600, [paso] { paso(2); });
        agenda(3700, [ventana] { ventana->setProperty("pagina", QStringLiteral("rendimiento")); });
        agenda(6200, [paso] { paso(3); });
        agenda(6300, [ventana] { ventana->setProperty("pagina", QStringLiteral("servicios")); });
        agenda(9300, [&app, paso, &errores] {
            QJsonObject datos = QJsonDocument::fromJson(paso(4).toUtf8()).object();
            datos[QStringLiteral("errores")] = QJsonArray::fromStringList(errores);
            QFile estado(QStringLiteral("/proc/self/status"));
            if (estado.open(QIODevice::ReadOnly))
                for (const auto &l : QString::fromUtf8(estado.readAll()).split(u'\n'))
                    if (l.startsWith(QStringLiteral("VmHWM:")))
                        datos[QStringLiteral("vmhwm")] = l.mid(6).trimmed();
            std::printf("DATOS%s\n", QJsonDocument(datos).toJson(QJsonDocument::Compact).constData());
            std::fflush(stdout);
            app.quit();
        });
    }
    const int codigo = app.exec();
    g_modelo = nullptr;
    return codigo;
}
