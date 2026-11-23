# Guía para agentes (IA local o Claude)

Vortexine's OS: Arch + kernel CachyOS + Hyprland + shell Serpantinum (Quickshell/QML). Habla y escribe
siempre en **español**.

## Dónde está cada cosa
- `bin/vortex`: el comando `vortex` (bash). `vortex actualizar` instala lo nuevo y reinicia el shell.
- `shell/src/quickshell/`: el shell (QML). `shell/bin/serpantinumd` lo arranca.
- `rust/`: las apps en Rust (workspace). Todo software nuevo va en Rust.
- `config/hypr/`: Hyprland (Lua). `install/`: el instalador. `packaging/*/PKGBUILD`: los paquetes.
- `pruebas/*.sh`: pruebas. `docs/notas-X.Y.Z.md`: notas de cada versión.

## Reglas
- Cada cambio en `shell/src` o `shell/bin` sube `pkgrel` en `packaging/vortex-shell/PKGBUILD`.
  Cada cambio en `rust/` sube `pkgrel` en `packaging/vortex-rust/PKGBUILD`. Sin eso no llega a nadie.
- Versión nueva: `VORTEX_VERSION` en `install/lib.sh`, `version`/`versionedName` en
  `iso/airootfs/etc/calamares/branding/vortex/branding.desc` y `docs/notas-X.Y.Z.md`.
- Las pruebas llevan límite de memoria y tiempo:
  `systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- timeout 120 pruebas/ALGO.sh`
- Nunca abras ventanas ni un quickshell/Hyprland de verdad en la sesión de Mike: las pruebas de QML van
  con `env -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen`.
- Nada de `sudo`, contraseñas ni la nota «SUdo»: si hace falta root, dale el comando a Mike.
- No hagas `git push`, Releases ni ISOs sin que Mike lo pida. Commits con mensaje en español.
- No toques los cambios sin commit de Mike ni uses `git stash` a secas.
- Rust: `cd rust && cargo test -p NOMBRE` (NOMBRE del `[package]`, p. ej. `vortex-player`).
