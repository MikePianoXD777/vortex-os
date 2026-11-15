# Vortexine's OS 3.5.1

## Arreglado

### El ecualizador por fin cambia el sonido (panel de música y Vortex Player 1.7.3)

Mover las bandas del ecualizador **no hacía nada**: a EasyEffects, el motor que usa el ecualizador, le faltaban sus plugins
(`lsp-plugins-lv2`) y el audio pasaba tal cual.

- **Los plugins se instalan solos** con `vortex actualizar` (pesan unos 40 MB), y también en las instalaciones nuevas.
  Al instalarlos, el ecualizador se reinicia y vuelve a poner lo que tenías guardado.
- **Mover una banda suena al momento**, sin tocar «Aplicar». «Aplicar» la guarda; si cierras el panel sin aplicar, se deshace.
- **Si algo falta, te lo dice:** una notificación (como mucho una cada 10 minutos) y, en Vortex Player, la página del ecualizador explica qué falta.
- **«Plano»** sigue restableciéndolo todo.
- Puede que, al empezar a sonar, se aplique lo que tenías guardado antes (balance, modo noche). Si se oye raro, usa «Restablecer» y «Plano».

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña). Se reconstruyen el shell y Vortex Player.
