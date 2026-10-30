# Vortexine's OS 2.2.0

Tus propios sonidos de notificación, con solo pegar un enlace.

## Lo nuevo

### Sonidos de notificación desde un enlace

- En **Ajustes → Notificaciones**, debajo de «Efecto de sonido», hay una fila nueva: **Agregar desde un
  enlace**. Pega ahí:
  - el enlace de un botón de [myinstants.com](https://www.myinstants.com) (la página del botón, por ejemplo
    `myinstants.com/es/instant/vine-boom-sound-70972/`): Vortex saca el sonido y su nombre solo;
  - o un enlace directo a un `mp3`, `wav`, `ogg`, `flac`, `opus` o `m4a`.
- Al terminar de bajarse, el sonido **queda elegido y suena** una vez para que lo oigas.
- Abajo salen **tus sonidos** (los que agregaste), cada uno con ▶ para escucharlo, **Usar** para elegirlo y
  la basura para borrarlo. Los que trae el shell no se pueden borrar.
- Si el enlace no sirve (no es audio, pesa más de 10 MB o no hay internet), la fila te dice qué pasó.

### En la terminal

```
vortex sonidos agregar ENLACE|ARCHIVO [NOMBRE]
vortex sonidos lista
vortex sonidos quitar NOMBRE
```

`agregar` también acepta un archivo de tu PC.

## Para actualizar

```
vortex actualizar
```
