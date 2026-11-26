//! Cómo se ve: los cuadros de cada animación, ya escalados y al espejo.
//!
//! - De fábrica, el diseño original de Vortexine's (`arte`), a escala 3.
//! - Si en `~/.local/share/vortex/mascota/` hay un paquete en formato Shimeji (`shime1.png` …, sueltos o
//!   en `img/`), se usa ese: así cada quien pone el personaje que quiera, con sus propias imágenes. El
//!   repo no trae ninguno (las imágenes de personajes con derechos nunca van a git). Las imágenes
//!   Shimeji miran a la izquierda y pisan en el centro de la orilla de abajo.
//!
//! Un `mascota.json` opcional en esa carpeta cambia la escala o qué imágenes usa cada animación:
//! `{"escala": 1.5, "acciones": {"sentada": [11], "caminando": [1, 2, 1, 3]}}`.
//!
//! - O un paquete de tiras: `animations.json` (cuántos cuadros y cuánto dura cada uno) y una tira por
//!   animación en `spritesheets_128/` (o `spritesheets_32/`, que se agranda 4 veces): `idle.png`,
//!   `walk_side.png`, `sit.png`… Los cuadros son cuadrados, miran de frente y pisan en la fila de abajo;
//!   `walk_side`, `run_side` y `point` miran a la derecha. Ver `TABLA` y `OCIOS` para qué usa cada cosa.

use crate::arte::{self, Cara, Pose};
use crate::mundo::Anim;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Una imagen lista para copiar a la ventana: ARGB premultiplicado.
#[derive(Clone, Debug)]
pub struct Imagen {
    pub ancho: usize,
    pub alto: usize,
    pub px: Vec<u32>,
    /// Dónde pisa, dentro de la imagen.
    pub ancla_x: usize,
    pub ancla_y: usize,
    /// Lo que no es transparente: (x0, y0, x1, y1).
    pub caja: (usize, usize, usize, usize),
}

impl Imagen {
    fn nueva(ancho: usize, alto: usize, px: Vec<u32>, ancla_x: usize, ancla_y: usize) -> Self {
        let mut caja = (ancho, alto, 0, 0);
        for y in 0..alto {
            for x in 0..ancho {
                if px[y * ancho + x] >> 24 != 0 {
                    caja = (caja.0.min(x), caja.1.min(y), caja.2.max(x + 1), caja.3.max(y + 1));
                }
            }
        }
        if caja.2 < caja.0 {
            caja = (0, 0, 0, 0);
        }
        Self {
            ancho,
            alto,
            px,
            ancla_x,
            ancla_y,
            caja,
        }
    }

    fn espejo(&self) -> Self {
        let mut px = vec![0; self.px.len()];
        for y in 0..self.alto {
            for x in 0..self.ancho {
                px[y * self.ancho + x] = self.px[y * self.ancho + (self.ancho - 1 - x)];
            }
        }
        Self::nueva(self.ancho, self.alto, px, self.ancho - self.ancla_x, self.ancla_y)
    }

    /// Escala al vecino más cercano (pixel art nítido).
    fn escalar(&self, f: f32) -> Self {
        if (f - 1.0).abs() < 0.01 {
            return self.clone();
        }
        let (w, h) = (
            ((self.ancho as f32) * f).round().max(1.0) as usize,
            ((self.alto as f32) * f).round().max(1.0) as usize,
        );
        let mut px = vec![0; w * h];
        for y in 0..h {
            let sy = ((y as f32) / f) as usize;
            for x in 0..w {
                let sx = ((x as f32) / f) as usize;
                px[y * w + x] = self.px[sy.min(self.alto - 1) * self.ancho + sx.min(self.ancho - 1)];
            }
        }
        Self::nueva(
            w,
            h,
            px,
            (self.ancla_x as f32 * f) as usize,
            (self.ancla_y as f32 * f) as usize,
        )
    }
}

