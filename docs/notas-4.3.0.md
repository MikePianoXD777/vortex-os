# Vortexine's OS 4.3

## Nuevo

### Lluvia de dígitos al abrir la terminal

Al abrir la terminal caen columnas de números en blanco y grises durante poco más de un segundo, y luego
aparece el saludo de siempre. Cualquier tecla la salta, y no deja rastro en la terminal.

Si prefieres que la terminal abra directo: `vortex saludo lluvia off` (y `on` para volver a ponerla).

### El visualizador de música va a la frecuencia de tu monitor

Las barras del visualizador (en la barra, el widget de música y la portada) se movían a 60 cuadros por
segundo aunque tu monitor diera más. Ahora van a la frecuencia de cada pantalla: 144 en un monitor de
144 Hz, 200 en uno de 200 Hz. En el perfil de rendimiento «bajo» siguen a 60, para no gastar de más.

## Arreglado

### Textos del shell que salían en otro idioma

Faltaban 81 textos: 33 del instalador salían en inglés aunque usaras el sistema en español, y 48 de
Ajustes (inicio automático, avisos del clima y los presets de widgets) salían en español con el sistema
en inglés. Ya están todos en los dos idiomas.

### Los juegos de Android se quedaban sin internet

El cortafuegos que trae el sistema (ufw) bloqueaba la red de Android: no le daba dirección ni DNS, y la
Play Store no cargaba. Ahora Vortexine's le abre el paso solo a la red de Android. Si ya habías preparado
Android, `vortex actualizar` lo arregla una sola vez (te pide la contraseña).

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
