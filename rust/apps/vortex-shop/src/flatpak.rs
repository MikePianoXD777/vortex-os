//! Instalar, quitar y actualizar apps de Flathub para TU usuario (`flatpak --user`): sin contraseña, sin
//! polkit y sin terminal.
//!
//! Se habla con la CLI «flatpak --noninteractive», corriendo en una pseudo-terminal para que Flatpak dibuje
//! sus barras de progreso y de ahí se saca el porcentaje. (La app de Python también puede usar libflatpak
//! por GObject; aquí no hay GObject, así que solo está esta forma, que en Python es el respaldo.)
//!
//! Las apps que ya están a nivel sistema (las que pone vortex-flatpaks al arrancar: Sober, Heroic,
//! Spotify…) cuentan como instaladas y se pueden abrir, pero no se pueden quitar desde aquí: quitarlas pide
//! ser administrador. Los avisos y errores son siempre mensajes en español para mostrar tal cual.
//!
//! Las funciones que cambian algo (instalar, quitar, actualizar) corren en el hilo que las llama y van
//! avisando por `progreso(porcentaje, texto)` desde ese mismo hilo. De a una por vez: si llega otra mientras
//! corre una, espera su turno.
#![allow(unsafe_code)] // pseudo-terminal (openpty, poll)

use crate::errores::{Error, Resultado};
use crate::ids::{es_id_valido, validar_id};
use crate::proceso::{self, Entorno, Fallo};
use regex::Regex;
use std::cell::RefCell;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

pub const REMOTO: &str = "flathub";
pub const URL_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// El progreso (porcentaje, texto) de la interfaz.
pub type Progreso<'a> = &'a (dyn Fn(i32, &str) + Sync);

fn re(patron: &str) -> Regex {
    Regex::new(patron).expect("expresión regular de la tienda")
}

static RED: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)could not resolve|unable to connect|network is unreachable|temporary failure in name|name or service not known|connection (refused|reset|timed out)|timed out|no route to host|failed to connect|unable to load summary|error (while|when) (downloading|fetching)|while fetching|no se pudo resolver|no se puede conectar|no se pudo conectar|sin conexi|error de red|tiempo de espera|nombre o servicio no conocido|red inaccesible",
    )
});
static ESPACIO: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)no space left|not enough disk space|insufficient (disk )?space|no hay espacio|espacio (insuficiente|en disco)|sin espacio|no queda espacio",
    )
});
static NO_ESTA: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)no remote refs found|nothing matches|not found in remote|no matches? found|no se encontr|ninguna coincidencia|ningún elemento coincide|no hay refs",
    )
});
static YA_ESTA: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)already installed|ya est[aá] instalad"));
static PERMISO: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)permission denied|not allowed|authentication|polkit|permiso denegado"));
static ANSI: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b[()][A-Za-z0-9]"));
static OPERACION: LazyLock<Regex> = LazyLock::new(|| re(r"(Installing|Updating|Uninstalling)\s+(\d+)/(\d+)"));
static PORCIENTO: LazyLock<Regex> = LazyLock::new(|| re(r"(\d{1,3})\s?%"));
static ERROR_LINEA: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)error"));
static ERROR_PREFIJO: LazyLock<Regex> = LazyLock::new(|| re(r"^(error|Error)\s*:\s*"));

fn q(nombre: &str) -> String {
    format!("«{nombre}»")
}

/// Los BaseApp (io.qt.qtwebengine.BaseApp…) son andamios de otras apps, no se abren ni se muestran.
fn es_app(app_id: &str) -> bool {
    es_id_valido(app_id) && !app_id.ends_with(".BaseApp")
}

fn validar(app_id: &str) -> Resultado<()> {
    validar_id(app_id).map(|_| ()).map_err(Error::no_encontrada)
}

/// Convierte lo que dijo Flatpak (en inglés o en el idioma del sistema) en un error en español. `objeto`
/// es lo que se estaba tocando, ya con comillas si es una app: «Steam» o tus aplicaciones.
fn error(texto: &str, accion: &str, objeto: &str) -> Error {
    let texto = texto.trim();
    if RED.is_match(texto) {
        return Error::flatpak_sin_red();
    }
    if ESPACIO.is_match(texto) {
        return Error::flatpak(format!(
            "No hay espacio suficiente en el disco para {accion} {objeto}. Libera espacio e inténtalo de nuevo."
        ));
    }
    if NO_ESTA.is_match(texto) {
        return Error::no_encontrada(format!("Flathub no tiene {objeto}."));
    }
    if PERMISO.is_match(texto) {
        return Error::flatpak(format!(
            "No hay permiso para {accion} {objeto}: es del sistema, no de tu usuario."
        ));
    }
    let lineas: Vec<&str> = texto.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    // La última línea que diga «error» (Flatpak la deja al final), sin el «error:» de adelante.
    let motivo = lineas
        .iter()
        .rev()
        .find(|l| ERROR_LINEA.is_match(l))
        .or(lineas.last())
        .copied()
        .unwrap_or("");
    let motivo: String = ERROR_PREFIJO.replace(motivo, "").chars().take(200).collect();
    let extra = if motivo.is_empty() {
        String::new()
    } else {
        format!(" {motivo}")
    };
    Error::flatpak(format!("No se pudo {accion} {objeto}.{extra}"))
}

