//! Elegir el formato de la cámara (lo mismo que `motor.py`).

/// Con menos cuadros por segundo una resolución se ve a saltos: solo se usa si no hay otra.
pub const FPS_FLUIDO: f64 = 24.0;

/// Un formato que ofrece la cámara (`QCameraFormat` en QML: resolución y cuadros por segundo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Formato {
    pub ancho: u32,
    pub alto: u32,
    pub fps: f64,
}

impl Formato {
    pub fn new(ancho: u32, alto: u32, fps: f64) -> Self {
        Formato { ancho, alto, fps }
    }

    /// «1920x1080».
    pub fn resolucion(&self) -> String {
        format!("{}x{}", self.ancho, self.alto)
    }

    fn pixeles(&self) -> u64 {
        u64::from(self.ancho) * u64::from(self.alto)
    }
}

/// Las resoluciones que se ven fluidas, de mayor a menor. Si la cámara no tiene ninguna fluida, todas.
pub fn resoluciones(formatos: &[Formato]) -> Vec<String> {
    let fluidas: Vec<&Formato> = formatos.iter().filter(|f| f.fps >= FPS_FLUIDO).collect();
    let fluidas: Vec<&Formato> = if fluidas.is_empty() {
        formatos.iter().collect()
    } else {
        fluidas
    };
    let mut unicas: Vec<(String, u64)> = Vec::new();
    for f in fluidas {
        let r = f.resolucion();
        if !unicas.iter().any(|(u, _)| *u == r) {
            unicas.push((r, f.pixeles()));
        }
    }
    // sorted() de Python es estable: con los mismos píxeles queda la que apareció primero.
    unicas.sort_by_key(|u| std::cmp::Reverse(u.1));
    unicas.into_iter().map(|(r, _)| r).collect()
}

/// El índice del formato a usar: la resolución elegida si la cámara la tiene; si no, la mayor fluida.
/// Una resolución puede venir en varios formatos (a 5 cuadros sin comprimir y a 30 en MJPEG): gana el de
/// más cuadros por segundo (el primero, si empatan). `None` si la cámara no dice ninguno.
pub fn elegir(formatos: &[Formato], preferida: &str) -> Option<usize> {
    let opciones = resoluciones(formatos);
    let objetivo = if opciones.iter().any(|o| o == preferida) {
        preferida
    } else {
        opciones.first()?
    };
    let mut mejor: Option<usize> = None;
    for (i, f) in formatos.iter().enumerate() {
        if f.resolucion() == objetivo && mejor.is_none_or(|m| f.fps > formatos[m].fps) {
            mejor = Some(i);
        }
    }
    mejor
}

/// «1920x1080» → «1920 × 1080 (Full HD)».
pub fn etiqueta(texto: &str) -> String {
    let (ancho, alto) = texto.split_once('x').unwrap_or((texto, ""));
    let base = format!("{ancho} × {alto}");
    let nombre = match texto {
        "3840x2160" => "4K",
        "2560x1440" => "2K",
        "1920x1080" => "Full HD",
        "1280x720" => "HD",
        _ => return base,
    };
    format!("{base} ({nombre})")
}

/// Milisegundos → «01:15» o «1:01:01».
pub fn duracion(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}
