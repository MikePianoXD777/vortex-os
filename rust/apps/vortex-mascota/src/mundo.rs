//! Lo que hace la compañera en cada momento. Pura lógica: recibe el entorno (tamaño de la pantalla,
//! orillas de las ventanas, cuánto lleva quieto el mouse) y el tiempo, y dice dónde está y qué está
//! haciendo. Las pruebas la mueven sin abrir ninguna ventana.
//!
//! Coordenadas de la pantalla donde vive, en pixeles lógicos, con y hacia abajo. `x` es el centro de los
//! pies y `y` la línea donde pisa.

use crate::arte;

/// Lo que dibuja la ventana: cada acción tiene su animación (ver `aspecto`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Anim {
    Parada,
    Caminando,
    Sentada,
    Dormida,
    Saludando,
    Agarrada,
    Cayendo,
    Aterrizando,
    /// Le hiciste clic (con la original, saluda).
    Caricia,
    /// Se despierta (con la original, saluda).
    Despertando,
    /// Se despide al quitarla (con la original, saluda).
    Despedida,
    /// Un pasatiempo del paquete (café, celular, leer…); la original no tiene y se queda parada.
    Ocio(u8),
}

/// Una orilla donde se puede parar: la de arriba de una ventana, o el piso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orilla {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entorno {
    pub ancho: f32,
    /// Donde está el piso (abajo, encima de lo que reserva la barra si está abajo).
    pub piso: f32,
    /// Las orillas de arriba de las ventanas del escritorio que se ve.
    pub orillas: Vec<Orilla>,
    /// Segundos sin que se mueva el mouse.
    pub mouse_quieto: f32,
    /// Hay un juego o algo en pantalla completa: se esconde.
    pub esconder: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Accion {
    Parada { falta: f32 },
    Caminando { hasta: f32 },
    Sentada { falta: f32 },
    Dormida,
    Agarrada,
    Cayendo,
    Aterrizando { falta: f32 },
    Despidiendose { falta: f32 },
    Caricia { falta: f32 },
    Despertando { falta: f32 },
    Ocio { cual: u8, falta: f32 },
}

/// Cuánto tarda en dormirse sin que muevas el mouse.
pub const SUENO: f32 = 120.0;
const GRAVEDAD: f32 = 1800.0;
const VELOCIDAD: f32 = 45.0;
const VELOCIDAD_MAX_CAIDA: f32 = 1400.0;

pub struct Mascota {
    pub x: f32,
    pub y: f32,
    /// -1 a la izquierda, 1 a la derecha.
    pub dir: i8,
    vy: f32,
    accion: Accion,
    /// Tiempo dentro de la acción (para la animación).
    pub t: f32,
    pub frase: Option<&'static str>,
    frase_falta: f32,
    /// Medio ancho del muñeco: no se sale de la pantalla.
    pub medio_ancho: f32,
    azar: u64,
    pub terminar: bool,
    /// Cuántos pasatiempos tiene el aspecto (0: ninguno).
    pub ocios: u8,
}

impl Mascota {
    /// Aparece arriba, en `x`, y cae hasta la primera orilla que tenga abajo.
    pub fn nueva(x: f32, medio_ancho: f32, semilla: u64) -> Self {
        Self {
            x,
            y: 0.0,
            dir: 1,
            vy: 0.0,
            accion: Accion::Cayendo,
            t: 0.0,
            frase: None,
            frase_falta: 0.0,
            medio_ancho,
            azar: semilla | 1,
            terminar: false,
            ocios: 0,
        }
    }

    fn azar(&mut self) -> f32 {
        // xorshift: no hace falta más para decidir si camina o se sienta.
        self.azar ^= self.azar << 13;
        self.azar ^= self.azar >> 7;
        self.azar ^= self.azar << 17;
        (self.azar % 10_000) as f32 / 10_000.0
    }

    fn entre(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.azar()
    }

    pub fn anim(&self) -> Anim {
        match self.accion {
            Accion::Parada { .. } => Anim::Parada,
            Accion::Caminando { .. } => Anim::Caminando,
            Accion::Sentada { .. } => Anim::Sentada,
            Accion::Dormida => Anim::Dormida,
            Accion::Despidiendose { .. } => Anim::Despedida,
            Accion::Caricia { .. } => Anim::Caricia,
            Accion::Despertando { .. } => Anim::Despertando,
            Accion::Ocio { cual, .. } => Anim::Ocio(cual),
            Accion::Agarrada => Anim::Agarrada,
            Accion::Cayendo => Anim::Cayendo,
            Accion::Aterrizando { .. } => Anim::Aterrizando,
        }
    }

