# Vortexine's OS 1.7.7

Una hoja de atajos de teclado para quien llega a Linux por primera vez.

## Lo nuevo

- **Super + F1 abre la hoja de atajos.** Todos los atajos que más sirven, por secciones: lo básico,
  ventanas, escritorios, paneles, capturas y los de Vortexine's (modo juego, Miku, monitor del
  sistema…). Cada atajo se ve con sus teclas dibujadas, como en el teclado.
  - **Buscador:** escribe «captura», «ventana» o «wifi» y quedan solo esos.
  - En **español o inglés**, según el idioma del shell.
  - También se abre desde el lanzador: busca **«Atajos de teclado»**.
  - Explica que **Super es la tecla de Windows**.
- **Aviso al entrar, una sola vez:** «Super + F1 muestra todos los atajos. Super solo abre las apps.»
  Así quien viene de Windows sabe por dónde empezar.

## Por dentro

- La lista está en `shell/src/assets/atajos.json` (cada atajo, con su texto en español e inglés).
  `pruebas/atajos.lua` comprueba que **cada atajo de la hoja exista de verdad**: si alguno cambia de
  tecla, la prueba falla y la hoja no se queda mintiendo.
- `shell/src/quickshell/keybinds/KeybindsPopup.qml`, registrada como panel «keybinds» (se libera al
  cerrarla, como los demás). vortex-shell 2.2.14-13.
- `config/hypr/hyprland.lua`: Super+F1 (`serpantinum:toggle keybinds`). `vortex actualizar` pone al día
  la config de Hyprland, así que el atajo llega también a las instalaciones de antes.
- `bin/vortex`: `aviso_atajos` (marca en `~/.local/state/vortex/atajos-avisado`).

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
