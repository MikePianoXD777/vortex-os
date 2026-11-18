//! El doble falso del motor (`falso_motor.py`): la MISMA interfaz que el real ([`vortex_shop::motor::Motor`]),
//! para probar la tienda sin red y sin tocar flatpak. Las perillas están en [`Control`].

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vortex_shop::catalogo::{App, AppDetalle, Categoria};
use vortex_shop::errores::{Error, Resultado};
use vortex_shop::flatpak::Progreso;
use vortex_shop::motor::Motor;

#[derive(Default)]
pub struct Control {
    pub sin_red: AtomicBool,
    /// Milisegundos que dura una instalación.
    pub duracion_ms: AtomicU64,
    pub latencia_ms: AtomicU64,
    /// Ids que fallan a media instalación.
    pub fallan: Mutex<HashSet<String>>,
    pub instaladas: Mutex<Vec<String>>,
    pub actualizaciones: Mutex<Vec<String>>,
    pub registro: Mutex<Vec<String>>,
    pub abiertas: Mutex<Vec<String>>,
    pub del_sistema: Mutex<HashSet<String>>,
    /// Cuántas veces se preguntó qué hay instalado.
    pub consultas_instaladas: AtomicU64,
}

pub struct MotorFalso {
    pub control: Arc<Control>,
    apps: Vec<AppDetalle>,
}

// (id, nombre, resumen, desarrollador, verificado, categoría, descargas/mes, MB)
type Fila = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    &'static str,
    i64,
    i64,
);
const DATOS: [Fila; 17] = [
    (
        "org.mozilla.firefox",
        "Firefox",
        "Navegador web rápido y privado",
        "Mozilla",
        true,
        "Network",
        920_000,
        82,
    ),
    (
        "org.chromium.Chromium",
        "Chromium",
        "El navegador de código abierto detrás de Chrome",
        "The Chromium Authors",
        false,
        "Network",
        310_000,
        140,
    ),
    (
        "com.valvesoftware.Steam",
        "Steam",
        "Tu biblioteca de juegos, tienda y comunidad",
        "Valve",
        true,
        "Game",
        640_000,
        48,
    ),
    (
        "com.discordapp.Discord",
        "Discord",
        "Chat de voz, video y texto para comunidades",
        "Discord Inc.",
        true,
        "Network",
        700_000,
        110,
    ),
    (
        "org.telegram.desktop",
        "Telegram",
        "Mensajería rápida y segura",
        "Telegram FZ-LLC",
        true,
        "Network",
        420_000,
        65,
    ),
    (
        "org.signal.Signal",
        "Signal",
        "Mensajes privados con cifrado de extremo a extremo",
        "Signal Foundation",
        true,
        "Network",
        150_000,
        98,
    ),
    (
        "com.spotify.Client",
        "Spotify",
        "Música y podcasts en streaming",
        "Spotify",
        true,
        "AudioVideo",
        520_000,
        215,
    ),
    (
        "org.videolan.VLC",
        "VLC",
        "Reproduce cualquier video o audio, sin complicaciones",
        "VideoLAN",
        true,
        "AudioVideo",
        800_000,
        105,
    ),
    (
        "com.obsproject.Studio",
        "OBS Studio",
        "Graba y transmite en vivo con escenas y fuentes",
        "OBS Project",
        true,
        "AudioVideo",
        410_000,
        170,
    ),
    (
        "org.audacityteam.Audacity",
        "Audacity",
        "Edita y graba audio multipista",
        "Audacity Team",
        true,
        "AudioVideo",
        160_000,
        62,
    ),
    (
        "org.gimp.GIMP",
        "GIMP",
        "Edita fotos y crea imágenes con herramientas avanzadas",
        "GIMP",
        true,
        "Graphics",
        450_000,
        200,
    ),
    (
        "org.kde.krita",
        "Krita",
        "Pintura digital para artistas",
        "KDE",
        true,
        "Graphics",
        230_000,
        310,
    ),
    (
        "org.blender.Blender",
        "Blender",
        "Modelado 3D, animación y renderizado",
        "Blender Foundation",
        true,
        "Graphics",
        270_000,
        380,
    ),
    (
        "org.libreoffice.LibreOffice",
        "LibreOffice",
        "Suite de oficina completa: texto, hojas de cálculo y presentaciones",
        "The Document Foundation",
        true,
        "Office",
        880_000,
        520,
    ),
    (
        "org.qbittorrent.qBittorrent",
        "qBittorrent",
        "Cliente BitTorrent ligero y sin anuncios",
        "qBittorrent",
        true,
        "Network",
        290_000,
        70,
    ),
    (
        "us.zoom.Zoom",
        "Zoom",
        "Videollamadas y reuniones en línea",
        "Zoom",
        false,
        "Network",
        200_000,
        160,
    ),
    (
        "org.sinicono.Sencilla",
        "App sin ícono",
        "Su ícono no carga: se ve la inicial sobre un color",
        "Alguien",
        false,
        "Utility",
        0,
        2,
    ),
];

