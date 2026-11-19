#include "modelo.h"

static const int BASE_ROL = Qt::UserRole + 1;

ModeloLista::ModeloLista(const QStringList &roles, const QVariantMap &defectos, QObject *padre)
    : QAbstractListModel(padre)
    , m_roles(roles)
    , m_defectos(defectos)
{
}

int ModeloLista::rowCount(const QModelIndex &padre) const
{
    return padre.isValid() ? 0 : int(m_filas.size());
}

QVariant ModeloLista::data(const QModelIndex &indice, int rol) const
{
    if (!indice.isValid() || indice.row() < 0 || indice.row() >= m_filas.size())
        return {};
    const int i = rol - BASE_ROL;
    if (i < 0 || i >= m_roles.size())
        return {};
    return m_filas.at(indice.row()).value(m_roles.at(i));
}

QHash<int, QByteArray> ModeloLista::roleNames() const
{
    QHash<int, QByteArray> r;
    for (int i = 0; i < m_roles.size(); ++i)
        r.insert(BASE_ROL + i, m_roles.at(i).toUtf8());
    return r;
}

QVariantMap ModeloLista::completa(const QVariantMap &fila) const
{
    QVariantMap r;
    for (const auto &k : m_roles) {
        const QVariant v = fila.value(k);
        r.insert(k, v.isNull() || !v.isValid() ? m_defectos.value(k) : v);
    }
    return r;
}

QVariantMap ModeloLista::obtener(int i) const
{
    return i >= 0 && i < m_filas.size() ? m_filas.at(i) : QVariantMap();
}

void ModeloLista::poner(const QVariantList &filas)
{
    beginResetModel();
    m_filas.clear();
    for (const auto &f : filas)
        m_filas.append(completa(f.toMap()));
    endResetModel();
    emit cuentaCambio();
}

void ModeloLista::agregar(const QVariantMap &fila)
{
    const int n = int(m_filas.size());
    beginInsertRows(QModelIndex(), n, n);
    m_filas.append(completa(fila));
    endInsertRows();
    emit cuentaCambio();
}

void ModeloLista::quitarDesde(int i)
{
    if (i < 0 || i >= m_filas.size())
        return;
    beginRemoveRows(QModelIndex(), i, int(m_filas.size()) - 1);
    m_filas.remove(i, m_filas.size() - i);
    endRemoveRows();
    emit cuentaCambio();
}

void ModeloLista::cambiar(int i, const QVariantMap &campos)
{
    if (i < 0 || i >= m_filas.size())
        return;
    QVariantMap &fila = m_filas[i];
    QList<int> roles;
    for (auto it = campos.cbegin(); it != campos.cend(); ++it) {
        const int r = int(m_roles.indexOf(it.key()));
        if (r < 0)
            continue;
        const QVariant nuevo = it.value().isNull() ? m_defectos.value(it.key()) : it.value();
        if (fila.value(it.key()) != nuevo) {
            fila.insert(it.key(), nuevo);
            roles << BASE_ROL + r;
        }
    }
    if (roles.isEmpty())
        return;
    const QModelIndex indice = index(i, 0);
    emit dataChanged(indice, indice, roles);
}
