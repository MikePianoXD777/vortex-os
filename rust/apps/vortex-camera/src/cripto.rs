//! Lo poquito de criptografía que piden las cámaras, sin crates de afuera: MD5 (la autenticación Digest de
//! RTSP y HTTP), SHA-1 (el resumen de WS-Security de ONVIF) y base64. No es para guardar secretos: es para
//! hablar el protocolo que la cámara ya trae. Probados con los vectores oficiales (RFC 1321, FIPS 180, RFC 4648).

/// El resumen MD5 de `datos`.
pub fn md5(datos: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
        5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15,
        21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // K[i] = floor(|sin(i + 1)| · 2^32)
    let k: [u32; 64] = std::array::from_fn(|i| (((i + 1) as f64).sin().abs() * 4_294_967_296.0) as u32);
    let mut h: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    for bloque in relleno(datos, false).as_chunks::<64>().0 {
        let m: [u32; 16] = std::array::from_fn(|i| {
            u32::from_le_bytes(bloque[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4]))
        });
        let [mut a, mut b, mut c, mut d] = h;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
    }
    let mut r = [0u8; 16];
    for (i, v) in h.iter().enumerate() {
        r[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    r
}

/// El resumen SHA-1 de `datos`.
pub fn sha1(datos: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0];
    for bloque in relleno(datos, true).as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(bloque[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4]));
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((b & c) | (!b & d), 0x5a82_7999),
                1 => (b ^ c ^ d, 0x6ed9_eba1),
                2 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
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
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut r = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        r[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    r
}

/// El relleno de MD5 y SHA-1: un 1, ceros y el largo en bits (MD5 en little endian, SHA-1 en big endian).
fn relleno(datos: &[u8], big_endian: bool) -> Vec<u8> {
    let bits = (datos.len() as u64).wrapping_mul(8);
    let mut v = datos.to_vec();
    v.push(0x80);
    while v.len() % 64 != 56 {
        v.push(0);
    }
    v.extend_from_slice(&if big_endian {
        bits.to_be_bytes()
    } else {
        bits.to_le_bytes()
    });
    v
}

/// Los bytes en hexadecimal en minúsculas (`hexdigest()`).
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `md5(texto).hexdigest()`.
pub fn md5_hex(texto: &str) -> String {
    hex(&md5(texto.as_bytes()))
}

const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// base64 estándar, con `=` al final.
pub fn base64(datos: &[u8]) -> String {
    let mut s = String::with_capacity(datos.len().div_ceil(3) * 4);
    for trozo in datos.chunks(3) {
        let n = (u32::from(trozo[0]) << 16)
            | (u32::from(*trozo.get(1).unwrap_or(&0)) << 8)
            | u32::from(*trozo.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= trozo.len() {
                s.push(ALFABETO[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// base64 → bytes (ignora espacios); `None` si trae algo que no es base64.
pub fn base64_leer(texto: &str) -> Option<Vec<u8>> {
    let mut v = Vec::new();
    let mut n = 0u32;
    let mut bits = 0;
    for c in texto.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let x = ALFABETO.iter().position(|&a| a == c)? as u32;
        n = (n << 6) | x;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            v.push((n >> bits) as u8);
            n &= (1 << bits) - 1;
        }
    }
    Some(v)
}

/// Bytes al azar del kernel (para los nonce de ONVIF y de Digest).
pub fn azar(n: usize) -> Vec<u8> {
    use std::io::Read;
    let mut v = vec![0u8; n];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut v))
        .is_err()
    {
        // Sin /dev/urandom (no pasa en Linux): algo que cambie en cada llamada basta para un nonce.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        for (i, b) in v.iter_mut().enumerate() {
            *b = (t >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(31);
        }
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn md5_rfc1321() {
        for (entrada, esperado) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ] {
            assert_eq!(md5_hex(entrada), esperado, "{entrada:?}");
        }
    }

    #[test]
    fn sha1_fips180() {
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            hex(&sha1(&vec![b'a'; 1_000_000])),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn base64_rfc4648() {
        for (entrada, esperado) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(entrada.as_bytes()), esperado);
            assert_eq!(base64_leer(esperado).unwrap(), entrada.as_bytes());
        }
        assert!(base64_leer("no es*base64").is_none());
        assert_eq!(azar(16).len(), 16);
        assert_ne!(azar(16), azar(16));
    }
}
