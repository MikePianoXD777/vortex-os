# Vortexine's OS 2.6.0

Dos cosas: **compartir tus playlists con un enlace** (Vortex Player 1.6.0, en la PC, Windows y el cel) y
**un escritorio que gasta menos RAM** con tarjetas NVIDIA.

## Lo nuevo

### Compartir una playlist (Vortex Player 1.6.0)

Abre una playlist y toca **«Compartir»**, junto a «Aleatorio».

- **En la PC:** «Copiar el enlace» o «Mandar por WhatsApp».
- **En el cel:** se abre el menú de compartir de Android (WhatsApp, Telegram, lo que tengas).

**El enlace lo abre cualquiera:**

- **Sin Vortex Player:** se abre en YouTube y suena la playlist entera. No hace falta cuenta.
- **Con Vortex Player:** pégalo en «Agregar música» (en el cel, en Buscar). La playlist se baja entera, con
  su mismo nombre.

Las canciones que agregaste tú (sin enlace de YouTube) se buscan en YouTube Music. Si la playlist tiene más
de 50 canciones, salen varios enlaces, uno por cada 50, porque YouTube no acepta más en uno solo.

### Menos RAM con NVIDIA

El escritorio dibuja todas sus ventanas en un solo hilo. Con NVIDIA, cada hilo de dibujo apartaba su propia
memoria del driver.

- **Al prender:** el escritorio gasta 323 MB en vez de 370.
- **Ya usándolo:** unos 45 MB menos, todo el tiempo.

Se ve y se siente igual, también con dos monitores de distinta frecuencia.

## Para actualizar

```
vortex actualizar
```

Pone al día Vortex Player y reinicia el escritorio con el cambio, sin cerrar sesión.

- **Vortex Player para Windows:** el instalador nuevo está en la Release `vortex-player-v1.6.0`.
- **En el cel:** la app avisa que hay versión nueva, o se baja el APK de la Release v1.6.0 de
  `vortex-player-android`.
