//! La lluvia de dígitos de la terminal (4.3): columnas de números que caen en blanco y grises (la paleta
//! de fábrica de la 4.0) durante poco más de un segundo; luego dejan de nacer gotas, las que quedan se
//! van por abajo y sigue el saludo de siempre («vortex saludo»).
//!
//! Aquí solo está la simulación y el dibujo (texto ANSI), sin tocar la terminal: así se prueba sin
//! pantalla (`tests/lluvia.rs`). La terminal (tamaño, teclas, pantalla alterna) la maneja `main.rs`.

/// Segundos en los que nacen gotas nuevas.
pub const LLUEVE: f32 = 0.5;
/// Pase lo que pase, a los 2 s se acaba: la terminal nunca se queda esperando.
pub const TOPE: f32 = 2.0;
/// Menos que esto no vale la pena (una ventana chiquita o un panel): no se anima.
pub const ANCHO_MIN: u16 = 20;
pub const ALTO_MIN: u16 = 8;

/// Una celda de la pantalla: un dígito (o espacio) y qué tan blanco se ve (0 = apagado, 255 = blanco).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Celda {
    pub ch: u8,
    pub luz: u8,
}

impl Celda {
    pub const VACIA: Celda = Celda { ch: b' ', luz: 0 };
}

/// ¿Se anima? Solo en una terminal de verdad (entrada y salida), con espacio, y si no la apagaron.
pub fn debe_correr(salida_tty: bool, entrada_tty: bool, ancho: u16, alto: u16, apagada: bool) -> bool {
    salida_tty && entrada_tty && !apagada && ancho >= ANCHO_MIN && alto >= ALTO_MIN
}

/// Azar chiquito (xorshift64*): no hace falta más para que llueva distinto cada vez.
struct Azar(u64);

