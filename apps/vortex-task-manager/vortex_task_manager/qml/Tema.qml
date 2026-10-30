pragma Singleton
// Colores y medidas de Vortexine's (los mismos de Vortex Player). Los colores vienen de `colores.paleta` (colores.py): la paleta del fondo
// de pantalla (matugen) o, si no hay, la fija de Vortexine's. Cada color cambia con una transición suave
// cuando cambia el fondo; el resto de la interfaz solo usa Tema.<nombre>.
import QtQuick

QtObject {
    id: raiz
    readonly property var paleta: colores.paleta
    readonly property int duracion: 250
    property color fondo: paleta.fondo
    Behavior on fondo { ColorAnimation { duration: raiz.duracion } }
    property color barra: paleta.barra
    Behavior on barra { ColorAnimation { duration: raiz.duracion } }
    property color tarjeta: paleta.tarjeta
    Behavior on tarjeta { ColorAnimation { duration: raiz.duracion } }
    property color tarjetaHover: paleta.tarjetaHover
    Behavior on tarjetaHover { ColorAnimation { duration: raiz.duracion } }
    property color campo: paleta.campo
    Behavior on campo { ColorAnimation { duration: raiz.duracion } }
    property color borde: paleta.borde
    Behavior on borde { ColorAnimation { duration: raiz.duracion } }
    property color bordeCampo: paleta.bordeCampo
    Behavior on bordeCampo { ColorAnimation { duration: raiz.duracion } }
    property color bordeHover: paleta.bordeHover
    Behavior on bordeHover { ColorAnimation { duration: raiz.duracion } }
    property color texto: paleta.texto
    Behavior on texto { ColorAnimation { duration: raiz.duracion } }
    property color suave: paleta.suave
    Behavior on suave { ColorAnimation { duration: raiz.duracion } }
    property color tenue: paleta.tenue
    Behavior on tenue { ColorAnimation { duration: raiz.duracion } }
    property color azul: paleta.azul
    Behavior on azul { ColorAnimation { duration: raiz.duracion } }
    property color azulHover: paleta.azulHover
    Behavior on azulHover { ColorAnimation { duration: raiz.duracion } }
    property color azulPresion: paleta.azulPresion
    Behavior on azulPresion { ColorAnimation { duration: raiz.duracion } }
    property color violeta: paleta.violeta
    Behavior on violeta { ColorAnimation { duration: raiz.duracion } }
    property color sobreAcento: paleta.sobreAcento
    Behavior on sobreAcento { ColorAnimation { duration: raiz.duracion } }
    property color peligro: paleta.peligro
    Behavior on peligro { ColorAnimation { duration: raiz.duracion } }
    property color exito: paleta.exito
    Behavior on exito { ColorAnimation { duration: raiz.duracion } }
    property color realce: paleta.realce
    Behavior on realce { ColorAnimation { duration: raiz.duracion } }
    property color realceFuerte: paleta.realceFuerte
    Behavior on realceFuerte { ColorAnimation { duration: raiz.duracion } }
    property color relleno: paleta.relleno
    Behavior on relleno { ColorAnimation { duration: raiz.duracion } }
    property color barraSel: paleta.barraSel
    Behavior on barraSel { ColorAnimation { duration: raiz.duracion } }
    property color barraHover: paleta.barraHover
    Behavior on barraHover { ColorAnimation { duration: raiz.duracion } }
    property color peligroFondo: paleta.peligroFondo
    Behavior on peligroFondo { ColorAnimation { duration: raiz.duracion } }
    property color peligroBorde: paleta.peligroBorde
    Behavior on peligroBorde { ColorAnimation { duration: raiz.duracion } }
    property color velo: paleta.velo
    Behavior on velo { ColorAnimation { duration: raiz.duracion } }
    property color flotante: paleta.flotante
    Behavior on flotante { ColorAnimation { duration: raiz.duracion } }

    // Medidas
    readonly property int radioTarjeta: 22
    readonly property int radioGrande: 28
    readonly property int radioCampo: 18
    readonly property int anchoLateral: 244
}
