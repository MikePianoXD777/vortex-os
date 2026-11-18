//! De la descripción de AppStream (un HTML mínimo: `<p>`, `<ul>`, `<ol>`, `<li>`, `<em>`, `<code>`) a texto plano.
//!
//! Párrafos separados por una línea en blanco, viñetas «• » en las listas, números en las ordenadas.
//! También acepta texto que ya viene plano (las búsquedas de Flathub lo devuelven así): solo le
//! quita la sangría de cada renglón y las líneas en blanco de más.

const BLOQUES: [&str; 10] = [
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "pre",
];

#[derive(Clone, Copy, PartialEq)]
enum Tipo {
    P,
    Li,
}

struct Lista {
    ordenada: bool,
    contador: usize,
    numero: usize,
}

#[derive(Default)]
struct Convertidor {
    /// (tipo, texto, número de lista)
    bloques: Vec<(Tipo, String, usize)>,
    actual: Option<String>,
    tipo: Option<Tipo>,
    listas: Vec<Lista>,
    n_listas: usize,
}

fn es_espacio_horizontal(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\x0c' | '\x0b')
}

/// Lo que hace `_cerrar`: sin espacios alrededor de cada salto, espacios repetidos en uno, sin bordes.
fn limpiar_bloque(t: &str) -> String {
    let lineas: Vec<&str> = t.split('\n').collect();
    let mut sin_bordes = String::new();
    for (i, l) in lineas.iter().enumerate() {
        if i > 0 {
            sin_bordes.push('\n');
        }
        sin_bordes.push_str(l.trim_matches(es_espacio_horizontal));
    }
    // espacios repetidos (dentro de cada renglón) en uno
    let mut salida = String::new();
    let mut previo_espacio = false;
    for c in sin_bordes.chars() {
        if es_espacio_horizontal(c) {
            if !previo_espacio {
                salida.push(' ');
            }
            previo_espacio = true;
        } else {
            salida.push(c);
            previo_espacio = false;
        }
    }
    salida.trim().to_string()
}

impl Convertidor {
    fn cerrar(&mut self) {
        if let Some(actual) = self.actual.take() {
            let texto = limpiar_bloque(&actual);
            if !texto.is_empty() {
                let tipo = self.tipo.unwrap_or(Tipo::P);
                let lista = if tipo == Tipo::Li {
                    self.listas.last().map_or(0, |l| l.numero)
                } else {
                    0
                };
                self.bloques.push((tipo, texto, lista));
            }
        }
    }

    fn abrir(&mut self, tipo: Tipo) {
        self.cerrar();
        self.tipo = Some(tipo);
        self.actual = Some(String::new());
    }

    fn etiqueta_inicio(&mut self, tag: &str) {
        match tag {
            "ul" | "ol" => {
                self.cerrar();
                self.n_listas += 1;
                self.listas.push(Lista {
                    ordenada: tag == "ol",
                    contador: 0,
                    numero: self.n_listas,
                });
            }
            "li" => {
                let mut prefijo = "• ".to_string();
                if let Some(l) = self.listas.last_mut()
                    && l.ordenada
                {
                    l.contador += 1;
                    prefijo = format!("{}. ", l.contador);
                }
                self.abrir(Tipo::Li);
                self.actual = Some(prefijo);
            }
            t if BLOQUES.contains(&t) => self.abrir(Tipo::P),
            "br" => self.actual.get_or_insert_with(String::new).push('\n'),
            _ => {}
        }
    }

    fn etiqueta_fin(&mut self, tag: &str) {
        match tag {
            "ul" | "ol" => {
                self.cerrar();
                self.listas.pop();
            }
            "li" => self.cerrar(),
            t if BLOQUES.contains(&t) => self.cerrar(),
            _ => {}
        }
    }

    fn datos(&mut self, d: &str) {
        match &mut self.actual {
            Some(a) => a.push_str(d),
            None => {
                if d.trim().is_empty() {
                    return;
                }
                self.tipo = Some(Tipo::P);
                self.actual = Some(d.to_string());
            }
        }
    }