fn premultiplicar(argb: u32) -> u32 {
    let a = argb >> 24;
    if a == 255 || a == 0 {
        return if a == 0 { 0 } else { argb };
    }
    let c = |s: u32| (((argb >> s) & 0xFF) * a / 255) << s;
    (a << 24) | c(16) | c(8) | c(0)
}

/// Hacia dónde miran las imágenes de una animación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mira {
    /// De frente: nunca van al espejo.
    Frente,
    /// A la izquierda (Shimeji): a la derecha van al espejo.
    Izquierda,
    /// A la derecha: a la izquierda van al espejo.
    Derecha,
}

struct Secuencia {
    /// (cuadro, segundos)
    cuadros: Vec<(usize, f32)>,
    mira: Mira,
}

pub struct Aspecto {
    /// Cada cuadro dos veces: [normal, al espejo].
    cuadros: Vec<[Imagen; 2]>,
    anims: HashMap<Anim, Secuencia>,
    /// Cuántos pasatiempos (`Anim::Ocio`) tiene.
    ocios: u8,
    /// Animaciones que ya traen su dibujito (la zeta al dormir, los corazones): no se pone otro encima.
    con_emote: HashSet<Anim>,
    pub nombre: String,
}

impl Aspecto {
    pub fn cuadro(&self, anim: Anim, t: f32, dir: i8) -> &Imagen {
        let respaldo = match anim {
            Anim::Caricia | Anim::Despertando | Anim::Despedida => Some(Anim::Saludando),
            _ => None,
        };
        let secuencia = self
            .anims
            .get(&anim)
            .or_else(|| respaldo.and_then(|r| self.anims.get(&r)))
            .or_else(|| self.anims.get(&Anim::Parada))
            .expect("siempre hay Parada");
        let total: f32 = secuencia.cuadros.iter().map(|(_, d)| d).sum();
        let mut resto = if total > 0.0 { t % total } else { 0.0 };
        let mut i = secuencia.cuadros[0].0;
        for (c, d) in &secuencia.cuadros {
            i = *c;
            if resto < *d {
                break;
            }
            resto -= d;
        }
        let espejo = match secuencia.mira {
            Mira::Frente => false,
            Mira::Izquierda => dir > 0,
            Mira::Derecha => dir < 0,
        };
        &self.cuadros[i][usize::from(espejo)]
    }

    /// Cuántos pasatiempos tiene (para que la compañera elija uno).
    pub fn ocios(&self) -> u8 {
        self.ocios
    }

    /// Una hoja con lo que usa cada animación (una fila cada una, hasta 10 cuadros), para revisar un
    /// paquete sin abrir ventanas. Fondo gris para ver lo transparente.
    pub fn hoja(&self) -> (usize, usize, Vec<u32>) {
        let mut filas: Vec<(Anim, i8)> = vec![
            (Anim::Parada, 1),
            (Anim::Caminando, 1),
            (Anim::Caminando, -1),
            (Anim::Sentada, 1),
            (Anim::Dormida, 1),
            (Anim::Saludando, 1),
            (Anim::Caricia, 1),
            (Anim::Despertando, 1),
            (Anim::Despedida, 1),
            (Anim::Agarrada, 1),
            (Anim::Cayendo, 1),
            (Anim::Aterrizando, 1),
        ];
        filas.extend((0..self.ocios).map(|i| (Anim::Ocio(i), 1)));
        let (cw, ch) = self
            .cuadros
            .iter()
            .fold((1, 1), |(w, h), [c, _]| (w.max(c.ancho), h.max(c.alto)));
        let (cw, ch) = (cw + 4, ch + 4);
        let columnas = 10;
        let (w, h) = (cw * columnas, ch * filas.len());
        let mut px = vec![0xFF5A_5A5A; w * h];
        for (f, (anim, dir)) in filas.into_iter().enumerate() {
            let Some(sec) = self.anims.get(&anim) else {
                continue;
            };
            let espejo = match sec.mira {
                Mira::Frente => 0,
                Mira::Izquierda => usize::from(dir > 0),
                Mira::Derecha => usize::from(dir < 0),
            };
            let mut vistos = Vec::new();
            for &(i, _) in &sec.cuadros {
                if !vistos.contains(&i) {
                    vistos.push(i);
                }
            }
            for (k, i) in vistos.into_iter().take(columnas).enumerate() {
                let c = &self.cuadros[i][espejo];
                let (ox, oy) = (k * cw + 2, f * ch + 2 + (ch - 4 - c.alto));
                for y in 0..c.alto {
                    for x in 0..c.ancho {
                        let p = c.px[y * c.ancho + x];
                        if p >> 24 != 0 {
                            px[(oy + y) * w + ox + x] = p | 0xFF00_0000;
                        }
                    }
                }
            }
        }
        (w, h, px)
    }

