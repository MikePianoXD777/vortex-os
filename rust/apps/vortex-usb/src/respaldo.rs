//! El respaldo a una memoria USB (4.0). Al conectar una memoria, el aviso trae «Usar para respaldos»: deja en
//! ella el archivo `.vortex-respaldo` (las carpetas que se respaldan, una por línea, relativas a tu carpeta
//! personal) y respalda. Desde ahí, **cada vez que la conectas** copia solo lo nuevo o lo que cambió, con su
//! avance en el aviso, a `Respaldo de <usuario>/` en la memoria. Nunca borra nada de la memoria: lo que
//! borres en la PC sigue respaldado.
//!
//! No se copian las carpetas ocultas (`.git`, configuraciones), ni `node_modules`, ni las que se marcan como
//! caché (`CACHEDIR.TAG`, como `target/` de Rust), ni los enlaces.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// El archivo que marca una memoria de respaldo (en su raíz).
pub const MARCA: &str = ".vortex-respaldo";

/// Lo que se copió (o no).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Resumen {
    pub copiados: usize,
    pub bytes: u64,
    /// Ya estaban iguales en la memoria.
    pub iguales: usize,
    /// No cupieron o fallaron (el primer error, para el aviso).
    pub fallidos: usize,
    pub error: Option<String>,
    pub sin_espacio: bool,
}

/// Un archivo por copiar.
#[derive(Debug, Clone)]
pub struct Copia {
    pub origen: PathBuf,
    pub destino: PathBuf,
    pub bytes: u64,
    pub fecha: SystemTime,
}

/// Las carpetas de fábrica: Documentos, Imágenes y Escritorio (las de XDG, con su nombre de verdad).
pub fn carpetas_de_fabrica(home: &Path) -> Vec<String> {
    // `rutas::config()` es `~/.config/vortex`: user-dirs.dirs está un nivel arriba.
    let config = vortex_core::rutas::config();
    let dirs =
        fs::read_to_string(config.parent().unwrap_or(&config).join("user-dirs.dirs")).unwrap_or_default();
    ["XDG_DOCUMENTS_DIR", "XDG_PICTURES_DIR", "XDG_DESKTOP_DIR"]
        .iter()
        .zip(["Documentos", "Imágenes", "Escritorio"])
        .filter_map(|(clave, defecto)| {
            let valor = dirs
                .lines()
                .find_map(|l| l.trim().strip_prefix(clave)?.strip_prefix('='))
                .map(|v| v.trim_matches('"').replace("$HOME/", ""))
                .unwrap_or_else(|| defecto.to_string());
            (!valor.is_empty() && valor != "$HOME" && home.join(&valor).is_dir()).then_some(valor)
        })
        .collect()
}

/// Las carpetas de la marca (líneas que no están vacías ni son comentarios; nada de `..` ni rutas absolutas).
pub fn leer_marca(texto: &str) -> Vec<String> {
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|l| !l.starts_with('/') && !l.split('/').any(|p| p == ".."))
        .map(String::from)
        .collect()
}

pub fn texto_de_marca(carpetas: &[String]) -> String {
    let mut t = String::from(
        "# Vortexine's OS: esta memoria es de respaldo. Al conectarla se copian aquí estas carpetas de tu\n\
         # carpeta personal (una por línea), solo lo nuevo o lo que cambió. Nada se borra de la memoria.\n",
    );
    for c in carpetas {
        t.push_str(c);
        t.push('\n');
    }
    t
}

/// ¿Esta carpeta no se respalda?
fn se_salta(nombre: &str, ruta: &Path) -> bool {
    nombre.starts_with('.')
        || nombre == "node_modules"
        || nombre == "__pycache__"
        || ruta.join("CACHEDIR.TAG").is_file()
}

/// ¿Ya está igual en la memoria? Mismo tamaño y misma fecha (con 2 s de margen: FAT guarda la hora así).
fn igual(destino: &Path, bytes: u64, fecha: SystemTime) -> bool {
    let Ok(m) = fs::metadata(destino) else {
        return false;
    };
    if m.len() != bytes {
        return false;
    }
    let Ok(d) = m.modified() else { return false };
    let dif = if d > fecha {
        d.duration_since(fecha)
    } else {
        fecha.duration_since(d)
    };
    dif.is_ok_and(|x| x <= Duration::from_secs(2))
}

/// Lo que hay que copiar de `carpetas` (relativas a `home`) a `raiz` en la memoria. `iguales` cuenta lo
/// que ya estaba.
pub fn plan(home: &Path, carpetas: &[String], raiz: &Path) -> (Vec<Copia>, usize) {
    let mut lista = Vec::new();
    let mut iguales = 0;
    for c in carpetas {
        let mut pila = vec![(home.join(c), raiz.join(c))];
        while let Some((o, d)) = pila.pop() {
            let Ok(entradas) = fs::read_dir(&o) else { continue };
            for e in entradas.flatten() {
                let nombre = e.file_name().to_string_lossy().into_owned();
                let Ok(m) = e.metadata() else { continue }; // sin seguir enlaces
                let (eo, ed) = (e.path(), d.join(&nombre));
                if m.is_dir() {
                    if !se_salta(&nombre, &eo) {
                        pila.push((eo, ed));
                    }
                } else if m.is_file() && !nombre.ends_with(".vortex-tmp") {
                    let fecha = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    if igual(&ed, m.len(), fecha) {
                        iguales += 1;
                    } else {
                        lista.push(Copia {
                            origen: eo,
                            destino: ed,
                            bytes: m.len(),
                            fecha,
                        });
                    }
                }
            }
        }
    }
    (lista, iguales)
}

