//! Supertonic 3 (Supertone) sobre onnxruntime: duración → texto → difusión (vector_estimator) → vocoder,
//! 44,1 kHz mono. Es el mismo port compacto de supertonic-py 1.3.1 (MIT, © 2025 Supertone Inc.) que
//! `motor_voz.py`; los pesos son BigScience OpenRAIL-M (ver `apps/miku/licencias/`).
//!
//! onnxruntime no se enlaza al compilar ni se baja: se carga al arrancar la `libonnxruntime.so` del sistema
//! (la de python-onnxruntime-cpu, dependencia del paquete) o la de `ORT_DYLIB_PATH`.

use crate::texto::{self, MAX_TROZO};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Pasos de difusión (más = mejor y más lento): el de fábrica del SDK.
pub const PASOS: usize = 8;
/// Segundos de silencio entre trozos de una misma frase.
pub const SILENCIO_TROZOS: f64 = 0.3;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Dónde está `libonnxruntime.so`: `ORT_DYLIB_PATH`, o la de python-onnxruntime de cualquier Python del sistema.
pub fn buscar_onnxruntime() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ORT_DYLIB_PATH").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let mut candidatas: Vec<PathBuf> = vec![];
    for base in ["/usr/lib", "/usr/local/lib"] {
        let Ok(d) = std::fs::read_dir(base) else {
            continue;
        };
        for e in d.flatten() {
            if !e.file_name().to_string_lossy().starts_with("python3") {
                continue;
            }
            let capi = e.path().join("site-packages/onnxruntime/capi");
            if let Ok(libs) = std::fs::read_dir(&capi) {
                candidatas.extend(libs.flatten().map(|l| l.path()).filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("libonnxruntime.so"))
                }));
            }
        }
        let directa = Path::new(base).join("libonnxruntime.so");
        if directa.exists() {
            candidatas.push(directa);
        }
    }
    candidatas.sort();
    candidatas.pop()
}

