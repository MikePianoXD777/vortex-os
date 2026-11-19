//! Los eventos de udev directo del kernel (netlink), como hace libudev, sin lanzar `udevadm monitor`
//! (4.0: un proceso de ~5 MB menos en cada sesión).
//!
//! udev reenvía cada evento ya procesado (con `ID_FS_*`, `ID_BUS`…) al grupo 2 de `NETLINK_KOBJECT_UEVENT`
//! con una cabecera «libudev». Igual que libudev, solo se acepta lo que manda root desde un proceso (no el
//! kernel ni otro usuario): cualquiera puede escribir a ese grupo y no queremos montar lo que diga un extraño.
//!
//! Es el único módulo con `unsafe` (socket, bind, setsockopt y recvmsg no tienen versión segura en std).
#![allow(unsafe_code)]

use crate::Evento;
use std::io;
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// El grupo de netlink al que udev reenvía los eventos ya procesados (el 1 es el del kernel, sin procesar).
const GRUPO_UDEV: u32 = 2;
const MAGIA: u32 = 0xfeed_cafe;

pub struct Udev {
    fd: OwnedFd,
}

impl Udev {
    pub fn abrir() -> io::Result<Self> {
        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::NETLINK_KOBJECT_UEVENT,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let si: libc::c_int = 1;
        let r = unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PASSCRED,
                (&raw const si).cast(),
                size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut dir: libc::sockaddr_nl = unsafe { zeroed() };
        dir.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        dir.nl_groups = GRUPO_UDEV;
        let r = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&raw const dir).cast(),
                size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { fd })
    }

    /// Espera el siguiente evento de udev. `Ok(None)`: llegó algo que no es de udev (se ignora).
    pub fn recibir(&self, buf: &mut [u8]) -> io::Result<Option<Evento>> {
        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr().cast(),
            iov_len: buf.len(),
        };
        let mut origen: libc::sockaddr_nl = unsafe { zeroed() };
        // Lugar para un SCM_CREDENTIALS (alineado como pide cmsg).
        let mut control = [0u64; 8];
        let mut msg: libc::msghdr = unsafe { zeroed() };
        msg.msg_name = (&raw mut origen).cast();
        msg.msg_namelen = size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        msg.msg_iov = &raw mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = size_of::<[u64; 8]>();
        let n = loop {
            let n = unsafe { libc::recvmsg(self.fd.as_raw_fd(), &raw mut msg, 0) };
            if n >= 0 {
                break n as usize;
            }
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            // ENOBUFS: se perdieron eventos porque llegaron muchos juntos; se sigue escuchando.
            if e.raw_os_error() == Some(libc::ENOBUFS) {
                return Ok(None);
            }
            return Err(e);
        };
        // Recortado: no cabía (un evento de udev mide ~1-2 KB; el búfer es de 8 KB).
        if msg.msg_flags & libc::MSG_TRUNC != 0 {
            return Ok(None);
        }
        // Del grupo de udev y desde un proceso (nl_pid 0 es el kernel, que no manda la cabecera de udev).
        if origen.nl_groups != GRUPO_UDEV || origen.nl_pid == 0 {
            return Ok(None);
        }
        // Solo lo que manda root (systemd-udevd).
        let mut de_root = false;
        let mut c = unsafe { libc::CMSG_FIRSTHDR(&raw const msg) };
        while !c.is_null() {
            let h = unsafe { &*c };
            if h.cmsg_level == libc::SOL_SOCKET && h.cmsg_type == libc::SCM_CREDENTIALS {
                let cred: libc::ucred = unsafe { std::ptr::read_unaligned(libc::CMSG_DATA(c).cast()) };
                de_root = cred.uid == 0;
            }
            c = unsafe { libc::CMSG_NXTHDR(&raw const msg, c) };
        }
        if !de_root {
            return Ok(None);
        }
        Ok(propiedades(&buf[..n]))
    }
}

fn u32_en(datos: &[u8], i: usize) -> Option<u32> {
    datos
        .get(i..i + 4)
        .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
}

