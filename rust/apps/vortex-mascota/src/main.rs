//! `vortex-mascota`: la compañera de escritorio (ver lib.rs).
//!
//!   vortex-mascota              la pone en la pantalla donde está el mouse (una sola a la vez)
//!   vortex-mascota apagar       la quita
//!   vortex-mascota estado       dice si está y cuál aspecto usa
//!   vortex-mascota usar RUTA    copia un paquete como el tuyo: de tiras (animations.json +
//!                               spritesheets_128/) o Shimeji (shime1.png…); también un .zip
//!   vortex-mascota original     vuelve al diseño original
//!   vortex-mascota hoja A.png   saca los cuadros del aspecto que se usa a un PNG
//!
//! La ventana es una capa de Wayland (wlr-layer-shell, capa «top»: arriba de las ventanas, abajo de
//! los avisos y del lanzador) del tamaño del muñeco más el globito; se mueve cambiando sus márgenes.
//! Solo el muñeco recibe clics: lo transparente deja pasar el mouse a lo de abajo.

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};
use vortex_mascota::{
    aspecto::{self, Aspecto},
    hypr, lienzo,
    mundo::{Entorno, Mascota},
    png,
};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

const BOTON_IZQUIERDO: u32 = 0x110;
const BOTON_DERECHO: u32 = 0x111;
const ESCALA_ORIGINAL: f32 = 3.0;

fn archivo_pid() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
    dir.join("vortex-mascota.pid")
}

/// El PID de la que ya corre, si corre.
fn corriendo() -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(archivo_pid()).ok()?.trim().parse().ok()?;
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    (comm.trim() == "vortex-mascota" && pid != std::process::id()).then_some(pid)
}

