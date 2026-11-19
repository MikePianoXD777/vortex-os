//! `~/.config/vortex/miku.json` (el mismo de la Miku de Python). Lo que falta toma su valor de fábrica,
//! un JSON inválido no truena y al guardar no se pierden las claves que no conocemos.

use crate::rutas::Rutas;
use serde_json::{Map, Value};
use std::path::Path;
use vortex_core::config as nucleo;

pub const IAS: [&str; 5] = ["claude", "local", "gemini", "openai", "otra"];

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub avatar: bool,
    pub voz: String,
    pub modelo: String,
    pub seguir_escuchando: bool,
    pub silencio: f64,
    pub internet: bool,
    pub elevenlabs_voz: String,
    pub elevenlabs_modelo: String,
    pub ia: String,
    pub ia_url: String,
    pub ia_modelo: String,
    pub voz_natural_auto: bool,
    pub sentimientos: bool,
    pub habla_sola: bool,
    pub camara: bool,
    pub camara_aviso: bool,
    pub companera: bool,
    pub companera_tono: String,
    pub companera_consentimiento: bool,
    pub companera_sin_cuenta: bool,
    pub companera_mayor: bool,
    pub platicadora: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            avatar: true,
            voz: "supertonic-f1".into(),
            modelo: String::new(),
            seguir_escuchando: true,
            silencio: 1.5,
            internet: false,
            elevenlabs_voz: String::new(),
            elevenlabs_modelo: String::new(),
            ia: "claude".into(),
            ia_url: String::new(),
            ia_modelo: String::new(),
            voz_natural_auto: true,
            sentimientos: true,
            habla_sola: true,
            camara: false,
            camara_aviso: false,
            companera: false,
            companera_tono: "carinosa".into(),
            companera_consentimiento: false,
            companera_sin_cuenta: false,
            companera_mayor: false,
            platicadora: true,
        }
    }
}

fn bool_de(m: &Map<String, Value>, k: &str, defecto: bool) -> bool {
    m.get(k).and_then(Value::as_bool).unwrap_or(defecto)
}

fn texto_de(m: &Map<String, Value>, k: &str, defecto: &str) -> String {
    match m.get(k) {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => defecto.to_string(),
    }
}

impl Config {
    /// La configuración efectiva de `ruta`: cada clave con su valor de fábrica si falta o viene mal.
    pub fn cargar(ruta: &Path, rutas: &Rutas) -> Self {
        let m = nucleo::cargar_crudo(ruta);
        let d = Config::default();
        let mut c = Config {
            avatar: bool_de(&m, "avatar", d.avatar),
            seguir_escuchando: bool_de(&m, "seguir_escuchando", d.seguir_escuchando),
            voz_natural_auto: bool_de(&m, "voz_natural_auto", d.voz_natural_auto),
            sentimientos: bool_de(&m, "sentimientos", d.sentimientos),
            habla_sola: bool_de(&m, "habla_sola", d.habla_sola),
            platicadora: bool_de(&m, "platicadora", d.platicadora),
            camara: bool_de(&m, "camara", d.camara),
            camara_aviso: bool_de(&m, "camara_aviso", d.camara_aviso),
            companera: bool_de(&m, "companera", d.companera),
            companera_sin_cuenta: bool_de(&m, "companera_sin_cuenta", false),
            companera_mayor: bool_de(&m, "companera_mayor", false),
            companera_consentimiento: bool_de(&m, "companera_consentimiento", d.companera_consentimiento),
            // Sin «internet» en miku.json manda la casilla del instalador (la marca de vortex-miku-internet).
            internet: bool_de(&m, "internet", rutas.marca_internet.is_file()),
            ..d
        };
        if let Some(t) = m.get("companera_tono").and_then(Value::as_str)
            && (t == "carinosa" || t == "suave")
        {
            c.companera_tono = t.into();
        }
        if let Some(i) = m.get("ia").and_then(Value::as_str)
            && IAS.contains(&i)
        {
            c.ia = i.into();
        }
        c.voz = texto_de(&m, "voz", &c.voz);
        c.modelo = texto_de(&m, "modelo", &c.modelo);
        c.elevenlabs_voz = texto_de(&m, "elevenlabs_voz", "");
        c.elevenlabs_modelo = texto_de(&m, "elevenlabs_modelo", "");
        c.ia_url = texto_de(&m, "ia_url", "");
        c.ia_modelo = texto_de(&m, "ia_modelo", "");
        c.silencio = match m.get("silencio") {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(1.5).clamp(0.6, 5.0),
            _ => 1.5,
        };
        c
    }

    /// Pone las claves de `cambios` sin tocar las demás.
    pub fn guardar(ruta: &Path, cambios: &Map<String, Value>) -> std::io::Result<()> {
        nucleo::guardar(ruta, cambios)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn fabrica_y_claves_malas() {
        let d = tempfile::tempdir().unwrap();
        let r = Rutas::en(d.path());
        let c = Config::cargar(&r.config, &r);
        assert_eq!(c, Config::default());
        std::fs::create_dir_all(&r.config_dir).unwrap();
        std::fs::write(
            &r.config,
            r#"{"ia":"gemini","avatar":"si","silencio":99,"voz":" x ","companera_tono":"celosa","raro":1}"#,
        )
        .unwrap();
        let c = Config::cargar(&r.config, &r);
        assert_eq!(c.ia, "gemini");
        assert!(c.avatar); // un tipo equivocado se queda en el de fábrica
        assert_eq!(c.silencio, 5.0);
        assert_eq!(c.voz, "x");
        assert_eq!(c.companera_tono, "carinosa");
        // Guardar no pierde lo que no conoce.
        let mut cambios = Map::new();
        cambios.insert("camara".into(), json!(true));
        Config::guardar(&r.config, &cambios).unwrap();
        let t: Value = serde_json::from_str(&std::fs::read_to_string(&r.config).unwrap()).unwrap();
        assert_eq!(t["raro"], 1);
        assert_eq!(t["camara"], true);
        // La marca del instalador enciende internet si miku.json no lo dice.
        std::fs::write(&r.config, "{}").unwrap();
        std::fs::write(&r.marca_internet, "").unwrap();
        assert!(Config::cargar(&r.config, &r).internet);
    }
}
