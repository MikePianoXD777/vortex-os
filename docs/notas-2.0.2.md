# Vortexine's OS 2.0.2

Menos RAM al prender la PC: los clips ya no graban el escritorio todo el día.

## Lo nuevo

- **Los clips (Alt+F10) graban solo mientras juegas.** Antes, desde que entrabas, el grabador
  guardaba en la RAM los últimos 30 s del escritorio todo el día: 250-400 MB ocupados aunque no
  jugaras. Ahora empieza solo con el modo juego (se prende solo al abrir un juego) y al salir suelta
  esa memoria. Fuera de un juego, Alt+F10 empieza a grabar desde ese momento.

## Arreglos

- `vortex actualizar` se cortaba la primera vez que llegaba un fondo nuevo, antes de reconstruir el
  shell y los paquetes. Ya sigue hasta el final.
- El aviso «Hay actualizaciones» dejaba esperando al anterior cada vez que avisaba de algo nuevo.
  Ahora queda uno solo.
- `vortex clips apagar` paraba cualquier proceso que tuviera «vortex-clips servicio» en su línea de
  comandos; ahora para solo el suyo.

## Para actualizar

```
vortex actualizar
```

Al terminar, el grabador de clips se para solo si no estás jugando: no hace falta reiniciar.
