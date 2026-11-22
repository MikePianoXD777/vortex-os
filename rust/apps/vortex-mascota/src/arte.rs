//! El dibujo de la compañera: diseño ORIGINAL de Vortexine's (no es ningún personaje con derechos):
//! cabello negro carbón con un mechón blanco, audífonos grandes blancos con morado, ojos morados,
//! sudadera negra con cordones blancos, falda gris y tenis blancos.
//!
//! Cada cuadro mide 32×48 y se arma con partes (cabeza, cara, cuerpo, piernas) escritas como texto: un
//! carácter por pixel (ver `color`). La fila `PISO` es donde pisa: parada, los pies; sentada, la falda
//! (las piernas cuelgan abajo de la orilla de la ventana).

pub const ANCHO: usize = 32;
pub const ALTO: usize = 48;
/// La fila donde pisa (la última fila de los pies cuando está parada).
pub const PISO: usize = 41;

/// ARGB (sin premultiplicar: todos los colores son opacos o transparentes del todo).
pub fn color(c: u8) -> Option<u32> {
    Some(match c {
        b'K' => 0xFF14_1418, // contorno
        b'H' => 0xFF2B_2A33, // cabello
        b'h' => 0xFF4A_4858, // brillo del cabello
        b'w' => 0xFFE8_E8EE, // mechón blanco
        b'S' => 0xFFF6_D6C4, // piel
        b's' => 0xFFE3_B5A0, // sombra de la piel
        b'b' => 0xFFF2_A0A6, // chapitas
        b'E' => 0xFF8A_5CF5, // ojo
        b'e' => 0xFF4B_2E9A, // ojo oscuro
        b'L' => 0xFFFF_FFFF, // brillo del ojo
        b'P' => 0xFFF0_F0F4, // audífonos
        b'p' => 0xFFB9_B9C6, // sombra de los audífonos
        b'V' => 0xFF8A_5CF5, // morado de los audífonos
        b'D' => 0xFF22_2228, // sudadera
        b'd' => 0xFF14_1419, // sombra de la sudadera
        b'T' => 0xFFE8_E8EE, // cordones
        b'G' => 0xFF6B_6B78, // falda
        b'g' => 0xFF50_505C, // pliegues
        b'B' => 0xFFF4_F4F6, // tenis
        b'n' => 0xFF9A_9AA6, // suela
        b'M' => 0xFFB8_4A5C, // boca
        b'R' => 0xFFE0_4F6A, // corazón
        _ => return None,
    })
}

// ---- Partes ---------------------------------------------------------------------------------------------

/// La cabeza (18 filas), con los ojos abiertos y la boca chiquita.
const CABEZA: [&str; 18] = [
    "..........KKKKKKKKKKKK..........",
    "........KKPPPPPPPPPPPPKK........",
    ".......KPpKKKKKKKKKKKKpPK.......",
    "......KPpKHHHHHHHHHHHHKpPK......",
    ".....KPpKHHhhHHHHHHHHwwKpPK.....",
    ".....KPKHHhHHHHHHHHHHHwwKPK.....",
    "....KPPKHHHHHHHHHHHHHHwHKPPK....",
    "...KPPPKHHHHHHHHHHHHHHHHKPPPK...",
    "...KPVPKHHSHHHHSSHHHHSHHKPVPK...",
    "...KPVPKHSSSHHSSSSHHSSSHKPVPK...",
    "...KPVPKHSSSSSSSSSSSSSSHKPVPK...",
    "...KPPPKHSKKKSSSSSSKKKSHKPPPK...",
    "...KpppKHSELESSSSSSELESHKpppK...",
    "....KppKHSeeeSSSSSSeeeSHKppK....",
    ".....KKKHSbSSSSMMSSSSbSHKKK.....",
    "......KHHSSSSSSSSSSSSSSHHK......",
    "......KHHKsSSSSSSSSSSsKHHK......",
    ".....KHHHHKKKSSSSSSKKKHHHHK.....",
];

