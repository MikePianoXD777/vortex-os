//! Lo de `vp` con el sonido y la ventana: el ecualizador (el del panel del escritorio), por dónde suena,
//! los ajustes, el despertador, la ventana única y el arranque del puente (los relojes de `puente.py`).

use crate::instancia;
use crate::motor::{self, con_rep, leer};
use crate::puente::qobject::Puente;
use crate::puente::{
    ARGS, cada, de_qml, despues, guardar_ajuste, hay_programa, hoy, lanzar, numero, q, texto, variante,
};
use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::{QString, QVariant};
use rand::seq::SliceRandom;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime};
use vortex_player::audio;
use vortex_player::biblioteca::{Biblioteca, Orden};
use vortex_player::ecualizador::{self, PERFILES, PRESETS};
use vortex_player::fecha;
use vortex_player::reproductor::monotonico;

fn salida_valor(s: Option<audio::SalidaAudio>) -> Value {
    match s {
        Some(s) => json!({"nombre": s.nombre, "bluetooth": s.bluetooth, "codec": s.codec, "codecTexto": s.codec_texto,
                          "hz": s.hz, "tarjeta": s.tarjeta, "ldac": s.ldac}),
        None => json!({}),
    }
}

fn mtime(r: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(r).and_then(|m| m.modified()).ok()
}

/// Que Hyprland le dé el foco a esta ventana (al abrirla otra vez desde el lanzador).
fn enfocar_en_hyprland() {
    if !hay_programa("hyprctl") || std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let ventana = format!("pid:{}", std::process::id());
    for orden in [format!("hl.dsp.focus({{ window = \"{ventana}\" }})"), format!("focuswindow {ventana}")] {
        let ok = Command::new("hyprctl")
            .args(["dispatch", &orden])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            return;
        }
    }
}

impl Puente {
    // --- ecualizador --------------------------------------------------------------------------------------------

    pub fn poner_eq(mut self: Pin<&mut Self>) {
        let e = &self.es().eq;
        let v = serde_json::to_value(e).unwrap_or(json!({}));
        let perfil = ecualizador::perfil_de(&e.bandas).to_string();
        let problema = self.es().eq_problema.clone();
        self.as_mut().set_eq(variante(&v));
        self.as_mut().set_perfil_eq(q(&perfil));
        self.set_problema_eq(q(&problema));
    }

    fn eq_cambio(mut self: Pin<&mut Self>) {
        // Mientras mueves una banda aquí, el script escribe el archivo a medias (set_bands y luego apply) y
        // releerlo regresaba la barra a un valor viejo. Se vuelve a leer cuando terminó de aplicar.
        let espera = self.es().eq_local + 2.0 - monotonico();
        if espera > 0.0 {
            despues(self.qt_thread(), (espera * 1000.0) as u64 + 50, |o| o.eq_cambio());
            return;
        }
        let nuevo = ecualizador::estado();
        if nuevo != self.es().eq {
            self.as_mut().em().eq = nuevo;
            self.as_mut().poner_eq();
        }
        self.eq_a_mpv();
    }

    fn revisar_eq(self: Pin<&mut Self>) {
        lanzar(self.qt_thread(), ecualizador::problema, |mut obj, problema| {
            if problema != obj.es().eq_problema {
                obj.as_mut().em().eq_problema = problema;
                obj.poner_eq();
            }
        });
    }

    /// En Windows el ecualizador suena dentro de mpv.
    fn eq_a_mpv(&self) {
        if ecualizador::en_mpv() {
            let filtro = ecualizador::filtro_mpv(&self.es().eq.bandas);
            con_rep(|r| r.poner_eq(&filtro));
        }
    }

    pub fn poner_banda(mut self: Pin<&mut Self>, i: i32, g: f64) {
        let Ok(i) = usize::try_from(i) else { return };
        if i >= self.es().eq.bandas.len() {
            return;
        }
        {
            let e = self.as_mut().em();
            e.eq.bandas[i].g = (g.clamp(-12.0, 12.0) * 10.0).round() / 10.0;
            e.eq.preset = "Custom".into();
            e.eq_local = monotonico();
        }
        self.as_mut().poner_eq();
        // Al soltar la barra (o 400 ms sin moverla) se aplica, como «Aplicar» en el panel
        self.esperar("eq", 400, |o| o.aplicar_bandas());
    }

    pub fn poner_preset(self: Pin<&mut Self>, nombre: &QString) {
        let nombre = nombre.to_string();
        std::thread::spawn(move || ecualizador::poner_preset(&nombre));
    }

