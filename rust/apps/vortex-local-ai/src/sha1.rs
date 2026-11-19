//! SHA-1 (solo para nombrar el índice de la memoria y detectar notas que no cambiaron, igual que
//! `hashlib.sha1` de la de Python: los archivos tienen que llamarse igual). No sirve para seguridad.

pub fn sha1(datos: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut mensaje = datos.to_vec();
    mensaje.push(0x80);
    while mensaje.len() % 64 != 56 {
        mensaje.push(0);
    }
    mensaje.extend_from_slice(&((datos.len() as u64) * 8).to_be_bytes());
    for bloque in mensaje.chunks(64) {
        let mut w = [0u32; 80];
        for (i, c) in bloque.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut salida = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        salida[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    salida
}

/// El `hexdigest()` de Python.
pub fn hex(datos: &[u8]) -> String {
    sha1(datos).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod pruebas {
    use super::hex;

    #[test]
    fn vectores_conocidos() {
        assert_eq!(hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }
}
