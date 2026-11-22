# Vortexine's OS 4.1

## Nuevo

### Una compañera que camina por tu escritorio

Un personaje en pixel art que vive en tu pantalla: camina por la orilla de abajo, se sube a las
ventanas, se sienta en ellas y, si las mueves o las cierras, se cae. De vez en cuando se pone a hacer
algo, como tomar café, ver el celular, leer o jugar.

- **Clic:** te dice algo. **Doble clic:** abre a Miku. **Arrástrala** a donde quieras.
  **Clic derecho:** se despide y se va.
- Si no usas la PC un rato, se duerme; al mover el mouse, se despierta.
- **Cuando juegas o tienes algo en pantalla completa, se esconde sola.**
- Casi no gasta: está hecha en Rust y no usa Qt.

Viene apagada. Para encenderla:

```
vortex mascota on
```

Se queda encendida en los próximos inicios de sesión hasta que la apagues con `vortex mascota off`.

**Ponle tu propio personaje:** `vortex mascota usar ARCHIVO.zip` acepta un paquete de tiras
(`animations.json` y una tira por animación en `spritesheets_128/`) o uno de Shimeji (`shime1.png`…).
Para volver al diseño de fábrica: `vortex mascota original`.

### `vortex actualizar` avisa si cambiaste algo a mano

Antes, cualquier archivo de Vortexine's cambiado a mano hacía que la actualización fallara o se
quedara a medias. Ahora `vortex actualizar` te enseña qué archivos son distintos de la versión
oficial y te pregunta si los guarda en un respaldo y pone los oficiales.

- Tus cambios quedan en `~/.local/share/vortex/cambios-apartados/`, con un `LEEME.txt` que dice cómo
  recuperarlos.
- Si dices que no, no toca nada y no actualiza.
- También revisa los paquetes instalados: el que tenga archivos cambiados se reinstala.
- **`vortex cambios`** te enseña la lista sin cambiar nada.

### Miku trae su modelo 3D

El modelo 3D de Miku («MIKU NAKANO», de **Vewertyg**, en VRoid Hub) ya viene con el paquete de Miku:
antes solo lo traía la ISO. Su licencia permite incluirlo con crédito, sin uso comercial. El crédito
se ve en la ventana de Miku, en el botón de información. Con `vortex miku modelo RUTA.vrm` puedes
poner otro.

## Arreglado

### El panel de música ya deja elegir el tiempo con Vortex Player

Si abrías Vortex Player antes de poner una canción, la barra de tiempo del panel de música no hacía
nada: Vortex Player nunca le avisaba al panel que ya se podía adelantar. Ahora le avisa con cada
canción. Además le pasa la duración real de la canción cuando no la tiene guardada.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
