//! Una sola ventana por persona, compatible con la de Python (`QLocalServer` «vortex-player-<uid>», que en
//! Linux es un socket en la carpeta temporal): si ya hay una abierta, la nueva le pasa lo que recibió (un
//! enlace para bajar o archivos para agregar) y sale. Protocolo: «abrir\n» + un argumento por línea + «\n\n».

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn ruta() -> PathBuf {
    let tmp = std::env::var_os("TMPDIR")
        .filter(|t| !t.is_empty())
        .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    // SAFETY: getuid no falla ni toca memoria.
    #[allow(unsafe_code)]
    let uid = unsafe { libc::getuid() };
    tmp.join(format!("vortex-player-{uid}"))
}

/// ¿Había una abierta? Entonces ya le pasó `args` (y se le pidió que se muestre).
pub fn avisar_a_la_abierta(ruta: &Path, args: &[String]) -> bool {
    let Ok(mut s) = UnixStream::connect(ruta) else {
        return false;
    };
    let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
    let mut texto = String::from("abrir");
    for a in args {
        texto.push('\n');
        texto.push_str(a);
    }
    texto.push_str("\n\n");
    s.write_all(texto.as_bytes()).is_ok()
}

/// Se queda escuchando (borra un socket viejo de una que se cerró mal, como `QLocalServer.removeServer`).
pub fn escuchar(ruta: &Path) -> Option<UnixListener> {
    let _ = std::fs::remove_file(ruta);
    let l = UnixListener::bind(ruta).ok()?;
    let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o600));
    Some(l)
}

/// Lo que mandó la otra copia: los argumentos, o `None` si no era un «abrir».
pub fn leer(mut c: UnixStream) -> Option<Vec<String>> {
    let _ = c.set_read_timeout(Some(Duration::from_secs(2)));
    let mut b = Vec::new();
    let _ = Read::by_ref(&mut c).take(1 << 20).read_to_end(&mut b);
    let t = String::from_utf8_lossy(&b);
    let mut lineas = t.lines();
    (lineas.next()? == "abrir").then(|| lineas.filter(|x| !x.is_empty()).map(String::from).collect())
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn de_ida_y_vuelta() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("sock");
        assert!(!super::avisar_a_la_abierta(&r, &[]));
        let l = super::escuchar(&r).unwrap();
        let args = vec!["https://youtu.be/x".to_string(), "/m/a b.flac".to_string()];
        assert!(super::avisar_a_la_abierta(&r, &args));
        let (c, _) = l.accept().unwrap();
        assert_eq!(super::leer(c), Some(args));
    }
}
