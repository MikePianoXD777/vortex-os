#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Módulo de Calamares de Vortexine's OS: en PC con BIOS, que el módulo bootloader sí instale GRUB.

El módulo bootloader de Calamares 3.4 (main.py) toma la carpeta EFI de
globalstorage["efiSystemPartition"] sin mirar el firmware, y en la rama de BIOS de
run_grub_install() hace esto:

    if efi_directory is not None and not install_hybrid_grub:
        warning("Cannot install BIOS bootloader on UEFI installation ...")
        return

El módulo partition guarda esa clave siempre (partition_vortex.conf la fija en «/boot» porque en
UEFI la partición EFI va ahí), también en BIOS. Entonces, en BIOS, el módulo avisa en el log,
salta grub-install, corre grub-mkconfig igual, y la instalación termina «bien» con el disco
formateado y sin gestor de arranque.

Este trabajo corre justo antes de bootloader y, solo en BIOS, borra esa clave: en BIOS no hay
partición EFI. En UEFI no hace nada (ahí la clave es la que usa systemd-boot).
"""

import libcalamares

CLAVE = "efiSystemPartition"


def pretty_name():
    return "Preparando el arranque en BIOS"


def run():
    gs = libcalamares.globalstorage
    if gs.value("firmwareType") != "bios":
        return None
    if not gs.contains(CLAVE):
        return None
    try:
        gs.remove(CLAVE)
    except Exception:
        try:
            # Sin remove(): con None el módulo bootloader ve lo mismo que con la clave ausente.
            gs.insert(CLAVE, None)
        except Exception as e:
            libcalamares.utils.warning("vortex-bios: no pude quitar {}: {}".format(CLAVE, e))
            return None
    libcalamares.utils.debug("vortex-bios: firmware BIOS, sin carpeta EFI para el módulo bootloader")
    return None