/// Las propiedades `CLAVE=valor` de un mensaje de udev (cabecera «libudev»), o `None` si no lo es.
///
/// La cabecera (struct udev_monitor_netlink_header de libudev): «libudev\0», la magia 0xfeedcafe en orden de
/// red, y en el orden de la máquina el tamaño de la cabecera, dónde empiezan las propiedades y cuánto miden.
/// Las propiedades van separadas por un byte 0.
pub fn propiedades(datos: &[u8]) -> Option<Evento> {
    if !datos.starts_with(b"libudev\0") {
        return None;
    }
    let magia = datos
        .get(8..12)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))?;
    if magia != MAGIA {
        return None;
    }
    let desde = u32_en(datos, 16)? as usize;
    let largo = u32_en(datos, 20)? as usize;
    let cuerpo = datos.get(desde..desde.checked_add(largo)?)?;
    let mut e = Evento::new();
    for trozo in cuerpo.split(|&b| b == 0) {
        let Ok(texto) = std::str::from_utf8(trozo) else {
            continue;
        };
        if let Some((k, v)) = texto.split_once('=')
            && !k.is_empty()
        {
            e.insert(k.to_string(), v.to_string());
        }
    }
    if e.is_empty() { None } else { Some(e) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un mensaje como los de udev: cabecera de 40 bytes y las propiedades detrás.
    pub fn mensaje(props: &[&str]) -> Vec<u8> {
        let mut cuerpo = Vec::new();
        for p in props {
            cuerpo.extend_from_slice(p.as_bytes());
            cuerpo.push(0);
        }
        let mut m = b"libudev\0".to_vec();
        m.extend_from_slice(&MAGIA.to_be_bytes());
        m.extend_from_slice(&40u32.to_ne_bytes()); // tamaño de la cabecera
        m.extend_from_slice(&40u32.to_ne_bytes()); // dónde empiezan las propiedades
        m.extend_from_slice(&(cuerpo.len() as u32).to_ne_bytes());
        m.resize(40, 0); // los hashes del filtro
        m.extend_from_slice(&cuerpo);
        m
    }

    #[test]
    fn lee_las_propiedades() {
        let m = mensaje(&[
            "ACTION=add",
            "SUBSYSTEM=block",
            "DEVNAME=/dev/sdb1",
            "ID_FS_LABEL=MI USB",
            "ID_BUS=usb",
        ]);
        let e = propiedades(&m).unwrap();
        assert_eq!(e["ACTION"], "add");
        assert_eq!(e["ID_FS_LABEL"], "MI USB");
        assert_eq!(e.len(), 5);
    }

    #[test]
    fn rechaza_lo_que_no_es_de_udev() {
        // Un evento del kernel: «add@/devices/…» sin cabecera.
        assert!(propiedades(b"add@/devices/pci0000:00/x\0ACTION=add\0").is_none());
        // Magia equivocada.
        let mut m = mensaje(&["ACTION=add"]);
        m[8] ^= 0xff;
        assert!(propiedades(&m).is_none());
        // Las propiedades dicen medir más que el mensaje.
        let mut m = mensaje(&["ACTION=add"]);
        m[20..24].copy_from_slice(&5000u32.to_ne_bytes());
        assert!(propiedades(&m).is_none());
        // Un desfase absurdo no revienta.
        let mut m = mensaje(&["ACTION=add"]);
        m[16..20].copy_from_slice(&u32::MAX.to_ne_bytes());
        assert!(propiedades(&m).is_none());
        assert!(propiedades(b"libudev\0").is_none());
    }

    /// De punta a punta con el udev de verdad (en la VM: necesita `sudo -n udevadm trigger`).
    ///   cargo test -p vortex-usb -- --ignored
    #[test]
    #[ignore]
    fn recibe_de_udev_de_verdad() {
        let u = Udev::abrir().unwrap();
        let ok = std::process::Command::new("sudo")
            .args([
                "-n",
                "udevadm",
                "trigger",
                "--action=add",
                "--subsystem-match=block",
            ])
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "hace falta sudo sin contraseña para udevadm trigger");
        let mut buf = vec![0u8; 8192];
        let e = loop {
            if let Some(e) = u.recibir(&mut buf).unwrap() {
                break e;
            }
        };
        assert_eq!(e["SUBSYSTEM"], "block");
        assert_eq!(e["ACTION"], "add");
        assert!(e.contains_key("DEVNAME"), "{e:?}");
    }

    #[test]
    fn abre_el_socket() {
        // Sin permisos especiales: cualquier usuario puede escuchar el grupo de udev.
        assert!(Udev::abrir().is_ok());
    }
}