    pub fn emote(&self) -> Option<arte::Emote> {
        match self.accion {
            Accion::Dormida => Some(arte::Emote::Zeta),
            Accion::Caricia { .. } | Accion::Despertando { .. } if self.frase.is_none() => {
                Some(arte::Emote::Corazon)
            }
            Accion::Sentada { .. } if self.t % 12.0 < 3.0 => Some(arte::Emote::Nota),
            _ => None,
        }
    }

    /// ¿Se está moviendo o animando rápido? (si no, la ventana se redibuja menos seguido)
    pub fn activa(&self) -> bool {
        matches!(
            self.accion,
            Accion::Caminando { .. } | Accion::Cayendo | Accion::Agarrada | Accion::Aterrizando { .. }
        )
    }

    fn cambiar(&mut self, a: Accion) {
        self.accion = a;
        self.t = 0.0;
    }

    fn decir(&mut self, frase: &'static str, segundos: f32) {
        self.frase = Some(frase);
        self.frase_falta = segundos;
    }

    /// La orilla (o el piso) donde está parada, si sigue ahí.
    fn pisa(&self, e: &Entorno) -> Option<Orilla> {
        let piso = Orilla {
            x0: 0.0,
            x1: e.ancho,
            y: e.piso,
        };
        std::iter::once(piso)
            .chain(e.orillas.iter().copied())
            .find(|o| (o.y - self.y).abs() <= 2.0 && self.x >= o.x0 && self.x <= o.x1)
    }

    fn limitar_x(&mut self, e: &Entorno) {
        let (min, max) = (
            self.medio_ancho,
            (e.ancho - self.medio_ancho).max(self.medio_ancho),
        );
        self.x = self.x.clamp(min, max);
    }

    pub fn paso(&mut self, dt: f32, e: &Entorno) {
        self.t += dt;
        if self.frase.is_some() {
            self.frase_falta -= dt;
            if self.frase_falta <= 0.0 {
                self.frase = None;
            }
        }
        match self.accion {
            Accion::Agarrada => return,
            Accion::Despidiendose { falta } => {
                if falta - dt <= 0.0 {
                    self.terminar = true;
                } else {
                    self.accion = Accion::Despidiendose { falta: falta - dt };
                }
                return;
            }
            Accion::Cayendo => {
                self.caer(dt, e);
                return;
            }
            _ => {}
        }
        // Le movieron (o cerraron) la ventana donde estaba: se cae.
        let Some(orilla) = self.pisa(e) else {
            self.vy = 0.0;
            self.cambiar(Accion::Cayendo);
            return;
        };
        // Sin mover el mouse un rato, se duerme; al moverlo, despierta.
        if e.mouse_quieto >= SUENO && self.accion != Accion::Dormida {
            self.decir(arte::FRASE_SUENO, 2.5);
            self.cambiar(Accion::Dormida);
            return;
        }
        match self.accion {
            Accion::Dormida => {
                if e.mouse_quieto < 1.0 {
                    self.decir(arte::FRASE_DESPIERTA, 2.0);
                    self.cambiar(Accion::Despertando { falta: 2.0 });
                }
            }
            Accion::Parada { falta } => {
                if falta - dt > 0.0 {
                    self.accion = Accion::Parada { falta: falta - dt };
                } else {
                    self.decidir(e, orilla);
                }
            }
            Accion::Sentada { falta }
            | Accion::Aterrizando { falta }
            | Accion::Caricia { falta }
            | Accion::Despertando { falta }
            | Accion::Ocio { falta, .. } => {
                let resto = falta - dt;
                if resto > 0.0 {
                    self.accion = match self.accion {
                        Accion::Sentada { .. } => Accion::Sentada { falta: resto },
                        Accion::Caricia { .. } => Accion::Caricia { falta: resto },
                        Accion::Despertando { .. } => Accion::Despertando { falta: resto },
                        Accion::Ocio { cual, .. } => Accion::Ocio { cual, falta: resto },
                        _ => Accion::Aterrizando { falta: resto },
                    };
                } else {
                    let espera = self.entre(1.5, 4.0);
                    self.cambiar(Accion::Parada { falta: espera });
                }
            }
            Accion::Caminando { hasta } => self.caminar(dt, e, orilla, hasta),
            Accion::Agarrada | Accion::Cayendo | Accion::Despidiendose { .. } => {}
        }
    }

