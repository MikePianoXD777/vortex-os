//! La compañera de escritorio de Vortexine's OS (4.1): un sprite de pixel art, diseño original de
//! Vortexine's (audífonos blancos con morado, sudadera negra), que camina por la orilla de abajo de la
//! pantalla, se sube a las ventanas, se cae si las mueves, se sienta, se duerme cuando no usas la PC y
//! se esconde cuando juegas.
//!
//! - `arte`: el dibujo (cuadros de 32×48 escritos como texto) y la letra de los globitos.
//! - `mundo`: lo que hace en cada momento (pura lógica: se prueba sin ventanas).
//! - `hypr`: lo que le pregunta a Hyprland (pantallas, ventanas, cursor).
//! - `png`: para sacar la hoja de cuadros a un PNG (`vortex-mascota hoja ARCHIVO.png`).
pub mod arte;
pub mod aspecto;
pub mod hypr;
pub mod lienzo;
pub mod mundo;
pub mod png;
