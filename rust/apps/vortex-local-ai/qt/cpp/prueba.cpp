#include "prueba.h"

#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTimer>
#include <QtGui/QGuiApplication>
#include <QtQuick/QQuickWindow>
#include <cstdio>

Prueba::Prueba(QQuickWindow *ventana, const QStringList &errores, QObject *padre)
    : QObject(padre)
    , m_ventana(ventana)
    , m_errores(errores)
{
}

void Prueba::paso(int ms, const QJSValue &f)
{
    m_tiempo += ms;
    QTimer::singleShot(m_tiempo, this, [f]() mutable {
        const QJSValue r = f.call();
        if (r.isError()) {
            std::fprintf(stderr, "ERROR en el guion: %s (línea %d)\n", qPrintable(r.toString()),
                         r.property("lineNumber").toInt());
            std::fflush(stderr);
            QCoreApplication::exit(3);
        }
    });
}

QObject *Prueba::hijo(const QString &nombre) const
{
    return m_ventana->findChild<QObject *>(nombre);
}

void Prueba::procesar() const
{
    QCoreApplication::processEvents();
}

void Prueba::capturar(const QString &nombre) const
{
    const QString carpeta = qEnvironmentVariable("VORTEX_LOCAL_AI_CAPTURAS");
    if (!carpeta.isEmpty())
        m_ventana->grabWindow().save(carpeta + u'/' + nombre + QStringLiteral(".png"));
}

QString Prueba::leer(const QString &ruta) const
{
    QFile f(ruta);
    return f.open(QIODevice::ReadOnly) ? QString::fromUtf8(f.readAll()) : QString();
}

QStringList Prueba::listar(const QString &carpeta) const
{
    return QDir(carpeta).entryList(QDir::Files | QDir::NoDotAndDotDot, QDir::Name);
}

void Prueba::fin(const QString &json)
{
    QJsonObject d = QJsonDocument::fromJson(json.toUtf8()).object();
    d["errores"] = QJsonArray::fromStringList(m_errores);
    QFile estado(QStringLiteral("/proc/self/status"));
    if (estado.open(QIODevice::ReadOnly))
        for (const auto &l : QString::fromUtf8(estado.readAll()).split(u'\n'))
            if (l.startsWith(QStringLiteral("VmHWM:")) || l.startsWith(QStringLiteral("VmRSS:"))
                || l.startsWith(QStringLiteral("RssAnon:")))
                d[l.section(u':', 0, 0).toLower()] = l.section(u':', 1).trimmed();
    std::printf("DATOS%s\n", QJsonDocument(d).toJson(QJsonDocument::Compact).constData());
    std::fflush(stdout);
    QCoreApplication::quit();
}