impl Azar {
    fn new(semilla: u64) -> Self {
        Azar(semilla.max(1))
    }
    fn siguiente(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// Entre 0 y 1.
    fn real(&mut self) -> f32 {
        (self.siguiente() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn entre(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.real()
    }
    fn digito(&mut self) -> u8 {
        b'0' + (self.siguiente() % 10) as u8
    }
}

struct Gota {
    col: u16,
    /// Fila de la cabeza (puede ir arriba de la pantalla, negativa, antes de entrar).
    cabeza: f32,
    /// Filas por segundo.
    vel: f32,
    largo: u16,
    /// Un dígito por celda de la gota, contando desde la cabeza.
    digitos: Vec<u8>,
}

impl Gota {
    fn se_fue(&self, alto: u16) -> bool {
        self.cabeza - self.largo as f32 > alto as f32
    }
}

pub struct Lluvia {
    ancho: u16,
    alto: u16,
    t: f32,
    gotas: Vec<Gota>,
    azar: Azar,
    previo: Vec<Celda>,
}

impl Lluvia {
    pub fn new(ancho: u16, alto: u16, semilla: u64) -> Self {
        let mut l = Lluvia {
            ancho,
            alto,
            t: 0.0,
            gotas: Vec::new(),
            azar: Azar::new(semilla),
            previo: vec![Celda::VACIA; ancho as usize * alto as usize],
        };
        // Una gota por columna desde el principio, escalonadas arriba de la pantalla: así entran
        // casi juntas pero no como una pared.
        for col in 0..ancho {
            let arriba = l.azar.entre(0.0, alto as f32 * 0.9);
            l.nacer(col, -arriba);
        }
        l
    }

    fn nacer(&mut self, col: u16, cabeza: f32) {
        let alto = self.alto.max(1) as f32;
        // Cruzan la pantalla en 0,3–0,5 s: rápido, para que todo dure ~1,3 s.
        let vel = alto * self.azar.entre(2.0, 3.4);
        let largo = (alto * self.azar.entre(0.3, 0.85)).max(3.0) as u16;
        let digitos = (0..=largo).map(|_| self.azar.digito()).collect();
        self.gotas.push(Gota {
            col,
            cabeza,
            vel,
            largo,
            digitos,
        });
    }

    pub fn tiempo(&self) -> f32 {
        self.t
    }

    /// Avanza `dt` segundos.
    pub fn avanzar(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.t += dt;
        for g in &mut self.gotas {
            g.cabeza += g.vel * dt;
        }
        // Algunos dígitos cambian solos, como si parpadearan.
        let cambios = (self.gotas.len() as f32 * 2.0 * dt * 10.0) as usize + 1;
        for _ in 0..cambios {
            if self.gotas.is_empty() {
                break;
            }
            let i = (self.azar.siguiente() % self.gotas.len() as u64) as usize;
            let n = self.gotas[i].digitos.len() as u64;
            let j = (self.azar.siguiente() % n) as usize;
            let d = self.azar.digito();
            self.gotas[i].digitos[j] = d;
        }
        let alto = self.alto;
        self.gotas.retain(|g| !g.se_fue(alto));
        // Mientras llueve, una columna cuya gota ya va abajo puede soltar otra.
        if self.t < LLUEVE {
            for col in 0..self.ancho {
                let ocupada = self
                    .gotas
                    .iter()
                    .any(|g| g.col == col && g.cabeza - (g.largo as f32) < self.alto as f32 * 0.35);
                if !ocupada && self.azar.real() < 6.0 * dt {
                    self.nacer(col, 0.0);
                }
            }
        }
    }

    /// Se acabó: ya no llueve y no queda ninguna gota en la pantalla (o se pasó del tope).
    pub fn terminada(&self) -> bool {
        self.t >= TOPE || (self.t >= LLUEVE && self.gotas.is_empty())
    }

    /// Cómo se ve la pantalla ahora, fila por fila.
    pub fn rejilla(&self) -> Vec<Celda> {
        let (ancho, alto) = (self.ancho as usize, self.alto as usize);
        let mut r = vec![Celda::VACIA; ancho * alto];
        for g in &self.gotas {
            let cabeza = g.cabeza.floor() as i64;
            for k in 0..=g.largo as i64 {
                let fila = cabeza - k;
                if fila < 0 || fila >= alto as i64 {
                    continue;
                }
                let luz = if k == 0 {
                    255
                } else {
                    // La cola se apaga hacia arriba: de casi blanco a gris oscuro.
                    let f = 1.0 - k as f32 / (g.largo as f32 + 1.0);
                    // En escalones de 16: menos cambios de color que mandar por cuadro.
                    ((48.0 + 180.0 * f.powf(1.4)) as u8) & !15
                };
                let i = fila as usize * ancho + g.col as usize;
                if luz > r[i].luz {
                    r[i] = Celda {
                        ch: g.digitos[k as usize],
                        luz,
                    };
                }
            }
        }
        r
    }

    /// El texto ANSI que lleva la pantalla del cuadro anterior a este: solo las celdas que cambiaron.
    pub fn dibujar(&mut self) -> String {
        let ahora = self.rejilla();
        let ancho = self.ancho as usize;
        let mut s = String::new();
        let mut cursor: Option<usize> = None;
        let mut luz_actual: Option<u8> = None;
        for (i, (nueva, vieja)) in ahora.iter().zip(self.previo.iter()).enumerate() {
            if nueva == vieja {
                continue;
            }
            if cursor != Some(i) {
                s.push_str(&format!("\x1b[{};{}H", i / ancho + 1, i % ancho + 1));
            }
            if nueva.ch != b' ' && luz_actual != Some(nueva.luz) {
                let l = nueva.luz;
                if l == 255 {
                    s.push_str("\x1b[1;38;2;255;255;255m");
                } else {
                    s.push_str(&format!("\x1b[22;38;2;{l};{l};{l}m"));
                }
                luz_actual = Some(l);
            }
            s.push(nueva.ch as char);
            // Escribir en la última columna no avanza el cursor a la fila de abajo.
            cursor = if (i + 1) % ancho == 0 { None } else { Some(i + 1) };
        }
        self.previo = ahora;
        s
    }
}