    fn decidir(&mut self, e: &Entorno, orilla: Orilla) {
        let r = self.azar();
        if r < 0.55 {
            // Camina a un punto de la orilla donde está (en el piso, de toda la pantalla).
            let (min, max) = (
                orilla.x0.max(self.medio_ancho),
                orilla.x1.min(e.ancho - self.medio_ancho),
            );
            let hasta = if max > min { self.entre(min, max) } else { self.x };
            // En una ventana, a veces camina hasta la orilla y se cae a propósito.
            let hasta = if orilla.y < e.piso - 2.0 && self.azar() < 0.2 {
                if self.azar() < 0.5 {
                    orilla.x0 - 20.0
                } else {
                    orilla.x1 + 20.0
                }
            } else {
                hasta
            };
            self.dir = if hasta >= self.x { 1 } else { -1 };
            self.cambiar(Accion::Caminando { hasta });
        } else if r < 0.75 {
            let falta = self.entre(6.0, 15.0);
            self.cambiar(Accion::Sentada { falta });
        } else if r < 0.92 && self.ocios > 0 {
            let cual = (self.azar() * f32::from(self.ocios)) as u8 % self.ocios;
            let falta = self.entre(4.0, 10.0);
            self.cambiar(Accion::Ocio { cual, falta });
        } else {
            let falta = self.entre(2.0, 5.0);
            self.cambiar(Accion::Parada { falta });
        }
    }

    fn caminar(&mut self, dt: f32, e: &Entorno, orilla: Orilla, hasta: f32) {
        let paso = VELOCIDAD * dt * f32::from(self.dir);
        let llego = (self.dir > 0 && self.x + paso >= hasta) || (self.dir < 0 && self.x + paso <= hasta);
        self.x = if llego { hasta } else { self.x + paso };
        // En el piso no se sale de la pantalla; en una ventana, si se pasa de la orilla, cae.
        if orilla.y >= e.piso - 2.0 {
            self.limitar_x(e);
        }
        if llego || self.x <= self.medio_ancho || self.x >= e.ancho - self.medio_ancho {
            let espera = self.entre(1.0, 3.0);
            self.cambiar(Accion::Parada { falta: espera });
        }
    }

    fn caer(&mut self, dt: f32, e: &Entorno) {
        let antes = self.y;
        self.vy = (self.vy + GRAVEDAD * dt).min(VELOCIDAD_MAX_CAIDA);
        let despues = self.y + self.vy * dt;
        self.limitar_x(e);
        // La orilla más alta que cruzó al caer (el piso siempre la detiene).
        let mut aterriza = e.piso;
        for o in &e.orillas {
            if o.y >= antes && o.y <= despues && self.x >= o.x0 && self.x <= o.x1 && o.y < aterriza {
                aterriza = o.y;
            }
        }
        if despues >= aterriza {
            self.y = aterriza;
            self.vy = 0.0;
            self.cambiar(Accion::Aterrizando { falta: 0.35 });
        } else {
            self.y = despues;
        }
    }

    // ---- Lo que hace la persona con el mouse ------------------------------------------------------------

    pub fn agarrar(&mut self) {
        self.frase = None;
        self.cambiar(Accion::Agarrada);
    }

