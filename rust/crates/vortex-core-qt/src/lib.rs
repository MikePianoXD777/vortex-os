//! `vortex-core-qt`: lo de `vortex-core` que QML necesita ver.
//!
//! - `Colores` (singleton QML, módulo `Vortex.Colores`): `Colores.paleta`, el mapa nombre → color de
//!   `vortex_core::paleta`, que se actualiza solo cuando cambia el fondo de pantalla. `Tema.qml` lo lee
//!   igual que hoy lee `colores.paleta` en las apps de Python.

pub mod colores;
