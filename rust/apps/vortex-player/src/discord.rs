//! Protocolo IPC local de Discord Rich Presence (marcos binarios + construcción de actividad).

use std::env;
use std::path::Path;

use serde_json::{Value, json};

use crate::biblioteca;

pub const CLIENT_ID: &str = "1568275063319437433";
pub const LOGO: &str = "https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/apps/vortex-player/empaquetado/logo-discord.png";

pub const SALUDO: u32 = 0;
pub const MARCO: u32 = 1;
pub const CIERRE: u32 = 2;
pub const PING: u32 = 3;
pub const PONG: u32 = 4;
pub const ESCUCHANDO: u64 = 2;

/// Empaqueta un opcode y un JSON como marco IPC de Discord (`<II` little-endian + UTF-8).
pub fn marco(op: u32, datos: &Value) -> Vec<u8> {
    let cuerpo = serde_json::to_vec(datos).unwrap_or_default();
    let len = cuerpo.len() as u32;
    let mut salida = Vec::with_capacity(8 + cuerpo.len());
    salida.extend_from_slice(&op.to_le_bytes());
    salida.extend_from_slice(&len.to_le_bytes());
    salida.extend_from_slice(&cuerpo);
    salida
}

/// Decodifica todos los marcos completos del búfer y devuelve los restantes bytes incompletos.
pub fn leer_marcos(buffer: &[u8]) -> (Vec<(u32, Value)>, Vec<u8>) {
    let mut marcos = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= buffer.len() {
        let op = u32::from_le_bytes([buffer[pos], buffer[pos + 1], buffer[pos + 2], buffer[pos + 3]]);
        let largo =
            u32::from_le_bytes([buffer[pos + 4], buffer[pos + 5], buffer[pos + 6], buffer[pos + 7]]) as usize;
        if pos + 8 + largo > buffer.len() {
            break;
        }
        if let Ok(val) = serde_json::from_slice::<Value>(&buffer[pos + 8..pos + 8 + largo]) {
            marcos.push((op, val));
        }
        pos += 8 + largo;
    }
    (marcos, buffer[pos..].to_vec())
}

/// Posibles sockets IPC locales donde escucha Discord.
pub fn rutas_socket() -> Vec<String> {
    if cfg!(windows) {
        return (0..10).map(|i| format!("discord-ipc-{i}")).collect();
    }
    let raiz = env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/tmp".to_string());
    let carpetas = [
        "",
        "app/com.discordapp.Discord",
        ".flatpak/dev.vencord.Vesktop/xdg-run",
        "snap.discord",
    ];
    let mut salida = Vec::new();
    for c in carpetas {
        for i in 0..10 {
            let ruta = if c.is_empty() {
                Path::new(&raiz).join(format!("discord-ipc-{i}"))
            } else {
                Path::new(&raiz).join(c).join(format!("discord-ipc-{i}"))
            };
            if ruta.exists() {
                salida.push(ruta.to_string_lossy().to_string());
            }
        }
    }
    salida
}

fn texto_discord(t: &str) -> String {
    let mut limpio = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.chars().count() > 128 {
        limpio = limpio.chars().take(128).collect();
    }
    if limpio.chars().count() == 1 {
        limpio.push('\u{200b}');
    }
    limpio
}

/// Datos de entrada para construir una actividad Rich Presence.
#[derive(Debug, Clone, Copy)]
pub struct DatosActividad<'a> {
    pub ruta: &'a str,
    pub titulo: &'a str,
    pub artista: &'a str,
    pub album: &'a str,
    pub posicion: f64,
    pub duracion: f64,
    pub velocidad: f64,
    pub ahora_seg: f64,
}

/// Construye el objeto `activity` de Discord para la canción actual.
pub fn actividad(datos: DatosActividad<'_>) -> Value {
    let vid = biblioteca::id_de(datos.ruta);
    let large_image = if vid.is_empty() {
        LOGO.to_string()
    } else {
        format!("https://i.ytimg.com/vi/{vid}/mqdefault.jpg")
    };
    let stem = Path::new(datos.ruta)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Vortex Player");
    let details = texto_discord(if datos.titulo.is_empty() {
        stem
    } else {
        datos.titulo
    });
    let large_text = texto_discord(if datos.album.is_empty() {
        "Vortex Player"
    } else {
        datos.album
    });

    let mut obj = json!({
        "type": ESCUCHANDO,
        "details": details,
        "assets": {
            "large_image": large_image,
            "large_text": large_text,
        }
    });

    let art = texto_discord(datos.artista);
    if !art.is_empty() {
        obj["state"] = Value::String(art);
    }
    if datos.duracion > 0.0 && datos.velocidad > 0.0 {
        let inicio_ms = ((datos.ahora_seg - datos.posicion / datos.velocidad) * 1000.0) as i64;
        let fin_ms =
            ((datos.ahora_seg + (datos.duracion - datos.posicion) / datos.velocidad) * 1000.0) as i64;
        obj["timestamps"] = json!({
            "start": inicio_ms,
            "end": fin_ms,
        });
    }
    obj
}
