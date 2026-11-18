//! El motor tal como lo usa la interfaz: el catálogo de Flathub y las operaciones de flatpak detrás de un
//! trait, para que las pruebas de la interfaz usen un doble falso (`tests/comun/falso.rs`) sin red y sin
//! tocar flatpak. En producción es [`MotorReal`].

use crate::catalogo::{App, AppDetalle, Catalogo, Categoria};
use crate::errores::Resultado;
use crate::flatpak::{Flatpak, Progreso};

pub trait Motor: Send + Sync {
    /// Apps que trae cada página de una categoría (`None`: se supone que hay más si la página trajo algo).
    fn por_pagina(&self) -> Option<usize>;
    fn destacados(&self) -> Resultado<Vec<App>>;
    fn recomendadas(&self) -> Resultado<Vec<App>>;
    fn categorias(&self) -> Resultado<Vec<Categoria>>;
    fn por_categoria(&self, cat_id: &str, pagina: i64) -> Resultado<Vec<App>>;
    fn buscar(&self, texto: &str) -> Resultado<Vec<App>>;
    fn detalle(&self, app_id: &str) -> Resultado<AppDetalle>;

    fn instaladas(&self) -> Resultado<Vec<String>>;
    fn es_del_sistema(&self, app_id: &str) -> Resultado<bool>;
    fn actualizaciones(&self) -> Resultado<Vec<String>>;
    fn preparar(&self) -> Resultado<()>;
    fn instalar(&self, app_id: &str, progreso: Progreso) -> Resultado<()>;
    fn quitar(&self, app_id: &str, progreso: Progreso) -> Resultado<()>;
    fn actualizar(&self, app_ids: &[String], progreso: Progreso) -> Resultado<()>;
    fn abrir(&self, app_id: &str) -> Resultado<()>;
}

/// Los módulos de verdad: el catálogo de Flathub (con `curl`) y la CLI de `flatpak`.
pub struct MotorReal {
    pub catalogo: Catalogo,
    pub flatpak: Flatpak,
}

impl MotorReal {
    pub fn nuevo(catalogo: Catalogo, flatpak: Flatpak) -> Self {
        MotorReal { catalogo, flatpak }
    }

    /// El de siempre: todo con el entorno de este proceso.
    pub fn del_entorno() -> Self {
        Self::nuevo(Catalogo::nuevo(None, "es", None), Flatpak::default())
    }
}

impl Motor for MotorReal {
    fn por_pagina(&self) -> Option<usize> {
        Some(Catalogo::POR_PAGINA)
    }

    fn destacados(&self) -> Resultado<Vec<App>> {
        self.catalogo.destacados()
    }

    fn recomendadas(&self) -> Resultado<Vec<App>> {
        self.catalogo.recomendadas()
    }

    fn categorias(&self) -> Resultado<Vec<Categoria>> {
        self.catalogo.categorias()
    }

    fn por_categoria(&self, cat_id: &str, pagina: i64) -> Resultado<Vec<App>> {
        self.catalogo.por_categoria(cat_id, pagina)
    }

    fn buscar(&self, texto: &str) -> Resultado<Vec<App>> {
        self.catalogo.buscar(texto)
    }

    fn detalle(&self, app_id: &str) -> Resultado<AppDetalle> {
        self.catalogo.detalle(app_id)
    }

    fn instaladas(&self) -> Resultado<Vec<String>> {
        self.flatpak.instaladas()
    }

    fn es_del_sistema(&self, app_id: &str) -> Resultado<bool> {
        self.flatpak.es_del_sistema(app_id)
    }

    fn actualizaciones(&self) -> Resultado<Vec<String>> {
        self.flatpak.actualizaciones()
    }

    fn preparar(&self) -> Resultado<()> {
        self.flatpak.preparar().map(|_| ())
    }

    fn instalar(&self, app_id: &str, progreso: Progreso) -> Resultado<()> {
        self.flatpak.instalar(app_id, Some(progreso))
    }

    fn quitar(&self, app_id: &str, progreso: Progreso) -> Resultado<()> {
        self.flatpak.quitar(app_id, Some(progreso))
    }

    fn actualizar(&self, app_ids: &[String], progreso: Progreso) -> Resultado<()> {
        self.flatpak.actualizar(Some(app_ids), Some(progreso))
    }

    fn abrir(&self, app_id: &str) -> Resultado<()> {
        self.flatpak.abrir(app_id)
    }
}