const CATEGORIAS: [(&str, &str, &str); 8] = [
    ("Game", "Juegos", "applications-games-symbolic"),
    ("Office", "Productividad", "applications-office-symbolic"),
    ("Graphics", "Gráficos y fotos", "applications-graphics-symbolic"),
    ("AudioVideo", "Audio y video", "applications-multimedia-symbolic"),
    ("Development", "Desarrollo", "applications-engineering-symbolic"),
    ("Education", "Educación", "applications-science-symbolic"),
    ("Network", "Internet", "applications-internet-symbolic"),
    ("Utility", "Utilidades", "applications-utilities-symbolic"),
];

const RECOMENDADAS: [&str; 6] = [
    "org.mozilla.firefox",
    "com.valvesoftware.Steam",
    "org.videolan.VLC",
    "org.gimp.GIMP",
    "com.obsproject.Studio",
    "org.libreoffice.LibreOffice",
];

impl MotorFalso {
    pub fn nuevo(duracion_ms: u64, instaladas: &[&str], actualizaciones: &[&str]) -> Self {
        let control = Arc::new(Control {
            duracion_ms: AtomicU64::new(duracion_ms),
            latencia_ms: AtomicU64::new(30),
            instaladas: Mutex::new(instaladas.iter().map(|s| (*s).into()).collect()),
            actualizaciones: Mutex::new(actualizaciones.iter().map(|s| (*s).into()).collect()),
            ..Control::default()
        });
        let apps = DATOS
            .iter()
            .map(|(id, nombre, resumen, dev, ver, cat, desc, mb)| AppDetalle {
                app: App {
                    id: (*id).into(),
                    nombre: (*nombre).into(),
                    resumen: (*resumen).into(),
                    icono_url: format!("file:///no/existe/{id}.png"),
                    desarrollador: (*dev).into(),
                    verificado: *ver,
                    descargas_mes: (*desc > 0).then_some(*desc),
                    ..App::default()
                },
                descripcion: format!(
                    "{nombre}: {resumen}.\n\nFunciones principales:\n• Fácil de usar.\n• Se actualiza solo."
                ),
                capturas: vec![],
                licencia: "GPL-3.0-or-later".into(),
                web: Some(format!(
                    "https://example.org/{}",
                    nombre.split(' ').next().unwrap_or("x").to_lowercase()
                )),
                version: Some("1.2.3".into()),
                tamano_descarga: Some(mb * 1024 * 1024),
                categorias: vec![(*cat).into()],
            })
            .collect();
        MotorFalso { control, apps }
    }

    fn revisar(&self) -> Resultado<()> {
        std::thread::sleep(Duration::from_millis(
            self.control.latencia_ms.load(Ordering::SeqCst),
        ));
        if self.control.sin_red.load(Ordering::SeqCst) {
            return Err(Error::SinRed("sin red".into()));
        }
        Ok(())
    }

    fn app(&self, id: &str) -> App {
        self.apps
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.app.clone())
            .unwrap()
    }

    fn marchar(&self, app_id: &str, progreso: Progreso, textos: &[&str]) -> Resultado<()> {
        let pasos = 20;
        for k in 0..=pasos {
            std::thread::sleep(Duration::from_millis(
                self.control.duracion_ms.load(Ordering::SeqCst) / pasos as u64,
            ));
            if k == pasos / 2 && self.control.fallan.lock().unwrap().contains(app_id) {
                return Err(Error::flatpak(format!(
                    "No se pudo completar la operación con {app_id}: se acabó el espacio en disco."
                )));
            }
            progreso(
                100 * k / pasos,
                textos[(textos.len() - 1).min(k as usize * textos.len() / pasos as usize)],
            );
        }
        Ok(())
    }
}

