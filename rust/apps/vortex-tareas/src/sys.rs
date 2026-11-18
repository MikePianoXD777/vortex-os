//! Lo poco que pide al sistema y la biblioteca estándar no trae: la hora local y `setsid`.
//! Es el único módulo con `unsafe`; cada bloque es una sola llamada con datos locales.
#![allow(unsafe_code)]

use std::os::unix::process::CommandExt;
use std::process::Command;

/// `(año, mes, día, hora)` de ahora en la zona horaria local.
pub fn hora_local() -> (i32, u32, u32, u32) {
    // SAFETY: `tm` es un struct local que localtime_r llena por completo si devuelve no nulo.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return (1970, 1, 1, 0);
        }
        (
            tm.tm_year + 1900,
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
        )
    }
}

/// Que el programa lanzado viva en su propia sesión (como `start_new_session=True` de Python).
pub fn en_sesion_nueva(orden: &mut Command) {
    // SAFETY: entre el fork y el exec solo se llama a setsid, que es seguro con señales.
    unsafe {
        orden.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}