/// Las caras: reemplazan las filas 11 a 14 de la cabeza (ojos, mejillas y boca).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cara {
    Normal,
    Parpadeo,
    Feliz,
    Hablando,
    Dormida,
    Sorpresa,
}

fn cara(c: Cara) -> [&'static str; 4] {
    match c {
        Cara::Normal => [CABEZA[11], CABEZA[12], CABEZA[13], CABEZA[14]],
        Cara::Parpadeo => [
            "...KPPPKHSSSSSSSSSSSSSSHKPPPK...",
            "...KpppKHSSSSSSSSSSSSSSHKpppK...",
            "....KppKHSKKKSSSSSSKKKSHKppK....",
            ".....KKKHSbSSSSMMSSSSbSHKKK.....",
        ],
        Cara::Feliz => [
            "...KPPPKHSSSSSSSSSSSSSSHKPPPK...",
            "...KpppKHSSKSSSSSSSSKSSHKpppK...",
            "....KppKHSKSKSSSSSSKSKSHKppK....",
            ".....KKKHSbSSSKMMKSSSbSHKKK.....",
        ],
        Cara::Hablando => [
            CABEZA[11],
            CABEZA[12],
            CABEZA[13],
            ".....KKKHSbSSSSKKSSSSbSHKKK.....",
        ],
        Cara::Dormida => [
            "...KPPPKHSSSSSSSSSSSSSSHKPPPK...",
            "...KpppKHSSSSSSSSSSSSSSHKpppK...",
            "....KppKHSKKKSSSSSSKKKSHKppK....",
            ".....KKKHSbSSSSSSSSSSbSHKKK.....",
        ],
        Cara::Sorpresa => [
            "...KPPPKHSKKKSSSSSSKKKSHKPPPK...",
            "...KpppKHSKLKSSSSSSKLKSHKpppK...",
            "....KppKHSKKKSSSSSSKKKSHKppK....",
            ".....KKKHSbSSSSKKSSSSbSHKKK.....",
        ],
    }
}

/// El cuerpo (14 filas): sudadera con cordones y bolsa, manos a los lados, falda.
const CUERPO: [&str; 14] = [
    "......KHHKDDDDDDDDDDDDKHHK......",
    ".....KHHKDDDDTDDDDTDDDDKHHK.....",
    "....KHHKDDDDDTDDDDTDDDDDKHHK....",
    "....KHKDDDDDDTDDDDTDDDDDDKHK....",
    "....KHKDdDDDDTDDDDTDDDDdDKHK....",
    "....KDdKDDDDDDDDDDDDDDDDKdDK....",
    "....KDdKDDDDKKKKKKKKDDDDKdDK....",
    "....KDdKDDDKddddddddKDDDKdDK....",
    "....KSSKDDDKddddddddKDDDKSSK....",
    "....KSSKDDDDDDDDDDDDDDDDKSSK....",
    ".....KKKddddddddddddddddKKK.....",
    ".......KGGGGGGGGGGGGGGGGK.......",
    "......KGGgGGGgGGGGgGGGgGGK......",
    "......KKKKKKKKKKKKKKKKKKKK......",
];

/// El cuerpo saludando: la mano derecha (de ella; a la izquierda del dibujo) arriba.
const CUERPO_SALUDO: [&str; 14] = [
    "..KSSKKHHKDDDDDDDDDDDDKHHK......",
    "..KSSKKHKDDDDTDDDDTDDDDKHHK.....",
    "..KDDKKHKDDDDTDDDDTDDDDDKHHK....",
    "..KDdDKKDDDDDTDDDDTDDDDDDKHK....",
    "...KDdDKdDDDDTDDDDTDDDDdDKHK....",
    "....KDdKDDDDDDDDDDDDDDDDKdDK....",
    ".....KKKDDDDKKKKKKKKDDDDKdDK....",
    ".......KDDDKddddddddKDDDKdDK....",
    ".......KDDDKddddddddKDDDKSSK....",
    ".......KDDDDDDDDDDDDDDDDKSSK....",
    ".......KddddddddddddddddKKK.....",
    ".......KGGGGGGGGGGGGGGGGK.......",
    "......KGGgGGGgGGGGgGGGgGGK......",
    "......KKKKKKKKKKKKKKKKKKKK......",
];

