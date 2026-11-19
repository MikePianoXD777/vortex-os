//! La hora de aquí (zona del sistema) sin crates de fechas: para «Tu resumen» (de lunes a lunes), los días que
//! escuchaste y el despertador. Con `localtime_r`/`mktime` de libc, como el `time` y el `datetime` de Python
//! (cambios de horario incluidos).

use std::time::{SystemTime, UNIX_EPOCH};

/// Un momento en la hora local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
    pub hora: u32,
    pub minuto: u32,
    pub segundo: u32,
    /// 0 = lunes … 6 = domingo (el `weekday()` de Python).
    pub dia_semana: u32,
}

pub fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(unix)]
#[allow(unsafe_code)] // localtime_r y mktime: la única forma sin crates de fechas; los structs son locales
mod sistema {
    use super::Local;

    pub fn local(t: f64) -> Local {
        let s = t.floor() as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if unsafe { libc::localtime_r(&s, &mut tm) }.is_null() {
            return super::utc(t);
        }
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

    /// La medianoche local de ese día más `dias` (se normaliza: el día 0 o el 40 valen).
    pub fn medianoche(anio: i32, mes: u32, dia: i64) -> f64 {
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        tm.tm_year = anio - 1900;
        tm.tm_mon = mes as i32 - 1;
        tm.tm_mday = dia as i32;
        tm.tm_isdst = -1; // que el sistema decida si es horario de verano
        unsafe { libc::mktime(&mut tm) as f64 }
    }
}

#[cfg(not(unix))]
mod sistema {
    use super::Local;
    pub fn local(t: f64) -> Local {
        super::utc(t)
    }
    pub fn medianoche(anio: i32, mes: u32, dia: i64) -> f64 {
        (super::dias_desde_civil(anio, mes, 1) + dia - 1) as f64 * 86400.0
    }
}

#[cfg_attr(unix, allow(dead_code))]
fn dias_desde_civil(anio: i32, mes: u32, dia: u32) -> i64 {
    let y = i64::from(anio) - i64::from(mes <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(mes);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(dia) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn utc(t: f64) -> Local {
    let s = t.floor() as i64;
    let dias = s.div_euclid(86400);
    let resto = s.rem_euclid(86400);
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let anio = (yoe + era * 400 + i64::from(mes <= 2)) as i32;
    Local {
        anio,
        mes,
        dia,
        hora: (resto / 3600) as u32,
        minuto: (resto % 3600 / 60) as u32,
        segundo: (resto % 60) as u32,
        dia_semana: (dias + 3).rem_euclid(7) as u32, // el 1/1/1970 fue jueves
    }
}

pub fn local(t: f64) -> Local {
    sistema::local(t)
}

/// Las `hora:minuto` de ese día, en la hora de aquí (para las pruebas y el despertador).
pub fn marca_local(anio: i32, mes: u32, dia: u32, hora: u32, minuto: u32) -> f64 {
    sistema::medianoche(anio, mes, i64::from(dia)) + f64::from(hora * 3600 + minuto * 60)
}

/// «2026-11-19» (el `%Y-%m-%d` de Python), en la hora de aquí.
pub fn dia_texto(t: f64) -> String {
    let l = local(t);
    format!("{:04}-{:02}-{:02}", l.anio, l.mes, l.dia)
}

/// De lunes a lunes (a la medianoche de aquí): 0 = esta semana, 1 = la pasada. Como `biblioteca.semana`.
pub fn semana(atras: u32, ahora_: Option<f64>) -> (f64, f64) {
    let l = local(ahora_.filter(|x| *x != 0.0).unwrap_or_else(ahora));
    let lunes = i64::from(l.dia) - i64::from(l.dia_semana) - 7 * i64::from(atras);
    (
        sistema::medianoche(l.anio, l.mes, lunes),
        sistema::medianoche(l.anio, l.mes, lunes + 7),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn utc_conocido() {
        let l = utc(1_795_089_658.0); // 2026-11-19 (jueves)
        assert_eq!((l.anio, l.mes, l.dia, l.dia_semana), (2026, 11, 19, 3));
    }

    #[test]
    fn la_semana_empieza_en_lunes() {
        let (a, b) = semana(0, Some(1_795_089_658.0));
        let la = local(a);
        assert_eq!((la.dia_semana, la.hora, la.minuto), (0, 0, 0));
        assert!(a <= 1_795_089_658.0 && 1_795_089_658.0 < b);
        let (c, d) = semana(1, Some(1_795_089_658.0));
        assert_eq!(d, a);
        assert!((b - a - 7.0 * 86400.0).abs() <= 3600.0 && (d - c - 7.0 * 86400.0).abs() <= 3600.0);
    }
}
