//! Deja esta PC igual que tu biblioteca en la nube (y la nube con lo nuevo de aquí): sube lo que cambió, quita
//! lo que borraste en el teléfono y arma las playlists. Lo que falta bajar lo baja el puente con su cola de
//! descargas, y al terminar se vuelve a sincronizar para meter lo nuevo a sus playlists.

use std::sync::Mutex;

use crate::biblioteca::Biblioteca;
use crate::cuenta_google::Carpeta;
use crate::nube::{self, DocNube, Plan};

fn texto_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// `borrar(id, ruta)` quita una canción de esta PC. La base va en un candado y lo de internet fuera de él.
/// Devuelve la biblioteca de la nube ya mezclada y el plan (en `plan.bajar` lo que falta bajar).
pub fn sincronizar(
    bib: &Mutex<Biblioteca>,
    drive: &dyn Carpeta,
    borrar: &mut dyn FnMut(&Biblioteca, &str, &str),
) -> Result<(DocNube, Plan), String> {
    let candado = || bib.lock().unwrap_or_else(|e| e.into_inner());
    let archivo = drive.buscar().map_err(texto_err)?;
    let remoto = match &archivo {
        Some(f) => nube::leer(&String::from_utf8_lossy(&drive.leer(f).map_err(texto_err)?)),
        None => nube::vacio(),
    };
    let nuevo = {
        let b = candado();
        nube::mezclar(&remoto, &nube::desde(&b.estado_nube().map_err(texto_err)?))
    };
    match &archivo {
        None => {
            drive.crear(&nube::texto(&nuevo)).map_err(texto_err)?;
        }
        Some(f) if !nube::iguales(&nuevo, &remoto) => {
            drive.escribir(f, &nube::texto(&nuevo)).map_err(texto_err)?
        }
        Some(_) => {}
    }
    let b = candado();
    let plan = nube::plan(&nuevo, &b.estado_nube().map_err(texto_err)?);
    let rutas = b.rutas_por_id().map_err(texto_err)?;
    for vid in &plan.borrar {
        if let Some(r) = rutas.get(vid) {
            borrar(&b, vid, r);
        }
    }
    for u in &plan.quitar_playlists {
        b.quitar_playlist_nube(u).map_err(texto_err)?;
    }
    armar_playlists(&b, &nuevo)?;
    Ok((nuevo, plan))
}

pub fn armar_playlists(b: &Biblioteca, doc: &DocNube) -> Result<(), String> {
    let rutas = b.rutas_por_id().map_err(texto_err)?;
    for (u, p) in nube::plan(doc, &b.estado_nube().map_err(texto_err)?).playlists {
        b.poner_playlist(&u, &p, &rutas).map_err(texto_err)?;
    }
    Ok(())
}