/// Las piernas paradas (8 filas).
const PIERNAS: [&str; 8] = [
    "..........KSSK....KSSK..........",
    "..........KSSK....KSSK..........",
    "..........KDDK....KDDK..........",
    "..........KDDK....KDDK..........",
    "..........KDDK....KDDK..........",
    ".........KBBBBK..KBBBBK.........",
    ".........KnnnnK..KnnnnK.........",
    "..........KKKK....KKKK..........",
];

/// Caminando: las piernas abiertas.
const PASO_ABIERTO: [&str; 8] = [
    ".........KSSK......KSSK.........",
    "........KSSK........KSSK........",
    "........KDDK........KDDK........",
    ".......KDDK..........KDDK.......",
    ".......KDDK..........KDDK.......",
    "......KBBBBK........KBBBBK......",
    "......KnnnnK........KnnnnK......",
    ".......KKKK..........KKKK.......",
];

/// Caminando: un pie en el aire (el otro paso es el mismo, al espejo).
const PASO_ARRIBA: [&str; 8] = [
    "..........KSSK....KSSK..........",
    "..........KSSK....KSSK..........",
    "..........KDDK....KDDK..........",
    "..........KDDK....KDDK..........",
    "..........KDDK...KBBBBK.........",
    ".........KBBBBK..KnnnnK.........",
    ".........KnnnnK...KKKK..........",
    "..........KKKK..................",
];

/// Sentada en una orilla o colgando: las piernas cuelgan (6 filas).
const PIERNAS_COLGANDO: [&str; 6] = [
    "..........KSSK....KSSK..........",
    "..........KDDK....KDDK..........",
    "..........KDDK....KDDK..........",
    ".........KBBBBK..KBBBBK.........",
    ".........KnnnnK..KnnnnK.........",
    "..........KKKK....KKKK..........",
];

/// Colgando las piernas se balancean: un pie más adelante.
const PIERNAS_BALANCEO: [&str; 6] = [
    "..........KSSK....KSSK..........",
    "..........KDDK....KDDK..........",
    ".........KDDK......KDDK.........",
    "........KBBBBK....KBBBBK........",
    "........KnnnnK....KnnnnK........",
    ".........KKKK......KKKK.........",
];

// ---- Cuadros --------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Parada,
    /// Los 4 tiempos del paso: abierto, un pie arriba, abierto, el otro pie arriba.
    Caminando(u8),
    Saludando,
    /// Sentada en la orilla; el número balancea las piernas (0 o 1).
    Sentada(u8),
    /// Agarrada con el mouse o cayendo: las piernas cuelgan.
    Colgando,
}

/// Un cuadro listo para pintar: ANCHO×ALTO pixeles ARGB (0 = transparente).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cuadro {
    pub px: Vec<u32>,
}

impl Cuadro {
    fn vacio() -> Self {
        Self {
            px: vec![0; ANCHO * ALTO],
        }
    }

    fn poner(&mut self, fila0: usize, filas: &[&str]) {
        for (i, fila) in filas.iter().enumerate() {
            let y = fila0 + i;
            if y >= ALTO {
                break;
            }
            for (x, c) in fila.bytes().enumerate().take(ANCHO) {
                if let Some(argb) = color(c) {
                    self.px[y * ANCHO + x] = argb;
                }
            }
        }
    }

