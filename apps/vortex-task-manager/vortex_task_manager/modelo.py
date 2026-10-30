"""La lista de procesos para QML (secciones, apps y procesos en una sola lista plana).

Cada segundo llega una lista nueva. Si se la diéramos a QML como arreglo, rehacería todas las filas
(se pierde el desplazamiento y parpadea); aquí se compara por la clave de cada fila y solo se quitan,
meten o actualizan las que cambiaron, como haría un modelo de Qt «de verdad».
"""
from __future__ import annotations

import difflib

from PySide6.QtCore import QAbstractListModel, QByteArray, QModelIndex, QObject, Qt, Signal, Property


class ModeloFilas(QAbstractListModel):
    FILA = Qt.ItemDataRole.UserRole + 1
    cuentaCambio = Signal()

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._filas: list[dict] = []

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:  # noqa: B008
        return 0 if parent.isValid() else len(self._filas)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or not 0 <= index.row() < len(self._filas):
            return None
        return self._filas[index.row()] if role == self.FILA else None

    def roleNames(self) -> dict[int, QByteArray]:
        return {self.FILA: QByteArray(b"fila")}

    def filas(self) -> list[dict]:
        return self._filas

    def actualizar(self, nuevas: list[dict]) -> None:
        viejas = [f["clave"] for f in self._filas]
        claves = [f["clave"] for f in nuevas]
        antes = len(self._filas)
        if viejas != claves:
            # De atrás para adelante: así los índices de lo que falta por aplicar siguen valiendo.
            ops = difflib.SequenceMatcher(a=viejas, b=claves, autojunk=False).get_opcodes()
            for tag, i1, i2, j1, j2 in reversed(ops):
                if tag in ("delete", "replace"):
                    self.beginRemoveRows(QModelIndex(), i1, i2 - 1)
                    del self._filas[i1:i2]
                    self.endRemoveRows()
                if tag in ("insert", "replace"):
                    self.beginInsertRows(QModelIndex(), i1, i1 + (j2 - j1) - 1)
                    self._filas[i1:i1] = nuevas[j1:j2]
                    self.endInsertRows()
        self._filas = list(nuevas)
        if self._filas:
            self.dataChanged.emit(self.index(0), self.index(len(self._filas) - 1), [self.FILA])
        if antes != len(self._filas):
            self.cuentaCambio.emit()

    cuenta = Property(int, lambda s: len(s._filas), notify=cuentaCambio)
