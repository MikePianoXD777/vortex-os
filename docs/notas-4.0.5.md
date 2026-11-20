# Vortexine's OS 4.0.5

## Arreglado

### Los monitores rápidos regresaban a 60 Hz

Si tu pantalla da 144, 165, 200 o 240 Hz, Vortexine's la ponía a su máximo al entrar a la sesión.
Pero cada vez que Hyprland recargaba su configuración (por ejemplo, después de `vortex actualizar`),
la pantalla regresaba a 60 Hz hasta que volvías a entrar.

- **Ahora se queda en sus Hz máximos**, también después de recargar, siempre en su resolución nativa.
- Si elegiste tu frecuencia en Ajustes → Pantalla o en `~/.config/vortex/hypr-mio.lua`, se respeta.

### En la terminal salía el logo de CachyOS

Si instalaste Vortexine's sobre CachyOS, la terminal fish enseñaba el saludo de CachyOS (fastfetch
con su logo) en lugar del de Vortexine's.

- **Ahora sale el saludo de Vortexine's**, con su logo. Tu `config.fish` de antes queda respaldado
  en `~/.config/fish/config.fish.antes-vortex`.
- Si habías quitado el saludo, no vuelve.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