    /// Al espejo (para que camine hacia el otro lado).
    pub fn espejo(&self) -> Self {
        let mut px = vec![0; ANCHO * ALTO];
        for y in 0..ALTO {
            for x in 0..ANCHO {
                px[y * ANCHO + x] = self.px[y * ANCHO + (ANCHO - 1 - x)];
            }
        }
        Self { px }
    }

    /// Lo que no es transparente: (x0, y0, x1, y1), con x1/y1 fuera.
    pub fn caja(&self) -> (usize, usize, usize, usize) {
        let (mut x0, mut y0, mut x1, mut y1) = (ANCHO, ALTO, 0, 0);
        for y in 0..ALTO {
            for x in 0..ANCHO {
                if self.px[y * ANCHO + x] != 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        (x0, y0, x1.max(x0), y1.max(y0))
    }
}

fn espejo_filas(filas: &[&str]) -> Vec<String> {
    filas.iter().map(|f| f.chars().rev().collect()).collect()
}

/// Arma un cuadro. `bote` sube cabeza y cuerpo un pixel (el saltito de cada paso).
pub fn cuadro(pose: Pose, c: Cara) -> Cuadro {
    let mut q = Cuadro::vacio();
    let cuerpo: &[&str] = if pose == Pose::Saludando {
        &CUERPO_SALUDO
    } else {
        &CUERPO
    };
    // Parada, el pie pisa en PISO: la cabeza empieza en PISO-39. Sentada o colgando, lo que toca la
    // orilla es la falda (la fila 31 del muñeco), así que todo baja 8 y las piernas cuelgan abajo.
    let (arriba, piernas): (usize, Vec<String>) = match pose {
        Pose::Parada | Pose::Saludando => (PISO - 39, PIERNAS.iter().map(|s| s.to_string()).collect()),
        Pose::Caminando(t) => match t % 4 {
            0 | 2 => (PISO - 39, PASO_ABIERTO.iter().map(|s| s.to_string()).collect()),
            1 => (PISO - 40, PASO_ARRIBA.iter().map(|s| s.to_string()).collect()),
            _ => (PISO - 40, espejo_filas(&PASO_ARRIBA)),
        },
        Pose::Sentada(t) => {
            let p: &[&str] = if t % 2 == 0 {
                &PIERNAS_COLGANDO
            } else {
                &PIERNAS_BALANCEO
            };
            (PISO - 31, p.iter().map(|s| s.to_string()).collect())
        }
        Pose::Colgando => (
            PISO - 39,
            PIERNAS_COLGANDO.iter().map(|s| s.to_string()).collect(),
        ),
    };
    q.poner(arriba, &CABEZA);
    q.poner(arriba + 11, &cara(c));
    q.poner(arriba + 18, cuerpo);
    let piernas: Vec<&str> = piernas.iter().map(String::as_str).collect();
    // Al caminar el cuerpo sube un pixel pero los pies siguen en el piso: las piernas van fijas abajo.
    let fila_piernas = match pose {
        Pose::Caminando(_) | Pose::Parada | Pose::Saludando => PISO + 1 - piernas.len(),
        _ => arriba + 32,
    };
    q.poner(fila_piernas, &piernas);
    q
}

// ---- Emotes y globitos -----------------------------------------------------------------------------------

/// Los dibujitos chiquitos que salen arriba de la cabeza.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emote {
    Zeta,
    Corazon,
    Nota,
}

pub fn emote(e: Emote) -> &'static [&'static str] {
    match e {
        Emote::Zeta => &["KKKKK", "...K.", "..K..", ".K...", "KKKKK"],
        Emote::Corazon => &[".RR.RR.", "RRRRRRR", "RRRRRRR", ".RRRRR.", "..RRR..", "...R..."],
        Emote::Nota => &["..KKK", "..K.K", "..K.K", "KKK.K", "KKK..", "KKK.."],
    }
}