    /// El dibujito que va encima, salvo que la animación ya traiga el suyo.
    pub fn emote(&self, anim: Anim, e: Option<arte::Emote>) -> Option<arte::Emote> {
        e.filter(|_| !self.con_emote.contains(&anim))
    }

    /// El más ancho y el más alto de todos (la ventana tiene que caber cualquiera).
    pub fn tamano_maximo(&self) -> (usize, usize, usize) {
        let mut m = (0, 0, 0);
        for [c, _] in &self.cuadros {
            m.0 = m.0.max(c.ancho.max(2 * c.ancla_x.max(c.ancho - c.ancla_x)));
            m.1 = m.1.max(c.ancla_y);
            m.2 = m.2.max(c.alto.saturating_sub(c.ancla_y));
        }
        m // (ancho, alto arriba del piso, alto abajo del piso)
    }

    /// El diseño original de Vortexine's.
    pub fn original(escala: f32) -> Self {
        let mut cuadros = Vec::new();
        let mut agregar = |p: Pose, c: Cara| {
            let q = arte::cuadro(p, c);
            let img =
                Imagen::nueva(arte::ANCHO, arte::ALTO, q.px, arte::ANCHO / 2, arte::PISO + 1).escalar(escala);
            let esp = img.espejo();
            cuadros.push([img, esp]);
            cuadros.len() - 1
        };
        let parada = agregar(Pose::Parada, Cara::Normal);
        let parpadeo = agregar(Pose::Parada, Cara::Parpadeo);
        let pasos: Vec<usize> = (0..4)
            .map(|t| agregar(Pose::Caminando(t), Cara::Normal))
            .collect();
        let sentada0 = agregar(Pose::Sentada(0), Cara::Normal);
        let sentada1 = agregar(Pose::Sentada(1), Cara::Normal);
        let sentada_parpadeo = agregar(Pose::Sentada(0), Cara::Parpadeo);
        let dormida = agregar(Pose::Sentada(0), Cara::Dormida);
        let saludo = agregar(Pose::Saludando, Cara::Feliz);
        let saludo_habla = agregar(Pose::Saludando, Cara::Hablando);
        let feliz = agregar(Pose::Parada, Cara::Feliz);
        let colgando = agregar(Pose::Colgando, Cara::Sorpresa);
        let mut anims = HashMap::new();
        let mut poner = |a: Anim, cuadros: Vec<(usize, f32)>| {
            anims.insert(
                a,
                Secuencia {
                    cuadros,
                    mira: Mira::Frente,
                },
            );
        };
        poner(Anim::Parada, vec![(parada, 3.2), (parpadeo, 0.15)]);
        poner(Anim::Caminando, pasos.iter().map(|&p| (p, 0.16)).collect());
        poner(
            Anim::Sentada,
            vec![
                (sentada0, 0.6),
                (sentada1, 0.6),
                (sentada0, 0.6),
                (sentada1, 0.6),
                (sentada_parpadeo, 0.15),
            ],
        );
        poner(Anim::Dormida, vec![(dormida, 1.0)]);
        poner(
            Anim::Saludando,
            vec![(saludo, 0.3), (saludo_habla, 0.2), (feliz, 0.3)],
        );
        poner(Anim::Agarrada, vec![(colgando, 1.0)]);
        poner(Anim::Cayendo, vec![(colgando, 1.0)]);
        poner(Anim::Aterrizando, vec![(parpadeo, 1.0)]);
        Self {
            cuadros,
            anims,
            ocios: 0,
            con_emote: HashSet::new(),
            nombre: "original".into(),
        }
    }