impl Motor for MotorFalso {
    fn por_pagina(&self) -> Option<usize> {
        Some(6) // el real da 30; aquí se ve el «Mostrar más»
    }

    fn destacados(&self) -> Resultado<Vec<App>> {
        self.revisar()?;
        Ok(self.apps.iter().take(12).map(|a| a.app.clone()).collect())
    }

    fn recomendadas(&self) -> Resultado<Vec<App>> {
        self.revisar()?;
        Ok(RECOMENDADAS.iter().map(|i| self.app(i)).collect())
    }

    fn categorias(&self) -> Resultado<Vec<Categoria>> {
        self.revisar()?;
        Ok(CATEGORIAS
            .iter()
            .map(|(id, n, i)| Categoria {
                id: (*id).into(),
                nombre: (*n).into(),
                icono: (*i).into(),
                ..Categoria::default()
            })
            .collect())
    }

    fn por_categoria(&self, cat_id: &str, pagina: i64) -> Resultado<Vec<App>> {
        self.revisar()?;
        let todas: Vec<App> = self
            .apps
            .iter()
            .filter(|a| a.categorias.iter().any(|c| c == cat_id))
            .map(|a| a.app.clone())
            .collect();
        let ini = ((pagina - 1) * 6) as usize;
        Ok(todas.into_iter().skip(ini).take(6).collect())
    }

    fn buscar(&self, texto: &str) -> Resultado<Vec<App>> {
        self.revisar()?;
        let t = texto.to_lowercase();
        Ok(self
            .apps
            .iter()
            .filter(|a| a.nombre.to_lowercase().contains(&t) || a.resumen.to_lowercase().contains(&t))
            .map(|a| a.app.clone())
            .collect())
    }

    fn detalle(&self, app_id: &str) -> Resultado<AppDetalle> {
        self.revisar()?;
        self.apps
            .iter()
            .find(|a| a.id == app_id)
            .cloned()
            .ok_or_else(|| Error::no_encontrada("No se encontró esa app en la tienda."))
    }

    fn instaladas(&self) -> Resultado<Vec<String>> {
        self.control.consultas_instaladas.fetch_add(1, Ordering::SeqCst);
        Ok(self.control.instaladas.lock().unwrap().clone())
    }

    fn es_del_sistema(&self, app_id: &str) -> Resultado<bool> {
        Ok(self.control.del_sistema.lock().unwrap().contains(app_id))
    }

    fn actualizaciones(&self) -> Resultado<Vec<String>> {
        if self.control.sin_red.load(Ordering::SeqCst) {
            return Err(Error::SinRed("sin red".into()));
        }
        Ok(self.control.actualizaciones.lock().unwrap().clone())
    }

    fn preparar(&self) -> Resultado<()> {
        Ok(())
    }

    fn instalar(&self, app_id: &str, progreso: Progreso) -> Resultado<()> {
        self.control
            .registro
            .lock()
            .unwrap()
            .push(format!("instalar {app_id}"));
        self.marchar(app_id, progreso, &["Descargando…", "Instalando…", "Terminando…"])?;
        let mut i = self.control.instaladas.lock().unwrap();
        if !i.iter().any(|x| x == app_id) {
            i.push(app_id.into());
        }
        Ok(())
    }

    fn quitar(&self, app_id: &str, progreso: Progreso) -> Resultado<()> {
        self.control
            .registro
            .lock()
            .unwrap()
            .push(format!("quitar {app_id}"));
        self.marchar(app_id, progreso, &["Quitando…"])?;
        self.control.instaladas.lock().unwrap().retain(|i| i != app_id);
        self.control
            .actualizaciones
            .lock()
            .unwrap()
            .retain(|i| i != app_id);
        Ok(())
    }

    fn actualizar(&self, app_ids: &[String], progreso: Progreso) -> Resultado<()> {
        self.control
            .registro
            .lock()
            .unwrap()
            .push(format!("actualizar {}", app_ids.join(",")));
        self.marchar(
            app_ids.first().map_or("", String::as_str),
            progreso,
            &["Descargando…", "Actualizando…"],
        )?;
        self.control
            .actualizaciones
            .lock()
            .unwrap()
            .retain(|i| !app_ids.contains(i));
        Ok(())
    }

    fn abrir(&self, app_id: &str) -> Resultado<()> {
        self.control.abiertas.lock().unwrap().push(app_id.into());
        Ok(())
    }
}