/// Letra de 5×7 solo con lo que usan las frases (mayúsculas con acentos y signos de español).
fn glifo(c: char) -> Option<[u8; 7]> {
    // Cada fila: 5 bits, el de la izquierda arriba (0b10000).
    Some(match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        // Con acento o tilde: la letra se aplasta y el acento va en la fila de arriba.
        'Á' => [0x02, 0x04, 0x0E, 0x11, 0x1F, 0x11, 0x11],
        'Í' => [0x02, 0x04, 0x0E, 0x04, 0x04, 0x04, 0x0E],
        'Ó' => [0x02, 0x04, 0x0E, 0x11, 0x11, 0x11, 0x0E],
        'Ú' => [0x02, 0x04, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'É' => [0x02, 0x04, 0x1F, 0x10, 0x1E, 0x10, 0x1F],
        'Ñ' => [0x0D, 0x16, 0x11, 0x19, 0x15, 0x13, 0x11],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '¡' => [0x04, 0x00, 0x04, 0x04, 0x04, 0x04, 0x04],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '¿' => [0x04, 0x00, 0x04, 0x08, 0x10, 0x11, 0x0E],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x08],
        ' ' => [0; 7],
        _ => return None,
    })
}

/// Las frases del globito al darle clic (en mayúsculas: la letra no trae minúsculas).
pub const FRASES: &[&str] = &[
    "¡HOLA!",
    "¿TE AYUDO?",
    "AQUÍ ANDO",
    "¿QUÉ HACEMOS?",
    "¡ÁNIMO!",
    "¿UN DESCANSO?",
    "TODO BIEN",
];
pub const FRASE_SUENO: &str = "TENGO SUEÑO";
pub const FRASE_ADIOS: &str = "¡ADIÓS!";
pub const FRASE_DESPIERTA: &str = "¡YA DESPERTÉ!";

/// La frase como se dice en voz alta: «¡HOLA!» → «¡Hola!», «TENGO SUEÑO» → «Tengo sueño».
pub fn frase_hablada(frase: &str) -> String {
    let mut primera = true;
    frase
        .chars()
        .flat_map(|c| {
            let mayuscula = primera && c.is_alphabetic();
            if c.is_alphabetic() {
                primera = false;
            }
            let v: Vec<char> = if mayuscula {
                c.to_uppercase().collect()
            } else {
                c.to_lowercase().collect()
            };
            v
        })
        .collect()
}

/// Cuántos pixeles (sin escalar) mide el texto: 6 por letra (5 + 1 de aire).
pub fn ancho_texto(t: &str) -> usize {
    t.chars().count() * 6 - 1
}

/// Las filas del texto como máscara (true = tinta), de ancho_texto × 7.
pub fn mascara_texto(t: &str) -> Vec<Vec<bool>> {
    let ancho = ancho_texto(t);
    let mut filas = vec![vec![false; ancho]; 7];
    for (i, c) in t.chars().enumerate() {
        let g = glifo(c).or_else(|| glifo('?')).unwrap_or([0; 7]);
        for (y, bits) in g.iter().enumerate() {
            for x in 0..5 {
                if bits & (0x10 >> x) != 0 {
                    filas[y][i * 6 + x] = true;
                }
            }
        }
    }
    filas
}

/// ¿Tiene letra cada carácter? (para las pruebas: una frase con algo sin dibujar saldría con «?»)
pub fn se_puede_escribir(t: &str) -> bool {
    t.chars().all(|c| glifo(c).is_some())
}

