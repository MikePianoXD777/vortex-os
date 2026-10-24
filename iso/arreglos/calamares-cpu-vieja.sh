#!/usr/bin/env bash
# Arreglo para la ISO 1.3 (y anteriores) en procesadores sin AVX2 (Celeron/Pentium, Intel anteriores a
# Haswell, AMD viejos): el instalador no abre y dice «calamares: CPU ISA level is lower than required».
#
# Calamares se compiló con banderas genéricas, pero en una VM con el glibc «v3» de CachyOS, y su
# arranque (crt1.o) le dejó al ejecutable la marca «necesita x86-64-v3» (GNU_PROPERTY_X86_ISA_1_NEEDED):
# el glibc del live se niega a abrirlo. Esto copia el Calamares de la imagen de la ISO con esa marca en
# «x86-64 básico» (un solo byte), lo pone en la sesión live y abre el instalador. No toca el disco.
#
#   curl -fsSL https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/iso/arreglos/calamares-cpu-vieja.sh | bash
set -euo pipefail
origen=/run/archiso/airootfs/usr/bin/calamares        # el de la imagen, sin tocar
[[ -f "$origen" ]] || origen=/usr/bin/calamares
python3 - "$origen" /tmp/calamares-generico <<'PY'
import struct, sys
b = bytearray(open(sys.argv[1], "rb").read())
i = b.find(struct.pack("<II", 0xC0008002, 4))          # GNU_PROPERTY_X86_ISA_1_NEEDED, 4 bytes
if i < 0:
    sys.exit("No encontré la marca de CPU en Calamares: no cambio nada.")
b[i + 8:i + 12] = struct.pack("<I", 1)                  # 1 = x86-64 básico
open(sys.argv[2], "wb").write(b)
PY
sudo install -m755 /tmp/calamares-generico /usr/bin/calamares
echo "Listo: Calamares ya abre en este procesador. Abriendo el instalador…"
exec vortex-instalador