    fn aplicar_bandas(self: Pin<&mut Self>) {
        let bandas = self.es().eq.bandas.clone();
        self.eq_a_mpv();
        lanzar(
            self.qt_thread(),
            move || ecualizador::poner_bandas(&bandas),
            |mut obj, ok| {
                if !ok {
                    obj.as_mut().revisar_eq(); // si no sonó, que la página diga por qué
                }
                // EasyEffects puede tardar más que la espera de eq_cambio: se cuenta desde que terminó
                let e = obj.as_mut().em();
                e.eq_local = e.eq_local.max(monotonico() - 1.5);
            },
        );
    }

    /// Un perfil del ecualizador (los mismos que en el teléfono) sobre tus bandas.
    pub fn poner_perfil(mut self: Pin<&mut Self>, nombre: &QString) {
        let nombre = nombre.to_string();
        if ecualizador::buscar_perfil(&nombre).is_none() {
            return;
        }
        {
            let e = self.as_mut().em();
            e.eq.bandas = ecualizador::bandas_de_perfil(&nombre, &e.eq.bandas);
            e.eq.preset = "Custom".into();
            e.eq_local = monotonico();
        }
        self.as_mut().poner_eq();
        self.aplicar_bandas();
    }

    // --- salida de audio y ajustes -------------------------------------------------------------------------------

    pub fn actualizar_salida(self: Pin<&mut Self>) {
        lanzar(
            self.qt_thread(),
            || salida_valor(audio::salida("pactl")),
            |mut obj, s| {
                if s != obj.es().salida {
                    obj.as_mut().em().salida = s.clone();
                    obj.set_salida(variante(&s));
                }
            },
        );
    }

