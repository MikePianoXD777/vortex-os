//! Las pocas llamadas a libc que hacen falta (señales, prioridad, uid, disco, direcciones de red).
//! Es el único módulo del motor con `unsafe`; cada bloque es una sola llamada con datos locales.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::ffi::{CStr, CString};

pub const ESRCH: i32 = libc::ESRCH;
pub const EPERM: i32 = libc::EPERM;

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// `kill(2)`. Error = el `errno` (ESRCH: ya no existe; EPERM: no es tuyo).
pub fn kill(pid: i32, senal: i32) -> Result<(), i32> {
    // SAFETY: kill no toca memoria nuestra.
    if unsafe { libc::kill(pid, senal) } == 0 {
        Ok(())
    } else {
        Err(errno())
    }
}

pub fn poner_prioridad(pid: i32, valor: i32) -> Result<(), i32> {
    // SAFETY: setpriority no toca memoria nuestra.
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, valor) } == 0 {
        Ok(())
    } else {
        Err(errno())
    }
}

/// El «nice» del proceso (puede ser negativo). `None` si no existe.
pub fn prioridad(pid: i32) -> Option<i32> {
    // SAFETY: errno es del hilo; getpriority devuelve -1 también como valor válido, por eso se limpia antes.
    unsafe {
        *libc::__errno_location() = 0;
        let n = libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t);
        if n == -1 && *libc::__errno_location() != 0 {
            None
        } else {
            Some(n)
        }
    }
}

pub fn uid() -> u32 {
    // SAFETY: getuid no falla.
    unsafe { libc::getuid() }
}

pub fn procesadores() -> usize {
    // SAFETY: sysconf no toca memoria nuestra.
    let n = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    if n > 0 { n as usize } else { 1 }
}

pub fn tam_pagina() -> u64 {
    // SAFETY: sysconf no toca memoria nuestra.
    let n = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if n > 0 { n as u64 } else { 4096 }
}

pub fn ticks_por_segundo() -> f64 {
    // SAFETY: sysconf no toca memoria nuestra.
    let n = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if n > 0 { n as f64 } else { 100.0 }
}

/// `(usado, total)` en bytes de lo que está montado en `ruta` (como `psutil.disk_usage`).
pub fn uso_de_disco(ruta: &str) -> Option<(u64, u64)> {
    let c = CString::new(ruta).ok()?;
    // SAFETY: `datos` es un struct local que statvfs llena por completo si devuelve 0.
    unsafe {
        let mut datos: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut datos) != 0 {
            return None;
        }
        let bloque = datos.f_frsize as u64;
        Some((
            (datos.f_blocks as u64).saturating_sub(datos.f_bfree as u64) * bloque,
            datos.f_blocks as u64 * bloque,
        ))
    }
}

/// La primera dirección IPv4 de cada interfaz (`getifaddrs`).
pub fn direcciones_ipv4() -> HashMap<String, String> {
    let mut salida = HashMap::new();
    // SAFETY: la lista de getifaddrs se recorre solo de lectura y se libera con freeifaddrs.
    unsafe {
        let mut lista: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut lista) != 0 {
            return salida;
        }
        let mut actual = lista;
        while !actual.is_null() {
            let i = &*actual;
            if !i.ifa_addr.is_null() && i.ifa_addr.read().sa_family as i32 == libc::AF_INET {
                let nombre = CStr::from_ptr(i.ifa_name).to_string_lossy().into_owned();
                let sin = &*(i.ifa_addr as *const libc::sockaddr_in);
                let b = sin.sin_addr.s_addr.to_ne_bytes();
                salida
                    .entry(nombre)
                    .or_insert_with(|| format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]));
            }
            actual = i.ifa_next;
        }
        libc::freeifaddrs(lista);
    }
    salida
}
