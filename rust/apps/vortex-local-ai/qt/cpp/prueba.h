// El modo de pruebas (`VORTEX_LOCAL_AI_PRUEBA=<guion.js>`): carga la ventana de verdad (offscreen) y corre un
// guion de JavaScript con `ia`, `chat`, `modelos`, `boveda`, `ventana` y `prueba` a la mano, como los guiones
// de `pruebas/test_interfaz.py`. El guion llama `prueba.fin(JSON.stringify(datos))`: se imprime «DATOS{json}» y
// la app sale. Solo existe para `qt/tests/interfaz.rs`.
#pragma once
#include <QtCore/QObject>
#include <QtCore/QStringList>
#include <QtQml/QJSValue>

class QCoreApplication;
class QQuickWindow;

class Prueba : public QObject
{
    Q_OBJECT

public:
    Prueba(QQuickWindow *ventana, const QStringList &errores, QObject *padre);

    // Corre `f` dentro de `ms` milisegundos MÁS que el paso anterior (los pasos se acumulan).
    Q_INVOKABLE void paso(int ms, const QJSValue &f);
    Q_INVOKABLE QObject *hijo(const QString &nombre) const;
    Q_INVOKABLE void procesar() const;
    Q_INVOKABLE void capturar(const QString &nombre) const;
    Q_INVOKABLE QString leer(const QString &ruta) const;
    Q_INVOKABLE QStringList listar(const QString &carpeta) const;
    Q_INVOKABLE void fin(const QString &json);

private:
    QQuickWindow *m_ventana;
    const QStringList &m_errores;
    int m_tiempo = 0;
};
