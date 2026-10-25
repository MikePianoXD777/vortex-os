#!/usr/bin/env bash
# Pruebas del arranque y los discos de la ISO (BIOS con GRUB, la partición EFI chica junto a
# Windows, los drivers de la GPU sin red), sin construir la ISO, sin root, sin red y sin tocar tu
# HOME ni tu sistema: todo pasa en carpetas temporales, con una raíz falsa (VORTEX_RAIZ) y
# comandos de mentira (df, pacman, chwd, lspci...) en el PATH. Lo que necesita Calamares de
# verdad (el instalador corriendo en BIOS) solo se puede probar en una VM con la ISO.
#   1. sintaxis, YAML de Calamares (pyyaml: si no está ni se puede traer con uv, se salta);
#   2. vortex-bios (módulo de Calamares) con un libcalamares de mentira;
#   3. vortex-post-instalacion kernel: sin fallback y, si /boot no alcanza, solo linux-cachyos;
#   4. vortex-post-instalacion grub / arranque / final (BIOS y UEFI, NVIDIA);
#   5. vortex-post-instalacion gpu y vortex-gpu: la marca chwd-pendiente y el servicio.
#
#   pruebas/arranque.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-arranque-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
salto() { printf '  --  %s\n' "$*"; }
probar() { local d="$1"; shift; if "$@"; then bien "$d"; else mal "$d"; fi; }
# El archivo contiene el texto (literal, no regex).
contiene() { grep -qF -- "$2" "$1" 2>/dev/null; }
no_contiene() { ! grep -qF -- "$2" "$1" 2>/dev/null; }

POST="$AQUI/iso/airootfs/usr/local/bin/vortex-post-instalacion"
GPU="$AQUI/iso/airootfs/usr/local/bin/vortex-gpu"
MODBIOS="$AQUI/iso/airootfs/usr/lib/calamares/modules/vortex-bios"

# Comandos de mentira. df: el espacio libre de /boot sale de $FAKE_LIBRE (MB); pacman, chwd,
# notify-send, runuser y loginctl anotan lo que les piden en $T/llamadas.
mkdir -p "$T/shim"
cat >"$T/shim/df" <<'EOF'
#!/usr/bin/env bash
[[ -n "${FAKE_LIBRE:-}" ]] || exit 1
echo "S.ficheros     1048576-bloques Usados Disponibles Capacidad Montado en"
echo "/dev/fake      500 100 $FAKE_LIBRE 20% /boot"
EOF
cat >"$T/shim/pacman" <<'EOF'
#!/usr/bin/env bash
echo "pacman $*" >>"$T/llamadas"
case "$1" in
  -Qq) [[ -z "${FAKE_SIN_PAQUETES:-}" ]] ;;
  -Sy|-Su) [[ -z "${FAKE_PACMAN_FALLA:-}" ]] ;;
  *) true ;;
esac
EOF
cat >"$T/shim/chwd" <<'EOF'
#!/usr/bin/env bash
echo "chwd $*" >>"$T/llamadas"
[[ -z "${FAKE_CHWD_FALLA:-}" ]]
EOF
cat >"$T/shim/lspci" <<'EOF'
#!/usr/bin/env bash
echo "lspci $*" >>"$T/llamadas"
if [[ "$*" == "-d 10de:*:03xx" && -n "${FAKE_NVIDIA:-}" ]]; then
  echo "01:00.0 VGA compatible controller: NVIDIA Corporation GA102 [GeForce RTX 3080] (rev a1)"
