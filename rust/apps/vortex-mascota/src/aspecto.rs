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

use crate::arte::{self, Cara, Pose};
use crate::mundo::Anim;
use std::collections::HashMap;
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

pub struct Aspecto {
    /// Cada cuadro dos veces: [normal, al espejo].
    cuadros: Vec<[Imagen; 2]>,
    anims: HashMap<Anim, Vec<(usize, f32)>>,
    /// Las imágenes miran a la izquierda (Shimeji): a la derecha van al espejo.
    miran_izquierda: bool,
    pub nombre: String,
}

impl Aspecto {
    pub fn cuadro(&self, anim: Anim, t: f32, dir: i8) -> &Imagen {
        let secuencia = self
            .anims
            .get(&anim)
            .or_else(|| self.anims.get(&Anim::Parada))
            .expect("siempre hay Parada");
        let total: f32 = secuencia.iter().map(|(_, d)| d).sum();
        let mut resto = if total > 0.0 { t % total } else { 0.0 };
        let mut i = secuencia[0].0;
        for (c, d) in secuencia {
            i = *c;
            if resto < *d {
                break;
            }
            resto -= d;
        }
        let espejo = usize::from(self.miran_izquierda && dir > 0);
        &self.cuadros[i][espejo]
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
        anims.insert(Anim::Parada, vec![(parada, 3.2), (parpadeo, 0.15)]);
        anims.insert(Anim::Caminando, pasos.iter().map(|&p| (p, 0.16)).collect());
        anims.insert(
            Anim::Sentada,
            vec![
                (sentada0, 0.6),
                (sentada1, 0.6),
                (sentada0, 0.6),
                (sentada1, 0.6),
                (sentada_parpadeo, 0.15),
            ],
        );
        anims.insert(Anim::Dormida, vec![(dormida, 1.0)]);
        anims.insert(
            Anim::Saludando,
            vec![(saludo, 0.3), (saludo_habla, 0.2), (feliz, 0.3)],
        );
        anims.insert(Anim::Agarrada, vec![(colgando, 1.0)]);
        anims.insert(Anim::Cayendo, vec![(colgando, 1.0)]);
        anims.insert(Anim::Aterrizando, vec![(parpadeo, 1.0)]);
        Self {
            cuadros,
            anims,
            miran_izquierda: false,
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
        type Secuencia = &'static [(u32, f32)];
        let de_fabrica: [(Anim, &str, Secuencia); 8] = [
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
            anims.insert(anim, secuencia);
        }
        let nombre = carpeta
            .file_name()
            .map_or("propia".into(), |n| n.to_string_lossy().into_owned());
        Ok(Some(Self {
            cuadros,
            anims,
            miran_izquierda: true,
            nombre,
        }))
    }
}

/// La carpeta de la mascota propia.
pub fn carpeta() -> PathBuf {
    let datos = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    datos.join("vortex/mascota")
}

/// La propia si hay una que sirva; si no (o si falla al leerla), la original.
pub fn cargar(escala_original: f32) -> (Aspecto, Option<String>) {
    match Aspecto::shimeji(&carpeta()) {
        Ok(Some(a)) => (a, None),
        Ok(None) => (Aspecto::original(escala_original), None),
        Err(e) => (Aspecto::original(escala_original), Some(e)),
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
    if w == 0 || h == 0 || w > 1024 || h > 1024 {
        return Err(error(&"la imagen tiene que medir entre 1 y 1024 pixeles"));
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
