//! Lo que hace hoy `puente.py`, sin Qt: los ajustes, la última captura y dónde va cada captura nueva.
//! La interfaz (cxx-qt) solo envuelve esto y le pasa los formatos de la cámara que le da Qt Multimedia.

use crate::ajustes::Ajustes;
use crate::capturas::{self, Carpetas};
use crate::hora::FechaHora;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Estado {
    pub carpetas: Carpetas,
    pub ajustes: Ajustes,
    ruta_ajustes: PathBuf,
    ultima: Option<PathBuf>,
}

impl Estado {
    /// Con las carpetas y los ajustes de la persona.
    pub fn new() -> Self {
        Self::con(Carpetas::de_la_persona(), crate::ajustes::ruta())
    }

    pub fn con(carpetas: Carpetas, ruta_ajustes: PathBuf) -> Self {
        let ajustes = Ajustes::leer_de(&ruta_ajustes);
        let ultima = carpetas.ultima();
        Estado {
            carpetas,
            ajustes,
            ruta_ajustes,
            ultima,
        }
    }

    /// `vv.ajuste(clave)`.
    pub fn ajuste(&self, clave: &str) -> Option<Value> {
        self.ajustes.valor(clave)
    }

    /// `vv.guardar(clave, valor)`: guarda solo si cambió.
    pub fn guardar(&mut self, clave: &str, valor: Value) -> bool {
        let cambio = self.ajustes.poner(clave, valor);
        if cambio {
            self.ajustes.guardar_en(&self.ruta_ajustes);
        }
        cambio
    }

    /// `vv.rutaFoto()`: la ruta de la foto nueva (crea la carpeta). `None` si no se puede crear.
    pub fn ruta_foto(&self, ahora: FechaHora) -> Option<PathBuf> {
        nueva(&self.carpetas.fotos, "Foto", ".jpg", ahora)
    }

    /// `vv.urlVideo()` (como ruta: la interfaz la vuelve URL).
    pub fn ruta_video(&self, ahora: FechaHora) -> Option<PathBuf> {
        nueva(&self.carpetas.videos, "Video", ".mp4", ahora)
    }

    /// `vv.registrar(ruta)`: una captura recién hecha. Acepta una ruta o un «file://». `true` si cambió.
    pub fn registrar(&mut self, ruta_o_url: &str) -> bool {
        let ruta = PathBuf::from(desde_url(ruta_o_url));
        if ruta.as_os_str().is_empty() || !ruta.exists() {
            return false;
        }
        self.ultima = Some(ruta);
        true
    }

    pub fn ultima(&self) -> Option<&Path> {
        self.ultima.as_deref()
    }

    pub fn ultima_es_video(&self) -> bool {
        self.ultima.as_deref().is_some_and(capturas::es_video)
    }

    /// La carpeta que abre «Abrir carpeta» (la crea si falta).
    pub fn carpeta_de(&self, modo: &str) -> Option<&Path> {
        let c = if modo == "video" {
            &self.carpetas.videos
        } else {
            &self.carpetas.fotos
        };
        std::fs::create_dir_all(c).ok()?;
        Some(c)
    }
}

impl Default for Estado {
    fn default() -> Self {
        Self::new()
    }
}

fn nueva(carpeta: &Path, prefijo: &str, extension: &str, ahora: FechaHora) -> Option<PathBuf> {
    std::fs::create_dir_all(carpeta).ok()?;
    Some(capturas::nombre_nuevo(carpeta, prefijo, extension, ahora))
}

/// «file:///home/ana/Foto%201.jpg» → «/home/ana/Foto 1.jpg» (lo que hace `QUrl(ruta).toLocalFile()`).
pub fn desde_url(texto: &str) -> String {
    let Some(resto) = texto.strip_prefix("file:") else {
        return texto.to_string();
    };
    let resto = resto
        .strip_prefix("//")
        .map(|r| r.find('/').map_or("", |i| &r[i..]))
        .unwrap_or(resto);
    let bytes = resto.as_bytes();
    let mut salida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = bytes.get(i + 1..i + 3).and_then(hex)
        {
            salida.push(b);
            i += 3;
            continue;
        }
        salida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&salida).into_owned()
}

fn hex(par: &[u8]) -> Option<u8> {
    u8::from_str_radix(std::str::from_utf8(par).ok()?, 16).ok()
}