    /// Mientras la arrastras: los pies van donde la lleves.
    pub fn arrastrar(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn soltar(&mut self) {
        self.vy = 0.0;
        self.cambiar(Accion::Cayendo);
    }

    /// Un clic: saluda y dice algo.
    pub fn clic(&mut self) {
        if matches!(
            self.accion,
            Accion::Cayendo | Accion::Agarrada | Accion::Despidiendose { .. }
        ) {
            return;
        }
        let i = (self.azar() * arte::FRASES.len() as f32) as usize % arte::FRASES.len();
        self.decir(arte::FRASES[i], 2.5);
        self.cambiar(Accion::Caricia { falta: 2.5 });
    }

    /// Clic derecho: se despide y se va.
    pub fn despedir(&mut self) {
        self.decir(arte::FRASE_ADIOS, 1.5);
        self.cambiar(Accion::Despidiendose { falta: 1.5 });
    }

    pub fn dormida(&self) -> bool {
        self.accion == Accion::Dormida
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entorno() -> Entorno {
        Entorno {
            ancho: 1920.0,
            piso: 1040.0,
            orillas: vec![],
            mouse_quieto: 0.0,
            esconder: false,
        }
    }

    fn correr(m: &mut Mascota, e: &Entorno, segundos: f32) {
        let pasos = (segundos * 30.0) as usize;
        for _ in 0..pasos {
            m.paso(1.0 / 30.0, e);
        }
    }

    #[test]
    fn aparece_y_cae_al_piso() {
        let mut m = Mascota::nueva(500.0, 48.0, 7);
        correr(&mut m, &entorno(), 3.0);
        assert_eq!(m.y, 1040.0);
        assert_ne!(m.anim(), Anim::Cayendo);
    }

    #[test]
    fn cae_en_la_ventana_que_tiene_abajo() {
        let mut e = entorno();
        e.orillas.push(Orilla {
            x0: 300.0,
            x1: 900.0,
            y: 400.0,
        });
        let mut m = Mascota::nueva(500.0, 48.0, 7);
        m.paso(1.0 / 30.0, &e);
        // Sin dejar que decida caminar fuera: solo lo que tarda en caer.
        for _ in 0..40 {
            if m.anim() != Anim::Cayendo {
                break;
            }
            m.paso(1.0 / 30.0, &e);
        }
        assert_eq!(m.y, 400.0);
    }

    #[test]
    fn si_quitas_la_ventana_se_cae() {
        let mut e = entorno();
        e.orillas.push(Orilla {
            x0: 300.0,
            x1: 900.0,
            y: 400.0,
        });
        let mut m = Mascota::nueva(500.0, 48.0, 7);
        for _ in 0..40 {
            m.paso(1.0 / 30.0, &e);
        }
        assert_eq!(m.y, 400.0);
        e.orillas.clear();
        m.paso(1.0 / 30.0, &e);
        assert_eq!(m.anim(), Anim::Cayendo);
        correr(&mut m, &e, 2.0);
        assert_eq!(m.y, 1040.0);
    }

    #[test]
    fn camina_sin_salirse_de_la_pantalla() {
        let e = entorno();
        let mut m = Mascota::nueva(60.0, 48.0, 12345);
        let mut camino = false;
        for _ in 0..(120 * 30) {
            m.paso(1.0 / 30.0, &e);
            camino |= m.anim() == Anim::Caminando;
            assert!(m.x >= 48.0 && m.x <= 1920.0 - 48.0, "x = {}", m.x);
        }
        assert!(camino, "en dos minutos debió caminar alguna vez");
    }

    #[test]
    fn se_duerme_sin_mouse_y_despierta() {
        let mut e = entorno();
        let mut m = Mascota::nueva(500.0, 48.0, 3);
        correr(&mut m, &e, 2.0);
        e.mouse_quieto = SUENO + 1.0;
        m.paso(0.1, &e);
        assert!(m.dormida());
        assert_eq!(m.emote(), Some(arte::Emote::Zeta));
        correr(&mut m, &e, 10.0);
        assert!(m.dormida(), "sigue dormida mientras no muevas el mouse");
        e.mouse_quieto = 0.0;
        m.paso(0.1, &e);
        assert_eq!(m.anim(), Anim::Despertando);
        assert_eq!(m.frase, Some(arte::FRASE_DESPIERTA));
    }

    #[test]
    fn con_pasatiempos_a_veces_hace_uno() {
        let e = entorno();
        let mut m = Mascota::nueva(500.0, 48.0, 99);
        m.ocios = 5;
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..(600 * 30) {
            m.paso(1.0 / 30.0, &e);
            if let Anim::Ocio(n) = m.anim() {
                assert!(n < 5);
                vistos.insert(n);
            }
        }
        assert!(vistos.len() >= 2, "en 10 minutos debió hacer varios: {vistos:?}");
        // Sin pasatiempos nunca hace uno.
        let mut m = Mascota::nueva(500.0, 48.0, 99);
        for _ in 0..(600 * 30) {
            m.paso(1.0 / 30.0, &e);
            assert!(!matches!(m.anim(), Anim::Ocio(_)));
        }
    }

    #[test]
    fn la_agarras_la_sueltas_y_cae() {
        let e = entorno();
        let mut m = Mascota::nueva(500.0, 48.0, 3);
        correr(&mut m, &e, 2.0);
        m.agarrar();
        m.arrastrar(800.0, 300.0);
        correr(&mut m, &e, 1.0);
        assert_eq!((m.x, m.y), (800.0, 300.0), "agarrada no se mueve sola");
        m.soltar();
        correr(&mut m, &e, 2.0);
        assert_eq!(m.y, 1040.0);
    }

    #[test]
    fn clic_saluda_y_clic_derecho_se_va() {
        let e = entorno();
        let mut m = Mascota::nueva(500.0, 48.0, 3);
        correr(&mut m, &e, 2.0);
        m.clic();
        assert_eq!(m.anim(), Anim::Caricia);
        assert!(m.frase.is_some());
        m.despedir();
        correr(&mut m, &e, 2.0);
        assert!(m.terminar);
    }
}