/// Copia uno: a un temporal y luego en su lugar (si se saca la memoria a medias no queda uno roto), con la
/// fecha del original para que la próxima vez se vea igual.
fn copiar(c: &Copia) -> io::Result<()> {
    if let Some(p) = c.destino.parent() {
        fs::create_dir_all(p)?;
    }
    let mut tmp = c.destino.clone().into_os_string();
    tmp.push(".vortex-tmp");
    let tmp = PathBuf::from(tmp);
    let r = (|| {
        fs::copy(&c.origen, &tmp)?;
        fs::File::options()
            .write(true)
            .open(&tmp)?
            .set_modified(c.fecha)?;
        fs::rename(&tmp, &c.destino)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// Copia el plan. `avance(hecho, total)` se llama cada `cada` (para el aviso).
pub fn copiar_todo(
    lista: &[Copia],
    iguales: usize,
    cada: Duration,
    avance: &mut dyn FnMut(u64, u64),
) -> Resumen {
    let total: u64 = lista.iter().map(|c| c.bytes).sum();
    let mut r = Resumen {
        iguales,
        ..Resumen::default()
    };
    let mut ultimo = Instant::now();
    for c in lista {
        match copiar(c) {
            Ok(()) => {
                r.copiados += 1;
                r.bytes += c.bytes;
            }
            Err(e) => {
                r.fallidos += 1;
                // ENOSPC: ya no cabe nada más; se para.
                if e.raw_os_error() == Some(28) {
                    r.sin_espacio = true;
                    r.fallidos += lista.len() - r.copiados - r.fallidos;
                    break;
                }
                // EFBIG: más de 4 GB en una memoria FAT32.
                if r.error.is_none() {
                    r.error = Some(if e.raw_os_error() == Some(27) {
                        format!("«{}» pesa más de 4 GB y la memoria es FAT32", nombre(&c.origen))
                    } else {
                        format!("«{}»: {e}", nombre(&c.origen))
                    });
                }
            }
        }
        if ultimo.elapsed() >= cada {
            avance(r.bytes, total);
            ultimo = Instant::now();
        }
    }
    r
}

fn nombre(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// La carpeta del respaldo en la memoria: `Respaldo de <usuario>`.
pub fn raiz_en(punto: &Path) -> PathBuf {
    let usuario = std::env::var("USER")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "ti".into());
    punto.join(format!("Respaldo de {usuario}"))
}

/// El texto del aviso del final.
pub fn texto_del_final(r: &Resumen, memoria: &str, tamano: &dyn Fn(u64) -> String) -> (String, String) {
    let titulo = if r.sin_espacio {
        "El respaldo no cupo completo".to_string()
    } else if r.copiados == 0 && r.fallidos == 0 {
        "Tu respaldo ya estaba al día".to_string()
    } else {
        "Respaldo listo".to_string()
    };
    let mut cuerpo = if r.copiados == 0 {
        format!("No había nada nuevo para «{memoria}».")
    } else if r.copiados == 1 {
        format!(
            "Se copió 1 archivo nuevo o cambiado ({}) a «{memoria}».",
            tamano(r.bytes)
        )
    } else {
        format!(
            "Se copiaron {} archivos nuevos o cambiados ({}) a «{memoria}».",
            r.copiados,
            tamano(r.bytes)
        )
    };
    if r.sin_espacio {
        cuerpo.push_str(&format!(
            " La memoria se llenó: faltaron {} archivos.",
            r.fallidos
        ));
    } else if r.fallidos > 0 {
        let e = r.error.clone().unwrap_or_default();
        cuerpo.push_str(&format!(" {} no se pudieron copiar ({e}).", r.fallidos));
    }
    (titulo, cuerpo)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_marca() {
        assert_eq!(
            leer_marca("# comentario\nDocumentos\n\n Imágenes \n/etc\n../x\nA/../b\nMúsica/Mías\n"),
            vec!["Documentos", "Imágenes", "Música/Mías"]
        );
        let c = vec!["Documentos".to_string(), "Imágenes".to_string()];
        assert_eq!(leer_marca(&texto_de_marca(&c)), c);
    }

    #[test]
    fn los_avisos() {
        let t = |b: u64| format!("{b} B");
        let r = Resumen {
            copiados: 3,
            bytes: 30,
            ..Resumen::default()
        };
        assert_eq!(texto_del_final(&r, "USB", &t).0, "Respaldo listo");
        assert!(
            texto_del_final(&r, "USB", &t)
                .1
                .contains("3 archivos nuevos o cambiados (30 B)")
        );
        assert_eq!(
            texto_del_final(&Resumen::default(), "USB", &t).0,
            "Tu respaldo ya estaba al día"
        );
        let lleno = Resumen {
            copiados: 1,
            bytes: 5,
            fallidos: 4,
            sin_espacio: true,
            ..Resumen::default()
        };
        let (ti, cu) = texto_del_final(&lleno, "USB", &t);
        assert_eq!(ti, "El respaldo no cupo completo");
        assert!(cu.contains("faltaron 4 archivos"));
    }
}
