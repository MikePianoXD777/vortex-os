#!/usr/bin/env bash
# Máquina virtual de pruebas de Vortex OS (QEMU + KVM, UEFI).
#
#   vm/vm.sh live [iso]   arranca una ISO (por defecto la de Arch) con acceso SSH por el puerto 2222
#   vm/vm.sh disco        arranca lo instalado en el disco
#   vm/vm.sh ssh [cmd]    entra por SSH (root en la ISO, vortex en el disco)
#   vm/vm.sh captura F    guarda una captura de la pantalla de la VM en F (.png)
#   vm/vm.sh copiar O D   copia con rsync (la ruta remota va como :ruta)
#   vm/vm.sh apagar       apaga la VM y espera (hasta 60 s) a que QEMU termine
#   vm/vm.sh matar        corta la VM de golpe si no se apaga
#   vm/vm.sh borrar       borra el disco para empezar de cero
#
# Todo vive en ~/.cache/vortex-os-vm (fuera de la bóveda, son varios GB).
set -euo pipefail

VM="${VORTEX_VM:-$HOME/.cache/vortex-os-vm}"
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ISO_ARCH="$(ls "$VM"/iso/archlinux-*-x86_64.iso 2>/dev/null | sort | tail -1 || true)"
DISCO="$VM/disco.qcow2"
LLAVE="$VM/llave"
PUERTO=2222
OVMF=/usr/share/edk2/x64

mkdir -p "$VM"

preparar() {
  [[ -f "$LLAVE" ]] || ssh-keygen -q -t ed25519 -N '' -C vortex-vm -f "$LLAVE"
  [[ -f "$DISCO" ]] || qemu-img create -q -f qcow2 "$DISCO" 40G
  [[ -f "$VM/OVMF_VARS.fd" ]] || cp "$OVMF/OVMF_VARS.4m.fd" "$VM/OVMF_VARS.fd"

  # Semilla de cloud-init: la ISO de Arch la lee al arrancar y así deja entrar por SSH con la llave.
  local semilla="$VM/semilla"
  mkdir -p "$semilla"
  printf 'instance-id: vortex-vm\nlocal-hostname: vortex-vm\n' >"$semilla/meta-data"
  cat >"$semilla/user-data" <<EOF
#cloud-config
runcmd:
  - mkdir -p /root/.ssh
  - echo "$(cat "$LLAVE.pub")" > /root/.ssh/authorized_keys
  - chmod 700 /root/.ssh
  - chmod 600 /root/.ssh/authorized_keys
  - systemctl start sshd
EOF
  xorriso -as mkisofs -quiet -o "$VM/semilla.iso" -V CIDATA -J -r "$semilla" 2>/dev/null
}

qemu_comun() {
  QEMU=(
    qemu-system-x86_64 -enable-kvm -machine q35 -cpu host
    -smp "${VORTEX_VM_CPUS:-6}" -m "${VORTEX_VM_RAM:-6G}"
    -drive "if=pflash,format=raw,readonly=on,file=$OVMF/OVMF_CODE.4m.fd"
    -drive "if=pflash,format=raw,file=$VM/OVMF_VARS.fd"
    -drive "file=$DISCO,if=virtio,cache=unsafe"
    -nic "user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:$PUERTO-:22"
    -device virtio-vga-gl -display egl-headless
    -device virtio-keyboard -device virtio-tablet
    -monitor "unix:$VM/monitor.sock,server,nowait"
    -serial "file:$VM/serial.log"
    -pidfile "$VM/qemu.pid" -daemonize
  )
}

corriendo() { [[ -f "$VM/qemu.pid" ]] && kill -0 "$(cat "$VM/qemu.pid")" 2>/dev/null; }

monitor() { printf '%s\n' "$*" | socat - "UNIX-CONNECT:$VM/monitor.sock" >/dev/null; }

case "${1:-}" in
  live)
    corriendo && { echo "La VM ya está corriendo." >&2; exit 1; }
    preparar
    iso="${2:-$ISO_ARCH}"
    [[ -f "$iso" ]] || { echo "No encuentro la ISO: $iso" >&2; exit 1; }
    qemu_comun
    "${QEMU[@]}" -drive "file=$iso,media=cdrom,index=2" -drive "file=$VM/semilla.iso,media=cdrom,index=3" -boot d
    echo "VM arrancando desde $(basename "$iso")"
    ;;
  disco)
    corriendo && { echo "La VM ya está corriendo." >&2; exit 1; }
    preparar
    qemu_comun
    "${QEMU[@]}" -boot c
    echo "VM arrancando desde el disco"
    ;;
  ssh)
    shift
    usuario="${VORTEX_VM_USUARIO:-root}"
    exec ssh -q -i "$LLAVE" -p "$PUERTO" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o ConnectTimeout=5 "$usuario@127.0.0.1" "$@"
    ;;
  copiar)
    # vm/vm.sh copiar ORIGEN DESTINO  (rutas remotas como :ruta)
    shift
    usuario="${VORTEX_VM_USUARIO:-root}"
    exec rsync -a --delete -e "ssh -q -i $LLAVE -p $PUERTO -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null" \
      "${1/#:/$usuario@127.0.0.1:}" "${2/#:/$usuario@127.0.0.1:}"
    ;;
  captura)
    destino="$(realpath -m "${2:?falta el archivo}")"
    monitor "screendump $destino.ppm"
    sleep 1
    magick "$destino.ppm" "$destino" 2>/dev/null || convert "$destino.ppm" "$destino"
    rm -f "$destino.ppm"
    echo "$destino"
    ;;
  apagar)
    corriendo || exit 0
    monitor system_powerdown
    # Espera a que QEMU termine, para que «apagar && disco» no choque con la VM aún viva.
    for _ in $(seq 60); do
      corriendo || exit 0
      sleep 1
    done
    echo "La VM no se apagó; usa vm.sh matar" >&2
    exit 1
    ;;
  matar)
    corriendo && kill "$(cat "$VM/qemu.pid")"
    ;;
  borrar)
    corriendo && { echo "Apaga la VM primero." >&2; exit 1; }
    rm -f "$DISCO" "$VM/OVMF_VARS.fd"
    echo "Disco borrado."
    ;;
  *)
    sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
    ;;
esac
