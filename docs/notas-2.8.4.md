# Vortexine's OS 2.8.4

## Lo arreglado

- **La PC se congelaba al abrir juegos con Proton en NVIDIA.** Al entrar a un juego se prendía la grabación de clips
  (Alt+F10) con un límite de memoria que no le alcanzaba. Se ahogaba dentro del driver de la gráfica y se trababa toda la
  pantalla hasta apagar la PC. Ahora el grabador corre con su propia memoria.

## Lo optimizado

- **Menos CPU con audio sonando.** El visualizador de la barra hacía redibujar el shell de más. En una prueba con un
  monitor de 200 Hz y otro de 60 Hz, el shell bajó de 29–30 % a 13–26 % de un núcleo.

## Para actualizar

```
vortex actualizar
```
