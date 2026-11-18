//! Mandar a la papelera (la de freedesktop, la misma que usa el explorador de archivos) (`papelera.py`).
//!
//! Deshacer nunca borra para siempre: lo que se desinstala o se quita va a la papelera y se puede recuperar.
//! Dentro de la carpeta personal se hace aquí mismo (`Trash/files` + `Trash/info`); fuera de ella (otro
//! disco) se le pide a «gio trash», que sabe usar la papelera de cada disco.

use crate::util::{abspath, escapar_url, mover, nombre_de, raiz_de_nombre, sufijo};
use crate::{Contexto, proc};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

fn dentro(ruta: &Path, carpeta: &Path) -> bool {
    abspath(ruta).starts_with(abspath(carpeta))
}

/// Manda `ruta` (archivo, carpeta o enlace) a la papelera. `false` si no existía.
pub fn mandar(ctx: &Contexto, ruta: &Path) -> std::io::Result<bool> {
    let ruta = abspath(ruta);
    if std::fs::symlink_metadata(&ruta).is_err() {
        return Ok(false);
    }
    if !dentro(&ruta, &ctx.rutas.casa)
        && let Some(gio) = ctx.which("gio")
    {
        let mut c = std::process::Command::new(gio);
        ctx.preparar(&mut c);
        c.args(["trash", "--"]).arg(&ruta);
        if let Ok(r) = proc::ejecutar(c, None, None)
            && r.ok()
        {
            return Ok(true);
        }
    }
    let base = ctx.rutas.papelera();
    let (archivos, info) = (base.join("files"), base.join("info"));
    std::fs::create_dir_all(&archivos)?;
    std::fs::create_dir_all(&info)?;
    let nombre_ruta = nombre_de(&ruta);
    let es_archivo = ruta.is_file();
    let mut nombre = nombre_ruta.clone();
    let mut n = 1;
    let (mut fd, trashinfo): (std::fs::File, PathBuf) = loop {
        let ti = info.join(format!("{nombre}.trashinfo"));
        // el .trashinfo se reserva primero (O_EXCL): así dos borrados no eligen el mismo nombre
        match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&ti) {
            Ok(f) => {
                if std::fs::symlink_metadata(archivos.join(&nombre)).is_err() {
                    break (f, ti);
                }
                drop(f);
                std::fs::remove_file(&ti)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        n += 1;
        nombre = if es_archivo {
            format!("{}.{n}{}", raiz_de_nombre(&nombre_ruta), sufijo(&nombre_ruta))
        } else {
            format!("{nombre_ruta}.{n}")
        };
    };
    let ahora = crate::util::ahora();
    write!(
        fd,
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        escapar_url(&ruta.to_string_lossy()),
        ahora.iso()
    )?;
    drop(fd);
    if let Err(e) = mover(&ruta, &archivos.join(&nombre)) {
        let _ = std::fs::remove_file(&trashinfo);
        return Err(e);
    }
    Ok(true)
}
