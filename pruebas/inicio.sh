#!/usr/bin/env bash
# Pruebas del inicio de la 4.0 (pantalla_inicio y plymouth_vortex de bin/vortex): directo a Vortex Shell
# bloqueado, el login gráfico al cerrar sesión y la animación de arranque. Todo en una raíz falsa
# (VORTEX_INICIO_RAIZ) con sudo y plymouth-set-default-theme falsos: no toca tu sistema, no arranca
# Hyprland ni Quickshell, no corre mkinitcpio ni grub-mkconfig.
#
#   pruebas/inicio.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-inicio-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin"
cat >"$T/bin/sudo" <<FALSO
#!/usr/bin/env bash
echo "\$*" >>"$T/sudo.log"
"\$@"
FALSO
cat >"$T/bin/plymouth-set-default-theme" <<FALSO
#!/usr/bin/env bash
if ((\$#)); then echo "\$1" >"$T/tema"; else cat "$T/tema" 2>/dev/null || echo cachyos; fi
FALSO
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH"

export VORTEX_DIR="$AQUI" VORTEX_INICIO_RAIZ="$T/raiz" USER=mike
aviso() { echo "aviso: $*" >>"$T/avisos.log"; }
eval "$(sed -n '/^greetd_config() {/,/^}/p; /^pantalla_inicio() {/,/^}/p; /^plymouth_vortex() {/,/^}/p' "$AQUI/bin/vortex")"
R="$T/raiz"
G="$R/etc/greetd/config.toml"

nueva_raiz() {
  rm -rf "$R" "$T/sudo.log" "$T/tema"
  mkdir -p "$R/etc/greetd" "$R/etc/mkinitcpio.conf.d" "$R/etc/default"
  cp "$AQUI/config/sistema/greetd.toml" "$G"
  echo 'HOOKS=(base systemd autodetect microcode modconf kms keyboard sd-vconsole block filesystems fsck)' >"$R/etc/mkinitcpio.conf"
  echo "GRUB_CMDLINE_LINUX_DEFAULT='loglevel=3 quiet'" >"$R/etc/default/grub"
}

echo "== Instalación de la ISO (sin Plymouth ni entrada directa)"
nueva_raiz
pantalla_inicio
probar "greetd entra directo con el shell bloqueado (mike)" \
  "grep -q '^\[initial_session\]' '$G' && grep -q 'entrar-bloqueado' '$G' && grep -q 'user = \"mike\"' '$G'"
probar "al cerrar sesión sale el login gráfico" "grep -q 'command = \"/usr/local/bin/vortex-greeter\"' '$G'"
probar "un solo [initial_session]" "[[ \$(grep -c '^\[initial_session\]' '$G') == 1 ]]"
probar "instala el login en /usr/share/vortex/greeter" \
  "[[ -f '$R/usr/share/vortex/greeter/shell.qml' && -x '$R/usr/share/vortex/greeter/entrar-bloqueado' && -x '$R/usr/local/bin/vortex-greeter' ]]"
probar "instala el tema de Plymouth" "[[ -f '$R/usr/share/plymouth/themes/vortex/vortex.script' && -f '$R/usr/share/plymouth/themes/vortex/logo.png' ]]"
probar "lo pone de tema" "[[ \$(cat '$T/tema') == vortex ]]"
probar "el hook de Plymouth va justo después de systemd" "grep -q '^HOOKS=(base systemd plymouth autodetect' '$R/etc/mkinitcpio.conf'"
probar "GRUB lleva splash" "grep -q \"^GRUB_CMDLINE_LINUX_DEFAULT='splash loglevel=3 quiet'\" '$R/etc/default/grub'"

echo "== Otra vez, sin cambios"
rm -f "$T/sudo.log"
pantalla_inicio
probar "no pide sudo si ya está todo" "[[ ! -s '$T/sudo.log' ]]"
probar "no repite plymouth ni splash" \
  "[[ \$(grep -o plymouth '$R/etc/mkinitcpio.conf' | wc -l) == 1 && \$(grep -o splash '$R/etc/default/grub' | wc -l) == 1 ]]"

echo "== Quien eligió entrar sin contraseña"
nueva_raiz
printf '\n[initial_session]\ncommand = "vortex-sesion"\nuser = "ana"\n' >>"$G"
pantalla_inicio
probar "se respeta: ana entra sin bloqueo" \
  "grep -q 'user = \"ana\"' '$G' && grep -q 'command = \"/usr/share/vortex/greeter/entrar\"' '$G' && ! grep -q 'entrar-bloqueado' '$G'"

echo "== CachyOS (como la PC de Mike): Plymouth ya puesto, HOOKS en conf.d y GRUB con comillas simples"
nueva_raiz
echo 'HOOKS=(base udev autodetect kms block filesystems)' >"$R/etc/mkinitcpio.conf.d/10-otro.conf"
echo 'HOOKS=(base systemd autodetect microcode kms modconf block keyboard sd-vconsole plymouth filesystems)' >"$R/etc/mkinitcpio.conf"
echo "GRUB_CMDLINE_LINUX_DEFAULT='nowatchdog nvme_load=YES splash loglevel=3'" >"$R/etc/default/grub"
pantalla_inicio
probar "cambia el HOOKS que vale (el de conf.d)" "grep -q '^HOOKS=(base udev plymouth autodetect' '$R/etc/mkinitcpio.conf.d/10-otro.conf'"
probar "no duplica splash con comillas simples" "[[ \$(grep -o splash '$R/etc/default/grub' | wc -l) == 1 ]]"

echo "== systemd-boot (las instalaciones de la ISO)"
nueva_raiz
rm -f "$R/etc/default/grub"
mkdir -p "$R/boot/loader/entries" "$R/etc/kernel"
printf 'title Vortexine'"'"'s OS\nlinux /vmlinuz-linux-cachyos\noptions root=PARTUUID=abc rw quiet nowatchdog\n' >"$R/boot/loader/entries/vortex-os.conf"
echo 'root=PARTUUID=abc rw quiet nowatchdog' >"$R/etc/kernel/cmdline"
pantalla_inicio
probar "la entrada lleva splash" "grep -q '^options splash root=PARTUUID=abc' '$R/boot/loader/entries/vortex-os.conf'"
probar "y /etc/kernel/cmdline también" "grep -q '^splash root=PARTUUID=abc' '$R/etc/kernel/cmdline'"
pantalla_inicio
probar "no lo repite" "[[ \$(grep -o splash '$R/boot/loader/entries/vortex-os.conf' | wc -l) == 1 && \$(grep -o splash '$R/etc/kernel/cmdline' | wc -l) == 1 ]]"

echo "== Un nombre de usuario raro no se escribe en greetd"
nueva_raiz
USER='x"; rm -rf /' pantalla_inicio
probar "greetd sin cambios" "cmp -s '$AQUI/config/sistema/greetd.toml' '$G'"

echo "== La entrada bloqueada"
mkdir -p "$T/g"
cp "$AQUI/config/greeter/entrar-bloqueado" "$T/g/"
printf '#!/bin/sh\necho entro >"%s/entro"\n' "$T" >"$T/g/entrar"
chmod +x "$T/g/"*
XDG_RUNTIME_DIR="$T" "$T/g/entrar-bloqueado"
probar "deja la marca para que el shell arranque bloqueado" "[[ -e '$T/vortex-bloquear-al-entrar' ]]"
probar "y entra a la sesión" "[[ -e '$T/entro' ]]"
# 4.0: si la última vez eligió otro escritorio, no entra directo (sale y greetd muestra el login).
rm -f "$T/entro" "$T/vortex-bloquear-al-entrar"
mkdir -p "$T/r/var/cache/vortex-greeter" "$T/r/usr/share/wayland-sessions"
echo plasma >"$T/r/var/cache/vortex-greeter/sesion-ana"
: >"$T/r/usr/share/wayland-sessions/plasma.desktop"
USER=ana VORTEX_INICIO_RAIZ="$T/r" XDG_RUNTIME_DIR="$T" "$T/g/entrar-bloqueado"
probar "con otro escritorio elegido no entra directo ni bloquea" "[[ ! -e '$T/entro' && ! -e '$T/vortex-bloquear-al-entrar' ]]"
rm "$T/r/usr/share/wayland-sessions/plasma.desktop"
USER=ana VORTEX_INICIO_RAIZ="$T/r" XDG_RUNTIME_DIR="$T" "$T/g/entrar-bloqueado"
probar "si ese escritorio ya no está, entra a Vortex Shell" "[[ -e '$T/entro' && -e '$T/vortex-bloquear-al-entrar' ]]"
ENT="$AQUI/config/greeter/entrar"
probar "entrar pasa el escritorio elegido a vortex-escritorio" "grep -q 'exec vortex-escritorio entrar \"\$1\"' '$ENT'"
probar "entrar detiene lo que quedó de una sesión gráfica cortada" "grep -q 'systemctl --user stop graphical-session.target' '$ENT'"
probar "el login lista los escritorios y recuerda el de cada quien" \
  "grep -q 'vortex-escritorio\", \"sesiones' '$AQUI/config/greeter/shell.qml' && grep -q '/sesion-' '$AQUI/config/greeter/shell.qml'"
probar "el gestor no arranca con otros escritorios" \
  "grep -q '^ConditionEnvironment=XDG_CURRENT_DESKTOP=Hyprland' '$AQUI/config/systemd/vortex-gestor.service'"
LOCK="$AQUI/shell/src/quickshell/lock/Lock.qml"
probar "el bloqueo del shell lee y borra esa marca, y bloquea" \
  "grep -q 'vortex-bloquear-al-entrar' '$LOCK' && grep -q 'rm -f' '$LOCK' && grep -q 'root.lockStatic()' '$LOCK'"
probar "al desbloquear se abre el llavero (PAM del shell)" \
  "grep -q 'pam_gnome_keyring.so' '$AQUI/packaging/vortex-shell/PKGBUILD'"

echo "== Respaldo del login: si falla, la de texto"
probar "vortex-greeter cae a tuigreet" "grep -q 'exec tuigreet' '$AQUI/config/greeter/vortex-greeter'"
probar "solo sale bien si el login entregó la sesión (rc 0)" "grep -q '\"\$rc\" == 0' '$AQUI/config/greeter/vortex-greeter'"

echo "== Llega a todos"
probar "«vortex actualizar» lo llama" "sed -n '/^cmd_actualizar() {/,/^}/p' '$AQUI/bin/vortex' | grep '^  pantalla_inicio$' >/dev/null"
probar "el instalador lo llama" "grep -q 'vortex\" inicio-sistema' '$AQUI/install/20-sistema.sh'"
probar "plymouth está en los paquetes nuevos" "grep -q '^  plymouth #' '$AQUI/bin/vortex'"

if [[ -s "$T/avisos.log" ]]; then echo "Avisos:"; cat "$T/avisos.log"; fi
echo
((FALLOS == 0)) && echo "Todo bien." || echo "$FALLOS fallas."
exit $((FALLOS > 0))
