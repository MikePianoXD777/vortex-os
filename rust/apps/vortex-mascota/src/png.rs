//! Un PNG mínimo (RGBA de 8 bits, zlib sin comprimir) para sacar la hoja de cuadros sin depender de nada.

fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in datos {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(datos: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in datos {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn trozo(salida: &mut Vec<u8>, tipo: &[u8; 4], datos: &[u8]) {
    salida.extend_from_slice(&(datos.len() as u32).to_be_bytes());
    let mut con_tipo = tipo.to_vec();
    con_tipo.extend_from_slice(datos);
    salida.extend_from_slice(&con_tipo);
    salida.extend_from_slice(&crc32(&con_tipo).to_be_bytes());
}

/// `px` en ARGB, fila por fila; `escala` agranda cada pixel a un cuadrito (pixel art nítido).
pub fn codificar(ancho: usize, alto: usize, px: &[u32], escala: usize) -> Vec<u8> {
    let (w, h) = (ancho * escala, alto * escala);
    let mut crudo = Vec::with_capacity(h * (w * 4 + 1));
    for y in 0..h {
        crudo.push(0); // sin filtro
        for x in 0..w {
            let p = px[(y / escala) * ancho + x / escala];
            crudo.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
        }
    }
    // zlib con bloques «stored» (sin comprimir) de hasta 65 535 bytes.
    let mut z = vec![0x78, 0x01];
    let trozos: Vec<&[u8]> = crudo.chunks(65_535).collect();
    for (i, t) in trozos.iter().enumerate() {
        z.push(u8::from(i + 1 == trozos.len()));
        let n = t.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(t);
    }
    z.extend_from_slice(&adler32(&crudo).to_be_bytes());

    let mut salida = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    trozo(&mut salida, b"IHDR", &ihdr);
    trozo(&mut salida, b"IDAT", &z);
    trozo(&mut salida, b"IEND", &[]);
    salida
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn crc_conocido() {
        assert_eq!(super::crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn empieza_como_png() {
        let png = super::codificar(2, 2, &[0xFF00_0000; 4], 3);
        assert!(png.starts_with(b"\x89PNG"));
    }
}
