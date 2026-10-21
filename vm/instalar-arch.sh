#!/usr/bin/env bash
# Instala un Arch mínimo en /dev/vda de la VM de pruebas (se corre como root en la ISO de Arch).
# Queda como un «archinstall» perfil minimal: UEFI + systemd-boot, ext4, NetworkManager y SSH.
# Usuario de prueba: vortex / vortex, con sudo sin contraseña (solo para la VM).
set -euo pipefail

DISCO=/dev/vda
USUARIO=vortex

timedatectl set-ntp true
sgdisk --zap-all "$DISCO"
sgdisk -n1:0:+1G -t1:ef00 -c1:EFI -n2:0:0 -t2:8304 -c2:raiz "$DISCO"
partprobe "$DISCO"
mkfs.fat -F32 "${DISCO}1"
mkfs.ext4 -q -F "${DISCO}2"
mount "${DISCO}2" /mnt
mount --mkdir "${DISCO}1" /mnt/boot

pacman -Sy --noconfirm archlinux-keyring
pacstrap -K /mnt base linux linux-firmware sudo networkmanager openssh nano
genfstab -U /mnt >>/mnt/etc/fstab

arch-chroot /mnt /bin/bash -e <<EOF
ln -sf /usr/share/zoneinfo/America/Managua /etc/localtime
hwclock --systohc
sed -i 's/^#es_NI.UTF-8/es_NI.UTF-8/; s/^#en_US.UTF-8/en_US.UTF-8/' /etc/locale.gen
locale-gen
echo LANG=es_NI.UTF-8 >/etc/locale.conf
echo KEYMAP=la-latin1 >/etc/vconsole.conf
echo vortex-vm >/etc/hostname

useradd -m -G wheel $USUARIO
echo "$USUARIO:vortex" | chpasswd
echo 'root:vortex' | chpasswd
echo '%wheel ALL=(ALL:ALL) NOPASSWD: ALL' >/etc/sudoers.d/10-wheel
chmod 440 /etc/sudoers.d/10-wheel

install -d -m700 -o $USUARIO -g $USUARIO /home/$USUARIO/.ssh
EOF
install -m600 /root/.ssh/authorized_keys /mnt/home/$USUARIO/.ssh/authorized_keys
arch-chroot /mnt chown -R "$USUARIO:$USUARIO" "/home/$USUARIO/.ssh"

arch-chroot /mnt bootctl install
uuid="$(blkid -s PARTUUID -o value "${DISCO}2")"
cat >/mnt/boot/loader/loader.conf <<EOF
default arch.conf
timeout 3
EOF
cat >/mnt/boot/loader/entries/arch.conf <<EOF
title   Arch Linux
linux   /vmlinuz-linux
initrd  /initramfs-linux.img
options root=PARTUUID=$uuid rw
EOF

arch-chroot /mnt systemctl enable NetworkManager sshd
echo "Arch instalado en $DISCO"
