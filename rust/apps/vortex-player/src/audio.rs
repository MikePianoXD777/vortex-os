//! Configuración de salida de audio en Linux (PipeWire/WirePlumber, LDAC y Hi-Res).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FRECUENCIAS: [i64; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

const TEXTO_HIRES: &str = "\
# Vortex Player: cada canción suena a su frecuencia original (sin pasar todo a 48 kHz).
# Se quita borrando este archivo.
context.properties = {
    default.clock.allowed-rates = [ 44100 48000 88200 96000 176400 192000 ]
}
";

const TEXTO_LDAC: &str = "\
# Vortex Player: con audífonos LDAC, siempre a 990 kbps (la calidad más alta) en vez de «auto».
# Se quita desde Vortex Player (Ajustes) o borrando este archivo.
monitor.bluez.rules = [
  {
    matches = [ { device.name = \"~bluez_card.*\" } ]
    actions = { update-props = { bluez5.a2dp.ldac.quality = \"hq\" } }
  }
]
";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalidaAudio {
    pub nombre: String,
    pub bluetooth: bool,
    pub codec: String,
    pub codec_texto: String,
    pub hz: i64,
    pub tarjeta: String,
    pub ldac: bool,
}

fn xdg_config() -> PathBuf {
    vortex_core::rutas::config()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| vortex_core::rutas::home().join(".config"))
}

pub fn archivo_hires() -> PathBuf {
    archivo_hires_en(&xdg_config())
}

pub fn archivo_hires_en(xdg_config_dir: &Path) -> PathBuf {
    xdg_config_dir
        .join("pipewire")
        .join("pipewire.conf.d")
        .join("90-vortex-player-hires.conf")
}

pub fn archivo_ldac() -> PathBuf {
    archivo_ldac_en(&xdg_config())
}

pub fn archivo_ldac_en(xdg_config_dir: &Path) -> PathBuf {
    xdg_config_dir
        .join("wireplumber")
        .join("wireplumber.conf.d")
        .join("90-vortex-player-ldac.conf")
}

fn escribir_si_cambia(ruta: &Path, texto: &str) -> bool {
    if fs::read_to_string(ruta).ok().as_deref() == Some(texto) {
        return false;
    }
    if let Some(padre) = ruta.parent()
        && fs::create_dir_all(padre).is_err()
    {
        return false;
    }
    fs::write(ruta, texto).is_ok()
}

pub fn config_hires() -> bool {
    config_hires_en(&xdg_config())
}

pub fn config_hires_en(xdg_config_dir: &Path) -> bool {
    escribir_si_cambia(&archivo_hires_en(xdg_config_dir), TEXTO_HIRES)
}

pub fn config_ldac(activar: bool) -> bool {
    config_ldac_en(&xdg_config(), activar)
}

pub fn config_ldac_en(xdg_config_dir: &Path, activar: bool) -> bool {
    let ruta = archivo_ldac_en(xdg_config_dir);
    if activar {
        return escribir_si_cambia(&ruta, TEXTO_LDAC);
    }
    if ruta.exists() {
        fs::remove_file(ruta).is_ok()
    } else {
        false
    }
}

pub fn cambia_la_salida(eventos: &str) -> bool {
    ["sink", "server", "card"]
        .iter()
        .any(|obj| eventos.contains(&format!(" {obj} #")))
}

fn nombre_codec_bt(codec: &str) -> String {
    match codec {
        "ldac" => "LDAC".into(),
        "aac" => "AAC".into(),
        "sbc" => "SBC".into(),
        "sbc_xq" => "SBC XQ".into(),
        "aptx" => "aptX".into(),
        "aptx_hd" => "aptX HD".into(),
        "aptx_ll" => "aptX LL".into(),
        "lc3" => "LC3".into(),
        "opus_05" => "Opus".into(),
        "faststream" => "FastStream".into(),
        "msbc" => "mSBC".into(),
        "cvsd" => "CVSD".into(),
        other => other.to_uppercase(),
    }
}

fn hz_de(spec: &str) -> i64 {
    for parte in spec.split_whitespace() {
        if let Some(num) = parte.strip_suffix("Hz")
            && let Ok(hz) = num.parse::<i64>()
        {
            return hz;
        }
    }
    0
}

fn ejecutar_pactl(pactl: &str, args: &[&str]) -> String {
    let Ok(out) = Command::new(pactl).args(args).output() else {
        return String::new();
    };
    if out.status.success() {
        String::from_utf8_lossy(&out.stdout).to_string()
    } else {
        String::new()
    }
}

pub fn perfiles(tarjeta: &str, pactl: &str) -> Vec<String> {
    let json_str = ejecutar_pactl(pactl, &["-f", "json", "list", "cards"]);
    let Ok(cards) = serde_json::from_str::<Vec<Value>>(&json_str) else {
        return Vec::new();
    };
    for card in cards {
        if card.get("name").and_then(Value::as_str) == Some(tarjeta)
            && let Some(p) = card.get("profiles").and_then(Value::as_object)
        {
            return p
                .iter()
                .filter(|(_, v)| {
                    !v.is_object() || v.get("available").and_then(Value::as_bool).unwrap_or(true)
                })
                .map(|(k, _)| k.clone())
                .collect();
        }
    }
    Vec::new()
}

pub fn salida(pactl: &str) -> Option<SalidaAudio> {
    let defecto = ejecutar_pactl(pactl, &["get-default-sink"]).trim().to_string();
    let json_str = ejecutar_pactl(pactl, &["-f", "json", "list", "sinks"]);
    let sinks = serde_json::from_str::<Vec<Value>>(&json_str).ok()?;
    let sink = sinks
        .iter()
        .find(|s| s.get("name").and_then(Value::as_str) == Some(defecto.as_str()))
        .or_else(|| sinks.first())?;

    let props = sink.get("properties").unwrap_or(&Value::Null);
    let nombre_sink = sink.get("name").and_then(Value::as_str).unwrap_or("");
    let bt = props.get("device.api").and_then(Value::as_str) == Some("bluez5")
        || nombre_sink.starts_with("bluez_output");
    let codec = if bt {
        props
            .get("api.bluez5.codec")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };
    let tarjeta = props
        .get("device.name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let nombre = props
        .get("device.description")
        .or_else(|| sink.get("description"))
        .or_else(|| sink.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let hz = hz_de(
        sink.get("sample_specification")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let ldac = if bt && !tarjeta.is_empty() {
        perfiles(&tarjeta, pactl).iter().any(|p| p == "a2dp-sink-ldac")
    } else {
        false
    };

    Some(SalidaAudio {
        nombre,
        bluetooth: bt,
        codec_texto: nombre_codec_bt(&codec),
        codec,
        hz,
        tarjeta,
        ldac,
    })
}

pub fn forzar_ldac(tarjeta: &str, pactl: &str) -> bool {
    if !perfiles(tarjeta, pactl).iter().any(|p| p == "a2dp-sink-ldac") {
        return false;
    }
    Command::new(pactl)
        .args(["set-card-profile", tarjeta, "a2dp-sink-ldac"])
        .output()
        .is_ok_and(|o| o.status.success())
}
