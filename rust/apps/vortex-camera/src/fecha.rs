//! Fechas y horas de pared (sin zona: la hora de la cámara), como los `datetime` de la de Python: leer
//! «AAAA-MM-DD» «HH:MM», sumar minutos y escribirlas para las URL de lo grabado y los nombres de archivo.

/// Una fecha y hora sin zona horaria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FechaHora {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
    pub hora: u32,
    pub minuto: u32,
    pub segundo: u32,
}

/// Días desde el 1970-01-01 (algoritmo de Howard Hinnant, calendario gregoriano proléptico).
fn dias_desde_civil(anio: i32, mes: u32, dia: u32) -> i64 {
    let y = i64::from(anio) - i64::from(mes <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(mes);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(dia) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_desde_dias(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let anio = (yoe + era * 400 + i64::from(mes <= 2)) as i32;
    (anio, mes, dia)
}

fn dias_del_mes(anio: i32, mes: u32) -> u32 {
    match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (anio % 4 == 0 && anio % 100 != 0) || anio % 400 == 0 => 29,
        _ => 28,
    }
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

    /// «2026-10-30» «14:05» → la fecha (`strptime("%Y-%m-%d %H:%M")`); `None` si no es una fecha real.
    pub fn leer(fecha: &str, hora: &str) -> Option<Self> {
        let mut f = fecha.trim().split('-');
        let (a, m, d) = (f.next()?, f.next()?, f.next()?);
        if f.next().is_some() {
            return None;
        }
        let mut h = hora.trim().split(':');
        let (hh, mm) = (h.next()?, h.next()?);
        if h.next().is_some() {
            return None;
        }
        let num = |t: &str, max: usize| -> Option<u32> {
            (!t.is_empty() && t.len() <= max && t.bytes().all(|b| b.is_ascii_digit()))
                .then(|| t.parse().ok())
                .flatten()
        };
        let anio = num(a, 4)? as i32;
        let (mes, dia, hora, minuto) = (num(m, 2)?, num(d, 2)?, num(hh, 2)?, num(mm, 2)?);
        if !(1..=12).contains(&mes) || dia == 0 || dia > dias_del_mes(anio, mes) || hora > 23 || minuto > 59 {
            return None;
        }
        Some(FechaHora::new(anio, mes, dia, hora, minuto, 0))
    }

    fn segundos(&self) -> i64 {
        dias_desde_civil(self.anio, self.mes, self.dia) * 86_400
            + i64::from(self.hora) * 3600
            + i64::from(self.minuto) * 60
            + i64::from(self.segundo)
    }

    fn desde_segundos(s: i64) -> Self {
        let (anio, mes, dia) = civil_desde_dias(s.div_euclid(86_400));
        let r = s.rem_euclid(86_400);
        FechaHora::new(
            anio,
            mes,
            dia,
            (r / 3600) as u32,
            (r % 3600 / 60) as u32,
            (r % 60) as u32,
        )
    }

    /// La misma hora corrida `minutos` (negativos para atrás), cruzando días, meses y años.
    pub fn mas_minutos(&self, minutos: i64) -> Self {
        Self::desde_segundos(self.segundos() + minutos * 60)
    }

    /// «2026_10_30_14_05_00», como piden las Dahua en `starttime`/`endtime`.
    pub fn para_dahua(&self) -> String {
        format!(
            "{:04}_{:02}_{:02}_{:02}_{:02}_{:02}",
            self.anio, self.mes, self.dia, self.hora, self.minuto, self.segundo
        )
    }

    /// «2026-10-30».
    pub fn dia_texto(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.anio, self.mes, self.dia)
    }

    /// «14:05».
    pub fn hora_texto(&self) -> String {
        format!("{:02}:{:02}", self.hora, self.minuto)
    }

    /// «2026-10-30 14.05.33», para los nombres de las fotos y los videos.
    pub fn sello(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}.{:02}.{:02}",
            self.anio, self.mes, self.dia, self.hora, self.minuto, self.segundo
        )
    }

    /// Ahora, en hora local (`localtime_r` de la libc: respeta TZ y /etc/localtime, como Python).
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
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_y_rechaza() {
        assert_eq!(
            FechaHora::leer(" 2026-10-30 ", "14:05"),
            Some(FechaHora::new(2026, 10, 30, 14, 5, 0))
        );
        assert_eq!(
            FechaHora::leer("2026-2-3", "4:5"),
            Some(FechaHora::new(2026, 2, 3, 4, 5, 0))
        );
        for (f, h) in [
            ("2026-02-30", "10:00"),
            ("2026-13-01", "10:00"),
            ("2026-10-30", "24:00"),
            ("2026-10-30", "10:60"),
            ("hoy", "10:00"),
            ("2026-10-30", "10:00:00"),
            ("", ""),
        ] {
            assert_eq!(FechaHora::leer(f, h), None, "{f} {h}");
        }
        assert!(FechaHora::leer("2028-02-29", "00:00").is_some());
        assert!(FechaHora::leer("2026-02-29", "00:00").is_none());
    }

    #[test]
    fn suma_minutos_cruzando_dias_y_anios() {
        let f = FechaHora::leer("2026-10-30", "23:55").unwrap();
        let g = f.mas_minutos(10);
        assert_eq!(
            (g.dia_texto(), g.hora_texto()),
            ("2026-10-31".into(), "00:05".into())
        );
        let h = FechaHora::leer("2026-12-31", "23:59").unwrap().mas_minutos(1);
        assert_eq!(h.dia_texto(), "2027-01-01");
        let i = FechaHora::leer("2026-03-01", "00:00").unwrap().mas_minutos(-1);
        assert_eq!(
            (i.dia_texto(), i.hora_texto()),
            ("2026-02-28".into(), "23:59".into())
        );
        assert_eq!(f.mas_minutos(24 * 60).para_dahua(), "2026_10_31_23_55_00");
    }
}