fi
exit 0
EOF
cat >"$T/shim/loginctl" <<'EOF'
#!/usr/bin/env bash
echo "         UID USER   LINGER STATE"
echo "        1000 mike   no     active"
echo "        1001 ana    no     active"
EOF
cat >"$T/shim/runuser" <<'EOF'
#!/usr/bin/env bash
echo "runuser $*" >>"$T/llamadas"
# runuser -u USUARIO -- env VAR=... comando args
shift 3
exec "$@"
EOF
cat >"$T/shim/notify-send" <<'EOF'
#!/usr/bin/env bash
echo "notify-send $DBUS_SESSION_BUS_ADDRESS $*" >>"$T/llamadas"
EOF
chmod +x "$T/shim"/*
export T
# Cada prueba arranca con un registro de llamadas vacío.
limpia() { : >"$T/llamadas"; }
en_shim() { env PATH="$T/shim:/usr/bin:/bin" "$@"; }

# ---------------------------------------------------------------------------------------------
echo "== Sintaxis y YAML"
for f in iso/airootfs/usr/local/bin/vortex-post-instalacion iso/airootfs/usr/local/bin/vortex-gpu \
  iso/construir.sh pruebas/arranque.sh; do
  probar "bash -n $f" bash -n "$AQUI/$f"
done
probar "python -m py_compile vortex-bios/main.py" python3 -I -c \
  "import sys; compile(open(sys.argv[1], encoding='utf-8').read(), sys.argv[1], 'exec')" "$MODBIOS/main.py"
if command -v systemd-analyze >/dev/null; then
  # El servicio apunta a /usr/local/bin/vortex-gpu: para verificarlo se usa una copia del script
  # en una ruta sin espacios (la carpeta del repo puede tenerlos y systemd no los admite ahí).
  mkdir -p "$T/unit"
  install -m755 "$GPU" "$T/unit/vortex-gpu"
  sed "s|/usr/local/bin/vortex-gpu|$T/unit/vortex-gpu|" "$AQUI/iso/airootfs/etc/systemd/system/vortex-gpu.service" >"$T/unit/vortex-gpu.service"
  systemd-analyze verify --man=no "$T/unit/vortex-gpu.service" >"$T/verify.out" 2>&1
  probar "systemd-analyze verify vortex-gpu.service sin avisos" test ! -s "$T/verify.out"
  [[ -s "$T/verify.out" ]] && sed 's/^/      /' "$T/verify.out"
else
  salto "sin systemd-analyze: se salta la revisión del servicio"
fi

cat >"$T/yaml.py" <<'PY'
import glob, sys
import yaml
raiz = sys.argv[1]
cal = raiz + "/iso/airootfs/etc/calamares"
mal = []
def cargar(f):
    try:
        return yaml.safe_load(open(f, encoding="utf-8"))
    except Exception as e:
        mal.append(f"YAML roto: {f}: {e}")
        return None
for f in sorted(glob.glob(cal + "/**/*.conf", recursive=True) + glob.glob(raiz + "/iso/airootfs/usr/lib/calamares/modules/vortex-bios/module.desc")):
    cargar(f)

s = cargar(cal + "/settings.conf") or {}
ejecutar = [x for b in s.get("sequence", []) if "exec" in b for x in b["exec"]]
for paso in ("grubcfg@vortex", "vortex-bios", "bootloader@vortex"):
    if paso not in ejecutar:
        mal.append(f"settings.conf: falta {paso} en la secuencia de instalación")
if all(p in ejecutar for p in ("grubcfg@vortex", "vortex-bios", "bootloader@vortex", "shellprocess@vortex-final")):
    i = ejecutar.index
    if not (i("shellprocess@vortex-final") < i("grubcfg@vortex") < i("vortex-bios") < i("bootloader@vortex")):
        mal.append("settings.conf: el orden tiene que ser vortex-final, grubcfg, vortex-bios, bootloader")
instancias = {(x["module"], x["id"]): x["config"] for x in s.get("instances", [])}
if instancias.get(("grubcfg", "vortex")) != "grubcfg_vortex.conf":
    mal.append("settings.conf: falta la instancia grubcfg@vortex con grubcfg_vortex.conf")
for modulo, conf in instancias.items():
    if not glob.glob(cal + "/modules/" + conf):
        mal.append(f"settings.conf: {modulo} apunta a {conf}, que no existe")

b = cargar(cal + "/modules/bootloader_vortex.conf") or {}
if b.get("efiBootLoader") != "systemd-boot":
    mal.append("bootloader_vortex.conf: en UEFI sigue siendo systemd-boot")
for k, v in (("grubInstall", "grub-install"), ("grubMkconfig", "grub-mkconfig"), ("grubCfg", "/boot/grub/grub.cfg"), ("grubProbe", "grub-probe")):
    if b.get(k) != v:
        mal.append(f"bootloader_vortex.conf: {k} debe ser {v} (BIOS: el módulo lo lee sin valor por omisión)")

