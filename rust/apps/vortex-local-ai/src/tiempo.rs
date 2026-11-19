//! La hora local (para los ids de los chats y las fechas cortas), con `libc`.
#![allow(unsafe_code)] // localtime_r y mktime de libc

use std::time::{SystemTime, UNIX_EPOCH};

/// Segundos desde 1970, con decimales (el `time.time()` de Python).
pub fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Una fecha y hora locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    pub anio: i32,
    pub mes: u32, // 1..=12
    pub dia: u32, // 1..=31
    pub hora: u32,
    pub minuto: u32,
    pub segundo: u32,
    /// 0 = lunes … 6 = domingo (el `tm_wday` de Python).
    pub dia_semana: u32,
}

pub fn local(epoch: f64) -> Local {
    let t = epoch.floor() as libc::time_t;
    // SAFETY: `tm` se llena por completo con localtime_r (que es segura entre hilos).
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    };
    Local {
        anio: tm.tm_year + 1900,
        mes: (tm.tm_mon + 1) as u32,
        dia: tm.tm_mday as u32,
        hora: tm.tm_hour as u32,
        minuto: tm.tm_min as u32,
        segundo: tm.tm_sec as u32,
        dia_semana: ((tm.tm_wday + 6) % 7) as u32,
    }
}

/// Medianoche local de ese día (epoch).
pub fn medianoche(l: &Local) -> f64 {
    // SAFETY: mktime solo lee y normaliza el `tm` que le pasamos.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = l.anio - 1900;
        tm.tm_mon = l.mes as i32 - 1;
        tm.tm_mday = l.dia as i32;
        tm.tm_isdst = -1;
        libc::mktime(&mut tm) as f64
    }
}

/// `AAAAMMDD-HHMMSS`, como `time.strftime("%Y%m%d-%H%M%S")`.
pub fn sello(epoch: f64) -> String {
    let l = local(epoch);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        l.anio, l.mes, l.dia, l.hora, l.minuto, l.segundo
    )
}

/// `AAAA-MM-DD HH:MM`, como `datetime.fromtimestamp(x).strftime("%Y-%m-%d %H:%M")`.
pub fn fecha_hora(epoch: f64) -> String {
    let l = local(epoch);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        l.anio, l.mes, l.dia, l.hora, l.minuto
    )
}
