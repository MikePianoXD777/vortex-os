//! `vortex-lluvia`: la lluvia de dígitos que abre la terminal antes del saludo (ver lib.rs).
//!
//!   vortex-lluvia        llueve ~1 s en la pantalla alterna y la deja como estaba
//!
//! Cualquier tecla la salta (y no llega a la terminal). No hace nada si la salida o la entrada no son
//! una terminal, si la ventana es muy chica, con VORTEX_SIN_LLUVIA=1 o si existe
//! ~/.config/vortex/sin-lluvia («vortex saludo lluvia off»).

// termios, el tamaño de la ventana, poll e isatty no tienen versión segura en la biblioteca estándar:
// son llamadas de libc puntuales, cada una con su SAFETY. La lógica (lib.rs) no tiene unsafe.
#![allow(unsafe_code)]

use std::io::Write;
use std::time::{Duration, Instant};

use vortex_lluvia::{Lluvia, debe_correr};

/// Un cuadro cada 8 ms (~120 por segundo): la terminal dibuja a la frecuencia del monitor.
const CUADRO: Duration = Duration::from_millis(8);

fn apagada() -> bool {
    if std::env::var_os("VORTEX_SIN_LLUVIA").is_some_and(|v| !v.is_empty() && v != "0") {
        return true;
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")));
    config.is_some_and(|c| c.join("vortex/sin-lluvia").exists())
}

fn tamano() -> (u16, u16) {
    // SAFETY: TIOCGWINSZ solo llena la estructura que le pasamos.
    unsafe {
        let mut w: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut w) == 0 {
            (w.ws_col, w.ws_row)
        } else {
            (0, 0)
        }
    }
}

/// Deja la terminal como estaba aunque algo truene: modo de teclado, cursor y pantalla normal.
struct Terminal {
    original: libc::termios,
}

impl Terminal {
    fn tomar() -> Option<Terminal> {
        // SAFETY: tcgetattr/tcsetattr sobre la entrada estándar, que ya sabemos que es una terminal.
        unsafe {
            let mut original: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut original) != 0 {
                return None;
            }
            let mut crudo = original;
            // Sin eco, sin esperar Enter y sin señales: una tecla (Ctrl+C incluida) solo la salta.
            crudo.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
            crudo.c_cc[libc::VMIN] = 0;
            crudo.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &crudo) != 0 {
                return None;
            }
            // Pantalla alterna: al salir, la terminal queda igual que antes, sin rastro de la lluvia.
            print!("\x1b[?1049h\x1b[?25l\x1b[2J");
            let _ = std::io::stdout().flush();
            Some(Terminal { original })
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = std::io::stdout().flush();
        // SAFETY: tira lo que se haya tecleado durante la lluvia y regresa el modo original.
        unsafe {
            libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH);
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.original);
        }
    }
}

/// Espera hasta `espera` a que llegue una tecla. Solo cuenta si de verdad se leyó algo: una entrada
/// cerrada (EOF o colgada) no es una tecla; entonces se espera el cuadro completo y se deja de mirarla.
fn tecla(espera: Duration, mirar: &mut bool) -> bool {
    if !*mirar {
        std::thread::sleep(espera);
        return false;
    }
    let mut fd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll sobre un solo descriptor válido.
    if unsafe { libc::poll(&mut fd, 1, espera.as_millis() as libc::c_int) } <= 0 {
        return false;
    }
    let mut b = [0u8; 64];
    // SAFETY: read a un búfer propio del tamaño que le decimos.
    let n = unsafe { libc::read(libc::STDIN_FILENO, b.as_mut_ptr().cast(), b.len()) };
    if n > 0 {
        return true;
    }
    *mirar = false;
    false
}

fn main() {
    // SAFETY: isatty solo pregunta.
    let (salida, entrada) = unsafe {
        (
            libc::isatty(libc::STDOUT_FILENO) == 1,
            libc::isatty(libc::STDIN_FILENO) == 1,
        )
    };
    let (ancho, alto) = tamano();
    if !debe_correr(salida, entrada, ancho, alto, apagada()) {
        return;
    }
    let Some(_terminal) = Terminal::tomar() else {
        return;
    };

    let semilla = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(7)
        ^ std::process::id() as u64;
    let mut lluvia = Lluvia::new(ancho, alto, semilla);
    let mut salida = std::io::stdout().lock();
    let mut antes = Instant::now();
    let mut mirar = true;
    while !lluvia.terminada() {
        let ahora = Instant::now();
        lluvia.avanzar((ahora - antes).as_secs_f32());
        antes = ahora;
        let cuadro = lluvia.dibujar();
        if salida
            .write_all(cuadro.as_bytes())
            .and_then(|()| salida.flush())
            .is_err()
        {
            break;
        }
        let gastado = ahora.elapsed();
        if tecla(CUADRO.saturating_sub(gastado), &mut mirar) {
            break;
        }
    }
}