p = cargar(cal + "/modules/partition_vortex.conf") or {}
if p.get("initialPartitioningChoice") != "none":
    mal.append("partition_vortex.conf: initialPartitioningChoice debe ser none")
if "defaultPartitionTableType" in p:
    mal.append("partition_vortex.conf: sin defaultPartitionTableType (GPT en UEFI, MBR en BIOS)")
if p.get("efiSystemPartition") != "/boot":
    mal.append("partition_vortex.conf: la partición EFI se monta en /boot")
if (p.get("efi") or {}).get("minimumSize") != "300MiB":
    mal.append("partition_vortex.conf: falta efi.minimumSize")

unidades = {u["name"]: u for u in (cargar(cal + "/modules/services-systemd_vortex.conf") or {}).get("units", [])}
if unidades.get("vortex-gpu", {}).get("action") != "enable":
    mal.append("services-systemd_vortex.conf: falta vortex-gpu")

desc = cargar(raiz + "/iso/airootfs/usr/lib/calamares/modules/vortex-bios/module.desc") or {}
if (desc.get("type"), desc.get("name"), desc.get("interface"), desc.get("script")) != ("job", "vortex-bios", "python", "main.py"):
    mal.append(f"vortex-bios/module.desc: {desc}")

paquetes = [l.split("#")[0].strip() for l in open(raiz + "/iso/paquetes.txt", encoding="utf-8")]
if "grub" not in paquetes:
    mal.append("paquetes.txt: falta grub (BIOS)")
if "os-prober" not in paquetes:
    mal.append("paquetes.txt: falta os-prober (el menú de GRUB con Windows)")
print("\n".join(mal))
sys.exit(1 if mal else 0)
PY
python_yaml=()
if python3 -I -c 'import yaml' 2>/dev/null; then
  python_yaml=(python3 -I)
elif command -v uv >/dev/null && timeout 90 uv run --no-project --with pyyaml python -I -c 'import yaml' >/dev/null 2>&1; then
  python_yaml=(uv run --no-project --with pyyaml python -I)
