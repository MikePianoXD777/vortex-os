//! Las cámaras y las ventanas del mosaico, y las direcciones RTSP de cada una (`camaras.py`).
//!
//! Todo se guarda en `~/.config/vortex-camera/camaras.json` con permisos 600 (solo tú lo lees), porque ahí
//! van los códigos de seguridad; es el mismo archivo que lee la de Python. Cada «ventana» es una pestaña del
//! mosaico: su división (1, 4, 9, 16, 25 o 32 cuadros) y qué cámara va en cada cuadro.
//!
//! Las Imou son Dahua por dentro: el usuario es «admin» y la contraseña, el código de la etiqueta.
//!     rtsp://admin:<código>@<ip>:554/cam/realmonitor?channel=1&subtype=0     (1 = subflujo, el ligero)
//!     rtsp://admin:<código>@<ip>:554/cam/playback?channel=1&starttime=AAAA_MM_DD_hh_mm_ss&endtime=…
//!
//! Para pruebas: `VORTEX_CAMERA_CONFIG=/otra/carpeta` cambia dónde se guarda.

use crate::fecha::FechaHora;
use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Hasta 32 cámaras por ventana.
pub const DIVISIONES: [usize; 6] = [1, 4, 9, 16, 25, 32];

pub fn carpeta_config() -> PathBuf {
    if let Some(c) = std::env::var_os("VORTEX_CAMERA_CONFIG").filter(|c| !c.is_empty()) {
        return PathBuf::from(c);
    }
    vortex_core::rutas::config().join("vortex-camera")
}

/// « 9a0 b-1234 » → «9A0B1234»: así se escribe en la etiqueta y así la devuelve la cámara.
pub fn limpiar_serie(serie: &str) -> String {
    serie
        .chars()
        .flat_map(char::to_uppercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// `quote(texto, safe="")` de Python: todo menos letras, números y `_.-~` va como %XX.
pub fn escapar(texto: &str) -> String {
    let mut s = String::new();
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// `str(valor)` de Python, para los datos que llegan de la interfaz o del archivo.
fn texto(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && n.is_f64() => format!("{f:.1}"),
            _ => n.to_string(),
        },
        otro => otro.to_string(),
    }
}

/// `int(valor)` de Python: números (los de QML llegan como 554.0) y textos con un entero.
fn entero(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.trunc() as i64)),
        Value::Bool(b) => Some(i64::from(*b)),
        Value::String(s) => s.trim().replace('_', "").parse().ok(),
        _ => None,
    }
}

