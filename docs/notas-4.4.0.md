# Vortexine's OS 4.4

## Nuevo

### Filtro para daltonismo

Si te cuesta distinguir algún color, Vortexine's corrige la pantalla entera: escritorio, apps y juegos.
Elige cuál en **Ajustes → Pantalla → Daltonismo**:

- **Rojo** (protanopia), **Verde** (deuteranopia, la más común) o **Azul** (tritanopia): los colores que
  se confunden se mueven a los que sí se distinguen.
- **Grises**: toda la pantalla en escala de grises.

Se queda puesto aunque juegues o reinicies, y se quita con «No». Desde la terminal:
`vortex daltonismo rojo`, `verde`, `azul`, `grises` o `no`.

Además, lo importante ya no se nota solo por el color: las notificaciones urgentes dicen **«Urgente»** junto
a la hora, la batería baja de tus dispositivos lleva su icono, y con el filtro puesto el saludo de la terminal
le pone una barra a la RAM y al disco.

### La batería de tus audífonos y controles, junto al reloj

Conectas tus audífonos Bluetooth, un control (de Switch, de Xbox…), un mouse o un teclado inalámbrico y su
batería aparece junto al reloj, con su icono. Cuando les queda 15 % o menos te avisa una vez, para que los
pongas a cargar antes de que se apaguen a la mitad. Al desconectarlos, desaparece solo.

### El audio te sigue

Conectas audífonos (Bluetooth o USB) y el sonido se pasa solo a ellos. Si se desconectan o se les acaba la
pila mientras sonaba algo, la música **se pausa** en vez de seguir por las bocinas.

### No molestar automático al compartir pantalla

Cuando haces stream, grabas o compartes pantalla (Discord, OBS, el navegador), las notificaciones se callan
solas para que nadie vea tus mensajes. Te esperan en el panel y, al terminar, te dice cuántas llegaron.
Se apaga en **Ajustes → Notificaciones**.

### Descargas que no se llenan

Una vez por semana, Vortexine's revisa tu carpeta de Descargas: los instaladores, .zip y descargas a medias
de hace más de un mes te los ofrece mandar a la papelera (de ahí los recuperas si hace falta). Con «Se
quedan», esos ya no te los vuelve a preguntar. Tus fotos, documentos, música y videos nunca se tocan.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