    fn texto(&mut self) -> String {
        self.cerrar();
        let mut salida = String::new();
        let mut previo: Option<&(Tipo, String, usize)> = None;
        for b in &self.bloques {
            if let Some(p) = previo {
                // Entre elementos de una misma lista, un solo salto; una introducción que termina en «:»
                // queda pegada a su lista; lo demás, una línea en blanco.
                let pegado = (b.0 == Tipo::Li && p.0 == Tipo::Li && p.2 == b.2)
                    || (b.0 == Tipo::Li && p.0 == Tipo::P && p.1.ends_with(':'));
                salida.push_str(if pegado { "\n" } else { "\n\n" });
            }
            salida.push_str(&b.1);
            previo = Some(b);
        }
        salida
    }
}

/// Convierte las entidades de HTML que pueden venir (`&amp;`, `&#233;`, `&#xe9;`…).
fn entidades(t: &str) -> String {
    let mut salida = String::with_capacity(t.len());
    let mut resto = t;
    while let Some(i) = resto.find('&') {
        salida.push_str(&resto[..i]);
        resto = &resto[i..];
        let fin = resto.find(';').filter(|&f| f <= 10);
        let Some(f) = fin else {
            salida.push('&');
            resto = &resto[1..];
            continue;
        };
        let nombre = &resto[1..f];
        let c = match nombre {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            n if n.starts_with("#x") || n.starts_with("#X") => {
                u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32)
            }
            n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                salida.push(c);
                resto = &resto[f + 1..];
            }
            None => {
                salida.push('&');
                resto = &resto[1..];
            }
        }
    }
    salida.push_str(resto);
    salida
}

/// Convierte una descripción de AppStream (HTML o texto) en texto plano.
pub fn html_a_texto(contenido: &str) -> String {
    if contenido.is_empty() {
        return String::new();
    }
    if !contenido.contains('<') {
        let lineas: Vec<&str> = contenido.lines().map(str::trim).collect();
        let mut t = lineas.join("\n");
        while t.contains("\n\n\n") {
            t = t.replace("\n\n\n", "\n\n");
        }
        return t.trim().to_string();
    }
    let mut c = Convertidor::default();
    let mut resto = contenido;
    while !resto.is_empty() {
        let Some(i) = resto.find('<') else {
            c.datos(&entidades(resto));
            break;
        };
        if i > 0 {
            c.datos(&entidades(&resto[..i]));
        }
        resto = &resto[i..];
        // un comentario
        if let Some(r) = resto.strip_prefix("<!--") {
            resto = r.find("-->").map_or("", |f| &r[f + 3..]);
            continue;
        }
        let Some(fin) = fin_de_etiqueta(resto) else {
            // «<» sin cerrar: es texto
            c.datos(&entidades(resto));
            break;
        };
        let cuerpo = &resto[1..fin];
        resto = &resto[fin + 1..];
        let cierre = cuerpo.starts_with('/');
        let nombre: String = cuerpo
            .trim_start_matches('/')
            .chars()
            .take_while(|ch| !ch.is_whitespace() && *ch != '/')
            .collect::<String>()
            .to_ascii_lowercase();
        if nombre.is_empty() || !nombre.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            continue;
        }
        if cierre {
            c.etiqueta_fin(&nombre);
        } else {
            c.etiqueta_inicio(&nombre);
            if cuerpo.ends_with('/') {
                c.etiqueta_fin(&nombre);
            }
        }
    }
    c.texto()
}

/// Dónde cierra la etiqueta que empieza en `t[0] == '<'` (respeta comillas).
fn fin_de_etiqueta(t: &str) -> Option<usize> {
    let mut comilla: Option<char> = None;
    for (i, ch) in t.char_indices().skip(1) {
        match (comilla, ch) {
            (Some(q), c) if c == q => comilla = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => comilla = Some(ch),
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}