    /// Un paquete Shimeji de la carpeta (o de su `img/`). None si no hay `shime1.png`.
    pub fn shimeji(carpeta: &Path) -> Result<Option<Self>, String> {
        let Some(img) = [carpeta.join("img"), carpeta.to_path_buf()]
            .into_iter()
            .find(|d| d.join("shime1.png").is_file())
        else {
            return Ok(None);
        };
        let ajustes: serde_json::Value = std::fs::read_to_string(carpeta.join("mascota.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(serde_json::Value::Null);
        let escala = ajustes["escala"]
            .as_f64()
            .map_or(1.0, |e| e.clamp(0.25, 4.0) as f32);

        // Las de siempre de Shimeji: 1 parada, 2 y 3 los pasos, 4 cayendo, 5 agarrada, 11 sentada,
        // 18 y 19 al aterrizar, 21 acostada, 31 a 33 sentada en una orilla con las piernas colgando.
        type Lista = &'static [(u32, f32)];
        let de_fabrica: [(Anim, &str, Lista); 8] = [
            (Anim::Parada, "parada", &[(1, 1.0)]),
            (
                Anim::Caminando,
                "caminando",
                &[(1, 0.16), (2, 0.16), (1, 0.16), (3, 0.16)],
            ),
            (
                Anim::Sentada,
                "sentada",
                &[(31, 0.45), (32, 0.45), (31, 0.45), (33, 0.45)],
            ),
            (Anim::Dormida, "dormida", &[(21, 1.0)]),
            (Anim::Saludando, "saludando", &[(1, 1.0)]),
            (Anim::Agarrada, "agarrada", &[(5, 1.0)]),
            (Anim::Cayendo, "cayendo", &[(4, 1.0)]),
            (Anim::Aterrizando, "aterrizando", &[(18, 0.15), (19, 0.2)]),
        ];
        let mut cargadas: HashMap<u32, usize> = HashMap::new();
        let mut cuadros = Vec::new();
        let mut cargar = |n: u32| -> Result<Option<usize>, String> {
            if let Some(&i) = cargadas.get(&n) {
                return Ok(Some(i));
            }
            let ruta = img.join(format!("shime{n}.png"));
            if !ruta.is_file() {
                return Ok(None);
            }
            let i = leer_png(&ruta)?;
            let img = Imagen::nueva(i.0, i.1, i.2, i.0 / 2, i.1).escalar(escala);
            let esp = img.espejo();
            cuadros.push([img, esp]);
            cargadas.insert(n, cuadros.len() - 1);
            Ok(Some(cuadros.len() - 1))
        };
        let Some(parada) = cargar(1)? else { return Ok(None) };
        let mut anims = HashMap::new();
        for (anim, nombre, fabrica) in de_fabrica {
            let propia: Option<Vec<(u32, f32)>> = ajustes["acciones"][nombre].as_array().map(|a| {
                let paso = if anim == Anim::Caminando { 0.16 } else { 0.4 };
                a.iter()
                    .filter_map(|n| n.as_u64())
                    .map(|n| (n as u32, paso))
                    .collect()
            });
            let lista = propia.unwrap_or_else(|| fabrica.to_vec());
            let mut secuencia = Vec::new();
            for (n, d) in lista {
                if let Some(i) = cargar(n)? {
                    secuencia.push((i, d));
                }
            }
            // Sin las de colgar las piernas, la de sentada (11); sin ninguna, parada.
            if secuencia.is_empty()
                && anim == Anim::Sentada
                && let Some(i) = cargar(11)?
            {
                secuencia.push((i, 1.0));
            }
            if secuencia.is_empty() {
                secuencia.push((parada, 1.0));
            }
            anims.insert(
                anim,
                Secuencia {
                    cuadros: secuencia,
                    mira: Mira::Izquierda,
                },
            );
        }
        let nombre = carpeta
            .file_name()
            .map_or("propia".into(), |n| n.to_string_lossy().into_owned());
        Ok(Some(Self {
            cuadros,
            anims,
            ocios: 0,
            con_emote: HashSet::new(),
            nombre,
        }))
    }

    /// Un paquete de tiras (`animations.json` + `spritesheets_128/`). None si no hay `animations.json`.
    pub fn tiras(carpeta: &Path) -> Result<Option<Self>, String> {
        let Ok(texto) = std::fs::read_to_string(carpeta.join("animations.json")) else {
            return Ok(None);
        };
        let info: serde_json::Value =
            serde_json::from_str(&texto).map_err(|e| format!("animations.json: {e}"))?;
        let (dir_tiras, agrandar) = carpeta_de_tiras(carpeta)
            .ok_or_else(|| "no hay carpeta spritesheets_128/ ni spritesheets_32/".to_string())?;
        let ajustes: serde_json::Value = std::fs::read_to_string(carpeta.join("mascota.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(serde_json::Value::Null);
        let escala = ajustes["escala"]
            .as_f64()
            .map_or(1.0, |e| e.clamp(0.25, 4.0) as f32)
            * agrandar;

        let mut cuadros = Vec::new();
        // Cada tira se lee una vez: nombre → [(cuadro, segundos)].
        let mut leidas: HashMap<&str, Option<Vec<(usize, f32)>>> = HashMap::new();
        let mut leer = |nombre: &'static str| -> Result<Option<Vec<(usize, f32)>>, String> {
            if let Some(l) = leidas.get(nombre) {
                return Ok(l.clone());
            }
            let anim = &info["animations"][nombre];
            let ruta = dir_tiras.join(format!("{nombre}.png"));
            let lista = if anim.is_object() && ruta.is_file() {
                let (w, h, px) = leer_png(&ruta)?;
                let n = anim["frames"].as_u64().unwrap_or(1).max(1) as usize;
                let cw = w / n;
                if cw == 0 {
                    return Err(format!("{}: más cuadros que pixeles", ruta.display()));
                }
                let duraciones = anim["durations_ms"].as_array();
                let mut l = Vec::new();
                for k in 0..n {
                    let mut c = vec![0; cw * h];
                    for y in 0..h {
                        c[y * cw..(y + 1) * cw].copy_from_slice(&px[y * w + k * cw..y * w + (k + 1) * cw]);
                    }
                    let img = Imagen::nueva(cw, h, c, cw / 2, h).escalar(escala);
                    let esp = img.espejo();
                    cuadros.push([img, esp]);
                    let ms = duraciones
                        .and_then(|d| d.get(k))
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(200.0);
                    l.push((cuadros.len() - 1, (ms as f32 / 1000.0).max(0.02)));
                }
                Some(l)
            } else {
                None
            };
            leidas.insert(nombre, lista.clone());
            Ok(lista)
        };

        let mut anims = HashMap::new();
        for (anim, partes) in TABLA {
            let mut secuencia = Vec::new();
            let mut mira = None;
            for &(nombre, desde, veces) in partes {
                let Some(l) = leer(nombre)? else { continue };
                mira.get_or_insert(mira_de(nombre));
                for _ in 0..veces {
                    secuencia.extend(l.iter().skip(desde).copied());
                }
            }
            if !secuencia.is_empty() {
                anims.insert(
                    anim,
                    Secuencia {
                        cuadros: secuencia,
                        mira: mira.unwrap_or(Mira::Frente),
                    },
                );
            }
        }
        if !anims.contains_key(&Anim::Parada) {
            return Err("el paquete no trae idle.png".into());
        }
        let mut ocios = 0u8;
        for nombre in OCIOS {
            if let Some(l) = leer(nombre)? {
                anims.insert(
                    Anim::Ocio(ocios),
                    Secuencia {
                        cuadros: l,
                        mira: mira_de(nombre),
                    },
                );
                ocios += 1;
            }
        }
        // La de dormir ya trae su zeta y la caricia sus corazones.
        let con_emote = [Anim::Dormida, Anim::Caricia]
            .into_iter()
            .filter(|a| anims.contains_key(a))
            .collect();
        let nombre = carpeta
            .file_name()
            .map_or("propia".into(), |n| n.to_string_lossy().into_owned());
        Ok(Some(Self {
            cuadros,
            anims,
            ocios,
            con_emote,
            nombre,
        }))
    }
}

/// Qué tiras usa cada animación: (tira, desde qué cuadro, cuántas veces). Las que falten se saltan.
type Partes = &'static [(&'static str, usize, u8)];
const TABLA: [(Anim, Partes); 11] = [
    (Anim::Parada, &[("idle", 0, 3), ("blink", 0, 1)]),
    (Anim::Caminando, &[("walk_side", 0, 1)]),
    (Anim::Sentada, &[("sit", 0, 2), ("sit_swing", 0, 4)]),
    (Anim::Dormida, &[("sleep_sit", 0, 1)]),
    (Anim::Saludando, &[("wave", 0, 2)]),
    (Anim::Caricia, &[("headpat", 0, 2)]),
    (Anim::Despertando, &[("stretch", 0, 1), ("wave", 0, 1)]),
    (Anim::Despedida, &[("wave_both", 0, 3)]),
    (Anim::Agarrada, &[("drag", 0, 1)]),
    (Anim::Cayendo, &[("fall", 0, 1)]),
    // Los últimos cuadros del salto: cuando cae.
    (Anim::Aterrizando, &[("jump", 3, 1)]),
];

/// Los pasatiempos que hace de vez en cuando, si el paquete los trae.
const OCIOS: [&str; 12] = [
    "look_around",
    "bored",
    "think",
    "sing",
    "dance",
    "phone",
    "coffee",
    "read",
    "gaming",
    "typing",
    "nod",
    "wink",
];

fn mira_de(tira: &str) -> Mira {
    if matches!(tira, "walk_side" | "run_side" | "point") {
        Mira::Derecha
    } else {
        Mira::Frente
    }
}

/// La carpeta con las tiras y cuánto hay que agrandarlas.
pub fn carpeta_de_tiras(carpeta: &Path) -> Option<(PathBuf, f32)> {
    [("spritesheets_128", 1.0), ("spritesheets_32", 4.0)]
        .into_iter()
        .map(|(d, f)| (carpeta.join(d), f))
        .find(|(d, _)| d.is_dir())
}

/// La carpeta de la mascota propia.
pub fn carpeta() -> PathBuf {
    let datos = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    datos.join("vortex/mascota")
}

/// La compañera de fábrica (4.5.1): la Miku de Mike y Ameyali, que trae el paquete vortex-rust.
pub fn fabrica() -> PathBuf {
    std::env::var_os("VORTEX_MASCOTA_FABRICA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/vortex/mascota/miku"))
}

