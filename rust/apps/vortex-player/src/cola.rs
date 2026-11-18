//! Lógica pura de la cola de reproducción: orden, aleatorio, repetir, «A continuación» y sesión.

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

/// Modo de repetición de la cola.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repetir {
    #[default]
    No,
    Todo,
    Una,
}

impl Repetir {
    pub fn siguiente_modo(self) -> Self {
        match self {
            Self::No => Self::Todo,
            Self::Todo => Self::Una,
            Self::Una => Self::No,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Todo => "todo",
            Self::Una => "una",
        }
    }

    pub fn desde_str(s: &str) -> Self {
        match s {
            "todo" => Self::Todo,
            "una" => Self::Una,
            _ => Self::No,
        }
    }
}

/// Elemento mínimo de la cola de reproducción.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pista {
    pub ruta: String,
    pub titulo: String,
    pub artista: String,
    pub album: String,
    pub duracion: f64,
}

/// Estado serializable para restaurar la sesión al volver a abrir la app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SesionCola {
    pub rutas: Vec<String>,
    pub orden: Vec<usize>,
    pub i: isize,
    pub posicion: f64,
    pub aleatorio: bool,
    pub repetir: String,
    pub origen: String,
}

/// Gestor puro de cola de reproducción (sin procesos externos ni E/S).
#[derive(Debug, Clone, Default)]
pub struct Cola {
    pistas: Vec<Pista>,
    orden: Vec<usize>,
    indice: Option<usize>,
    aleatorio: bool,
    repetir: Repetir,
    despues: usize,
    origen: String,
}

impl Cola {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// Inicia una nueva cola desde `inicio` (en modo aleatorio, esa va primero).
    pub fn tocar(&mut self, pistas: Vec<Pista>, inicio: usize) -> Option<&Pista> {
        self.pistas = pistas.into_iter().filter(|p| !p.ruta.is_empty()).collect();
        if self.pistas.is_empty() {
            self.orden.clear();
            self.indice = None;
            self.despues = 0;
            return None;
        }
        let inicio = inicio.min(self.pistas.len() - 1);
        self.despues = 0;
        if self.aleatorio {
            let mut resto: Vec<usize> = (0..self.pistas.len()).filter(|&i| i != inicio).collect();
            resto.shuffle(&mut rand::rng());
            self.orden = std::iter::once(inicio).chain(resto).collect();
            self.indice = Some(0);
        } else {
            self.orden = (0..self.pistas.len()).collect();
            self.indice = Some(inicio);
        }
        self.actual()
    }

    /// Pista actualmente seleccionada.
    pub fn actual(&self) -> Option<&Pista> {
        let i = self.indice?;
        let idx = *self.orden.get(i)?;
        self.pistas.get(idx)
    }

    /// Índice actual dentro de `orden`.
    pub fn indice(&self) -> Option<usize> {
        self.indice
    }

    pub fn aleatorio(&self) -> bool {
        self.aleatorio
    }

    pub fn repetir(&self) -> Repetir {
        self.repetir
    }

    pub fn origen(&self) -> &str {
        &self.origen
    }

    pub fn poner_origen(&mut self, origen: impl Into<String>) {
        self.origen = origen.into();
    }

    /// Canciones en el orden efectivo de reproducción.
    pub fn lista_ordenada(&self) -> Vec<&Pista> {
        self.orden
            .iter()
            .filter_map(|&idx| self.pistas.get(idx))
            .collect()
    }

    /// Inserta una pista justo después de la actual y de las ya encoladas con «tocar después».
    pub fn tocar_despues(&mut self, pista: Pista) {
        if pista.ruta.is_empty() {
            return;
        }
        if self.orden.is_empty() {
            self.tocar(vec![pista], 0);
            return;
        }
        let nueva_idx = self.pistas.len();
        self.pistas.push(pista);
        let pos = self.indice.unwrap_or(0) + 1 + self.despues;
        let pos = pos.min(self.orden.len());
        self.orden.insert(pos, nueva_idx);
        self.despues += 1;
    }

    /// Añade una pista al final de la cola.
    pub fn agregar(&mut self, pista: Pista) {
        if pista.ruta.is_empty() {
            return;
        }
        if self.orden.is_empty() {
            self.tocar(vec![pista], 0);
            return;
        }
        let nueva_idx = self.pistas.len();
        self.pistas.push(pista);
        self.orden.push(nueva_idx);
    }

