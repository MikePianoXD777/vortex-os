//! Arma lo que se ve en la ventana: el cuadro, el dibujito de arriba (zeta, corazón, nota) y el
//! globito con la frase. Todo en un buffer ARGB premultiplicado del tamaño de la ventana.

use crate::arte::{self, Emote};
use crate::aspecto::Imagen;

/// Espacio arriba para el globito.
pub const BURBUJA: usize = 52;
/// Ancho mínimo de la ventana (que quepa la frase más larga).
pub const ANCHO_MINIMO: usize = 200;
const ESCALA_LETRA: usize = 2;
const ESCALA_EMOTE: usize = 3;

pub struct Dibujo {
    pub px: Vec<u32>,
    /// Donde se le puede dar clic (el muñeco): x, y, ancho, alto.
    pub clic: (i32, i32, i32, i32),
}

#[allow(clippy::too_many_arguments)] // un rectángulo: buffer, su tamaño, posición, tamaño y color
fn rect(px: &mut [u32], w: usize, h: usize, x: i32, y: i32, rw: i32, rh: i32, c: u32) {
    for yy in y.max(0)..(y + rh).min(h as i32) {
        for xx in x.max(0)..(x + rw).min(w as i32) {
            px[yy as usize * w + xx as usize] = c;
        }
    }
}

/// `ancla` es dónde pisa dentro de la ventana; `t` el tiempo (para que la zeta flote).
pub fn componer(
    w: usize,
    h: usize,
    ancla: (usize, usize),
    img: &Imagen,
    emote: Option<Emote>,
    frase: Option<&str>,
    t: f32,
) -> Dibujo {
    let mut px = vec![0u32; w * h];
    let ox = ancla.0 as i32 - img.ancla_x as i32;
    let oy = ancla.1 as i32 - img.ancla_y as i32;
    for y in 0..img.alto {
        let dy = oy + y as i32;
        if dy < 0 || dy >= h as i32 {
            continue;
        }
        for x in 0..img.ancho {
            let dx = ox + x as i32;
            let p = img.px[y * img.ancho + x];
            if p >> 24 != 0 && dx >= 0 && dx < w as i32 {
                px[dy as usize * w + dx as usize] = p;
            }
        }
    }
    let (cx0, cy0, cx1, cy1) = img.caja;
    let arriba = oy + cy0 as i32;

    if let Some(e) = emote {
        let filas = arte::emote(e);
        let (ew, eh) = (
            filas[0].len() as i32 * ESCALA_EMOTE as i32,
            filas.len() as i32 * ESCALA_EMOTE as i32,
        );
        let flota = if e == Emote::Zeta {
            ((t * 8.0) % 12.0) as i32
        } else {
            ((t * 3.0).sin() * 2.0) as i32
        };
        let ex = (ox + cx1 as i32 - ew / 2).min(w as i32 - ew);
        let ey = (arriba - eh - 2 - flota).max(0);
        for (fy, fila) in filas.iter().enumerate() {
            for (fx, c) in fila.bytes().enumerate() {
                if let Some(col) = arte::color(c) {
                    let s = ESCALA_EMOTE as i32;
                    rect(&mut px, w, h, ex + fx as i32 * s, ey + fy as i32 * s, s, s, col);
                }
            }
        }
    }

    if let Some(texto) = frase {
        let m = arte::mascara_texto(texto);
        let (tw, th) = ((m[0].len() * ESCALA_LETRA) as i32, (7 * ESCALA_LETRA) as i32);
        let (bw, bh) = (tw + 16, th + 12);
        let bx = (ancla.0 as i32 - bw / 2).clamp(0, (w as i32 - bw).max(0));
        let by = (arriba - bh - 10).max(0);
        let (borde, fondo, tinta) = (0xFF14_1418, 0xFFFF_FFFF, 0xFF14_1418);
        // Globito con esquinas recortadas (pixel art): borde de 2, fondo blanco, colita abajo.
        rect(&mut px, w, h, bx + 2, by, bw - 4, bh, borde);
        rect(&mut px, w, h, bx, by + 2, bw, bh - 4, borde);
        rect(&mut px, w, h, bx + 2, by + 2, bw - 4, bh - 4, fondo);
        let colita = (ancla.0 as i32).clamp(bx + 8, bx + bw - 14);
        rect(&mut px, w, h, colita, by + bh - 2, 8, 2, fondo);
        rect(&mut px, w, h, colita, by + bh, 6, 2, borde);
        rect(&mut px, w, h, colita + 2, by + bh, 2, 2, fondo);
        rect(&mut px, w, h, colita + 2, by + bh + 2, 4, 2, borde);
        for (fy, fila) in m.iter().enumerate() {
            for (fx, &tinta_aqui) in fila.iter().enumerate() {
                if tinta_aqui {
                    let s = ESCALA_LETRA as i32;
                    rect(
                        &mut px,
                        w,
                        h,
                        bx + 8 + fx as i32 * s,
                        by + 6 + fy as i32 * s,
                        s,
                        s,
                        tinta,
                    );
                }
            }
        }
    }

    let clic = (
        ox + cx0 as i32,
        oy + cy0 as i32,
        (cx1 - cx0) as i32,
        (cy1 - cy0) as i32,
    );
    Dibujo { px, clic }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::aspecto::Aspecto;
    use crate::mundo::Anim;

    #[test]
    fn el_muneco_pisa_en_el_ancla_y_el_globito_va_arriba() {
        let a = Aspecto::original(3.0);
        let img = a.cuadro(Anim::Parada, 0.0, 1);
        let (w, h) = (ANCHO_MINIMO, BURBUJA + img.ancla_y + 30);
        let d = componer(
            w,
            h,
            (w / 2, BURBUJA + img.ancla_y),
            img,
            None,
            Some("¡HOLA!"),
            0.0,
        );
        // La última fila con muñeco es la de arriba del ancla.
        let fila_pie = BURBUJA + img.ancla_y - 1;
        assert!(d.px[fila_pie * w..(fila_pie + 1) * w].iter().any(|&p| p != 0));
        assert!(
            d.px[(fila_pie + 1) * w..(fila_pie + 2) * w]
                .iter()
                .all(|&p| p == 0)
        );
        // Hay globito (blanco) arriba de la cabeza.
        assert!(d.px[..(d.clic.1 as usize) * w].contains(&0xFFFF_FFFF));
        assert!(d.clic.2 > 0 && d.clic.3 > 0);
    }

    #[test]
    fn la_frase_mas_larga_cabe() {
        let mas_larga = arte::FRASES
            .iter()
            .chain([arte::FRASE_SUENO, arte::FRASE_DESPIERTA].iter())
            .map(|f| arte::ancho_texto(f) * ESCALA_LETRA + 16)
            .max()
            .unwrap();
        assert!(mas_larga <= ANCHO_MINIMO, "{mas_larga}");
    }
}