fn modo_juego() -> bool {
    let estado = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_default();
    estado.join("vortex/modo-juego").exists()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("iniciar") => iniciar(),
        Some("apagar") => {
            if let Some(pid) = corriendo() {
                let _ = Command::new("kill").arg(pid.to_string()).status();
            }
            ExitCode::SUCCESS
        }
        Some("estado") => {
            let (a, error) = aspecto::cargar(ESCALA_ORIGINAL);
            match corriendo() {
                Some(pid) => println!("encendida (PID {pid}), aspecto: {}", a.nombre),
                None => println!("apagada, aspecto: {}", a.nombre),
            }
            if let Some(e) = error {
                println!("tu paquete no se pudo leer ({e}): se usa la original");
            }
            ExitCode::SUCCESS
        }
        Some("usar") => match args.get(1) {
            Some(ruta) => usar(Path::new(ruta)),
            None => {
                eprintln!(
                    "uso: vortex-mascota usar CARPETA (la de animations.json o la de shime1.png…) o ARCHIVO.zip"
                );
                ExitCode::from(2)
            }
        },
        Some("original") => {
            let _ = std::fs::remove_dir_all(aspecto::carpeta());
            println!("Listo: vuelve el diseño original.");
            reiniciar_si_corre();
            ExitCode::SUCCESS
        }
        Some("hoja") => {
            let Some(ruta) = args.get(1) else {
                eprintln!("uso: vortex-mascota hoja ARCHIVO.png [ESCALA]");
                return ExitCode::from(2);
            };
            // La del aspecto que se usa (la original sale a escala 3; el paquete, a su tamaño).
            let escala = args.get(2).and_then(|e| e.parse().ok()).unwrap_or(1);
            let (a, _) = aspecto::cargar(ESCALA_ORIGINAL);
            let (w, h, px) = a.hoja();
            match std::fs::write(ruta, png::codificar(w, h, &px, escala)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("vortex-mascota: no pude escribir {ruta}: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(otro) => {
            eprintln!("vortex-mascota: no conozco «{otro}» (iniciar, apagar, estado, usar, original, hoja)");
            ExitCode::from(2)
        }
    }
}

fn reiniciar_si_corre() {
    if let Some(pid) = corriendo() {
        let _ = Command::new("kill").arg(pid.to_string()).status();
        std::thread::sleep(Duration::from_millis(300));
        let _ = Command::new("vortex-mascota")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// Copia un paquete a la carpeta propia (solo PNG que se puedan leer). Un .zip se abre primero.
fn usar(origen: &Path) -> ExitCode {
    if origen.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let destino = aspecto::carpeta();
        let abierto = destino.with_extension("zip.d");
        let _ = std::fs::remove_dir_all(&abierto);
        // bsdtar (de libarchive, siempre está en Arch) no saca nada fuera de la carpeta.
        let ok = std::fs::create_dir_all(&abierto).is_ok()
            && Command::new("bsdtar")
                .arg("-xf")
                .arg(origen)
                .arg("-C")
                .arg(&abierto)
                .status()
                .is_ok_and(|s| s.success());
        if !ok {
            let _ = std::fs::remove_dir_all(&abierto);
            eprintln!("No pude abrir {}.", origen.display());
            return ExitCode::FAILURE;
        }
        // El paquete puede venir dentro de una carpeta.
        let raiz = buscar_paquete(&abierto, 3).unwrap_or_else(|| abierto.clone());
        let r = usar(&raiz);
        let _ = std::fs::remove_dir_all(&abierto);
        return r;
    }
    if origen.join("animations.json").is_file() {
        return usar_tiras(origen);
    }
    let img = [origen.join("img"), origen.to_path_buf()]
        .into_iter()
        .find(|d| d.join("shime1.png").is_file());
    let Some(img) = img else {
        eprintln!(
            "No encontré animations.json ni shime1.png en {} (ni en su carpeta img/).",
            origen.display()
        );
        return ExitCode::FAILURE;
    };
    let destino = aspecto::carpeta();
    let nueva = destino.with_extension("nueva");
    let _ = std::fs::remove_dir_all(&nueva);
    if let Err(e) = std::fs::create_dir_all(nueva.join("img")) {
        eprintln!("No pude crear {}: {e}", nueva.display());
        return ExitCode::FAILURE;
    }
    let mut copiadas = 0;
    for n in 1..=100 {
        let f = img.join(format!("shime{n}.png"));
        if !f.is_file() {
            continue;
        }
        if let Err(e) = aspecto::leer_png(&f) {
            eprintln!("Me salto {}: {e}", f.display());
            continue;
        }
        if std::fs::copy(&f, nueva.join("img").join(format!("shime{n}.png"))).is_ok() {
            copiadas += 1;
        }
    }
    let _ = std::fs::copy(origen.join("mascota.json"), nueva.join("mascota.json"));
    match Aspecto::shimeji(&nueva) {
        Ok(Some(_)) => {}
        Ok(None) | Err(_) => {
            let _ = std::fs::remove_dir_all(&nueva);
            eprintln!("El paquete no sirve: shime1.png no se pudo leer.");
            return ExitCode::FAILURE;
        }
    }
    guardar(&nueva, &destino, copiadas)
}

/// La primera carpeta (hasta `hondo` niveles abajo) que tenga un paquete.
fn buscar_paquete(dir: &Path, hondo: u8) -> Option<PathBuf> {
    if dir.join("animations.json").is_file()
        || dir.join("shime1.png").is_file()
        || dir.join("img/shime1.png").is_file()
    {
        return Some(dir.to_path_buf());
    }
    if hondo == 0 {
        return None;
    }
    let mut hijas: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    hijas.sort();
    hijas.iter().find_map(|h| buscar_paquete(h, hondo - 1))
}

/// Un paquete de tiras: animations.json, mascota.json y las tiras que nombra.
fn usar_tiras(origen: &Path) -> ExitCode {
    let Some((tiras, _)) = aspecto::carpeta_de_tiras(origen) else {
        eprintln!(
            "{} no tiene spritesheets_128/ ni spritesheets_32/.",
            origen.display()
        );
        return ExitCode::FAILURE;
    };
    let info: serde_json::Value = match std::fs::read_to_string(origen.join("animations.json"))
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("animations.json no se pudo leer: {e}");
            return ExitCode::FAILURE;
        }
    };
    let destino = aspecto::carpeta();
    let nueva = destino.with_extension("nueva");
    let _ = std::fs::remove_dir_all(&nueva);
    let dir_nuevo = nueva.join(tiras.file_name().unwrap_or_default());
    if let Err(e) = std::fs::create_dir_all(&dir_nuevo) {
        eprintln!("No pude crear {}: {e}", nueva.display());
        return ExitCode::FAILURE;
    }
    let mut copiadas = 0;
    let nombres = info["animations"]
        .as_object()
        .map(|m| m.keys().cloned().collect::<Vec<_>>());
    for nombre in nombres.unwrap_or_default() {
        // Solo nombres simples: nada de rutas.
        if nombre.is_empty()
            || !nombre
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            continue;
        }
        let f = tiras.join(format!("{nombre}.png"));
        if !f.is_file() {
            continue;
        }
        if let Err(e) = aspecto::leer_png(&f) {
            eprintln!("Me salto {}: {e}", f.display());
            continue;
        }
        if std::fs::copy(&f, dir_nuevo.join(format!("{nombre}.png"))).is_ok() {
            copiadas += 1;
        }
    }
    let _ = std::fs::copy(origen.join("animations.json"), nueva.join("animations.json"));
    let _ = std::fs::copy(origen.join("mascota.json"), nueva.join("mascota.json"));
    if let Err(e) = Aspecto::tiras(&nueva).and_then(|a| a.ok_or_else(|| "sin animations.json".into())) {
        let _ = std::fs::remove_dir_all(&nueva);
        eprintln!("El paquete no sirve: {e}");
        return ExitCode::FAILURE;
    }
    guardar(&nueva, &destino, copiadas)
}

