# Vortexine's OS 2.7.0

Lo nuevo funciona sin configurar nada.

## Lo nuevo

- **Vortexine Video.** La app de la cámara, parecida a la de Windows: la imagen a toda la ventana, foto o
  video, temporizador, cuadrícula y la última captura a un clic. Las fotos van a Imágenes/Cámara y los videos
  a Vídeos/Cámara. Si conectas la cámara con la app abierta, aparece sola.
- **Vortexine Shared.** Pasa archivos a otra compu con Vortexine's OS, también de lejos. Escribes su IP o su
  nombre en Tailscale y su usuario; allá sale un aviso y nada baja hasta que aceptan. Va cifrado y sin abrir
  puertos del módem. ¿Sin Tailscale? Creas un código y lo pegas en la otra compu. También está en el clic
  derecho de Thunar. Con el cortafuegos prendido, la primera vez te pide tu contraseña.
- **Aviso antes de quedarte sin memoria.** Te dice qué app se la está comiendo y te da un botón para cerrarla,
  antes de que algo se cierre solo. Con un juego abierto no sale.
- **Dicta en cualquier app.** Super+Shift+D, hablas y se escribe donde estés: en el navegador, en un chat,
  en un documento. Se corta solo cuando dejas de hablar. Funciona en español, sin internet, y el audio no
  se guarda. Si Miku no lo había bajado, la primera vez baja su modelo (~181 MB).
- **Pasa archivos al cel con un QR.** En Thunar, clic derecho → **Pasar al cel**, y lo escaneas con la
  cámara. Se baja por tu wifi, sin instalar nada, y si se corta el cel puede reanudar. Varios archivos o una carpeta se bajan en un .zip. Si
  tienes el cortafuegos prendido, la primera vez te pide tu contraseña.
- **«Este juego no abre».** Si un juego de Steam se cierra en menos de un minuto, «Algo no funciona» te
  dice en español qué pasó: falta reiniciar por el driver, poco disco, poca memoria, un anti-trampas. Si
  no dejó pista, el botón **Abrir con registro** lo abre otra vez para averiguarlo (Steam se reinicia).
- **Crear reporte.** Un botón en «Algo no funciona» junta en un archivo lo que hace falta para saber qué
  le pasa a tu equipo, sin tu usuario, tus IP ni tus correos. No se manda solo: tú decides a quién dárselo.
- **Los avisos importantes enseñan sus botones.** Ya no hay que abrir la flechita para verlos.
- **Novedades.** Esta ventana. Sale sola después de actualizar y siempre está en el lanzador. Si estabas
  jugando, espera a que vuelvas a entrar.

## Para actualizar

```
vortex actualizar
```
