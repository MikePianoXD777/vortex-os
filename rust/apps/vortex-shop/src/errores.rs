//! Errores de la tienda. Todos llevan un mensaje en español listo para mostrárselo a la persona.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No hay forma de hablar con Flathub y tampoco hay nada guardado en la caché.
    SinRed(String),
    /// Una operación de Flatpak falló por falta de internet: cuenta a la vez como `SinRed` y como `Flatpak`.
    FlatpakSinRed(String),
    /// Flatpak no pudo hacer lo que se le pidió (instalar, quitar, actualizar, abrir…).
    Flatpak(String),
    /// Flathub no tiene esa app (o el identificador no es válido). También cuenta como `Flatpak`.
    NoEncontrada(String),
    /// No se pudo instalar o quitar una app propia de Vortexine's.
    Propia(String),
    /// Algo de Android no salió.
    Android(String),
    /// Cualquier otra cosa (un fallo del programa: se muestra un mensaje genérico).
    Otro(String),
}

pub type Resultado<T> = Result<T, Error>;

impl Error {
    pub fn sin_red() -> Self {
        Error::SinRed("No hay conexión con Flathub. Revisa tu internet e inténtalo de nuevo.".into())
    }

    pub fn flatpak_sin_red() -> Self {
        Error::FlatpakSinRed("No hay conexión a internet. Revísala e inténtalo de nuevo.".into())
    }

    pub fn flatpak(m: impl Into<String>) -> Self {
        Error::Flatpak(m.into())
    }

    pub fn no_encontrada(m: impl Into<String>) -> Self {
        Error::NoEncontrada(m.into())
    }

    /// ¿Es de falta de internet? (`SinRed` o `FlatpakSinRed`)
    pub fn es_sin_red(&self) -> bool {
        matches!(self, Error::SinRed(_) | Error::FlatpakSinRed(_))
    }

    /// ¿Es de Flatpak? (`Flatpak`, `NoEncontrada` o `FlatpakSinRed`)
    pub fn es_flatpak(&self) -> bool {
        matches!(
            self,
            Error::Flatpak(_) | Error::NoEncontrada(_) | Error::FlatpakSinRed(_)
        )
    }

    pub fn mensaje(&self) -> &str {
        match self {
            Error::SinRed(m)
            | Error::FlatpakSinRed(m)
            | Error::Flatpak(m)
            | Error::NoEncontrada(m)
            | Error::Propia(m)
            | Error::Android(m)
            | Error::Otro(m) => m,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mensaje())
    }
}

impl std::error::Error for Error {}