/// Lo que Python toma por verdadero en un `if valor:` (ni None, ni False, ni 0, ni «»).
fn verdadero(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Camara {
    pub id: String,
    pub nombre: String,
    pub serie: String,
    /// El código de seguridad (Safety Code / SC) de la etiqueta.
    pub codigo: String,
    pub ip: String,
    pub puerto: u16,
    pub canal: u32,
    pub usuario: String,
    /// Cómo se mueve: «onvif», «cgi», «no» o «» (sin probar).
    pub ptz: String,
}

impl Camara {
    /// De un objeto JSON (el archivo o lo que manda la interfaz), aguantando datos raros como la de Python.
    pub fn desde(datos: &Map<String, Value>) -> Camara {
        let campo = |k: &str| datos.get(k).map(texto);
        let nombre = campo("nombre").unwrap_or_default();
        let usuario = campo("usuario").unwrap_or_else(|| "admin".into());
        Camara {
            id: campo("id").unwrap_or_default(),
            nombre: match nombre.trim() {
                "" => "Cámara".into(),
                n => n.into(),
            },
            serie: limpiar_serie(&campo("serie").unwrap_or_default()),
            codigo: campo("codigo").unwrap_or_default().trim().into(),
            ip: campo("ip").unwrap_or_default().trim().into(),
            puerto: match datos.get("puerto").map_or(Some(554), entero) {
                Some(p) if 0 < p && p < 65536 => p as u16,
                _ => 554,
            },
            canal: datos
                .get("canal")
                .map_or(Some(1), entero)
                .map_or(1, |c| c.clamp(1, i64::from(u32::MAX)) as u32),
            usuario: match usuario.trim() {
                "" => "admin".into(),
                u => u.into(),
            },
            ptz: campo("ptz").unwrap_or_default(),
        }
    }

    /// Todos los campos, como el `asdict` que se guarda en el archivo.
    pub fn a_json(&self) -> Map<String, Value> {
        let Value::Object(m) = json!({
            "id": self.id, "nombre": self.nombre, "serie": self.serie, "codigo": self.codigo,
            "ip": self.ip, "puerto": self.puerto, "canal": self.canal, "usuario": self.usuario,
            "ptz": self.ptz,
        }) else {
            unreachable!()
        };
        m
    }

    fn base(&self) -> String {
        let cred = if self.codigo.is_empty() {
            String::new()
        } else {
            format!("{}:{}@", escapar(&self.usuario), escapar(&self.codigo))
        };
        let host = if self.ip.contains(':') {
            format!("[{}]", self.ip)
        } else {
            self.ip.clone()
        };
        format!("rtsp://{cred}{host}:{}", self.puerto)
    }

    /// El video en vivo. El subflujo (`principal = false`) es el ligero, para el mosaico.
    pub fn url_vivo(&self, principal: bool) -> String {
        if self.ip.is_empty() {
            return String::new();
        }
        format!(
            "{}/cam/realmonitor?channel={}&subtype={}",
            self.base(),
            self.canal,
            if principal { 0 } else { 1 }
        )
    }

    /// Lo que la cámara guardó en su microSD, de `inicio` a `fin` (hora de la cámara).
    pub fn url_grabado(&self, inicio: &FechaHora, fin: &FechaHora) -> String {
        if self.ip.is_empty() {
            return String::new();
        }
        format!(
            "{}/cam/playback?channel={}&starttime={}&endtime={}",
            self.base(),
            self.canal,
            inicio.para_dahua(),
            fin.para_dahua()
        )
    }

    /// Para la interfaz: todo menos el código (QML pide las URL al puente cuando las necesita).
    pub fn publica(&self) -> Value {
        let mut m = self.a_json();
        m.remove("codigo");
        m.insert("tieneCodigo".into(), Value::Bool(!self.codigo.is_empty()));
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ventana {
    pub nombre: String,
    pub division: usize,
    pub celdas: Vec<String>,
}

impl Ventana {
    fn nueva(n: usize) -> Ventana {
        Ventana {
            nombre: format!("Ventana {n}"),
            division: 4,
            celdas: vec![String::new(); 4],
        }
    }

    fn rellenar(&mut self) {
        if self.celdas.len() < self.division {
            self.celdas.resize(self.division, String::new());
        }
    }

    fn a_json(&self, solo_visibles: bool) -> Value {
        let celdas = if solo_visibles {
            &self.celdas[..self.division.min(self.celdas.len())]
        } else {
            &self.celdas[..]
        };
        json!({"nombre": self.nombre, "division": self.division, "celdas": celdas})
    }
}

/// Las cámaras y las ventanas, en memoria y en el archivo.
#[derive(Debug, Clone)]
pub struct Almacen {
    pub ruta: PathBuf,
    pub camaras: Vec<Camara>,
    pub ventanas: Vec<Ventana>,
}

impl Almacen {
    pub fn new(carpeta: Option<&Path>) -> Almacen {
        let carpeta = carpeta.map_or_else(carpeta_config, Path::to_path_buf);
        let mut a = Almacen {
            ruta: carpeta.join("camaras.json"),
            camaras: Vec::new(),
            ventanas: Vec::new(),
        };
        a.cargar();
        a
    }

    // --- archivo ---

    pub fn cargar(&mut self) {
        let datos: Value = std::fs::read_to_string(&self.ruta)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let vacia = Vec::new();
        let lista = |k: &str| datos.get(k).and_then(Value::as_array).unwrap_or(&vacia);
        self.camaras = lista("camaras")
            .iter()
            .filter_map(Value::as_object)
            .filter(|c| c.get("id").is_some_and(verdadero))
            .map(Camara::desde)
            .collect();
        let ids: Vec<&str> = self.camaras.iter().map(|c| c.id.as_str()).collect();
        self.ventanas = lista("ventanas")
            .iter()
            .filter_map(Value::as_object)
            .map(|v| {
                let division = v
                    .get("division")
                    .and_then(Value::as_u64)
                    .map(|d| d as usize)
                    .filter(|d| DIVISIONES.contains(d))
                    .unwrap_or(4);
                let celdas = v
                    .get("celdas")
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .map(|x| match x.as_str() {
                                Some(s) if ids.contains(&s) => s.to_string(),
                                _ => String::new(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let nombre = match v.get("nombre") {
                    Some(n) if verdadero(n) => texto(n),
                    _ => "Ventana".into(),
                };
                Ventana {
                    nombre,
                    division,
                    celdas,
                }
            })
            .collect();
        if self.ventanas.is_empty() {
            self.ventanas.push(Ventana::nueva(1));
        }
        for v in &mut self.ventanas {
            v.rellenar();
        }
    }

    /// Escribe completo en un archivo temporal (600) y lo cambia por el bueno: nunca queda a medias.
    pub fn guardar(&self) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let carpeta = self.ruta.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(carpeta)?;
        std::fs::set_permissions(carpeta, std::fs::Permissions::from_mode(0o700))?;
        let datos = json!({
            "version": 1,
            "camaras": self.camaras.iter().map(|c| Value::Object(c.a_json())).collect::<Vec<_>>(),
            "ventanas": self.ventanas.iter().map(|v| v.a_json(false)).collect::<Vec<_>>(),
        });
        let mut tmp = tempfile::Builder::new()
            .prefix(".camaras-")
            .tempfile_in(carpeta)?;
        tmp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        tmp.write_all(serde_json::to_string_pretty(&datos)?.as_bytes())?;
        tmp.persist(&self.ruta).map_err(|e| e.error)?;
        Ok(())
    }

    fn guardar_o_avisar(&self) {
        if let Err(e) = self.guardar() {
            vortex_core::registro::error(&format!("No se pudo guardar camaras.json: {e}"));
        }
    }

    // --- cámaras ---

    pub fn camara(&self, ident: &str) -> Option<&Camara> {
        self.camaras.iter().find(|c| c.id == ident)
    }

    pub fn por_serie(&self, serie: &str) -> Option<&Camara> {
        let serie = limpiar_serie(serie);
        if serie.is_empty() {
            return None;
        }
        self.camaras.iter().find(|c| c.serie == serie)
    }

    /// La agrega y la pone en el primer cuadro libre: si la ventana se llena, crece (hasta 32 cuadros) y,
    /// si ya no puede, se abre otra ventana.
    pub fn agregar(&mut self, datos: &Map<String, Value>) -> Camara {
        let mut d = datos.clone();
        let id: String = uuid::Uuid::new_v4().simple().to_string()[..10].into();
        d.insert("id".into(), Value::String(id));
        let c = Camara::desde(&d);
        self.camaras.push(c.clone());
        self.acomodar(&c.id);
        self.guardar_o_avisar();
        c
    }

    pub fn actualizar(&mut self, ident: &str, datos: &Map<String, Value>) -> Option<Camara> {
        let i = self.camaras.iter().position(|c| c.id == ident)?;
        let vieja = &self.camaras[i];
        let mut d = vieja.a_json();
        for (k, v) in datos {
            d.insert(k.clone(), v.clone());
        }
        d.insert("id".into(), Value::String(vieja.id.clone()));
        let mut nueva = Camara::desde(&d);
        if (&nueva.ip, nueva.puerto, &nueva.codigo, &nueva.usuario)
            != (&vieja.ip, vieja.puerto, &vieja.codigo, &vieja.usuario)
        {
            nueva.ptz = String::new(); // otra cámara u otra contraseña: se vuelve a probar cómo se mueve
        }
        self.camaras[i] = nueva.clone();
        self.guardar_o_avisar();
        Some(nueva)
    }

    pub fn quitar(&mut self, ident: &str) {
        self.camaras.retain(|c| c.id != ident);
        for v in &mut self.ventanas {
            for x in &mut v.celdas {
                if x == ident {
                    x.clear();
                }
            }
        }
        self.guardar_o_avisar();
    }

    // --- ventanas ---

    fn acomodar(&mut self, ident: &str) {
        if self.ventanas.iter().any(|v| {
            v.celdas[..v.division.min(v.celdas.len())]
                .iter()
                .any(|x| x == ident)
        }) {
            return;
        }
        for v in &mut self.ventanas {
            loop {
                v.rellenar();
                if let Some(p) = v.celdas[..v.division].iter().position(String::is_empty) {
                    v.celdas[p] = ident.to_string();
                    return;
                }
                match DIVISIONES.iter().find(|&&d| d > v.division) {
                    Some(&mas) => v.division = mas,
                    None => break,
                }
            }
        }
        let mut v = Ventana::nueva(self.ventanas.len() + 1);
        v.celdas[0] = ident.to_string();
        self.ventanas.push(v);
    }

    pub fn nueva_ventana(&mut self) -> usize {
        self.ventanas.push(Ventana::nueva(self.ventanas.len() + 1));
        self.guardar_o_avisar();
        self.ventanas.len() - 1
    }

    pub fn quitar_ventana(&mut self, i: usize) {
        if self.ventanas.len() > 1 && i < self.ventanas.len() {
            self.ventanas.remove(i);
            self.guardar_o_avisar();
        }
    }

    pub fn renombrar_ventana(&mut self, i: usize, nombre: &str) {
        let nombre = nombre.trim();
        if i < self.ventanas.len() && !nombre.is_empty() {
            self.ventanas[i].nombre = nombre.chars().take(40).collect();
            self.guardar_o_avisar();
        }
    }

    pub fn poner_division(&mut self, i: usize, division: usize) {
        if i < self.ventanas.len() && DIVISIONES.contains(&division) {
            self.ventanas[i].division = division;
            self.ventanas[i].rellenar();
            self.guardar_o_avisar();
        }
    }

    /// Pone la cámara en el cuadro `pos` («» lo vacía). Si ya estaba en otro cuadro de la misma ventana, se
    /// intercambian: así no sale dos veces.
    pub fn poner_celda(&mut self, i: usize, pos: usize, ident: &str) {
        if i >= self.ventanas.len() || (!ident.is_empty() && self.camara(ident).is_none()) {
            return;
        }
        let v = &mut self.ventanas[i];
        if pos >= v.division {
            return;
        }
        v.rellenar();
        if !ident.is_empty()
            && let Some(antes) = v.celdas.iter().position(|x| x == ident)
        {
            v.celdas[antes] = v.celdas[pos].clone();
        }
        v.celdas[pos] = ident.to_string();
        self.guardar_o_avisar();
    }

    /// Las ventanas para la interfaz: cada una con solo los cuadros que se ven.
    pub fn ventanas_publicas(&self) -> Value {
        Value::Array(self.ventanas.iter().map(|v| v.a_json(true)).collect())
    }

    pub fn camaras_publicas(&self) -> Value {
        Value::Array(self.camaras.iter().map(Camara::publica).collect())
    }
}
