//! El registro de cada app: `~/.local/state/vortex/<app>.log`.
//!
//! Una línea por evento («2026-11-16 18:03:00Z INFO …», en UTC). Cuando pasa de [`TOPE`] se mueve a
//! `<app>.log.1` y empieza otro, así nunca crece sin fin. En Linux el archivo es 600.
//! **No se registran datos personales**: ni contraseñas, ni lo que escribe la persona, ni imágenes.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tamaño máximo del archivo antes de rotarlo.
pub const TOPE: u64 = 512 * 1024;

struct Registro {
    ruta: PathBuf,
    archivo: File,
}

static REGISTRO: Mutex<Option<Registro>> = Mutex::new(None);

/// Abre el registro de `app` en `rutas::estado()`. Devuelve la ruta del archivo.
pub fn iniciar(app: &str) -> io::Result<PathBuf> {
    iniciar_en(&crate::rutas::estado().join(format!("{app}.log")))
}

/// Abre el registro en una ruta concreta (pruebas).
pub fn iniciar_en(ruta: &Path) -> io::Result<PathBuf> {
    let archivo = abrir(ruta)?;
    *REGISTRO.lock().unwrap_or_else(|e| e.into_inner()) = Some(Registro {
        ruta: ruta.to_path_buf(),
        archivo,
    });
    Ok(ruta.to_path_buf())
}

fn abrir(ruta: &Path) -> io::Result<File> {
    if let Some(c) = ruta.parent() {
        fs::create_dir_all(c)?;
    }
    let mut op = OpenOptions::new();
    op.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        op.mode(0o600);
    }
    op.open(ruta)
}

pub fn info(mensaje: &str) {
    escribir("INFO", mensaje);
}

pub fn aviso(mensaje: &str) {
    escribir("AVISO", mensaje);
}

pub fn error(mensaje: &str) {
    escribir("ERROR", mensaje);
}

fn escribir(nivel: &str, mensaje: &str) {
    let mut guardia = REGISTRO.lock().unwrap_or_else(|e| e.into_inner());
    let Some(r) = guardia.as_mut() else {
        return; // sin iniciar: no se registra nada (y no truena)
    };
    if r.archivo.metadata().map(|m| m.len()).unwrap_or(0) >= TOPE {
        let mut viejo = r.ruta.as_os_str().to_owned();
        viejo.push(".1");
        let _ = fs::rename(&r.ruta, PathBuf::from(viejo));
        if let Ok(f) = abrir(&r.ruta) {
            r.archivo = f;
        }
    }
    let linea = mensaje.replace(['\n', '\r'], " ");
    let _ = writeln!(r.archivo, "{} {nivel} {linea}", fecha_utc(SystemTime::now()));
}

/// «AAAA-MM-DD HH:MM:SSZ» sin depender de crates de fechas.
pub fn fecha_utc(t: SystemTime) -> String {
    let s = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (dias, resto) = ((s / 86_400) as i64, s % 86_400);
    // De días desde 1970 a fecha civil (algoritmo de Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = yoe + era * 400 + i64::from(mes <= 2);
    format!(
        "{anio:04}-{mes:02}-{dia:02} {:02}:{:02}:{:02}Z",
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fechas() {
        assert_eq!(fecha_utc(UNIX_EPOCH), "1970-01-01 00:00:00Z");
        // 2026-11-16 18:03:05 UTC
        assert_eq!(
            fecha_utc(UNIX_EPOCH + Duration::from_secs(1_794_852_185)),
            "2026-11-16 18:03:05Z"
        );
        // 29 de febrero de 2024
        assert_eq!(
            fecha_utc(UNIX_EPOCH + Duration::from_secs(1_709_164_800)),
            "2024-02-29 00:00:00Z"
        );
    }

    // Una sola prueba usa el registro global (las pruebas corren en hilos paralelos).
    #[test]
    fn escribe_una_linea_por_evento_y_rota() {
        let c = crate::carpeta_de_prueba("registro");
        let r = c.join("app.log");
        iniciar_en(&r).unwrap();
        info("arrancó");
        error("algo falló\ncon dos renglones");
        let texto = fs::read_to_string(&r).unwrap();
        let lineas: Vec<_> = texto.lines().collect();
        assert_eq!(lineas.len(), 2);
        assert!(lineas[0].ends_with(" INFO arrancó"));
        assert!(lineas[1].ends_with(" ERROR algo falló con dos renglones"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&r).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let largo = "x".repeat(1024);
        for _ in 0..(TOPE / 1024 + 2) {
            info(&largo);
        }
        assert!(c.join("app.log.1").exists(), "al pasar el tope se mueve a .log.1");
        assert!(fs::metadata(&r).unwrap().len() < TOPE);
    }
}
