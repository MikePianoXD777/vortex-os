# Vortexine's OS 3.0

## Lo nuevo

### Tu juego con memoria de sobra

Al abrir un juego, Vortexine's espera unos segundos a que cargue y mira cuánta memoria le queda. Si va justo (menos de
3 GB libres), te avisa qué apps la están ocupando y te ofrece cerrarlas con un botón:

> **Tu juego va justo de memoria**
> Cierra Discord y Chrome y Terraria tendrá unos 2.8 GB más (ahora le quedan 1.4 GB).

- Nunca ofrece cerrar el juego ni su lanzador (Steam, Prism Launcher, Heroic, Lutris…), ni apps que usan poco.
- Las cierra como la X: si una tiene algo sin guardar, te pregunta.
- Sale una vez por partida y se va solo. Si tienes memoria de sobra, no sale nunca.

### Aviso de disco casi lleno

Con el disco lleno fallan las actualizaciones, los juegos no arrancan y las apps pierden lo que guardan. Ahora
Vortexine's te avisa antes, con un botón **Liberar espacio** que borra solo lo que se rehace solo: cachés, apps de Flatpak
que nadie usa, versiones viejas de paquetes y registros del sistema. Tus archivos no se tocan.

- **Quedan menos de 5 GB:** aviso urgente, una vez al día.
- **Quedan menos de 15 GB** y el disco va al 85 % o más: aviso normal, una vez a la semana.
- Nunca mientras juegas.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