    /// Avanza a la siguiente pista según `repetir`. Devuelve `None` si terminó la cola.
    pub fn avanzar_automatico(&mut self) -> Option<&Pista> {
        if self.orden.is_empty() {
            return None;
        }
        if self.repetir == Repetir::Una {
            return self.actual();
        }
        let i = self.indice?;
        if i + 1 < self.orden.len() {
            self.indice = Some(i + 1);
            self.despues = self.despues.saturating_sub(1);
            return self.actual();
        }
        if self.repetir == Repetir::Todo {
            if self.aleatorio {
                self.orden.shuffle(&mut rand::rng());
            }
            self.indice = Some(0);
            return self.actual();
        }
        None
    }

    /// Salta manualmente a la siguiente pista (con `repetir != No`, vuelve al inicio al llegar al final).
    pub fn siguiente(&mut self) -> Option<&Pista> {
        let i = self.indice?;
        if i + 1 < self.orden.len() {
            self.indice = Some(i + 1);
            self.despues = self.despues.saturating_sub(1);
            return self.actual();
        }
        if self.repetir != Repetir::No && !self.orden.is_empty() {
            self.indice = Some(0);
            return self.actual();
        }
        None
    }

    /// Retrocede a la pista anterior si `posicion <= 3.0` y hay anterior; si no, indica reiniciar a 0.
    pub fn anterior(&mut self, posicion: f64) -> AccionAnterior {
        if posicion > 3.0 || self.indice.unwrap_or(0) == 0 {
            return AccionAnterior::ReiniciarPosicion;
        }
        if let Some(i) = self.indice
            && i > 0
        {
            self.indice = Some(i - 1);
            self.despues = 0;
            return AccionAnterior::CargarPista;
        }
        AccionAnterior::ReiniciarPosicion
    }

    /// Salta a la posición `k` dentro del orden actual.
    pub fn saltar_a(&mut self, k: usize) -> Option<&Pista> {
        if k < self.orden.len() {
            self.indice = Some(k);
            self.despues = 0;
            self.actual()
        } else {
            None
        }
    }

    /// Quita de la cola la posición `k` (solo si viene después de la actual).
    pub fn quitar_en(&mut self, k: usize) -> bool {
        let Some(i) = self.indice else {
            return false;
        };
        if k > i && k < self.orden.len() {
            if k <= i + self.despues {
                self.despues = self.despues.saturating_sub(1);
            }
            self.orden.remove(k);
            return true;
        }
        false
    }

    /// Alterna el modo aleatorio conservando la pista actual en el índice 0 al activar.
    pub fn alternar_aleatorio(&mut self) {
        self.aleatorio = !self.aleatorio;
        if let Some(i) = self.indice
            && !self.orden.is_empty()
        {
            let actual_idx = self.orden[i];
            if self.aleatorio {
                let mut resto: Vec<usize> = (0..self.pistas.len()).filter(|&x| x != actual_idx).collect();
                resto.shuffle(&mut rand::rng());
                self.orden = std::iter::once(actual_idx).chain(resto).collect();
                self.indice = Some(0);
            } else {
                self.orden = (0..self.pistas.len()).collect();
                self.indice = Some(actual_idx);
            }
        }
    }

    /// Cicla el modo repetir: `no` -> `todo` -> `una` -> `no`.
    pub fn alternar_repetir(&mut self) {
        self.repetir = self.repetir.siguiente_modo();
    }

    /// Indica si la siguiente pista será la de otro índice (para preparar crossfade).
    pub fn siguiente_indice_crossfade(&self) -> Option<usize> {
        let i = self.indice?;
        if i + 1 < self.orden.len() {
            Some(i + 1)
        } else if self.repetir == Repetir::Todo && self.orden.len() > 1 {
            Some(0)
        } else {
            None
        }
    }

    /// Exporta el estado actual para guardarlo entre sesiones.
    pub fn sesion(&self, posicion: f64) -> SesionCola {
        SesionCola {
            rutas: self.pistas.iter().map(|p| p.ruta.clone()).collect(),
            orden: self.orden.clone(),
            i: self.indice.map(|i| i as isize).unwrap_or(-1),
            posicion: (posicion * 100.0).round() / 100.0,
            aleatorio: self.aleatorio,
            repetir: self.repetir.as_str().to_string(),
            origen: self.origen.clone(),
        }
    }
}

/// Resultado de pedir la canción anterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccionAnterior {
    ReiniciarPosicion,
    CargarPista,
}
