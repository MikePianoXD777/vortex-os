//! Fechas como `datetime.date` de Python (calendario gregoriano, años 1 a 9999), sin dependencias.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fecha {
    pub anio: i64,
    pub mes: u32,
    pub dia: u32,
}

fn bisiesto(a: i64) -> bool {
    a % 4 == 0 && (a % 100 != 0 || a % 400 == 0)
}

fn dias_del_mes(a: i64, m: u32) -> u32 {
    match m {
        2 if bisiesto(a) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Fecha {
    /// `None` si no existe (como el `ValueError` de `dt.date`).
    pub fn nueva(anio: i64, mes: i64, dia: i64) -> Option<Fecha> {
        if !(1..=9999).contains(&anio) || !(1..=12).contains(&mes) || dia < 1 {
            return None;
        }
        if dia > i64::from(dias_del_mes(anio, mes as u32)) {
            return None;
        }
        Some(Fecha {
            anio,
            mes: mes as u32,
            dia: dia as u32,
        })
    }

    /// Días desde 1970-01-01 (para restar fechas).
    pub fn dias(&self) -> i64 {
        let m = i64::from(self.mes);
        let y = if m <= 2 { self.anio - 1 } else { self.anio };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.dia) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// AAAA-MM-DD
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.anio, self.mes, self.dia)
    }
}

fn digitos(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `date.fromisoformat`: AAAA-MM-DD o AAAAMMDD.
pub fn de_iso(s: &str) -> Option<Fecha> {
    let b = s.as_bytes();
    if s.len() == 10 && b[4] == b'-' && b[7] == b'-' {
        Fecha::nueva(digitos(&s[..4])?, digitos(&s[5..7])?, digitos(&s[8..])?)
    } else if s.len() == 8 {
        Fecha::nueva(digitos(&s[..4])?, digitos(&s[4..6])?, digitos(&s[6..])?)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Momento {
    pub fecha: Fecha,
    pub hora: u32,
}

/// `datetime.fromisoformat` (lo que se usa en `VORTEX_TAREAS_AHORA`: AAAA-MM-DD[THH[:MM[:SS]]]).
pub fn momento_de_iso(s: &str) -> Option<Momento> {
    if !s.is_ascii() {
        return None;
    }
    let (f, resto) = if s.len() >= 10 && s.as_bytes()[4] == b'-' {
        s.split_at(10)
    } else if s.len() >= 8 {
        s.split_at(8)
    } else {
        return None;
    };
    let fecha = de_iso(f)?;
    if resto.is_empty() {
        return Some(Momento { fecha, hora: 0 });
    }
    let hora = digitos(resto.get(1..3)?)?;
    let cola = &resto[3..];
    if hora > 23
        || !(cola.is_empty()
            || cola.starts_with([':', '.', '+', '-', 'Z'])
            || cola.as_bytes()[0].is_ascii_digit())
    {
        return None;
    }
    Some(Momento {
        fecha,
        hora: hora as u32,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn valida_como_python() {
        assert!(Fecha::nueva(2026, 2, 29).is_none());
        assert!(Fecha::nueva(2028, 2, 29).is_some());
        assert!(Fecha::nueva(2026, 13, 1).is_none());
        assert!(Fecha::nueva(0, 1, 1).is_none());
        assert!(Fecha::nueva(10000, 1, 1).is_none());
        assert_eq!(de_iso("2026-11-04").unwrap().iso(), "2026-11-04");
        assert!(de_iso("2026-13-40").is_none());
    }

    #[test]
    fn resta_de_dias() {
        let a = Fecha::nueva(2026, 11, 2).unwrap();
        let b = Fecha::nueva(2026, 10, 30).unwrap();
        assert_eq!(b.dias() - a.dias(), -3);
        assert_eq!(Fecha::nueva(1970, 1, 1).unwrap().dias(), 0);
    }

    #[test]
    fn momento() {
        let m = momento_de_iso("2026-11-02T18:00").unwrap();
        assert_eq!((m.fecha.iso().as_str(), m.hora), ("2026-11-02", 18));
        assert!(momento_de_iso("ayer").is_none());
    }
}
