//! Vortexine Task Manager: el motor, sin Qt (lo mismo que `sistema.py`, `servicios.py` y la lógica de
//! `puente.py`). La ventana está en `qt/` (cxx-qt) y usa los mismos QML que la de Python.
//!
//! - [`procesos`]: lee `/proc`, nombres, CPU, memoria y disco de cada proceso.
//! - [`lanzadores`]: el índice de lanzadores `.desktop` (nombre e ícono de cada app).
//! - [`grupos`]: apps, segundo plano y sistema; ordenar y buscar.
//! - [`acciones`]: finalizar, suspender, prioridad, cerrar ventanas.
//! - [`rendimiento`]: CPU, memoria, discos, red y tarjetas de video.
//! - [`vram`]: la memoria de la gráfica de cada proceso.
//! - [`servicios`]: los servicios de systemd.
//! - [`lista`], [`modelo`], [`historia`], [`vista`]: lo que ve QML (filas, diferencias, gráficas).

pub mod acciones;
pub mod ejecutar;
pub mod grupos;
pub mod historia;
pub mod instancia;
pub mod lanzadores;
pub mod lista;
pub mod modelo;
pub mod procesos;
pub mod rendimiento;
pub mod servicios;
pub mod sys;
pub mod vista;
pub mod vram;

pub const VERSION: &str = "1.0.2";
pub const NOMBRE: &str = "Vortexine Task Manager";
pub const APP_ID: &str = "vortex-task-manager";