fi
if ((${#python_yaml[@]})); then
  yaml_rc=1
  "${python_yaml[@]}" "$T/yaml.py" "$AQUI" >"$T/yaml.out" 2>&1 && yaml_rc=0
  probar "YAML de Calamares válido; secuencia grubcfg → vortex-bios → bootloader; claves de GRUB; partition; vortex-gpu" test "$yaml_rc" -eq 0
  sed 's/^/      /' "$T/yaml.out"
else
  salto "sin pyyaml (ni uv con red para traerlo): se salta la validación del YAML de Calamares"
fi

probar "construir.sh enciende vortex-gpu.service por preset" grep -qx 'enable vortex-gpu.service' "$AQUI/iso/construir.sh"
probar "construir.sh lo deja en multi-user.target.wants" grep -qF 'quiere multi-user vortex-gpu.service /etc/systemd/system/vortex-gpu.service' "$AQUI/iso/construir.sh"
probar "vortex-gpu.service y vortex-gpu vienen en airootfs (construir.sh copia todo airootfs)" \
  test -f "$AQUI/iso/airootfs/etc/systemd/system/vortex-gpu.service" -a -x "$GPU"

# ---------------------------------------------------------------------------------------------
echo "== vortex-bios: Calamares no debe saltarse grub-install en BIOS"
cat >"$T/bios.py" <<'PY'
import importlib.util, sys, types

class GS:
    def __init__(self, datos, sin_remove=False):
        self.d = dict(datos)
        self.sin_remove = sin_remove
    def value(self, k): return self.d.get(k)
    def contains(self, k): return k in self.d
    def insert(self, k, v): self.d[k] = v
    def remove(self, k):
        if self.sin_remove:
            raise AttributeError("remove")
        del self.d[k]

def cargar(gs):
    cal = types.ModuleType("libcalamares")
    cal.globalstorage = gs
    cal.utils = types.SimpleNamespace(debug=lambda *a: None, warning=lambda *a: None)
    sys.modules["libcalamares"] = cal
    spec = importlib.util.spec_from_file_location("vortex_bios", sys.argv[1])
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m

mal = []
def comprobar(nombre, gs, esperado, **kw):
    m = cargar(gs)
    r = m.run()
    if r is not None:
        mal.append(f"{nombre}: run() devolvió {r!r} (tiene que ser None: el trabajo no falla nunca)")
    if gs.value("efiSystemPartition") != esperado:
        mal.append(f"{nombre}: efiSystemPartition quedó {gs.value('efiSystemPartition')!r}, esperaba {esperado!r}")

comprobar("BIOS", GS({"firmwareType": "bios", "efiSystemPartition": "/boot"}), None)
g = GS({"firmwareType": "bios", "efiSystemPartition": "/boot"}, sin_remove=True)
comprobar("BIOS sin remove()", g, None)
comprobar("UEFI", GS({"firmwareType": "efi", "efiSystemPartition": "/boot"}), "/boot")
comprobar("sin firmwareType", GS({"efiSystemPartition": "/boot"}), "/boot")
g = GS({"firmwareType": "bios"})
comprobar("BIOS sin la clave", g, None)
if not cargar(GS({})).pretty_name():
    mal.append("pretty_name vacío")
print("\n".join(mal))
sys.exit(1 if mal else 0)
PY
bios_rc=1
python3 -I -B "$T/bios.py" "$MODBIOS/main.py" >"$T/bios.out" 2>&1 && bios_rc=0
probar "BIOS quita efiSystemPartition; UEFI y firmware desconocido no la tocan; nunca falla" test "$bios_rc" -eq 0
sed 's/^/      /' "$T/bios.out"

# ---------------------------------------------------------------------------------------------
echo "== vortex-post-instalacion kernel: sin fallback, y si /boot no alcanza solo linux-cachyos"
# Raíz falsa con los dos kernels de la imagen (2 MB cada vmlinuz) y presets como los de Arch.
hacer_kernels() {
  rm -rf "$T/raiz"
  local k
  for k in linux-cachyos linux; do
    mkdir -p "$T/raiz/usr/lib/modules/6.1-$k"
    truncate -s 2M "$T/raiz/usr/lib/modules/6.1-$k/vmlinuz"
    echo "$k" >"$T/raiz/usr/lib/modules/6.1-$k/pkgbase"
  done
  mkdir -p "$T/raiz/etc/mkinitcpio.d" "$T/raiz/etc/mkinitcpio.conf.d" "$T/raiz/usr/share/mkinitcpio" "$T/raiz/boot"
  touch "$T/raiz/etc/mkinitcpio.conf.d/archiso.conf"
  # El de Arch trae default+fallback; el de CachyOS no trae preset (se arma con hook.preset).
  printf "PRESETS=('default' 'fallback')\ndefault_image=\"/boot/initramfs-linux.img\"\nfallback_image=\"/boot/initramfs-linux-fallback.img\"\n" >"$T/raiz/etc/mkinitcpio.d/linux.preset"
  printf "# preset de ejemplo\nPRESETS=('default' 'fallback')\ndefault_image=\"/boot/initramfs-%%PKGBASE%%.img\"\nfallback_image=\"/boot/initramfs-%%PKGBASE%%-fallback.img\"\n" >"$T/raiz/usr/share/mkinitcpio/hook.preset"
}
kernel() { limpia; VORTEX_RAIZ="$T/raiz" FAKE_LIBRE="${1:-}" en_shim bash "$POST" kernel >"$T/kernel.out" 2>&1; }

hacer_kernels; kernel 2000
probar "con espacio: copia los dos kernels a /boot" test -f "$T/raiz/boot/vmlinuz-linux-cachyos" -a -f "$T/raiz/boot/vmlinuz-linux"
probar "con espacio: no quita ningún paquete" no_contiene "$T/llamadas" "-Rdd"
probar "con espacio: sin la imagen fallback en el preset de Arch" contiene "$T/raiz/etc/mkinitcpio.d/linux.preset" "PRESETS=('default')"
probar "con espacio: sin la imagen fallback en el preset de linux-cachyos (armado con hook.preset)" \
  contiene "$T/raiz/etc/mkinitcpio.d/linux-cachyos.preset" "PRESETS=('default')"
probar "con espacio: el preset de linux-cachyos se armó con su nombre" contiene "$T/raiz/etc/mkinitcpio.d/linux-cachyos.preset" "initramfs-linux-cachyos.img"
probar "los hooks de archiso se quitan" test ! -e "$T/raiz/etc/mkinitcpio.conf.d/archiso.conf"
probar "con espacio: sin avisos" no_contiene "$T/kernel.out" "AVISO"

# Cada kernel pide 2 × (2 MB + 1 + 60 de initramfs) = 126 MB; los dos, 252.
hacer_kernels; kernel 200
probar "ESP chica (200 MB libres): se queda linux-cachyos" test -f "$T/raiz/boot/vmlinuz-linux-cachyos"
probar "ESP chica: no se copia el kernel de Arch" test ! -e "$T/raiz/boot/vmlinuz-linux"
probar "ESP chica: se quita el paquete linux (si no, el hook de mkinitcpio lo traería de vuelta)" \
  contiene "$T/llamadas" "pacman --root $T/raiz -Rdd --noconfirm linux"
probar "ESP chica: no se quita linux-cachyos" no_contiene "$T/llamadas" "-Rdd --noconfirm linux-cachyos"
probar "ESP chica: el preset de Arch ya no está (initcpio no le armaría un initramfs sin kernel)" test ! -e "$T/raiz/etc/mkinitcpio.d/linux.preset"
probar "ESP chica: linux-cachyos sigue sin fallback" contiene "$T/raiz/etc/mkinitcpio.d/linux-cachyos.preset" "PRESETS=('default')"
probar "ESP chica: avisa en el log" contiene "$T/kernel.out" "dejo solo linux-cachyos"
probar "ESP chica: con uno solo cabe, no hay segundo aviso" no_contiene "$T/kernel.out" "puede faltar espacio"

hacer_kernels; kernel 100
probar "ni uno cabe (100 MB libres): se queda linux-cachyos y avisa que puede faltar espacio" \
  contiene "$T/kernel.out" "puede faltar espacio"
probar "ni uno cabe: igual se copia linux-cachyos (la instalación decide)" test -f "$T/raiz/boot/vmlinuz-linux-cachyos"

hacer_kernels; kernel ""
probar "sin poder medir el espacio (df falla): copia los dos, sin tocar nada" test -f "$T/raiz/boot/vmlinuz-linux" -a -f "$T/raiz/boot/vmlinuz-linux-cachyos"
probar "sin poder medir el espacio: no quita paquetes" no_contiene "$T/llamadas" "-Rdd"

hacer_kernels; rm -rf "$T/raiz/usr/lib/modules/6.1-linux"; kernel 100
probar "un solo kernel en la imagen (linux-cachyos): nunca lo quita aunque no quepa" \
  test -f "$T/raiz/boot/vmlinuz-linux-cachyos" -a ! -s "$T/llamadas"

# ---------------------------------------------------------------------------------------------
echo "== vortex-post-instalacion grub / arranque: BIOS con GRUB, UEFI con systemd-boot"
hacer_boot() { # hacer_boot uefi|bios
  rm -rf "$T/raiz"
  mkdir -p "$T/raiz/boot" "$T/raiz/etc/kernel"
  # /sys/firmware existe siempre; /sys/firmware/efi solo con firmware UEFI.
  mkdir -p "$T/raiz/sys/firmware"
  [[ "$1" == uefi ]] && mkdir -p "$T/raiz/sys/firmware/efi"
  touch "$T/raiz/boot/vmlinuz-linux-cachyos" "$T/raiz/boot/vmlinuz-linux"
}
grub() { VORTEX_RAIZ="$T/raiz" en_shim bash "$POST" grub >"$T/grub.out" 2>&1; }
arranque() { limpia; VORTEX_RAIZ="$T/raiz" FAKE_NVIDIA="${1:-}" en_shim bash "$POST" arranque >"$T/arranque.out" 2>&1; }

hacer_boot bios; grub
D="$T/raiz/etc/default/grub.d/10-vortex.cfg"
probar "BIOS: deja /etc/default/grub.d/10-vortex.cfg" test -f "$D"
probar "BIOS: título «Vortex OS»" bash -c "source '$D'; [[ \$GRUB_DISTRIBUTOR == \"Vortexine's OS\" ]]"
probar "BIOS: linux-cachyos va primero y por defecto (GRUB_TOP_LEVEL)" bash -c "source '$D'; [[ \$GRUB_TOP_LEVEL == /boot/vmlinuz-linux-cachyos ]]"
probar "BIOS: os-prober encendido (GRUB 2.12+ lo trae apagado; sin esto Windows no sale)" bash -c "source '$D'; [[ \$GRUB_DISABLE_OS_PROBER == false ]]"
probar "BIOS: tiempo de espera de 3 s, como systemd-boot" bash -c "source '$D'; [[ \$GRUB_TIMEOUT == 3 ]]"
# grub-mkconfig lee estos archivos con «.» dentro de sh: que no necesiten bash.
probar "BIOS: el archivo es sh válido" sh -n "$D"
hacer_boot uefi; grub
probar "UEFI: no deja nada de GRUB" test ! -e "$T/raiz/etc/default/grub.d"

# BIOS: el módulo bootloader ya instaló GRUB.
hacer_boot bios
mkdir -p "$T/raiz/boot/grub/i386-pc"; touch "$T/raiz/boot/grub/i386-pc/core.img"
printf 'menuentry "Vortexine'\''s OS Linux" {\n}\nmenuentry "Windows" {\n}\n' >"$T/raiz/boot/grub/grub.cfg"
arranque
probar "BIOS: con GRUB instalado lo dice y cuenta sus entradas" contiene "$T/arranque.out" "arranque (BIOS): GRUB con 2 entradas"
probar "BIOS: no crea entradas de systemd-boot" test ! -e "$T/raiz/boot/loader"
rm "$T/raiz/boot/grub/i386-pc/core.img"; arranque
probar "BIOS: si GRUB no quedó instalado en el disco, avisa" contiene "$T/arranque.out" "GRUB no quedó instalado en el disco"
touch "$T/raiz/boot/grub/i386-pc/core.img"; : >"$T/raiz/boot/grub/grub.cfg"; arranque
probar "BIOS: si falta el menú, avisa" contiene "$T/arranque.out" "falta su menú"

# UEFI: entradas de systemd-boot (con el cmdline que dejó Calamares).
hacer_uefi() {
  hacer_boot uefi
  mkdir -p "$T/raiz/boot/loader/entries"
  printf 'title Arch Linux\nlinux /x\noptions root=UUID=1234 rw quiet splash\n' >"$T/raiz/boot/loader/entries/arch.conf"
}
hacer_uefi; arranque ""
probar "UEFI: entrada «Vortexine's OS» con linux-cachyos" contiene "$T/raiz/boot/loader/entries/vortex-os.conf" "linux   /vmlinuz-linux-cachyos"
probar "UEFI: entrada del kernel de Arch de respaldo, sin aviso de NVIDIA" \
  contiene "$T/raiz/boot/loader/entries/vortex-os-arch.conf" "title   Vortexine's OS (kernel de Arch, respaldo)"
probar "UEFI: linux-cachyos por defecto" contiene "$T/raiz/boot/loader/loader.conf" "default vortex-os.conf"
probar "UEFI: sin NVIDIA no avisa" no_contiene "$T/arranque.out" "NVIDIA"
hacer_uefi; arranque 1
probar "UEFI con NVIDIA: el título del kernel de Arch dice que no tiene driver de video" \
  contiene "$T/raiz/boot/loader/entries/vortex-os-arch.conf" "sin driver de NVIDIA: solo consola"
probar "UEFI con NVIDIA: la entrada de Vortexine's OS sigue igual" contiene "$T/raiz/boot/loader/entries/vortex-os.conf" "title   Vortexine's OS"
probar "UEFI con NVIDIA: usa la misma consulta que chwd (clase 03xx)" contiene "$T/llamadas" "lspci -d 10de:*:03xx"
hacer_uefi; rm "$T/raiz/boot/vmlinuz-linux"; arranque 1
probar "UEFI sin kernel de Arch (se quitó por espacio): solo la entrada de Vortexine's OS" \
  test -f "$T/raiz/boot/loader/entries/vortex-os.conf" -a ! -e "$T/raiz/boot/loader/entries/vortex-os-arch.conf"

hacer_boot bios; rmdir "$T/raiz/sys/firmware" "$T/raiz/sys"; grub
probar "sin /sys montado no se supone BIOS (no toca GRUB)" test ! -e "$T/raiz/etc/default/grub.d"
mkdir -p "$T/raiz/boot/loader/entries"; printf 'options root=UUID=1 rw\n' >"$T/raiz/boot/loader/entries/a.conf"; arranque
probar "sin /sys montado el arranque sigue el camino UEFI (entradas de systemd-boot)" test -f "$T/raiz/boot/loader/entries/vortex-os.conf"

echo "== vortex-post-instalacion final: qué paquetes del live se quitan"
sobran() { VORTEX_RAIZ="$T/raiz" FAKE_SIN_PAQUETES="${1:-}" en_shim bash -c "source '$POST'; paquetes_del_live" | paste -sd ' '; }
hacer_boot uefi
probar "UEFI: se quitan calamares, ckbcomp, os-prober y mkinitcpio-archiso" test "$(sobran)" = "calamares ckbcomp os-prober mkinitcpio-archiso"
hacer_boot bios
probar "BIOS: os-prober se queda (GRUB lo usa para poner Windows en el menú)" test "$(sobran)" = "calamares ckbcomp mkinitcpio-archiso"
probar "los paquetes que no están instalados no se piden quitar" test -z "$(sobran 1)"

# ---------------------------------------------------------------------------------------------
echo "== vortex-post-instalacion gpu: chwd sin red deja la marca"
MARCA="$T/raiz/var/lib/vortex/chwd-pendiente"
gpu() { limpia; VORTEX_RAIZ="$T/raiz" FAKE_CHWD_FALLA="${1:-}" en_shim bash "$POST" gpu >"$T/gpu.out" 2>&1; }
rm -rf "$T/raiz"; mkdir -p "$T/raiz"
gpu 1
probar "chwd falla (sin red): deja /var/lib/vortex/chwd-pendiente" test -e "$MARCA"
probar "chwd falla: lo dice en el log" contiene "$T/gpu.out" "vortex-gpu.service"
probar "chwd falla: no hace fallar la instalación" bash -c "VORTEX_RAIZ='$T/raiz' FAKE_CHWD_FALLA=1 PATH='$T/shim:/usr/bin:/bin' bash '$POST' gpu >/dev/null 2>&1"
gpu ""
probar "chwd funciona: corre «chwd -a» y borra la marca" bash -c "grep -qx 'chwd -a' '$T/llamadas' && [[ ! -e '$MARCA' ]]"
# Sin chwd: un PATH vacío (el chwd de verdad de este equipo no debe correr nunca en las pruebas).
rm -rf "$T/raiz" "$T/vacio"; mkdir -p "$T/raiz" "$T/vacio"
limpia; VORTEX_RAIZ="$T/raiz" PATH="$T/vacio" /usr/bin/bash "$POST" gpu >"$T/gpu.out" 2>&1
probar "sin chwd (ni el paquete): no deja marca ni falla" bash -c "[[ ! -e '$MARCA' ]] && [[ ! -s '$T/gpu.out' ]]"

echo "== vortex-gpu: el servicio que termina los drivers cuando hay red"
mkdir -p "$T/run/1000"
python3 -I -c "import socket,sys; s=socket.socket(socket.AF_UNIX); s.bind(sys.argv[1])" "$T/run/1000/bus"
gpu_servicio() { # gpu_servicio [VAR=valor ...]
  limpia
  env PATH="$T/shim:/usr/bin:/bin" VORTEX_GPU_MARCA="$T/marca" VORTEX_GPU_RUN_USER="$T/run" "$@" bash "$GPU" >"$T/vgpu.out" 2>&1
}
rm -f "$T/marca"
gpu_servicio
probar "sin marca: no hace nada y termina bien" test $? -eq 0 -a ! -s "$T/llamadas"
touch "$T/marca"
gpu_servicio
rc=$?
probar "con marca y red: termina bien" test "$rc" -eq 0
probar "con marca y red: borra la marca" test ! -e "$T/marca"
probar "con marca y red: actualiza los llaveros y el sistema entero antes (nunca -Sy a secas)" \
  bash -c "mapfile -t l < <(grep -v '^notify\|^runuser' '$T/llamadas'); [[ \${l[0]} == 'pacman -Sy --noconfirm --needed archlinux-keyring cachyos-keyring' && \${l[1]} == 'pacman -Su --noconfirm' && \${l[2]} == 'chwd -a' && \${#l[@]} -eq 3 ]]"
probar "con marca y red: avisa en el escritorio de quien tiene sesión con bus" contiene "$T/llamadas" "notify-send unix:path=$T/run/1000/bus -a Vortexine's OS -i video-display Drivers de video instalados Reinicia para usarlos."
probar "con marca y red: no avisa a quien no tiene bus de sesión" no_contiene "$T/llamadas" "1001"
touch "$T/marca"
gpu_servicio FAKE_PACMAN_FALLA=1
rc=$?
probar "pacman falla (sin red): termina mal para que el servicio reintente" test "$rc" -ne 0
probar "pacman falla: la marca se queda" test -e "$T/marca"
probar "pacman falla: no corre chwd" no_contiene "$T/llamadas" "chwd"
gpu_servicio FAKE_CHWD_FALLA=1
rc=$?
probar "chwd falla: termina mal para que el servicio reintente" test "$rc" -ne 0
probar "chwd falla: la marca se queda y no avisa de nada" bash -c "[[ -e '$T/marca' ]] && ! grep -q '^notify' '$T/llamadas'"
rm -f "$T/marca"

echo "== vortex-post-instalacion imagen (antes de unpackfs)"
A="$T/archiso"
imagen() { VORTEX_ARCHISO="$A" bash "$POST" imagen >>"$T/imagen.out" 2>&1; }
mkdir -p "$A/bootmnt/arch/x86_64" && : >"$A/bootmnt/arch/x86_64/airootfs.sfs"
probar "USB montada (copytoram=n): no toca nada" bash -c "$(declare -f imagen); A='$A' T='$T' POST='$POST'; imagen && [[ -f '$A/bootmnt/arch/x86_64/airootfs.sfs' && ! -L '$A/bootmnt/arch/x86_64/airootfs.sfs' ]]"
rm -rf "$A" && mkdir -p "$A/copytoram" && : >"$A/copytoram/airootfs.sfs"
probar "copiada a la RAM y sin bootmnt (Ventoy): enlaza la copia donde la busca unpackfs" bash -c "$(declare -f imagen); A='$A' T='$T' POST='$POST'; imagen && [[ \"\$(readlink '$A/bootmnt/arch/x86_64/airootfs.sfs')\" == '$A/copytoram/airootfs.sfs' ]]"
rm -rf "$A" && mkdir -p "$A"
probar "sin imagen en ningún lado: falla (unpackfs no seguiría a ciegas)" bash -c "$(declare -f imagen); A='$A' T='$T' POST='$POST'; ! imagen"
probar "settings.conf corre el paso justo antes de unpackfs" bash -c "grep -A1 -- '- shellprocess@vortex-imagen' '$AQUI/iso/airootfs/etc/calamares/settings.conf' | grep -q -- '- unpackfs@vortex'"
probar "construir.sh pone copytoram=n en las entradas de arranque" grep -qF "& copytoram=n/" "$AQUI/iso/construir.sh"

echo
if ((FALLOS)); then
  echo "$FALLOS pruebas fallaron (salidas de lo que corrieron, abajo)"
  for f in "$T"/imagen.out "$T"/kernel.out "$T"/arranque.out "$T"/gpu.out "$T"/vgpu.out "$T"/grub.out "$T"/bios.out; do
    [[ -s "$f" ]] && { echo "--- ${f##*/}"; sed 's/^/    /' "$f"; }
  done
  exit 1
fi
echo "Todas las pruebas pasaron."
