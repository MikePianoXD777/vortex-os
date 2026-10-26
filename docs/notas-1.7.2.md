# Vortexine's OS 1.7.2

Super+D maximiza la ventana del mouse, y el aviso de actualizaciones llega en minutos y con su propio sonido.

## Lo nuevo

- **Super+D maximiza la ventana que tienes bajo el mouse:**
  - Las otras ventanas del escritorio se esconden detrás, **sin cerrarse**, y siguen en la barra.
  - Otra vez Super+D y todo vuelve al mosaico, como estaba, esté donde esté el mouse.
  - Antes (1.7.1) Super+D abría el escritorio especial de Discord y WhatsApp, como en Caelestia; sin esas
    apps solo oscurecía la pantalla.
- **El aviso «Hay actualizaciones» llega en minutos:**
  - Las versiones nuevas de Vortexine's se revisan **cada 10 minutos** (antes, cada 6 horas), y la primera vez
    a los 2 minutos de entrar. Es una consulta mínima a GitHub: no gasta.
  - Los paquetes del sistema y las apps de Flatpak se siguen revisando cada 6 horas, porque eso baja sus
    listas de paquetes.
- **El aviso de actualizaciones suena distinto:** trae su propio sonido (un arpegio que sube), así sabes sin
  mirar que es una actualización y no una notificación cualquiera. Si ya elegiste uno con
  `vortex actualizaciones sonido ARCHIVO`, se respeta el tuyo.

## Por dentro

- `config/sonidos/actualizacion.py` genera `actualizacion.wav` (sintetizado, sin grabaciones).
- `vortex actualizar` y cada inicio de sesión ponen el temporizador nuevo; si apagaste el aviso
  (`vortex actualizaciones off`), sigue apagado.
- `pruebas/actualizaciones.sh` prueba el ritmo nuevo, que el sistema no se revise cada vez, el sonido y el
  sonido elegido. `pruebas/atajos.lua` prueba Super+D.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