/// Todos los cuadros en una hoja (para verla en PNG): una fila por pose, una columna por cara.
pub fn hoja() -> (usize, usize, Vec<u32>) {
    let poses = [
        Pose::Parada,
        Pose::Caminando(0),
        Pose::Caminando(1),
        Pose::Caminando(2),
        Pose::Caminando(3),
        Pose::Saludando,
        Pose::Sentada(0),
        Pose::Sentada(1),
        Pose::Colgando,
    ];
    let caras = [
        Cara::Normal,
        Cara::Parpadeo,
        Cara::Feliz,
        Cara::Hablando,
        Cara::Dormida,
        Cara::Sorpresa,
    ];
    let (w, h) = (poses.len() * (ANCHO + 2), caras.len() * (ALTO + 2));
    let mut px = vec![0u32; w * h];
    for (i, p) in poses.iter().enumerate() {
        for (j, c) in caras.iter().enumerate() {
            let q = cuadro(*p, *c);
            for y in 0..ALTO {
                for x in 0..ANCHO {
                    px[(j * (ALTO + 2) + y) * w + i * (ANCHO + 2) + x] = q.px[y * ANCHO + x];
                }
            }
        }
    }
    (w, h, px)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn revisar(nombre: &str, filas: &[&str]) {
        for (i, f) in filas.iter().enumerate() {
            assert_eq!(f.chars().count(), ANCHO, "{nombre}, fila {i}: «{f}»");
            for c in f.bytes() {
                assert!(
                    c == b'.' || color(c).is_some(),
                    "{nombre}, fila {i}: color «{}» sin definir",
                    c as char
                );
            }
        }
    }

    #[test]
    fn todas_las_partes_miden_32() {
        revisar("cabeza", &CABEZA);
        revisar("cuerpo", &CUERPO);
        revisar("saludo", &CUERPO_SALUDO);
        revisar("piernas", &PIERNAS);
        revisar("paso abierto", &PASO_ABIERTO);
        revisar("paso arriba", &PASO_ARRIBA);
        revisar("colgando", &PIERNAS_COLGANDO);
        revisar("balanceo", &PIERNAS_BALANCEO);
        for c in [
            Cara::Normal,
            Cara::Parpadeo,
            Cara::Feliz,
            Cara::Hablando,
            Cara::Dormida,
            Cara::Sorpresa,
        ] {
            revisar(&format!("{c:?}"), &cara(c));
        }
    }

    #[test]
    fn parada_y_caminando_pisan_en_el_piso() {
        for p in [
            Pose::Parada,
            Pose::Caminando(0),
            Pose::Caminando(1),
            Pose::Caminando(3),
            Pose::Saludando,
        ] {
            let (_, _, _, y1) = cuadro(p, Cara::Normal).caja();
            assert_eq!(y1, PISO + 1, "{p:?}");
        }
    }

    #[test]
    fn sentada_la_falda_queda_en_el_piso() {
        let q = cuadro(Pose::Sentada(0), Cara::Normal);
        // La última fila de la falda es puro contorno (K) y queda en PISO.
        let fila: Vec<u32> = q.px[PISO * ANCHO..(PISO + 1) * ANCHO].to_vec();
        assert!(fila.iter().filter(|&&p| p == color(b'K').unwrap()).count() >= 20);
        assert_eq!(q.caja().3, ALTO);
    }

    #[test]
    fn el_espejo_dos_veces_es_el_mismo() {
        let q = cuadro(Pose::Caminando(1), Cara::Feliz);
        assert_eq!(q.espejo().espejo(), q);
        assert_ne!(q.espejo(), q);
    }

    #[test]
    fn las_frases_se_dicen_en_minusculas() {
        assert_eq!(frase_hablada("¡HOLA!"), "¡Hola!");
        assert_eq!(frase_hablada("¿TE AYUDO?"), "¿Te ayudo?");
        assert_eq!(frase_hablada("AQUÍ ANDO"), "Aquí ando");
        assert_eq!(frase_hablada(FRASE_SUENO), "Tengo sueño");
        assert_eq!(frase_hablada(FRASE_DESPIERTA), "¡Ya desperté!");
    }

    #[test]
    fn todas_las_frases_tienen_letra() {
        for f in FRASES
            .iter()
            .chain([FRASE_SUENO, FRASE_ADIOS, FRASE_DESPIERTA].iter())
        {
            assert!(se_puede_escribir(f), "falta una letra en «{f}»");
            assert_eq!(mascara_texto(f)[0].len(), ancho_texto(f));
        }
    }
}
