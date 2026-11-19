# Políticas de uso de Vortexine's OS

Vortexine's OS es gratis y lo mantiene una sola persona. Para que las actualizaciones lleguen bien a todos,
estas son las reglas.

## Si modificas tu sistema a fondo, te quedas sin actualizaciones ni soporte

Vortexine's se actualiza con `vortex actualizar`, que trae el código oficial de este repositorio y
reconstruye sus paquetes. Si cambias ese código en tu PC, la actualización ya no puede aplicarse:
**tu instalación queda fuera de las actualizaciones y del soporte**.

Cuenta como modificar a fondo:

- Editar, mover o borrar archivos en `/usr/share/vortex-os`, `/usr/share/vortex-*` o `/usr/share/serpantinum`.
- Cambiar los `PKGBUILD` de `packaging/` o instalar versiones propias de los paquetes de Vortexine's.
- Cambiar los comandos `vortex*` de `/usr/local/bin` o `/usr/bin`.
- **Dejar que una IA (Claude, Gemini, ChatGPT, Copilot o cualquier otra) modifique esos archivos por ti.**
  Es lo mismo que hacerlo a mano: si algo se rompe después, no se revisa.

En esos casos no se atienden reportes de errores ni se ayuda a reparar la instalación.

## Lo que sí puedes cambiar sin perder nada

Personalizar tu sistema está bien y no afecta las actualizaciones:

- Todo lo de **Ajustes**: temas, colores, fondo, liquid glass, perfiles de rendimiento.
- Tu configuración en `~/.config`, como los atajos y reglas propias de Hyprland en `~/.config/vortex/hypr-mio.lua`.
- Instalar o quitar programas con la Shop, `pacman`, `paru` o Flatpak.

## Cómo volver a tener actualizaciones

Regresa Vortexine's a la versión oficial. Tus cambios quedan en un respaldo por si los quieres:

```
sudo git -c safe.directory=/usr/share/vortex-os -C /usr/share/vortex-os diff > ~/mis-cambios-vortex.patch
sudo git -c safe.directory=/usr/share/vortex-os -C /usr/share/vortex-os checkout -- .
vortex actualizar
```

Si eso no basta, reinstala Vortexine's OS.

## Para reportar un error

Solo se revisan errores de instalaciones sin modificar. Junto con tu reporte manda lo que dicen estos comandos:

```
vortex version
git -c safe.directory=/usr/share/vortex-os -C /usr/share/vortex-os status --short
```

Si el segundo comando muestra archivos con `M` o `??` fuera de `packaging/`, primero sigue «Cómo volver a
tener actualizaciones».
