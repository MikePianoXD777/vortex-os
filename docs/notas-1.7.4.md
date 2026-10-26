# Vortexine's OS 1.7.4

Un panel de rendimiento en el panel del reloj, y el arreglo de «Seleccionar fondo» llega por fin a todas las PCs.

## Lo nuevo

- **Panel de rendimiento:** al picarle al reloj de la barra, abajo de los cuadros del clima, sale el
  **Panel de rendimiento** con el uso en vivo del **procesador**, la **memoria** (con los GB usados), la
  **temperatura** y el **disco**.
  - Solo mide mientras el panel está abierto: cerrado no gasta.
  - Al picarle a cualquiera de sus cuadros se abre el **monitor del sistema** (btop), como
    Ctrl+Shift+Esc, el Administrador de tareas de Windows.

## Arreglos

- **«Seleccionar fondo» del menú del escritorio (clic derecho):** el arreglo venía en la 1.7.1, pero no le
  llegaba a nadie porque el shell no se reconstruía al actualizar. Ahora sí (vortex-shell 2.2.14-10), igual
  que el del ecualizador de música, las apps del dock en las acciones rápidas y la ventana activa.

## Por dentro

- `shell/src/quickshell/calendar/CalendarPopup.qml`: el panel usa `SysData` (con `subscribe` mientras
  se ve). El panel del reloj pasa de 510 a 630 px de alto (`WindowRegistry.js`).
- `config/hypr/atajos.lua` exporta `especial(nombre)`, para abrir un escritorio especial desde el shell
  igual que con su atajo.
- Textos nuevos en `es.json` y `en.json` (`calendar.performance`).

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
