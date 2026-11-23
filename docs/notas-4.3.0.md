# Vortexine's OS 4.3

## Nuevo

### Lluvia de dígitos al abrir la terminal

Al abrir la terminal caen columnas de números en blanco y grises durante poco más de un segundo, y luego
aparece el saludo de siempre. Cualquier tecla la salta, y no deja rastro en la terminal.

Si prefieres que la terminal abra directo: `vortex saludo lluvia off` (y `on` para volver a ponerla).

## Arreglado

### Textos del shell que salían en otro idioma

Faltaban 81 textos: 33 del instalador salían en inglés aunque usaras el sistema en español, y 48 de
Ajustes (inicio automático, avisos del clima y los presets de widgets) salían en español con el sistema
en inglés. Ya están todos en los dos idiomas.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