/// La marca de «vortex mascota original»: dentro de la carpeta propia, así «usar» la borra sola.
pub const MARCA_ORIGINAL: &str = "usar-original";

/// La propia si hay una que sirva; si no, la Miku de fábrica; si tampoco (o con «original»), la original.
pub fn cargar(escala_original: f32) -> (Aspecto, Option<String>) {
    cargar_de(&carpeta(), &fabrica(), escala_original)
}

pub fn cargar_de(propia: &Path, fabrica: &Path, escala_original: f32) -> (Aspecto, Option<String>) {
    if propia.join(MARCA_ORIGINAL).exists() {
        return (Aspecto::original(escala_original), None);
    }
    let leida = match Aspecto::tiras(propia) {
        Ok(None) => Aspecto::shimeji(propia),
        otro => otro,
    };
    let error = match leida {
        Ok(Some(a)) => return (a, None),
        Ok(None) => None,
        Err(e) => Some(e),
    };
    match Aspecto::tiras(fabrica) {
        Ok(Some(a)) => (a, error),
        _ => (Aspecto::original(escala_original), error),
    }
}

/// Lee un PNG a (ancho, alto, ARGB premultiplicado).
pub fn leer_png(ruta: &Path) -> Result<(usize, usize, Vec<u32>), String> {
    let error = |e: &dyn std::fmt::Display| format!("{}: {e}", ruta.display());
    let archivo = std::fs::File::open(ruta).map_err(|e| error(&e))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(archivo));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut lector = dec.read_info().map_err(|e| error(&e))?;
    let (w, h) = {
        let i = lector.info();
        (i.width as usize, i.height as usize)
    };
    if w == 0 || h == 0 || w > 8192 || h > 2048 {
        return Err(error(&"la imagen tiene que medir hasta 8192 × 2048 pixeles"));
    }
    let mut buf = vec![0; lector.output_buffer_size()];
    let info = lector.next_frame(&mut buf).map_err(|e| error(&e))?;
    let bytes = &buf[..info.buffer_size()];
    let canales = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return Err(error(&"PNG con paleta sin expandir")),
    };
    let mut px = Vec::with_capacity(w * h);
    for p in bytes.chunks_exact(canales).take(w * h) {
        let (r, g, b, a) = match canales {
            4 => (p[0], p[1], p[2], p[3]),
            3 => (p[0], p[1], p[2], 255),
            2 => (p[0], p[0], p[0], p[1]),
            _ => (p[0], p[0], p[0], 255),
        };
        px.push(premultiplicar(
            u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        ));
    }
    Ok((w, h, px))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_original_tiene_todas_las_animaciones() {
        let a = Aspecto::original(3.0);
        for anim in [
            Anim::Parada,
            Anim::Caminando,
            Anim::Sentada,
            Anim::Dormida,
            Anim::Saludando,
            Anim::Agarrada,
            Anim::Cayendo,
            Anim::Aterrizando,
        ] {
            let c = a.cuadro(anim, 0.0, 1);
            assert_eq!((c.ancho, c.alto), (96, 144));
            assert_eq!(c.ancla_y, (arte::PISO + 1) * 3);
        }
        // Caminando cambia de cuadro con el tiempo.
        assert_ne!(
            a.cuadro(Anim::Caminando, 0.0, 1).px,
            a.cuadro(Anim::Caminando, 0.2, 1).px
        );
    }

    #[test]
    fn de_fabrica_sale_miku() {
        let miku = Path::new(env!("CARGO_MANIFEST_DIR")).join("miku");
        let t = std::env::temp_dir().join(format!("vortex-mascota-fabrica-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        // Sin paquete propio: la Miku de fábrica, con sus animaciones de ocio.
        let (a, e) = cargar_de(&t, &miku, 3.0);
        assert_eq!((a.nombre.as_str(), e), ("miku", None));
        assert!(a.ocios > 0);
        // Con «vortex mascota original»: la original.
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join(MARCA_ORIGINAL), "").unwrap();
        assert_eq!(cargar_de(&t, &miku, 3.0).0.nombre, "original");
        // Sin la de fábrica instalada: la original.
        std::fs::remove_file(t.join(MARCA_ORIGINAL)).unwrap();
        assert_eq!(cargar_de(&t, &t.join("nada"), 3.0).0.nombre, "original");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn premultiplica() {
        assert_eq!(premultiplicar(0x80FF_0000), 0x8080_0000);
        assert_eq!(premultiplicar(0x00FF_FFFF), 0);
    }

    #[test]
    fn el_espejo_mueve_el_ancla() {
        let i = Imagen::nueva(4, 1, vec![0xFF00_0000, 0, 0, 0], 1, 1);
        let e = i.espejo();
        assert_eq!(e.px[3], 0xFF00_0000);
        assert_eq!(e.ancla_x, 3);
    }
}