/// CRC-32 (IEEE), la semilla de cada frase: misma frase, misma voz (como `zlib.crc32` en Python).
pub fn crc32(datos: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in datos {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Ruido normal (SplitMix64 + Box-Muller): no da los mismos números que numpy, pero sí siempre los mismos.
pub struct Ruido {
    estado: u64,
    guardado: Option<f64>,
}

impl Ruido {
    pub fn new(semilla: u64) -> Self {
        Self {
            estado: semilla,
            guardado: None,
        }
    }

    fn siguiente(&mut self) -> u64 {
        self.estado = self.estado.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.estado;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn uniforme(&mut self) -> f64 {
        ((self.siguiente() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    pub fn normal(&mut self) -> f64 {
        if let Some(v) = self.guardado.take() {
            return v;
        }
        let (u, v) = (self.uniforme(), self.uniforme());
        let r = (-2.0 * u.ln()).sqrt();
        let a = 2.0 * std::f64::consts::PI * v;
        self.guardado = Some(r * a.sin());
        r * a.cos()
    }
}

/// Un tensor de un estilo de voz: `{"data": [[...]], "dims": [1, 50, 256]}` (data anidada o plana).
fn tensor_estilo(v: &Value) -> Result<(Vec<i64>, Vec<f32>), Error> {
    fn aplanar(v: &Value, salida: &mut Vec<f32>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| aplanar(x, salida)),
            Value::Number(n) => salida.push(n.as_f64().unwrap_or(0.0) as f32),
            _ => {}
        }
    }
    let dims: Vec<i64> = v["dims"]
        .as_array()
        .ok_or("al estilo de la voz le faltan «dims»")?
        .iter()
        .filter_map(Value::as_i64)
        .collect();
    let mut datos = vec![];
    aplanar(&v["data"], &mut datos);
    if dims.iter().product::<i64>() as usize != datos.len() {
        return Err("el estilo de la voz no cuadra con sus «dims»".into());
    }
    Ok((dims, datos))
}

fn leer_json(ruta: &Path) -> Result<Value, Error> {
    let t = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    Ok(serde_json::from_str(&t)?)
}

pub struct Supertonic {
    duracion: Session,
    texto: Session,
    vector: Session,
    vocoder: Session,
    indice: Vec<i64>,
    pub sr: u32,
    trozo_base: i64,
    compresion: i64,
    latente: i64,
    estilo_ttl: (Vec<i64>, Vec<f32>),
    estilo_dp: (Vec<i64>, Vec<f32>),
}

/// `[1, 1, maximo]` con 1 donde `i < largo`.
fn mascara(largo: i64, maximo: i64) -> Tensor<f32> {
    let v: Vec<f32> = (0..maximo).map(|i| if i < largo { 1.0 } else { 0.0 }).collect();
    Tensor::from_array(([1usize, 1, maximo as usize], v)).expect("máscara")
}

fn primera_salida(salidas: &ort::session::SessionOutputs) -> Result<Vec<f32>, Error> {
    let (_, datos) = salidas[0].try_extract_tensor::<f32>()?;
    Ok(datos.to_vec())
}

impl Supertonic {
    /// Carga los cuatro modelos de `carpeta` (la de `~/.local/share/vortex/miku/voz`) con el estilo `estilo`
    /// (F1…F5, M1…).
    pub fn cargar(carpeta: &Path, estilo: &str, hilos: usize) -> Result<Self, Error> {
        let lib = buscar_onnxruntime().ok_or(
            "no encuentro onnxruntime (libonnxruntime.so). Reinstala vortex-miku (trae python-onnxruntime-cpu).",
        )?;
        let entorno = ort::init_from(&lib)
            .map_err(|e| format!("no pude cargar {}: {e}", lib.display()))?
            .with_name("miku-voz")
            .build()?;
        let onnx = carpeta.join("onnx");
        let cfg = leer_json(&onnx.join("tts.json"))?;
        let indice: Vec<i64> = leer_json(&onnx.join("unicode_indexer.json"))?
            .as_array()
            .ok_or("unicode_indexer.json no es una lista")?
            .iter()
            .map(|v| v.as_i64().unwrap_or(-1))
            .collect();
        let sesion = |nombre: &str| -> Result<Session, Error> {
            let s = Session::builder(&entorno)?
                .with_intra_threads(hilos)
                .map_err(|e| e.to_string())?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| e.to_string())?
                .commit_from_file(onnx.join(format!("{nombre}.onnx")))?;
            Ok(s)
        };
        let estilo = leer_json(&carpeta.join("voice_styles").join(format!("{estilo}.json")))?;
        let num = |v: &Value| v.as_i64().ok_or("tts.json incompleto");
        Ok(Self {
            duracion: sesion("duration_predictor")?,
            texto: sesion("text_encoder")?,
            vector: sesion("vector_estimator")?,
            vocoder: sesion("vocoder")?,
            indice,
            sr: num(&cfg["ae"]["sample_rate"])? as u32,
            trozo_base: num(&cfg["ae"]["base_chunk_size"])?,
            compresion: num(&cfg["ttl"]["chunk_compress_factor"])?,
            latente: num(&cfg["ttl"]["latent_dim"])?,
            estilo_ttl: tensor_estilo(&estilo["style_ttl"])?,
            estilo_dp: tensor_estilo(&estilo["style_dp"])?,
        })
    }

    fn estilo(t: &(Vec<i64>, Vec<f32>)) -> Result<Tensor<f32>, Error> {
        let dims: Vec<usize> = t.0.iter().map(|&d| d as usize).collect();
        Ok(Tensor::from_array((dims, t.1.clone()))?)
    }

    fn trozo(&mut self, frase: &str, velocidad: f64, ruido: &mut Ruido) -> Result<Vec<f32>, Error> {
        let t = texto::preparar_texto(frase);
        let ids: Vec<i64> = t
            .chars()
            .map(|c| self.indice.get(c as usize).copied().unwrap_or(-1).max(0))
            .collect();
        let largo = ids.len() as i64;
        let ids_t = || Tensor::from_array(([1usize, largo as usize], ids.clone()));
        let sal = self.duracion.run(ort::inputs![
            "text_ids" => ids_t()?,
            "style_dp" => Self::estilo(&self.estilo_dp)?,
            "text_mask" => mascara(largo, largo),
        ])?;
        let dur = primera_salida(&sal)?[0] as f64 / velocidad;
        drop(sal);
        let sal = self.texto.run(ort::inputs![
            "text_ids" => ids_t()?,
            "style_ttl" => Self::estilo(&self.estilo_ttl)?,
            "text_mask" => mascara(largo, largo),
        ])?;
        let (forma, datos) = sal[0].try_extract_tensor::<f32>()?;
        let forma_emb: Vec<usize> = forma.iter().map(|&d| d as usize).collect();
        let emb = datos.to_vec();
        drop(sal);
        let sr = self.sr as f64;
        let largo_wav = (dur * sr) as i64;
        let paso_lat = self.trozo_base * self.compresion;
        let n_lat = (largo_wav + paso_lat - 1) / paso_lat;
        let lat_validos = ((dur * sr) as i64 + paso_lat - 1) / paso_lat;
        let canales = self.latente * self.compresion;
        let mut xt: Vec<f32> = (0..canales * n_lat)
            .map(|i| {
                let r = ruido.normal() as f32;
                if i % n_lat < lat_validos { r } else { 0.0 }
            })
            .collect();
        let forma_xt = [1usize, canales as usize, n_lat as usize];
        for i in 0..PASOS {
            let sal = self.vector.run(ort::inputs![
                "noisy_latent" => Tensor::from_array((forma_xt, xt))?,
                "text_emb" => Tensor::from_array((forma_emb.clone(), emb.clone()))?,
                "style_ttl" => Self::estilo(&self.estilo_ttl)?,
                "text_mask" => mascara(largo, largo),
                "latent_mask" => mascara(lat_validos, n_lat),
                "current_step" => Tensor::from_array(([1usize], vec![i as f32]))?,
                "total_step" => Tensor::from_array(([1usize], vec![PASOS as f32]))?,
            ])?;
            xt = primera_salida(&sal)?;
        }
        let sal = self
            .vocoder
            .run(ort::inputs!["latent" => Tensor::from_array((forma_xt, xt))?])?;
        let mut wav = primera_salida(&sal)?;
        wav.truncate((dur * sr) as usize);
        Ok(wav)
    }

    /// La frase (ya sin markdown) en audio crudo a `self.sr`; los trozos van separados por un silencio corto.
    pub fn sintetizar(&mut self, frase: &str, velocidad: f64) -> Result<Vec<f32>, Error> {
        let mut ruido = Ruido::new(crc32(frase.as_bytes()) as u64);
        let velocidad = velocidad.clamp(0.7, 2.0);
        let mut salida: Vec<f32> = vec![];
        for t in texto::trozos(frase, MAX_TROZO) {
            if !salida.is_empty() {
                salida.extend(std::iter::repeat_n(
                    0.0,
                    (SILENCIO_TROZOS * self.sr as f64) as usize,
                ));
            }
            salida.extend(self.trozo(&t, velocidad, &mut ruido)?);
        }
        Ok(salida)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn crc32_como_zlib() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"Hola."), 0x2C19_66AD);
    }

    #[test]
    fn ruido_normal_y_repetible() {
        let a: Vec<f64> = {
            let mut r = Ruido::new(7);
            (0..20000).map(|_| r.normal()).collect()
        };
        let b: Vec<f64> = {
            let mut r = Ruido::new(7);
            (0..20000).map(|_| r.normal()).collect()
        };
        assert_eq!(a, b);
        let media = a.iter().sum::<f64>() / a.len() as f64;
        let var = a.iter().map(|v| (v - media).powi(2)).sum::<f64>() / a.len() as f64;
        assert!(media.abs() < 0.03 && (var - 1.0).abs() < 0.05, "{media} {var}");
    }

    #[test]
    fn estilo_anidado() {
        let v: Value = serde_json::json!({"dims": [1, 2, 2], "data": [[[1, 2], [3, 4]]]});
        assert_eq!(
            tensor_estilo(&v).unwrap(),
            (vec![1, 2, 2], vec![1.0, 2.0, 3.0, 4.0])
        );
        assert!(tensor_estilo(&serde_json::json!({"dims": [3], "data": [1]})).is_err());
    }
}
