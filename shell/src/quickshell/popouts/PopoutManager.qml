import QtQuick
import Quickshell
import "../"

Item {
    id: root

    // El OSD (volumen y brillo) se queda cargado: sale con cada tecla y tiene que salir al instante.
    Osd {}

    // 4.0: los menús se cargan al abrirlos y se sueltan 20 s después de cerrarlos (BajoDemanda.qml).
    BajoDemanda {
        abierto: TrayMenuController.isVisible
        fuente: Qt.resolvedUrl("TrayBase.qml")
    }
    BajoDemanda {
        abierto: SideMusicController.isVisible
        fuente: Qt.resolvedUrl("SideMusicPopout.qml")
    }
    BajoDemanda {
        abierto: DesktopMenuController.isVisible
        fuente: Qt.resolvedUrl("DesktopMenu.qml")
    }
    BajoDemanda {
        abierto: DockAppMenuController.isVisible
        fuente: Qt.resolvedUrl("DockAppMenu.qml")
    }
}