fn guardar(nueva: &Path, destino: &Path, copiadas: usize) -> ExitCode {
    let _ = std::fs::remove_dir_all(destino);
    if let Err(e) = std::fs::rename(nueva, destino) {
        eprintln!("No pude guardarlo en {}: {e}", destino.display());
        return ExitCode::FAILURE;
    }
    println!("Listo: {copiadas} imágenes en {}.", destino.display());
    reiniciar_si_corre();
    ExitCode::SUCCESS
}

fn iniciar() -> ExitCode {
    if corriendo().is_some() {
        println!("Ya está en tu pantalla.");
        return ExitCode::SUCCESS;
    }
    let _ = std::fs::write(archivo_pid(), std::process::id().to_string());
    let r = correr();
    let _ = std::fs::remove_file(archivo_pid());
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vortex-mascota: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Estado {
    registro: RegistryState,
    asientos: SeatState,
    salidas: OutputState,
    compositor: CompositorState,
    shm: Shm,
    pool: SlotPool,
    capa: Option<LayerSurface>,
    puntero: Option<wl_pointer::WlPointer>,
    configurada: bool,

    aspecto: Aspecto,
    mascota: Mascota,
    pantalla: hypr::Pantalla,
    entorno: Entorno,
    w: usize,
    h: usize,
    /// Dónde pisa dentro de la ventana.
    ancla: (usize, usize),
    /// Márgenes que se le mandaron al compositor (la esquina de la ventana en la pantalla).
    margen: (i32, i32),
    ultimo_dibujo: Option<(
        usize,
        Option<vortex_mascota::arte::Emote>,
        Option<&'static str>,
        i32,
    )>,
    visible: bool,

    presionado: Option<((f64, f64), Instant)>,
    arrastrando: Option<(f64, f64)>,
    ultimo_clic: Option<Instant>,
    cursor: Option<(f32, f32)>,
    mouse_quieto: f32,
    reloj: Instant,
    revision: f32,
    salir: bool,
}

fn correr() -> Result<(), String> {
    let pantallas = hypr::pedir("monitors")
        .map(|v| hypr::pantallas(&v))
        .unwrap_or_default();
    let pantalla = pantallas
        .iter()
        .find(|p| p.enfocada)
        .or(pantallas.first())
        .cloned()
        .ok_or("no encontré Hyprland (¿estás en Vortexine's OS con sesión abierta?)")?;

    let conn = Connection::connect_to_env().map_err(|e| format!("sin Wayland: {e}"))?;
    let (globales, mut cola) = registry_queue_init(&conn).map_err(|e| e.to_string())?;
    let qh: QueueHandle<Estado> = cola.handle();
    let compositor = CompositorState::bind(&globales, &qh).map_err(|e| e.to_string())?;
    let capas = LayerShell::bind(&globales, &qh).map_err(|e| format!("sin layer-shell: {e}"))?;
    let shm = Shm::bind(&globales, &qh).map_err(|e| e.to_string())?;

    let (aspecto, error) = aspecto::cargar(ESCALA_ORIGINAL);
    if let Some(e) = error {
        eprintln!("vortex-mascota: tu paquete no se pudo leer ({e}); uso la original");
    }
    let (ancho, arriba, abajo) = aspecto.tamano_maximo();
    let w = ancho.max(lienzo::ANCHO_MINIMO);
    let h = lienzo::BURBUJA + arriba + abajo;
    let pool = SlotPool::new(w * h * 4 * 2, &shm).map_err(|e| e.to_string())?;
    let semilla = Instant::now().elapsed().as_nanos() as u64 ^ u64::from(std::process::id()) << 17;
    // Aparece arriba de donde está el mouse (o a un tercio de la pantalla) y cae.
    let x0 = hypr::cursor()
        .map(|(cx, _)| cx - pantalla.x)
        .filter(|x| *x > 0.0 && *x < pantalla.ancho)
        .unwrap_or(pantalla.ancho / 3.0);
    let mut mascota = Mascota::nueva(x0, (w / 2) as f32, semilla);
    mascota.ocios = aspecto.ocios();
    mascota.y = (lienzo::BURBUJA + arriba) as f32;

    let mut estado = Estado {
        registro: RegistryState::new(&globales),
        asientos: SeatState::new(&globales, &qh),
        salidas: OutputState::new(&globales, &qh),
        compositor,
        shm,
        pool,
        capa: None,
        puntero: None,
        configurada: false,
        aspecto,
        mascota,
        entorno: Entorno {
            ancho: pantalla.ancho,
            piso: pantalla.alto,
            orillas: vec![],
            mouse_quieto: 0.0,
            esconder: false,
        },
        pantalla,
        w,
        h,
        ancla: (w / 2, lienzo::BURBUJA + arriba),
        margen: (0, 0),
        ultimo_dibujo: None,
        visible: true,
        presionado: None,
        arrastrando: None,
        ultimo_clic: None,
        cursor: None,
        mouse_quieto: 0.0,
        reloj: Instant::now(),
        revision: 10.0,
        salir: false,
    };
    // Las salidas llegan en las primeras vueltas: con ellas se elige la de esa pantalla.
    cola.roundtrip(&mut estado).map_err(|e| e.to_string())?;
    cola.roundtrip(&mut estado).map_err(|e| e.to_string())?;
    let salida = estado.salidas.outputs().find(|o| {
        estado.salidas.info(o).and_then(|i| i.name).as_deref() == Some(estado.pantalla.nombre.as_str())
    });

    let superficie = estado.compositor.create_surface(&qh);
    let capa = capas.create_layer_surface(
        &qh,
        superficie,
        Layer::Top,
        Some("vortex-mascota"),
        salida.as_ref(),
    );
    capa.set_anchor(Anchor::TOP | Anchor::LEFT);
    capa.set_size(w as u32, h as u32);
    capa.set_exclusive_zone(-1);
    capa.set_keyboard_interactivity(KeyboardInteractivity::None);
    capa.set_margin(0, 0, 0, 0);
    capa.commit();
    estado.capa = Some(capa);
    estado.actualizar_entorno();

    let mut bucle: EventLoop<Estado> = EventLoop::try_new().map_err(|e| e.to_string())?;
    WaylandSource::new(conn.clone(), cola)
        .insert(bucle.handle())
        .map_err(|e| e.to_string())?;
    bucle
        .handle()
        .insert_source(Timer::immediate(), |_, _, estado: &mut Estado| {
            estado.tick();
            let espera = if !estado.visible {
                500
            } else if estado.mascota.activa() || estado.arrastrando.is_some() {
                33
            } else {
                100
            };
            TimeoutAction::ToDuration(Duration::from_millis(espera))
        })
        .map_err(|e| e.to_string())?;
    while !estado.salir {
        bucle.dispatch(None, &mut estado).map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl Estado {
    fn actualizar_entorno(&mut self) {
        if let Some(p) = hypr::pedir("monitors")
            .map(|v| hypr::pantallas(&v))
            .and_then(|ps| ps.into_iter().find(|p| p.nombre == self.pantalla.nombre))
        {
            self.pantalla = p;
        } else if hypr::pedir("version").is_some() {
            // Hyprland responde pero la pantalla ya no está (la desconectaste).
            self.salir = true;
            return;
        }
        let clientes = hypr::pedir("clients").unwrap_or_default();
        let activa = hypr::pedir("activewindow").unwrap_or_default();
        let mut e = hypr::entorno(&self.pantalla, &clientes, &activa);
        e.esconder |= modo_juego();
        let cursor = hypr::cursor();
        if cursor.is_some() && cursor != self.cursor {
            self.mouse_quieto = 0.0;
        }
        self.cursor = cursor;
        e.mouse_quieto = self.mouse_quieto;
        self.entorno = e;
    }

    fn tick(&mut self) {
        let dt = self.reloj.elapsed().as_secs_f32().min(0.25);
        self.reloj = Instant::now();
        self.mouse_quieto += dt;
        self.revision += dt;
        if self.revision >= 0.5 {
            self.revision = 0.0;
            self.actualizar_entorno();
        }
        self.entorno.mouse_quieto = self.mouse_quieto;
        if !self.configurada {
            return;
        }
        if self.entorno.esconder {
            if self.visible {
                self.visible = false;
                self.esconder();
            }
            return;
        }
        if !self.visible {
            self.visible = true;
            self.ultimo_dibujo = None;
        }
        self.mascota.paso(dt, &self.entorno);
        if self.mascota.terminar {
            self.salir = true;
            return;
        }
        // La ventana no se sale de arriba: la pisada nunca queda más arriba que el ancla.
        self.mascota.y = self.mascota.y.max(self.ancla.1 as f32);
        self.dibujar();
    }

    fn esconder(&mut self) {
        let Some(capa) = &self.capa else { return };
        let Ok((buffer, lienzo)) = self.pool.create_buffer(
            self.w as i32,
            self.h as i32,
            self.w as i32 * 4,
            wl_shm::Format::Argb8888,
        ) else {
            return;
        };
        lienzo.fill(0);
        let superficie = capa.wl_surface();
        if let Ok(r) = Region::new(&self.compositor) {
            superficie.set_input_region(Some(r.wl_region()));
        }
        superficie.damage_buffer(0, 0, self.w as i32, self.h as i32);
        let _ = buffer.attach_to(superficie);
        capa.commit();
        self.ultimo_dibujo = None;
    }

    fn dibujar(&mut self) {
        let Some(capa) = &self.capa else { return };
        let m = &self.mascota;
        let img = self.aspecto.cuadro(m.anim(), m.t, m.dir);
        let emote = self.aspecto.emote(m.anim(), m.emote());
        // La zeta flota: cambia de lugar cada tanto.
        let fase = if emote.is_some() { (m.t * 8.0) as i32 } else { 0 };
        let clave = (img as *const _ as usize, emote, m.frase, fase);
        let izquierda =
            (m.x as i32 - self.ancla.0 as i32).clamp(0, (self.pantalla.ancho as i32 - self.w as i32).max(0));
        let arriba = (m.y as i32 - self.ancla.1 as i32).max(0);
        let movio = (arriba, izquierda) != self.margen;
        let cambio = self.ultimo_dibujo != Some(clave);
        if !movio && !cambio {
            return;
        }
        if movio {
            capa.set_margin(arriba, 0, 0, izquierda);
            self.margen = (arriba, izquierda);
        }
        if cambio {
            let d = lienzo::componer(self.w, self.h, self.ancla, img, emote, m.frase, m.t);
            let Ok((buffer, lienzo)) = self.pool.create_buffer(
                self.w as i32,
                self.h as i32,
                self.w as i32 * 4,
                wl_shm::Format::Argb8888,
            ) else {
                return;
            };
            for (bytes, p) in lienzo.as_chunks_mut::<4>().0.iter_mut().zip(d.px.iter()) {
                bytes.copy_from_slice(&p.to_le_bytes());
            }
            let superficie = capa.wl_surface();
            if let Ok(r) = Region::new(&self.compositor) {
                r.add(d.clic.0, d.clic.1, d.clic.2, d.clic.3);
                superficie.set_input_region(Some(r.wl_region()));
            }
            superficie.damage_buffer(0, 0, self.w as i32, self.h as i32);
            let _ = buffer.attach_to(superficie);
            self.ultimo_dibujo = Some(clave);
        }
        capa.commit();
    }

    fn abrir_asistente(&self) {
        let _ = Command::new("vortex-miku")
            .arg("alternar")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

impl PointerHandler for Estado {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        eventos: &[PointerEvent],
    ) {
        let Some(capa) = &self.capa else { return };
        let superficie = capa.wl_surface().clone();
        for ev in eventos {
            if ev.surface != superficie {
                continue;
            }
            self.mouse_quieto = 0.0;
            let pos = ev.position;
            match ev.kind {
                PointerEventKind::Press {
                    button: BOTON_IZQUIERDO,
                    ..
                } => {
                    self.presionado = Some((pos, Instant::now()));
                }
                PointerEventKind::Press {
                    button: BOTON_DERECHO,
                    ..
                } => self.mascota.despedir(),
                PointerEventKind::Motion { .. } => {
                    let Some((inicio, _)) = self.presionado else {
                        continue;
                    };
                    if self.arrastrando.is_none() && (pos.0 - inicio.0).hypot(pos.1 - inicio.1) > 5.0 {
                        // Se agarra de donde la tomaste: ese punto se queda bajo el mouse.
                        self.arrastrando =
                            Some((inicio.0 - self.ancla.0 as f64, inicio.1 - self.ancla.1 as f64));
                        self.mascota.agarrar();
                    }
                    if let Some(agarre) = self.arrastrando {
                        let x = f64::from(self.margen.1) + pos.0 - agarre.0;
                        let y = f64::from(self.margen.0) + pos.1 - agarre.1;
                        self.mascota.arrastrar(x as f32, y as f32);
                        self.dibujar();
                    }
                }
                PointerEventKind::Release {
                    button: BOTON_IZQUIERDO,
                    ..
                } => {
                    if self.arrastrando.take().is_some() {
                        self.mascota.soltar();
                    } else if self.presionado.is_some() {
                        let doble = self
                            .ultimo_clic
                            .is_some_and(|t| t.elapsed() < Duration::from_millis(400));
                        if doble {
                            self.abrir_asistente();
                            self.ultimo_clic = None;
                        } else {
                            self.mascota.clic();
                            self.ultimo_clic = Some(Instant::now());
                        }
                    }
                    self.presionado = None;
                }
                _ => {}
            }
        }
    }
}

impl CompositorHandler for Estado {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Estado {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.salidas
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Estado {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.salir = true;
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if !self.configurada {
            self.configurada = true;
            self.dibujar();
        }
    }
}

impl SeatHandler for Estado {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.asientos
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        asiento: wl_seat::WlSeat,
        c: Capability,
    ) {
        if c == Capability::Pointer && self.puntero.is_none() {
            self.puntero = self.asientos.get_pointer(qh, &asiento).ok();
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        c: Capability,
    ) {
        if c == Capability::Pointer
            && let Some(p) = self.puntero.take()
        {
            p.release();
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl ShmHandler for Estado {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Estado {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registro
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Estado);
delegate_output!(Estado);
delegate_shm!(Estado);
delegate_seat!(Estado);
delegate_pointer!(Estado);
delegate_layer!(Estado);
delegate_registry!(Estado);
