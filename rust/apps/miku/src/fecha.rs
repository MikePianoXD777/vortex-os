//! Fechas y horas «de pared» (sin zona), como los `datetime` de la de Python: leer ISO, restar días y
//! segundos, y escribirlas. `ahora()` mira el reloj local (o `VORTEX_MIKU_AHORA=AAAA-MM-DDTHH:MM`, que
//! fija la hora para las pruebas).

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FechaHora {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
    pub hora: u32,
    pub minuto: u32,
    pub segundo: u32,
}

/// Días desde 1970-01-01 (algoritmo de Hinnant).
pub fn dias_civil(anio: i32, mes: u32, dia: u32) -> i64 {
    let y = i64::from(anio) - i64::from(mes <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(mes);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(dia) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl FechaHora {
    pub fn nueva(anio: i32, mes: u32, dia: u32, hora: u32, minuto: u32) -> Self {
        Self {
            anio,
            mes,
            dia,
            hora,
            minuto,
            segundo: 0,
        }
    }

    /// «AAAA-MM-DD», «AAAA-MM-DDTHH:MM» o «AAAA-MM-DDTHH:MM:SS» (lo que entiende `fromisoformat`).
    pub fn leer(texto: &str) -> Option<Self> {
        let t = texto.trim();
        let (f, h) = match t.split_once(['T', ' ']) {
            Some((f, h)) => (f, Some(h)),
            None => (t, None),
        };
        let mut p = f.split('-');
        let anio: i32 = p.next()?.parse().ok()?;
        let mes: u32 = p.next()?.parse().ok()?;
        let dia: u32 = p.next()?.parse().ok()?;
        if p.next().is_some() || !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
            return None;
        }
        let (mut hora, mut minuto, mut segundo) = (0, 0, 0);
        if let Some(h) = h {
            let h = h.split(['.', '+', 'Z']).next()?;
            let mut q = h.split(':');
            hora = q.next()?.parse().ok()?;
            minuto = q.next().unwrap_or("0").parse().ok()?;
            segundo = q.next().unwrap_or("0").parse().ok()?;
            if hora > 23 || minuto > 59 || segundo > 59 {
                return None;
            }
        }
        Some(Self {
            anio,
            mes,
            dia,
            hora,
            minuto,
            segundo,
        })
    }

    pub fn dias(&self) -> i64 {
        dias_civil(self.anio, self.mes, self.dia)
    }

    /// Segundos desde la época, sin zona (para restar dos fechas).
    pub fn segundos(&self) -> i64 {
        self.dias() * 86_400
            + i64::from(self.hora) * 3600
            + i64::from(self.minuto) * 60
            + i64::from(self.segundo)
    }

    /// 0 = lunes … 6 = domingo.
    pub fn dia_semana(&self) -> usize {
        (self.dias() + 3).rem_euclid(7) as usize
    }

    /// «AAAA-MM-DD».
    pub fn fecha_iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.anio, self.mes, self.dia)
    }

    /// `isoformat(timespec="minutes")`.
    pub fn iso_minutos(&self) -> String {
        format!("{}T{:02}:{:02}", self.fecha_iso(), self.hora, self.minuto)
    }

    /// `isoformat(timespec="seconds")`.
    pub fn iso_segundos(&self) -> String {
        format!("{}:{:02}", self.iso_minutos(), self.segundo)
    }

    /// Esta fecha más (o menos) esos segundos.
    pub fn mas_segundos(&self, s: i64) -> Self {
        desde_segundos_pared(self.segundos() + s)
    }

    /// Esta misma fecha a otra hora del día.
    pub fn con_hora(&self, hora: u32, minuto: u32) -> Self {
        Self {
            hora,
            minuto,
            segundo: 0,
            ..*self
        }
    }

    /// «dd/mm/AAAA».
    pub fn dmy(&self) -> String {
        format!("{:02}/{:02}/{:04}", self.dia, self.mes, self.anio)
    }

    /// «HH:MM».
    pub fn hm(&self) -> String {
        format!("{:02}:{:02}", self.hora, self.minuto)
    }
}

/// Inversa de `segundos()`: de segundos desde la época (sin zona) a fecha y hora.
pub fn desde_segundos_pared(s: i64) -> FechaHora {
    let dias = s.div_euclid(86_400);
    let resto = s.rem_euclid(86_400);
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let anio = (yoe + era * 400 + i64::from(mes <= 2)) as i32;
    FechaHora {
        anio,
        mes,
        dia,
        hora: (resto / 3600) as u32,
        minuto: (resto % 3600 / 60) as u32,
        segundo: (resto % 60) as u32,
    }
}

#[allow(unsafe_code)] // localtime_r de libc: no hay otra forma de saber la zona sin una crate
fn reloj_local() -> FechaHora {
    // SAFETY: `time` y `localtime_r` solo escriben en la `tm` que les damos.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        FechaHora {
            anio: tm.tm_year + 1900,
            mes: (tm.tm_mon + 1) as u32,
            dia: tm.tm_mday as u32,
            hora: tm.tm_hour as u32,
            minuto: tm.tm_min as u32,
            segundo: tm.tm_sec as u32,
        }
    }
}

/// Una marca de tiempo Unix en la hora local.
#[allow(unsafe_code)]
pub fn desde_epoch_local(segundos: i64) -> FechaHora {
    // SAFETY: `localtime_r` solo escribe en la `tm` que le damos.
    unsafe {
        let t: libc::time_t = segundos;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        FechaHora {
            anio: tm.tm_year + 1900,
            mes: (tm.tm_mon + 1) as u32,
            dia: tm.tm_mday as u32,
            hora: tm.tm_hour as u32,
            minuto: tm.tm_min as u32,
            segundo: tm.tm_sec as u32,
        }
    }
}

/// La hora de ahora (o la fijada con `VORTEX_MIKU_AHORA`).
pub fn ahora() -> FechaHora {
    std::env::var("VORTEX_MIKU_AHORA")
        .ok()
        .and_then(|f| FechaHora::leer(&f))
        .unwrap_or_else(reloj_local)
}

pub const DIAS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
pub const MESES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_y_escribe() {
        let f = FechaHora::leer("2026-11-19T22:05").unwrap();
        assert_eq!(f.iso_minutos(), "2026-11-19T22:05");
        assert_eq!(f.dmy(), "19/11/2026");
        assert_eq!(f.dia_semana(), 3); // jueves
        assert_eq!(FechaHora::leer("2026-11-19").unwrap().hm(), "00:00");
        assert!(FechaHora::leer("nada").is_none());
        let a = FechaHora::leer("2026-11-17T10:00").unwrap();
        assert_eq!(f.dias() - a.dias(), 2);
        assert_eq!(f.segundos() - a.segundos(), 2 * 86_400 + 12 * 3600 + 5 * 60);
    }

    #[test]
    fn suma_y_resta_segundos() {
        let f = FechaHora::leer("2026-11-19T23:30:15").unwrap();
        assert_eq!(f.mas_segundos(45 * 60).iso_segundos(), "2026-11-20T00:15:15");
        assert_eq!(desde_segundos_pared(f.segundos()), f);
        assert_eq!(
            FechaHora::leer("2028-02-28T12:00")
                .unwrap()
                .mas_segundos(86_400)
                .fecha_iso(),
            "2028-02-29"
        );
        assert_eq!(f.con_hora(9, 0).iso_segundos(), "2026-11-19T09:00:00");
    }
}