/// El progreso de la interfaz, a prueba de tonterías: entero de 0 a 100, sin repetir lo mismo, y una
/// falla de la interfaz no tumba la instalación.
struct Avance<'a> {
    progreso: Option<Progreso<'a>>,
    ultimo: RefCell<(i32, String)>,
}

impl<'a> Avance<'a> {
    fn nuevo(progreso: Option<Progreso<'a>>) -> Self {
        Avance {
            progreso,
            ultimo: RefCell::new((-1, String::new())),
        }
    }

    fn llamar(&self, porcentaje: i32, texto: &str) {
        let porcentaje = porcentaje.clamp(0, 100);
        {
            let mut u = self.ultimo.borrow_mut();
            if u.0 == porcentaje && u.1 == texto {
                return;
            }
            *u = (porcentaje, texto.to_string());
        }
        if let Some(p) = self.progreso {
            // lo que haga mal la interfaz no es problema de Flatpak
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| p(porcentaje, texto)));
        }
    }
}

/// La CLI de `flatpak` de este usuario, con lo que hace falta para llamarla.
pub struct Flatpak {
    entorno: Entorno,
    /// Una operación a la vez (Flatpak bloquea el repositorio de todos modos).
    cerrojo: Mutex<()>,
}

impl Default for Flatpak {
    fn default() -> Self {
        Self::nuevo(Entorno::default())
    }
}

impl Flatpak {
    /// `entorno`: cambios al entorno con que se lanza (las pruebas ponen su PATH con el flatpak falso).
    pub fn nuevo(entorno: Entorno) -> Self {
        Flatpak {
            entorno,
            cerrojo: Mutex::new(()),
        }
    }

    fn exe(&self) -> Resultado<std::path::PathBuf> {
        self.entorno.buscar("flatpak").ok_or_else(|| {
            Error::flatpak(
                "Flatpak no está instalado en este equipo, así que la tienda no puede instalar aplicaciones.",
            )
        })
    }

    /// Los mensajes de flatpak en inglés y sin adornos Unicode: así los patrones de arriba los entienden.
    fn comando(&self, argumentos: &[&str]) -> Resultado<Command> {
        let mut cmd = Command::new(self.exe()?);
        cmd.args(argumentos);
        self.entorno.aplicar(&mut cmd);
        cmd.env("LC_ALL", "C").env("LANG", "C").env("LANGUAGE", "C");
        let term = self.entorno.valor("TERM").unwrap_or_default();
        if term.is_empty() || term == "dumb" {
            cmd.env("TERM", "xterm"); // con «dumb» flatpak no dibuja la barra de progreso
        }
        cmd.env_remove("PYTHONPATH");
        Ok(cmd)
    }

