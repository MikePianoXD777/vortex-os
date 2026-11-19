# Vortexine's OS 3.5.2

## Arreglado

### El mouse no se veía con gráficas AMD

Con algunas gráficas AMD el mouse **funcionaba pero no se dibujaba**: podías dar clic y mover cosas, pero la flecha no aparecía.
El cursor «por hardware» de esas tarjetas a veces no se pinta en Hyprland.

- **Con cualquier gráfica AMD, el cursor ahora lo dibuja Hyprland por software**, igual que ya pasaba en las laptops desde la 2.8.5.
  Funciona solo, sin configurar nada. Con NVIDIA o Intel en una PC de escritorio todo sigue igual.
- Gracias a quien lo reportó por TikTok.

### También en esta versión

- La guía de bienvenida ya no tiene el botón «67», y el modo otaku ya no hace sonido al encenderse.

## Para actualizar

Si no ves el mouse, todo se puede hacer con el teclado:

1. Abre una terminal con **Super + T**.
2. Escribe `vortex actualizar` y presiona Enter (te pedirá tu contraseña).

Al terminar, el cursor aparece solo.

**Para verlo ya, sin actualizar**, en una terminal:

```
hyprctl eval 'hl.config({ cursor = { no_hardware_cursors = 1 } })'
```

(dura hasta que reinicies la sesión; `vortex actualizar` lo deja fijo).
