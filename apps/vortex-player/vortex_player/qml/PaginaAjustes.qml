// Ajustes: las carpetas de música, YouTube Music Premium y el audio (LDAC a 990 kbps, frecuencia nativa).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

Flickable {
    id: raiz
    contentHeight: columna.implicitHeight + 56
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}

    component Tarjeta: Rectangle {
        default property alias contenido: interior.data
        property string titulo: ""
        Layout.fillWidth: true
        implicitHeight: interior.implicitHeight + 40
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        ColumnLayout {
            id: interior
            anchors.fill: parent
            anchors.margins: 20
            spacing: 12
            Text { text: parent.parent.titulo; color: Tema.texto; font.pixelSize: 17; font.weight: Font.DemiBold }
        }
    }

    component Explicacion: Text {
        Layout.fillWidth: true
        color: Tema.suave
        font.pixelSize: 13
        wrapMode: Text.WordWrap
    }

    ColumnLayout {
        id: columna
        x: 28
        y: 28
        width: Math.min(raiz.width - 56, 860)
        spacing: 18

        Text { text: "Ajustes"; color: Tema.texto; font.pixelSize: 32; font.weight: Font.Bold }

        // 1.4: «Tu resumen»: lo que escuchaste esta semana o la pasada, siempre aquí arriba
        Tarjeta {
            id: tarjetaResumen
            titulo: "Tu resumen"
            property int cual: vp.resumen.aviso ? 1 : 0   // 0 = esta semana, 1 = la pasada
            readonly property var r: (cual ? vp.resumen.pasada : vp.resumen.semana) || ({})
            function fecha(t) { return t ? new Date(t * 1000).toLocaleDateString(Qt.locale(), "d MMM") : "" }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Explicacion {
                    text: tarjetaResumen.r.desde
                        ? "Del lunes " + tarjetaResumen.fecha(tarjetaResumen.r.desde) + " al domingo "
                          + tarjetaResumen.fecha(tarjetaResumen.r.hasta - 1) + ". Cuenta lo que escuchas en esta computadora (30 s o más de cada canción)."
                        : ""
                }
                Selector {
                    Layout.preferredWidth: 190
                    model: ["Esta semana", "La semana pasada"]
                    currentIndex: tarjetaResumen.cual
                    onActivated: (i) => tarjetaResumen.cual = i
                }
            }
            RowLayout {   // los números grandes, como el Wrapped de Spotify
                visible: (tarjetaResumen.r.escuchas || 0) > 0
                Layout.fillWidth: true
                spacing: 28
                Repeater {
                    model: [{ n: Tema.minutos(tarjetaResumen.r.minutos), t: "escuchando" },
                            { n: String(tarjetaResumen.r.distintas || 0), t: (tarjetaResumen.r.distintas === 1 ? "canción distinta" : "canciones distintas") },
                            { n: String(tarjetaResumen.r.dias || 0) + " de 7", t: "días con música" }]
                    ColumnLayout {
                        required property var modelData
                        spacing: 0
                        Text { text: modelData.n; color: Tema.azul; font.pixelSize: 28; font.weight: Font.Bold }
                        Text { text: modelData.t; color: Tema.tenue; font.pixelSize: 12 }
                    }
                }
            }
            RowLayout {
                visible: (tarjetaResumen.r.escuchas || 0) > 0
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 28
                ColumnLayout {   // las que más sonaron
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    spacing: 8
                    Text { text: "Lo que más escuchaste"; color: Tema.texto; font.pixelSize: 14; font.weight: Font.DemiBold }
                    Repeater {
                        model: tarjetaResumen.r.canciones || []
                        RowLayout {
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            spacing: 12
                            Text { Layout.preferredWidth: 16; text: index + 1; color: Tema.tenue; font.pixelSize: 13; horizontalAlignment: Text.AlignRight }
                            Portada { Layout.preferredWidth: 40; Layout.preferredHeight: 40; ruta: modelData.ruta || "" }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text { Layout.fillWidth: true; text: modelData.titulo; color: Tema.texto; font.pixelSize: 13; font.weight: Font.Medium; elide: Text.ElideRight }
                                Text { Layout.fillWidth: true; text: modelData.artista || "Artista desconocido"; color: Tema.tenue; font.pixelSize: 12; elide: Text.ElideRight }
                            }
                            Text { text: modelData.veces + (modelData.veces === 1 ? " vez" : " veces"); color: Tema.tenue; font.pixelSize: 12 }
                        }
                    }
                }
                ColumnLayout {   // tus artistas
                    Layout.preferredWidth: 230
                    Layout.alignment: Qt.AlignTop
                    spacing: 8
                    Text { text: "Tus artistas"; color: Tema.texto; font.pixelSize: 14; font.weight: Font.DemiBold }
                    Repeater {
                        model: tarjetaResumen.r.artistas || []
                        RowLayout {
                            required property var modelData
                            required property int index
                            Layout.fillWidth: true
                            spacing: 10
                            Text { Layout.preferredWidth: 16; text: index + 1; color: Tema.tenue; font.pixelSize: 13; horizontalAlignment: Text.AlignRight }
                            Text { Layout.fillWidth: true; text: modelData.artista; color: Tema.texto; font.pixelSize: 13; elide: Text.ElideRight }
                            Text { text: Tema.minutos(modelData.minutos); color: Tema.tenue; font.pixelSize: 12 }
                        }
                    }
                }
            }
            Explicacion {
                visible: !(tarjetaResumen.r.escuchas > 0)
                text: tarjetaResumen.cual ? "La semana pasada no escuchaste nada aquí." : "Todavía no hay nada esta semana: pon música y aquí irá apareciendo."
            }
        }

        // 2.4: las mismas canciones y playlists en el teléfono (por tu cuenta de Google)
        Tarjeta {
            titulo: "Tu música en el teléfono"
            function hace(t) {
                const min = Math.floor((Date.now() / 1000 - t) / 60)
                return min < 1 ? "hace un momento" : min < 60 ? "hace " + min + " min" : "hace " + Math.floor(min / 60) + " h"
            }
            Explicacion {
                text: vp.nube.correo
                    ? "Tus canciones y playlists son las mismas aquí y en Vortex Player de tu teléfono (cuenta " + vp.nube.correo + "). Lo que bajes en uno aparece en el otro, y lo que borres desde Vortex Player se borra en los dos."
                    : "Inicia sesión con tu cuenta de Google y tendrás las mismas canciones y playlists en Vortex Player de tu teléfono; cada uno baja solo lo que le falte. Se guardan en una carpeta oculta de tu Google Drive que solo ve Vortex Player."
            }
            Text {
                Layout.fillWidth: true
                visible: text !== ""
                text: vp.nube.error ? vp.nube.error
                    : vp.nube.trabajando || vp.nube.texto ? vp.nube.texto
                    : vp.nube.correo && vp.nube.ultima ? "Sincronizado " + parent.parent.hace(vp.nube.ultima) : ""
                color: vp.nube.error ? Tema.peligro : Tema.azul
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
            RowLayout {
                spacing: 10
                Boton {
                    visible: !vp.nube.correo
                    text: vp.nube.trabajando ? "Esperando al navegador…" : "Iniciar sesión con Google"
                    icono: "enlace"
                    enabled: vp.nube.configurada && !vp.nube.trabajando
                    onClicked: vp.iniciarSesionGoogle()
                }
                Boton {
                    visible: !!vp.nube.correo
                    text: vp.nube.trabajando ? "Sincronizando…" : "Sincronizar ahora"
                    icono: "girar"
                    tipo: "secundario"
                    enabled: !vp.nube.trabajando
                    onClicked: vp.sincronizar()
                }
                Boton { visible: !!vp.nube.correo; text: "Cerrar sesión"; tipo: "fantasma"; onClicked: vp.cerrarSesionGoogle() }
            }
            Explicacion { visible: !vp.nube.configurada; text: "Esta versión todavía no está registrada con Google: llega en una actualización." }
        }

        Tarjeta {
            titulo: "Audio"
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text { text: "LDAC a 990 kbps"; color: Tema.texto; font.pixelSize: 14; font.weight: Font.Medium }
                    Explicacion { text: "Con audífonos LDAC (como los SOUNDPEATS Air6 HS) siempre en la calidad más alta, en vez de «automático», que baja hasta 330 kbps cuando hay poca señal. Si se corta seguido, apágalo." }
                }
                Switch {
                    id: ldac
                    checked: !!vp.ajustes.ldac_hq
                    onToggled: vp.ponerLdacHq(checked)
                }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: Tema.borde; opacity: 0.6 }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                RowLayout {
                    spacing: 8
                    Text { text: "Frecuencia nativa"; color: Tema.texto; font.pixelSize: 14; font.weight: Font.Medium }
                    Etiqueta { texto: "Siempre activa"; nivel: "cd" }
                }
                Explicacion { text: "Cada canción suena a su frecuencia original (44,1, 88,2, 96, 176,4 o 192 kHz) sin pasar todo a 48 kHz, y sin tocar el volumen ni normalizarlo." }
            }
            RowLayout {
                visible: !!vp.ajustes.audioPendiente
                Layout.fillWidth: true
                spacing: 12
                Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "info"; color: Tema.azul }
                Explicacion { text: "Esto se nota al volver a entrar a tu sesión, o ahora mismo (el sonido se corta un segundo)." }
                Boton { text: "Aplicar ahora"; icono: "girar"; onClicked: vp.aplicarAudio() }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: Tema.borde; opacity: 0.6 }
            RowLayout {
                spacing: 8
                Icono { Layout.preferredWidth: 17; Layout.preferredHeight: 17; nombre: vp.salida.bluetooth ? "audifonos" : "bocina"; color: Tema.suave }
                Text {
                    Layout.fillWidth: true
                    text: !vp.salida.nombre ? "No encuentro la salida de audio"
                        : "Ahora suena por " + vp.salida.nombre
                          + (vp.salida.bluetooth ? " con " + vp.salida.codecTexto : "")
                          + (vp.salida.hz ? " a " + (vp.salida.hz / 1000).toLocaleString(Qt.locale(), "f", vp.salida.hz % 1000 ? 1 : 0) + " kHz" : "")
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Boton {
                    visible: !!vp.salida.ldac && vp.salida.codec !== "ldac"
                    text: "Usar LDAC"
                    tipo: "secundario"
                    onClicked: vp.forzarLdac()
                }
            }
        }

        // 2.4: crossfade y las letras guardadas para verlas sin internet (1.4: y la radio sin fin)
        Tarjeta {
            titulo: "Reproducción"
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Explicacion { text: "Radio sin fin: cuando se acaba lo que pusiste, siguen canciones parecidas de YouTube Music, sin bajarlas (si una te gusta, bájala con el botón de la barra)." }
                Switch {
                    checked: vp.ajustes.radio !== false
                    onToggled: vp.ponerRadio(checked)
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Explicacion { text: "Crossfade: la canción que termina se va apagando mientras la siguiente entra." }
                Selector {
                    Layout.preferredWidth: 190
                    readonly property var segundos: [0, 3, 5, 8, 12]
                    model: ["Sin crossfade", "3 segundos", "5 segundos", "8 segundos", "12 segundos"]
                    currentIndex: Math.max(0, segundos.indexOf(vp.ajustes.fundido === undefined ? 5 : vp.ajustes.fundido))
                    onActivated: (i) => vp.ponerFundido(segundos[i])
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Explicacion {
                    text: "Letras sin internet: " + (vp.letrasGuardadas.total
                        ? vp.letrasGuardadas.con + " de " + vp.letrasGuardadas.total + " canciones ya buscadas"
                          + (vp.letrasGuardadas.trabajando ? " · buscando las demás…" : "")
                        : "se buscan solas para toda tu música") + ". Se guardan en tu computadora."
                }
                Boton {
                    text: "Buscar las que faltan"; icono: "girar"; tipo: "fantasma"
                    enabled: !vp.letrasGuardadas.trabajando
                    onClicked: vp.bajarLetras()
                }
            }
        }

        // 1.5: el despertador: suena tu música a una hora, subiendo el volumen poco a poco
        Tarjeta {
            id: despertador
            objectName: "despertador"
            titulo: "Despertador"
            readonly property var d: vp.ajustes.despertador || ({})
            readonly property string hora: d.hora || "07:00"
            readonly property var listas: [{ id: -1, nombre: "Tu mezcla del día" }].concat(
                vp.playlists.filter(p => p.id > 0).map(p => ({ id: p.id, nombre: p.nombre })))
            function dos(n) { return (n < 10 ? "0" : "") + n }
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                Explicacion {
                    text: "A esa hora suena tu música y el volumen sube poco a poco durante un minuto. La computadora tiene que estar prendida con Vortex Player abierto (puede estar minimizado)."
                }
                Switch {
                    checked: !!despertador.d.activo
                    onToggled: vp.ponerDespertador({ activo: checked, hora: despertador.hora })
                }
            }
            RowLayout {
                spacing: 10
                enabled: !!despertador.d.activo
                opacity: enabled ? 1 : 0.5
                Icono { width: 20; height: 20; nombre: "despertador"; color: Tema.suave }
                Selector {
                    Layout.preferredWidth: 88
                    model: Array.from({ length: 24 }, (_, i) => despertador.dos(i))
                    currentIndex: parseInt(despertador.hora.split(":")[0], 10)
                    onActivated: (i) => vp.ponerDespertador({ hora: model[i] + ":" + despertador.hora.split(":")[1] })
                }
                Text { text: ":"; color: Tema.texto; font.pixelSize: 16 }
                Selector {
                    Layout.preferredWidth: 88
                    model: Array.from({ length: 12 }, (_, i) => despertador.dos(i * 5))
                    currentIndex: Math.round(parseInt(despertador.hora.split(":")[1], 10) / 5)
                    onActivated: (i) => vp.ponerDespertador({ hora: despertador.hora.split(":")[0] + ":" + model[i] })
                }
                Selector {
                    Layout.preferredWidth: 240
                    model: despertador.listas
                    textRole: "nombre"
                    currentIndex: Math.max(0, despertador.listas.findIndex(p => p.id === (despertador.d.playlist === undefined ? -1 : despertador.d.playlist)))
                    onActivated: (i) => vp.ponerDespertador({ playlist: despertador.listas[i].id })
                }
                Boton { text: "Probar"; icono: "play"; tipo: "fantasma"; onClicked: vp.despertar(despertador.d.playlist === undefined ? -1 : despertador.d.playlist) }
            }
        }

        Tarjeta {
            titulo: "YouTube Music Premium"
            Explicacion { text: "Sin Premium se baja en Opus a ~160 kbps. Con Premium (también el familiar), AAC a 256 kbps: entra a music.youtube.com con tu cuenta en tu navegador y elígelo aquí. Vortex Player usa esa sesión solo para bajar." }
            Selector {
                id: navegador
                Layout.preferredWidth: 260
                model: vp.ajustes.navegadores || []
                textoDe: (n) => n === "" ? "No tengo Premium" : "Mi sesión de " + n.charAt(0).toUpperCase() + n.slice(1)
                displayText: textoDe(currentText)
                currentIndex: Math.max(0, (vp.ajustes.navegadores || []).indexOf(vp.ajustes.navegador_premium || ""))
                onActivated: (i) => vp.ponerNavegador(model[i])
            }
        }

        Tarjeta {
            titulo: "Carpetas de música"
            Explicacion { text: "Lo que pongas en estas carpetas aparece solo en tu biblioteca. Lo que bajas queda en " + vp.ajustes.descargas + "." }
            RowLayout {
                spacing: 10
                Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "carpeta"; color: Tema.azul }
                Text { Layout.fillWidth: true; text: vp.ajustes.musica; color: Tema.texto; font.pixelSize: 13; elide: Text.ElideMiddle }
                Text { text: "Música"; color: Tema.tenue; font.pixelSize: 12 }
            }
            Repeater {
                model: vp.ajustes.carpetas || []
                delegate: RowLayout {
                    required property string modelData
                    spacing: 10
                    Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "carpeta"; color: Tema.azul }
                    Text { Layout.fillWidth: true; text: modelData; color: Tema.texto; font.pixelSize: 13; elide: Text.ElideMiddle }
                    Boton { text: "Quitar"; tipo: "peligro"; onClicked: vp.quitarCarpeta(modelData) }
                }
            }
            RowLayout {
                spacing: 10
                Boton { text: "Agregar una carpeta"; icono: "carpeta"; tipo: "secundario"; onClicked: dialogoCarpeta.open() }
                Boton { text: vp.escaneando ? "Buscando…" : "Buscar música nueva"; icono: "girar"; tipo: "fantasma"; enabled: !vp.escaneando; onClicked: vp.escanear() }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "Vortex Player 1.2 · para uso personal. Lo que bajas de YouTube es para escucharlo tú."
            color: Tema.tenue
            font.pixelSize: 12
        }
    }

    FolderDialog {
        id: dialogoCarpeta
        title: "Agregar una carpeta de música"
        onAccepted: vp.agregarCarpeta(selectedFolder)
    }
}