    /// `pactl subscribe` avisa cuando cambia por dónde suena, en vez de preguntar cada 3 s.
    fn escuchar_salida(self: Pin<&mut Self>) {
        let hilo = self.qt_thread();
        let vivo = self.es().vivo.clone();
        std::thread::spawn(move || {
            while vivo.load(Ordering::Relaxed) {
                // setpriv: si el Player muere de golpe, pactl se va con él y no se queda colgado
                let mut cmd = if hay_programa("setpriv") {
                    let mut c = Command::new("setpriv");
                    c.args(["--pdeathsig", "KILL", "pactl", "subscribe"]);
                    c
                } else {
                    let mut c = Command::new("pactl");
                    c.arg("subscribe");
                    c
                };
                let Ok(mut hijo) = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
                    return; // sin pactl
                };
                if let Some(out) = hijo.stdout.take() {
                    for linea in BufReader::new(out).lines() {
                        let Ok(l) = linea else { break };
                        if audio::cambia_la_salida(&l) {
                            // llegan varios juntos: se lee una sola vez
                            let _ = hilo.queue(|o| o.esperar("salida", 400, |o| o.actualizar_salida()));
                        }
                    }
                }
                let _ = hijo.wait();
                // Se reinició PipeWire, por ejemplo: pactl se sale. Se vuelve a escuchar.
                if hilo.queue(|o| o.actualizar_salida()).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_secs(5));
            }
        });
    }

    pub fn forzar_ldac(self: Pin<&mut Self>) {
        let tarjeta = texto(&self.es().salida, "tarjeta");
        lanzar(
            self.qt_thread(),
            move || audio::forzar_ldac(&tarjeta, "pactl"),
            |mut obj, ok| {
                if ok {
                    obj.as_mut().avisar("Cambiado a LDAC");
                    despues(obj.qt_thread(), 1500, |o| o.actualizar_salida());
                } else {
                    obj.avisar("Estos audífonos no ofrecen LDAC ahorita");
                }
            },
        );
    }

    pub fn poner_ldac_hq(mut self: Pin<&mut Self>, activar: bool) {
        guardar_ajuste("ldac_hq", json!(activar));
        if audio::config_ldac(activar) {
            self.as_mut().em().audio_pendiente = true;
        }
        self.poner_ajustes();
    }

    pub fn poner_navegador(self: Pin<&mut Self>, navegador: &QString) {
        let n = navegador.to_string();
        let n = if crate::puente::NAVEGADORES.contains(&n.as_str()) { n } else { String::new() };
        guardar_ajuste("navegador_premium", json!(n));
        self.poner_ajustes();
    }

    pub fn guardar_volumen(self: Pin<&mut Self>, v: i32) {
        con_rep(|r| r.poner_volumen(i64::from(v)));
        guardar_ajuste("volumen", json!(v));
    }

    /// Vuelve a arrancar PipeWire para que lea la frecuencia nativa y LDAC (corta el sonido 1 s).
    pub fn aplicar_audio(self: Pin<&mut Self>) {
        let (posicion, sonaba) = leer(|r| (r.posicion(), !r.pausado()));
        con_rep(|r| r.pausar());
        lanzar(self.qt_thread(), audio::reiniciar_audio, move |mut obj, ok| {
            if ok {
                obj.as_mut().em().audio_pendiente = false;
                obj.as_mut().poner_ajustes();
                obj.as_mut().avisar("Listo: frecuencia nativa y LDAC a 990 kbps");
                if sonaba {
                    despues(obj.qt_thread(), 2500, move |_| {
                        con_rep(|r| {
                            r.buscar(posicion);
                            r.seguir();
                        });
                    });
                }
            } else {
                obj.as_mut().avisar("No pude reiniciar el audio");
            }
            despues(obj.qt_thread(), 2500, |o| o.actualizar_salida());
        });
    }

    pub fn poner_discord(self: Pin<&mut Self>, activar: bool) {
        guardar_ajuste("discord", json!(activar));
        self.poner_ajustes();
        motor::poner_discord(activar);
    }

    pub fn poner_fundido(self: Pin<&mut Self>, segundos: i32) {
        guardar_ajuste("fundido", json!(segundos));
        con_rep(|r| r.poner_fundido(f64::from(segundos)));
        self.poner_ajustes();
    }

    // --- el despertador (con la app abierta) ---------------------------------------------------------------------

    /// {"activo", "hora": "07:00", "playlist": -1 (tu mezcla del día) o el id de una}
    pub fn poner_despertador(self: Pin<&mut Self>, d: &QVariant) {
        let d = de_qml(d);
        let mut actual = Biblioteca::ajustes().valor("despertador").and_then(|v| v.as_object().cloned()).unwrap_or_default();
        for k in ["activo", "hora", "playlist"] {
            if let Some(v) = d.get(k) {
                actual.insert(k.into(), v.clone());
            }
        }
        guardar_ajuste("despertador", Value::Object(actual));
        self.poner_ajustes();
    }

    fn revisar_despertador(mut self: Pin<&mut Self>) {
        let d = Biblioteca::ajustes().valor("despertador").cloned().unwrap_or(json!({}));
        let l = fecha::local(fecha::ahora());
        let ahora = format!("{:02}:{:02}", l.hora, l.minuto);
        let hoy = hoy();
        if d["activo"] != json!(true) || texto(&d, "hora") != ahora || self.es().despertado == hoy {
            return;
        }
        self.as_mut().em().despertado = hoy;
        let playlist = if d.get("playlist").is_some() { numero(&d, "playlist") as i32 } else { -1 };
        self.despertar(playlist);
    }

    /// Suena tu mezcla del día (o esa playlist) subiendo el volumen poco a poco durante un minuto.
    pub fn despertar(mut self: Pin<&mut Self>, playlist: i32) {
        let mut lista: Vec<Value> = if playlist < 0 {
            self.as_mut().canciones_mezcla()
        } else {
            self.bib()
                .canciones_de_playlist(i64::from(playlist))
                .unwrap_or_default()
                .iter()
                .map(|c| crate::puente::cancion_valor(c, ""))
                .collect()
        };
        if lista.is_empty() {
            lista = self
                .bib()
                .canciones("", Orden::Agregada)
                .unwrap_or_default()
                .iter()
                .map(|c| crate::puente::cancion_valor(c, ""))
                .collect();
            lista.shuffle(&mut rand::rng());
        }
        if lista.is_empty() {
            return;
        }
        let meta = leer(|r| r.volumen()).max(30);
        con_rep(|r| {
            r.poner_volumen(5);
            r.poner_origen("Despertador");
            r.tocar(&lista, 0);
        });
        self.as_mut().mostrar_ventana();
        fn subir(obj: Pin<&mut Puente>, meta: i64, paso: i64) {
            con_rep(|r| r.poner_volumen(5 + ((meta - 5) as f64 * paso as f64 / 30.0).round() as i64));
            if paso < 30 {
                despues(obj.qt_thread(), 2000, move |o| subir(o, meta, paso + 1));
            }
        }
        despues(self.qt_thread(), 2000, move |o| subir(o, meta, 1));
    }

    // --- lo que llega de otra copia (o de la línea de comandos) -----------------------------------------------------

    /// Enlaces → a bajar; archivos o carpetas → a la biblioteca.
    pub fn recibir(mut self: Pin<&mut Self>, args: Vec<String>) {
        let mut archivos = Vec::new();
        for a in args {
            let enlace = ["http://", "https://", "spotify:", "music.youtube", "youtu", "open.spotify"]
                .iter()
                .any(|p| a.starts_with(p));
            if enlace {
                self.as_mut().bajar_con(&a, -1, "", "");
            } else if !a.is_empty() {
                archivos.push(PathBuf::from(crate::puente::ruta_de(&a)));
            }
        }
        if !archivos.is_empty() {
            self.agregar_rutas(archivos);
        }
    }

    fn escuchar_otras_copias(self: Pin<&mut Self>) {
        let Some(escucha) = instancia::escuchar(&instancia::ruta()) else { return };
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            for conexion in escucha.incoming() {
                let Ok(c) = conexion else { continue };
                let Some(args) = instancia::leer(c) else { continue };
                let r = hilo.queue(move |mut o| {
                    o.as_mut().recibir(args);
                    o.mostrar_ventana();
                    enfocar_en_hyprland();
                });
                if r.is_err() {
                    break;
                }
            }
        });
    }
}

