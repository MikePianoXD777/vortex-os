// La lista de filas que ven los QML (`ModeloLista` de puente.py): cada clave de la fila es un rol. `cambiar`
// avisa solo de los roles que cambian, así una barra de progreso o el mensaje que se escribe no redibujan
// toda la lista. Se maneja desde Rust con las funciones `modelo_*` de arranque.h (siempre en el hilo de Qt).
#pragma once
#include <QtCore/QAbstractListModel>
#include <QtCore/QStringList>
#include <QtCore/QVariantList>
#include <QtCore/QVariantMap>

class ModeloLista : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int count READ count NOTIFY cuentaCambio)

public:
    ModeloLista(const QStringList &roles, const QVariantMap &defectos, QObject *padre = nullptr);

    int rowCount(const QModelIndex &padre = QModelIndex()) const override;
    QVariant data(const QModelIndex &indice, int rol = Qt::DisplayRole) const override;
    QHash<int, QByteArray> roleNames() const override;

    int count() const { return int(m_filas.size()); }
    Q_INVOKABLE QVariantMap obtener(int i) const;

    void poner(const QVariantList &filas);
    void agregar(const QVariantMap &fila);
    void quitarDesde(int i);
    void cambiar(int i, const QVariantMap &campos);

signals:
    void cuentaCambio();

private:
    QVariantMap completa(const QVariantMap &fila) const;

    QStringList m_roles;
    QVariantMap m_defectos;
    QList<QVariantMap> m_filas;
};
