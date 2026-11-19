//! Base64 estándar (para las imágenes que van a la IA y las que manda el servidor MCP).

const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn codificar(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len().div_ceil(3) * 4);
    for c in d.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn casos() {
        assert_eq!(super::codificar(b"hello"), "aGVsbG8=");
        assert_eq!(super::codificar(b""), "");
        assert_eq!(super::codificar(b"ab"), "YWI=");
        assert_eq!(super::codificar(b"abc"), "YWJj");
    }
}
