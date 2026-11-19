//! La voz de Miku en Rust (`miku-voz`): Supertonic 3 sobre onnxruntime, el mismo protocolo de líneas de JSON
//! que `miku/voz_worker.py` (que sigue para ElevenLabs y como respaldo con `VORTEX_MIKU_VOZ_PYTHON=1`).

#![allow(clippy::cast_possible_truncation)]

pub mod audio;
pub mod estilo;
pub mod motor;
pub mod texto;
