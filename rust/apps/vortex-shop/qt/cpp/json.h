#pragma once
#include <QtCore/QByteArray>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QString>
#include <QtCore/QVariant>

// Un valor JSON (texto) como QVariant: QML lo ve como objeto, lista, número, texto o booleano.
// Qt solo lee documentos que son objeto o lista, así que se envuelve en «[...]» y se saca el primero.
inline QVariant json_a_variant(const QString &json)
{
    const auto doc = QJsonDocument::fromJson(QByteArray("[") + json.toUtf8() + QByteArray("]"));
    if (!doc.isArray() || doc.array().isEmpty())
        return {};
    return doc.array().at(0).toVariant();
}