impl cxx_qt::Initialize for Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        motor::registrar_vp(self.qt_thread());
        let avance = self.as_mut().nuevo_avance();
        self.as_mut().em().avance = avance;
        let prueba = self.es().prueba;

        self.as_mut().set_presets(variante(&json!(PRESETS)));
        self.as_mut().set_perfiles(variante(&json!(PERFILES.iter().map(|(n, _)| *n).collect::<Vec<_>>())));
        self.as_mut().revisar_eq();
        self.as_mut().poner_eq();
        self.eq_a_mpv();
        con_rep(|r| r.poner_fundido(Biblioteca::ajustes().numero("fundido", 5.0)));

        // Sin configurar nada: frecuencia nativa y LDAC a 990 kbps quedan listos desde la primera vez (se notan
        // al volver a entrar, o con «Aplicar ahora» en Ajustes).
        if !prueba {
            let hires = audio::config_hires();
            let ldac = audio::config_ldac(Biblioteca::ajustes().booleano("ldac_hq", true));
            self.as_mut().em().audio_pendiente = hires || ldac;
            self.as_mut().escuchar_salida();
            self.as_mut().actualizar_salida();
        }

        for f in [
            Puente::poner_ajustes,
            Puente::poner_avance,
            Puente::poner_resumen,
            Puente::poner_mezcla,
            Puente::poner_novedades,
            Puente::poner_seguir,
            Puente::poner_descargas,
        ] {
            f(self.as_mut());
        }
        self.as_mut().poner_nube(json!({}));
        let letras = self.es().letras_estado.clone();
        self.as_mut().set_letras_guardadas(variante(&letras));
        self.as_mut().recargar();

        // El ecualizador es el del panel del escritorio: si cambia allá, cambia aquí.
        let hilo = self.qt_thread();
        let vivo = self.es().vivo.clone();
        std::thread::spawn(move || {
            let archivo = ecualizador::archivo_estado();
            let mut antes = mtime(&archivo);
            while vivo.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(700));
                let ahora = mtime(&archivo);
                if ahora != antes {
                    antes = ahora;
                    if hilo.queue(|o| o.eq_cambio()).is_err() {
                        break;
                    }
                }
            }
        });

        // Los relojes de la de Python.
        let (h, v) = (self.qt_thread(), self.es().vivo.clone());
        cada(h.clone(), v.clone(), 10 * 60 * 1000, |o| o.sincronizar());
        cada(h.clone(), v.clone(), 5000, |o| {
            if !leer(|r| r.pausado()) {
                o.guardar_sesion();
            }
        });
        cada(h.clone(), v.clone(), 60 * 60 * 1000, |mut o| {
            o.as_mut().em().resumen = json!({}); // por si la semana cambia con la app abierta
            o.poner_resumen();
        });
        cada(h.clone(), v.clone(), 60 * 1000, |o| {
            if !leer(|r| r.pausado()) {
                o.subir_sonando(0);
            }
        });
        cada(h.clone(), v.clone(), 5 * 60 * 1000, |o| o.revisar_otro_aparato());
        cada(h.clone(), v.clone(), 20 * 1000, |o| o.revisar_despertador());
        cada(h.clone(), v, 60 * 1000, |o| o.poner_seguir()); // «hace N min»
        despues(h.clone(), 4000, |o| o.revisar_otro_aparato());
        despues(h.clone(), 8000, |o| o.armar_mezcla());
        despues(h.clone(), 15000, |o| o.revisar_novedades());

        let _ = h.queue(|o| o.restaurar());
        let _ = h.queue(|o| o.escanear());
        if !prueba {
            self.as_mut().escuchar_otras_copias();
            motor::arrancar_extras(Biblioteca::ajustes().booleano("discord", true));
        }
        let args = ARGS.get().cloned().unwrap_or_default();
        if !args.is_empty() {
            let _ = h.queue(move |o| o.recibir(args));
        }
    }
}