    fn turno(&self, avance: &Avance) -> std::sync::MutexGuard<'_, ()> {
        match self.cerrojo.try_lock() {
            Ok(g) => g,
            Err(_) => {
                avance.llamar(0, "Esperando a que termine otra operación…");
                self.cerrojo.lock().unwrap_or_else(|e| e.into_inner())
            }
        }
    }

    /// Salida (líneas) de «flatpak …» en solo lectura. `Flatpak` si falla.
    fn lineas(&self, argumentos: &[&str], tiempo: Duration) -> Resultado<Vec<String>> {
        let cmd = self.comando(argumentos)?;
        match proceso::correr(cmd, None, tiempo) {
            Err(Fallo::Tiempo) => Err(Error::flatpak(
                "Flatpak tardó demasiado en responder. Inténtalo de nuevo.",
            )),
            Err(Fallo::NoArranca(e)) => Err(Error::flatpak(format!("No se pudo ejecutar Flatpak: {e}"))),
            Ok(s) if !s.bien() => {
                let texto = if s.stderr.is_empty() { &s.stdout } else { &s.stderr };
                Err(error(texto, "consultar", "tus aplicaciones"))
            }
            Ok(s) => Ok(s.stdout.lines().map(|l| l.trim().to_string()).collect()),
        }
    }

    fn ids(&self, argumentos: &[&str]) -> Resultado<Vec<String>> {
        let mut v: Vec<String> = self
            .lineas(argumentos, Duration::from_secs(120))?
            .into_iter()
            .filter(|l| es_app(l))
            .collect();
        v.sort();
        v.dedup();
        Ok(v)
    }

    fn instaladas_en(&self, donde: Option<&str>) -> Resultado<Vec<String>> {
        let mut a = vec!["list", "--app", "--columns=application"];
        a.extend(donde);
        self.ids(&a)
    }

    /// Corre «flatpak …» en una pseudo-terminal (para que dibuje su barra de progreso), saca los
    /// porcentajes de lo que escribe y los pasa a `avance`. Si falla, da el error en español.
    fn correr_con_barra(
        &self,
        argumentos: &[&str],
        avance: &Avance,
        accion: &str,
        objeto: &str,
        texto_descarga: &str,
    ) -> Resultado<()> {
        let mut cmd = self.comando(argumentos)?;
        let (maestro, esclavo) =
            abrir_pty().map_err(|e| Error::flatpak(format!("No se pudo ejecutar Flatpak: {e}")))?;
        let esclavo2 = esclavo
            .try_clone()
            .map_err(|e| Error::flatpak(format!("No se pudo ejecutar Flatpak: {e}")))?;
        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(esclavo))
            .stderr(Stdio::from(esclavo2))
            .process_group(0);
        let hijo = cmd.spawn();
        drop(cmd); // suelta nuestras copias del lado esclavo: así `read` avisa (EIO) cuando el hijo termina
        let mut hijo = hijo.map_err(|e| Error::flatpak(format!("No se pudo ejecutar Flatpak: {e}")))?;

        let mut cola: Vec<String> = Vec::new();
        let mut pendiente = String::new();
        let (mut op_actual, mut op_total) = (1_i32, 1_i32);
        avance.llamar(0, "Preparando…");
        let mut fichero = std::fs::File::from(maestro);
        let mut buf = [0u8; 4096];
        let mut pedazo: Vec<u8> = Vec::new();
        loop {
            let mut pfd = libc::pollfd {
                fd: fichero.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: un solo pollfd válido en la pila.
            let n = unsafe { libc::poll(&mut pfd, 1, 500) };
            if n < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }
            if n == 0 {
                if matches!(hijo.try_wait(), Ok(Some(_))) {
                    break;
                }
                continue;
            }
            let leidos = match fichero.read(&mut buf) {
                Ok(0) => break,
                Ok(k) => k,
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break, // el otro lado se cerró: terminó
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            pedazo.extend_from_slice(&buf[..leidos]);
            // Solo se decodifica lo completo en UTF-8 (un carácter puede venir partido entre lecturas).
            let corte = match std::str::from_utf8(&pedazo) {
                Ok(_) => pedazo.len(),
                Err(e) => e.valid_up_to(),
            };
            let texto = String::from_utf8_lossy(&pedazo[..corte]).into_owned();
            pedazo.drain(..corte);
            pendiente.push_str(&ANSI.replace_all(&texto, ""));
            let mut partes: Vec<String> = pendiente.split(['\r', '\n']).map(str::to_string).collect();
            pendiente = partes.pop().unwrap_or_default();
            let total = partes.len() + 1;
            for (i, linea) in partes.iter().chain(std::iter::once(&pendiente)).enumerate() {
                if linea.trim().is_empty() {
                    continue;
                }
                let m = OPERACION.captures(linea);
                if let Some(m) = &m {
                    op_actual = m[2].parse::<i32>().unwrap_or(1).max(1);
                    op_total = m[3].parse::<i32>().unwrap_or(1).max(1);
                }
                let p = PORCIENTO.captures(linea);
                if linea.contains("Uninstalling") {
                    avance.llamar(((op_actual - 1) * 100 / op_total).min(99), "Quitando…");
                } else if p.is_none() && m.is_some() {
                    // sin barra (terminal sin adornos): al menos cuántas operaciones van
                    let texto = if op_total > 1 {
                        format!("Instalando… {op_actual} de {op_total}")
                    } else {
                        "Instalando…".to_string()
                    };
                    avance.llamar(((op_actual - 1) * 100 / op_total).min(99), &texto);
                } else if let Some(p) = p {
                    let por_ciento = p[1].parse::<i32>().unwrap_or(0).min(100);
                    let total_p = (f64::from(op_actual - 1) + f64::from(por_ciento) / 100.0)
                        / f64::from(op_total)
                        * 100.0;
                    let total_p = (total_p as i32).min(99);
                    if por_ciento >= 100 {
                        avance.llamar(
                            total_p,
                            if accion != "actualizar" {
                                "Instalando…"
                            } else {
                                "Actualizando…"
                            },
                        );
                    } else {
                        avance.llamar(total_p, &format!("{texto_descarga} {por_ciento} %"));
                    }
                }
                if i + 1 < total {
                    cola.push(linea.trim().to_string()); // las completas; la pendiente se guarda al completarse
                }
            }
            if cola.len() > 60 {
                cola.drain(..cola.len() - 60);
            }
        }
        drop(fichero);
        let estado = hijo.wait();
        if !estado.as_ref().is_ok_and(|e| e.success()) {
            let mut todo = cola.clone();
            if !pendiente.trim().is_empty() {
                todo.push(pendiente.trim().to_string());
            }
            let salida = todo.join("\n");
            let err = error(&salida, accion, objeto);
            if YA_ESTA.is_match(&salida) {
                return Ok(());
            }
            return Err(err);
        }
        Ok(())
    }

    // ------------------------------------------------------------------ lo que se usa desde la tienda

    /// Ids de las apps instaladas (de tu usuario y del sistema), sin runtimes. Orden alfabético.
    pub fn instaladas(&self) -> Resultado<Vec<String>> {
        self.instaladas_en(None)
    }

    /// True si la app está instalada para todo el sistema y no en tu usuario (la tienda no puede quitarla).
    pub fn es_del_sistema(&self, app_id: &str) -> Resultado<bool> {
        validar(app_id)?;
        let sistema = self.instaladas_en(Some("--system"))?;
        let usuario = self.instaladas_en(Some("--user"))?;
        Ok(sistema.iter().any(|i| i == app_id) && !usuario.iter().any(|i| i == app_id))
    }

    /// Ids de tus apps (instaladas para tu usuario) que tienen una versión nueva. Pide la lista a Flathub:
    /// `SinRed` si no hay internet. Las del sistema no se cuentan (actualizarlas pide ser administrador).
    pub fn actualizaciones(&self) -> Resultado<Vec<String>> {
        self.ids(&[
            "remote-ls",
            "--user",
            "--updates",
            "--app",
            "--columns=application",
        ])
    }

    /// Deja Flathub como fuente de tu usuario si falta (`flatpak remote-add --user --if-not-exists`).
    /// Devuelve true si lo agregó. Sin internet y sin Flathub: `SinRed`.
    pub fn preparar(&self) -> Resultado<bool> {
        let existentes = self.lineas(&["remotes", "--user", "--columns=name"], Duration::from_secs(120))?;
        if existentes.iter().any(|r| r == REMOTO) {
            return Ok(false);
        }
        let cmd = self.comando(&["remote-add", "--user", "--if-not-exists", REMOTO, URL_REPO])?;
        match proceso::correr(cmd, None, Duration::from_secs(90)) {
            Err(Fallo::Tiempo) => Err(Error::flatpak_sin_red()),
            Err(Fallo::NoArranca(e)) => Err(Error::flatpak(format!("No se pudo ejecutar Flatpak: {e}"))),
            Ok(s) if !s.bien() => Err(error(
                if s.stderr.is_empty() { &s.stdout } else { &s.stderr },
                "preparar",
                "Flathub",
            )),
            Ok(_) => Ok(true),
        }
    }

    /// Instala la app para tu usuario. `progreso(0-100, texto)` va avisando; termina con (100, «Listo»).
    pub fn instalar(&self, app_id: &str, progreso: Option<Progreso>) -> Resultado<()> {
        validar(app_id)?;
        let avance = Avance::nuevo(progreso);
        let _turno = self.turno(&avance);
        if self.instaladas_en(None)?.iter().any(|i| i == app_id) {
            avance.llamar(100, "Ya estaba instalada");
            return Ok(());
        }
        avance.llamar(0, "Preparando…");
        self.preparar()?;
        self.correr_con_barra(
            &["install", "--user", "--noninteractive", "-y", REMOTO, app_id],
            &avance,
            "instalar",
            &q(app_id),
            "Descargando…",
        )?;
        avance.llamar(100, "Listo");
        Ok(())
    }

    /// Quita la app de tu usuario (sus datos en ~/.var/app se conservan). Si es del sistema, error
    /// explicando que desde aquí no se puede.
    pub fn quitar(&self, app_id: &str, progreso: Option<Progreso>) -> Resultado<()> {
        validar(app_id)?;
        let avance = Avance::nuevo(progreso);
        let _turno = self.turno(&avance);
        let nombre = app_id;
        let en_usuario = self.instaladas_en(Some("--user"))?.iter().any(|i| i == app_id);
        let en_sistema = self.instaladas_en(Some("--system"))?.iter().any(|i| i == app_id);
        if !en_usuario {
            if en_sistema {
                return Err(Error::flatpak(format!(
                    "«{nombre}» es parte del sistema: la instaló Vortexine's OS para todos los usuarios y desde la tienda no se puede quitar."
                )));
            }
            return Err(Error::flatpak(format!("«{nombre}» no está instalada.")));
        }
        avance.llamar(0, "Quitando…");
        self.correr_con_barra(
            &["uninstall", "--user", "--noninteractive", "-y", app_id],
            &avance,
            "quitar",
            &q(nombre),
            "Descargando…",
        )?;
        // Los runtimes que quedaron sin ninguna app que los use.
        let _ = self.correr_con_barra(
            &["uninstall", "--user", "--unused", "--noninteractive", "-y"],
            &avance,
            "quitar",
            &q(nombre),
            "Descargando…",
        );
        avance.llamar(100, "Listo");
        Ok(())
    }

    /// Actualiza las apps de tu usuario: las de la lista, o todas si `app_ids` es `None`.
    pub fn actualizar(&self, app_ids: Option<&[String]>, progreso: Option<Progreso>) -> Resultado<()> {
        if let Some(ids) = app_ids {
            for i in ids {
                validar(i)?;
            }
        }
        let avance = Avance::nuevo(progreso);
        let _turno = self.turno(&avance);
        avance.llamar(0, "Buscando actualizaciones…");
        let mut ids: Option<Vec<String>> = app_ids.map(<[String]>::to_vec);
        if let Some(pedidas) = &ids {
            let pendientes = self.actualizaciones()?;
            ids = Some(
                pedidas
                    .iter()
                    .filter(|i| pendientes.contains(i))
                    .cloned()
                    .collect(),
            );
        }
        let hubo = ids.as_ref().is_none_or(|v| !v.is_empty());
        if hubo {
            let mut args: Vec<&str> = vec!["update", "--user", "--noninteractive", "-y"];
            args.extend(ids.iter().flatten().map(String::as_str));
            self.correr_con_barra(&args, &avance, "actualizar", "tus aplicaciones", "Descargando…")?;
        }
        avance.llamar(100, if hubo { "Listo" } else { "Todo está al día" });
        Ok(())
    }

    /// Abre la app, desacoplada de la tienda (cerrar la tienda no la cierra, y no queda como hija).
    pub fn abrir(&self, app_id: &str) -> Resultado<()> {
        validar(app_id)?;
        let exe = self.exe()?;
        let instalada = match self.instaladas() {
            Ok(v) => v.iter().any(|i| i == app_id),
            Err(_) => true, // si ni se pudo consultar, se intenta abrir de todos modos
        };
        if !instalada {
            return Err(Error::flatpak(format!("«{app_id}» no está instalada.")));
        }
        let setsid = self.entorno.buscar("setsid");
        // «setsid -f» hace el fork y devuelve el control al instante: la app queda colgada de init, no de la tienda.
        let mut cmd = match &setsid {
            Some(s) => {
                let mut c = Command::new(s);
                c.arg("-f").arg(&exe);
                c
            }
            None => Command::new(&exe),
        };
        cmd.args(["run", app_id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.entorno.aplicar(&mut cmd);
        if let Some(casa) = self.entorno.valor("HOME") {
            cmd.current_dir(casa);
        }
        if setsid.is_none() {
            cmd.process_group(0);
        }
        let mut hijo = cmd
            .spawn()
            .map_err(|e| Error::flatpak(format!("No se pudo abrir «{app_id}»: {e}")))?;
        if setsid.is_some() {
            let _ = hijo.wait();
        } else {
            // recoge el cadáver cuando la app cierre
            std::thread::spawn(move || {
                let _ = hijo.wait();
            });
        }
        Ok(())
    }
}

/// Una pseudo-terminal ancha (con menos de unas 40 columnas flatpak no dibuja la barra): (maestro, esclavo).
fn abrir_pty() -> std::io::Result<(OwnedFd, OwnedFd)> {
    let (mut m, mut s) = (0, 0);
    let tam = libc::winsize {
        ws_row: 24,
        ws_col: 200,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: openpty escribe dos descriptores en m y s; si da 0, son nuestros.
    let r = unsafe { libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null(), &tam) };
    if r != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: descriptores recién creados que nadie más tiene.
    Ok(unsafe { (OwnedFd::from_raw_fd(m), OwnedFd::from_raw_fd(s)) })
}
