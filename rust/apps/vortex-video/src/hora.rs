//! La hora local (para el nombre de las capturas: «Foto 2026-11-05 14.03.21.jpg»).

/// Fecha y hora de pared, en la zona de la persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FechaHora {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
    pub hora: u32,
    pub minuto: u32,
    pub segundo: u32,
}

impl FechaHora {
    pub fn new(anio: i32, mes: u32, dia: u32, hora: u32, minuto: u32, segundo: u32) -> Self {
        FechaHora {
            anio,
            mes,
            dia,
            hora,
            minuto,
            segundo,
        }
    }

    /// «2026-11-05 14.03.21» (el `%Y-%m-%d %H.%M.%S` de la de Python).
    pub fn sello(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}.{:02}.{:02}",
            self.anio, self.mes, self.dia, self.hora, self.minuto, self.segundo
        )
    }

    /// Ahora, en hora local (`localtime_r` de la libc: respeta TZ y /etc/localtime, como Python).
    #[cfg(unix)]
    #[allow(unsafe_code)] // localtime_r es la única forma sin crates de fechas; el struct es local
    pub fn ahora() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as libc::time_t)
            .unwrap_or(0);
        // SAFETY: `tm` es nuestro y vive en la pila; localtime_r es reentrante y solo escribe en él.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
        if !ok {
            return FechaHora::new(1970, 1, 1, 0, 0, 0);
        }
        FechaHora::new(
            tm.tm_year + 1900,
            (tm.tm_mon + 1) as u32,
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    }

    #[cfg(not(unix))]
    pub fn ahora() -> Self {
        // En Windows la app no existe (usa Qt Multimedia de Linux); sin zona horaria, UTC.
        let s = vortex_core::registro::fecha_utc(std::time::SystemTime::now());
        let n = |a: usize, b: usize| s[a..b].parse().unwrap_or(0);
        FechaHora::new(n(0, 4) as i32, n(5, 7), n(8, 10), n(11, 13), n(14, 16), n(17, 19))
    }
}
