# Vortexine's OS 1.7.6

El modo de ventanas KDE ahora tiene su barra de tareas abajo, como en Windows.

## Lo nuevo

- **Barra de tareas en el modo KDE.** Con **Ajustes → Pantalla → Gestor de ventanas: KDE**, el dock de
  abajo hace de barra de tareas:
  - Muestra **las apps abiertas, también las minimizadas**: al picarle a una minimizada, vuelve al
    escritorio donde estás.
  - **Se esconde solo y sale al bajar el mouse** al borde de abajo.
  - Si la barra del shell está abajo, el dock sale **encima de ella**, sin taparla.
  - **Anclar apps:** clic derecho en una app → **Fijar en el dock** (y **Quitar del dock** para soltarla).
  - Al volver a **Mosaico**, el dock queda como estaba antes de pasar a KDE. Lo que cambies del dock
    estando en KDE se respeta; no se vuelve a poner como de fábrica.
- Si ya estabas en KDE con la 1.7.5, `vortex actualizar` enciende la barra de tareas.
- El dock reserva su espacio también cuando solo muestra apps abiertas (si se configura fijo, sin
  esconderse), para que una ventana maximizada no quede debajo.

## Por dentro

- `bin/vortex`: `dock_ventanas` cambia `dock` en `~/.config/serpantinum/settings.json` (lo de antes se
  guarda en `~/.config/vortex/dock-mosaico.json`).
- `shell/src/quickshell/dock/DockWindow.qml`: el dock se muestra y reserva espacio sin apps fijadas si
  muestra las abiertas. vortex-shell 2.2.14-12.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
