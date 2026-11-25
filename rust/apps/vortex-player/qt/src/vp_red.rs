//! Lo de `vp` que va a internet: buscar en YouTube Music, las recomendaciones, el avance de 30 s, las
//! descargas, «Para ti», la radio sin fin, tu mezcla del día, lo nuevo de tus artistas, compartir y las
//! letras (las mismas funciones de `puente.py`).

use crate::motor::{con_rep, leer};
use crate::puente::qobject::Puente;
use crate::puente::{
    a_la_papelera, abrir, cancion_valor, de_qml, guardar_json, hay_programa, hoy, id_o_ruta, lanzar, numero,
    semilla, tallo, texto, titulo_y, variante, verdadero,
};
use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::{QString, QVariant};
use rand::seq::{IndexedRandom, SliceRandom};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::Ordering;
use vortex_player::avance::{Avance, DeAvance};
use vortex_player::biblioteca::{Biblioteca, Cancion, Orden, id_de};
use vortex_player::letras::{self, Letra};
use vortex_player::{descargas, fecha, rutas};

const SIN_CONEXION: &str = "Sin conexión con YouTube";

fn claro(e: String) -> String {
    if e.is_empty() { SIN_CONEXION.to_string() } else { e }
}

/// Para «tienes» de las recomendaciones: {id, titulo, artista} de una de YouTube Music ya mostrada.
fn ya_mostrada(c: &Value) -> Value {
    json!({"id": texto(c, "id"), "titulo": texto(c, "titulo"), "artista": texto(c, "artistas")})
}

fn letra_youtube(vid: &str) -> Result<Option<String>, String> {
    descargas::letra_youtube(vid, &descargas::pedir_json)
}

impl Puente {
    // --- buscar en YouTube Music / YouTube --------------------------------------------------------------------

    pub fn buscar_en_linea(mut self: Pin<&mut Self>, texto_: &QString, fuente: &QString) {
        let n = {
            let e = self.as_mut().em();
            e.busqueda_linea += 1;
            e.error_linea.clear();
            e.busqueda_linea
        };
        let t = texto_.to_string().trim().to_string();
        let fuente = fuente.to_string();
        if t.is_empty() {
            let e = self.as_mut().em();
            e.en_linea.clear();
            e.buscando_linea = false;
            self.poner_en_linea();
            return;
        }
        self.as_mut().em().buscando_linea = true;
        self.as_mut().poner_en_linea();
        lanzar(
            self.qt_thread(),
            move || descargas::buscar(&t, &fuente, 20, "yt-dlp", &descargas::pedir_json),
            move |mut obj, r| {
                if obj.es().busqueda_linea != n {
                    return;
                }
                let e = obj.as_mut().em();
                match r {
                    Ok(l) => {
                        e.en_linea = l;
                        e.error_linea.clear();
                    }
                    Err(err) => {
                        e.en_linea.clear();
                        e.error_linea = claro(err);
                    }
                }
                e.buscando_linea = false;
                obj.poner_en_linea();
            },
        );
    }

    // --- recomendaciones para una playlist --------------------------------------------------------------------

    /// Las de una playlist: las guardadas, o con «otras» unas nuevas (otras canciones de semilla).
    pub fn recomendar(mut self: Pin<&mut Self>, pid: i32, otras: bool) {
        let pid = i64::from(pid);
        let vuelta = {
            let e = self.as_mut().em();
            e.recom_vuelta += 1;
            e.recom_para = pid;
            e.recom_error.clear();
            e.recom_vuelta
        };
        if !otras && let Some(g) = self.es().recom_guardadas.get(&pid).cloned() {
            let e = self.as_mut().em();
            e.recom = g;
            e.recom_cargando = false;
            self.poner_recomendaciones();
            return;
        }
        let (de_la_playlist, todas) = {
            let b = self.bib();
            (
                b.canciones_de_playlist(pid).unwrap_or_default(),
                b.canciones("", Orden::Agregada).unwrap_or_default(),
            )
        };
        if de_la_playlist.is_empty() {
            let e = self.as_mut().em();
            e.recom.clear();
            e.recom_cargando = false;
            self.poner_recomendaciones();
            return;
        }
        // Hasta 3 de semilla, al azar: «Otras» trae algo distinto cada vez. Primero las bajadas de YouTube
        // (ya traen su id y no hay que buscarlas).
        let mut rng = rand::rng();
        let (mut con_id, mut sin_id): (Vec<&Cancion>, Vec<&Cancion>) =
            de_la_playlist.iter().partition(|c| !id_de(&c.ruta).is_empty());
        con_id.shuffle(&mut rng);
        sin_id.shuffle(&mut rng);
        let semillas: Vec<Value> = con_id.into_iter().chain(sin_id).take(3).map(semilla).collect();
        let mut tienes: Vec<Value> = todas.iter().map(semilla).collect();
        if otras {
            // «Otras» no repite las que ya te había enseñado
            if let Some(g) = self.es().recom_guardadas.get(&pid) {
                tienes.extend(g.iter().map(ya_mostrada));
            }
        }
        {
            let e = self.as_mut().em();
            e.recom_cargando = true;
            if otras {
                e.recom.clear();
            }
        }
        self.as_mut().poner_recomendaciones();
        lanzar(
            self.qt_thread(),
            move || descargas::recomendar(&semillas, &tienes, 20, &descargas::pedir_json),
            move |mut obj, r| {
                let (canciones, error) = match r {
                    Ok(l) => (l, String::new()),
                    Err(e) => (Vec::new(), claro(e)),
                };
                let e = obj.as_mut().em();
                if !canciones.is_empty() {
                    e.recom_guardadas.insert(pid, canciones.clone());
                }
                if vuelta != e.recom_vuelta {
                    return;
                }
                e.recom = canciones;
                e.recom_error = error;
                e.recom_cargando = false;
                obj.poner_recomendaciones();
            },
        );
    }

    /// La baja a tu biblioteca y la agrega a la playlist para la que se recomendó.
    pub fn agregar_recomendada(self: Pin<&mut Self>, i: i32) {
        let lista = self.sin_las_tuyas(&self.es().recom);
        let para = self.es().recom_para;
        if let Some(c) = usize::try_from(i).ok().and_then(|i| lista.get(i))
            && para >= 0
        {
            let nombre = titulo_y(&texto(c, "titulo"), &texto(c, "artistas"));
            self.bajar_con(&texto(c, "url"), para, &texto(c, "id"), &nombre);
        }
    }

    // --- avance: 30 s antes de bajarla --------------------------------------------------------------------------

    pub fn nuevo_avance(self: Pin<&mut Self>) -> Avance {
        let hilo = self.qt_thread();
        Avance::new(move |d| {
            let _ = hilo.queue(move |o| o.de_avance(d));
        })
    }

    pub fn de_avance(mut self: Pin<&mut Self>, d: DeAvance) {
        match d {
            DeAvance::Cambio => {}
            DeAvance::Empezo => {
                let sonaba = !leer(|r| r.pausado());
                self.as_mut().em().seguir_tras_avance = sonaba;
                con_rep(|r| r.pausar());
            }
            DeAvance::Termino => {
                if std::mem::take(&mut self.as_mut().em().seguir_tras_avance) {
                    con_rep(|r| r.seguir());
                }
            }
            DeAvance::Error(t) => self.as_mut().avisar(&t),
        }
        self.poner_avance();
    }

    fn escuchar(self: Pin<&mut Self>, c: &Value) {
        let a = &self.es().avance;
        if a.id_actual() == texto(c, "id") {
            a.parar();
        } else {
            a.tocar(
                &texto(c, "id"),
                &texto(c, "url"),
                numero(c, "duracion"),
                leer(|r| r.volumen()),
            );
        }
    }

    /// Toca 30 s de la recomendada i (otra vez, la para).
    pub fn escuchar_recomendada(self: Pin<&mut Self>, i: i32) {
        let lista = self.sin_las_tuyas(&self.es().recom);
        if let Some(c) = usize::try_from(i).ok().and_then(|i| lista.get(i)).cloned() {
            self.escuchar(&c);
        }
    }

    pub fn escuchar_resultado(self: Pin<&mut Self>, i: i32) {
        if let Some(c) = usize::try_from(i)
            .ok()
            .and_then(|i| self.es().en_linea.get(i))
            .cloned()
        {
            self.escuchar(&c);
        }
    }

    pub fn parar_avance(self: Pin<&mut Self>) {
        self.es().avance.parar();
    }

    // --- descargas ---------------------------------------------------------------------------------------------

    /// Baja el resultado i a la biblioteca; con playlist ≥ 0, además lo agrega a esa playlist.
    pub fn bajar_resultado(self: Pin<&mut Self>, i: i32, playlist: i32) {
        if let Some(c) = usize::try_from(i)
            .ok()
            .and_then(|i| self.es().en_linea.get(i))
            .cloned()
        {
            let nombre = titulo_y(&texto(&c, "titulo"), &texto(&c, "artistas"));
            self.bajar_con(&texto(&c, "url"), i64::from(playlist), &texto(&c, "id"), &nombre);
        }
    }

    pub fn bajar(self: Pin<&mut Self>, url: &QString) {
        self.bajar_con(&url.to_string(), -1, "", "");
    }

    pub fn bajar_con(mut self: Pin<&mut Self>, url: &str, playlist: i64, vid: &str, nombre: &str) {
        let url = url.trim().to_string();
        let Some((servicio, _, _)) = descargas::tipo_de(&url) else {
            self.avisar("Pega un enlace de YouTube, YouTube Music o Spotify");
            return;
        };
        let n = self.es().descargas.len();
        let nombre = if nombre.is_empty() {
            url.clone()
        } else {
            nombre.to_string()
        };
        self.as_mut().em().descargas.push(json!({
            "url": url, "servicio": servicio, "nombre": nombre, "total": 0, "hechas": 0, "fallidas": 0,
            "actual": "En espera…", "porcentaje": 0.0, "estado": "espera", "avisos": [], "playlist": playlist,
            "id": vid,
        }));
        self.as_mut().poner_descargas();
        let cola = self.es().cola_urls.clone();
        let arrancar = {
            let mut c = cola.lock().unwrap_or_else(|e| e.into_inner());
            c.0.push_back((n, url));
            !std::mem::replace(&mut c.1, true)
        };
        if !arrancar {
            return;
        }
        let parar = self.es().parar_descargas.clone();
        parar.store(false, Ordering::SeqCst);
        let bib = self.es().bib.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                let siguiente = {
                    let mut c = cola.lock().unwrap_or_else(|e| e.into_inner());
                    let s = if parar.load(Ordering::SeqCst) {
                        None
                    } else {
                        c.0.pop_front()
                    };
                    if s.is_none() {
                        c.1 = false; // con el candado: una que llegue ahora arranca otro hilo
                    }
                    s
                };
                let Some((n, url)) = siguiente else { break };
                let _ = hilo.queue(move |o| o.en_evento(n, json!({"tipo": "empieza"})));
                let navegador = Biblioteca::ajustes().texto("navegador_premium");
                descargas::descargar(
                    &url,
                    &rutas::descargas(),
                    &navegador,
                    "yt-dlp",
                    &descargas::bajar_texto,
                    &|| parar.load(Ordering::SeqCst),
                    &mut |mut ev| {
                        if ev["tipo"] == "archivo" {
                            // Se lee aquí (en el hilo) para no trabar la ventana con ffprobe
                            let b = bib.lock().unwrap_or_else(|e| e.into_inner());
                            let ok = b
                                .agregar_archivo(Path::new(&texto(&ev, "ruta")), "youtube")
                                .ok()
                                .flatten()
                                .is_some();
                            ev["ok"] = json!(ok);
                        }
                        let _ = hilo.queue(move |o| o.en_evento(n, ev));
                    },
                );
            }
        });
    }

    pub fn en_evento(mut self: Pin<&mut Self>, n: usize, ev: Value) {
        let bib = self.es().bib.clone();
        let candado = || bib.lock().unwrap_or_else(|e| e.into_inner());
        let (mut recargar, mut local, mut listo, mut letras_) = (false, false, None, false);
        {
            let e = self.as_mut().em();
            let Some(d) = e.descargas.get_mut(n) else { return };
            match ev["tipo"].as_str().unwrap_or("") {
                "empieza" => {
                    d["estado"] = json!("bajando");
                    d["actual"] = json!("Leyendo el enlace…");
                }
                "inicio" => {
                    // Lo que viene de la búsqueda ya trae su nombre («Título · Artista»)
                    let es_lista = verdadero(ev.get("es_lista"));
                    let nombre = texto(&ev, "nombre");
                    d["nombre"] = json!(if !nombre.is_empty() {
                        nombre
                    } else if d["nombre"] != d["url"] {
                        texto(d, "nombre")
                    } else if es_lista {
                        "Playlist".into()
                    } else {
                        "Canción".into()
                    });
                    d["total"] = ev.get("total").cloned().unwrap_or(json!(0));
                    if es_lista {
                        d["playlist"] = json!(
                            candado()
                                .crear_playlist(&texto(d, "nombre"), &texto(d, "url"))
                                .unwrap_or(-1)
                        );
                    }
                }
                "cancion" => {
                    d["actual"] = json!(texto(&ev, "titulo"));
                    d["porcentaje"] = json!(0.0);
                }
                "progreso" => {
                    d["porcentaje"] = json!(numero(&ev, "porcentaje"));
                    let t = texto(&ev, "titulo");
                    if !t.is_empty() && d["servicio"] != "spotify" {
                        d["actual"] = json!(t);
                    }
                }
                "archivo" => {
                    d["hechas"] = json!(d["hechas"].as_i64().unwrap_or(0) + 1);
                    let nombre = texto(d, "nombre");
                    if nombre == texto(d, "url") || nombre == "Canción" {
                        let t = tallo(&texto(&ev, "ruta"));
                        d["nombre"] = json!(t.rsplit_once(" [").map_or(t.as_str(), |(a, _)| a));
                    }
                    let pid = d["playlist"].as_i64().unwrap_or(-1);
                    if pid >= 0 && verdadero(ev.get("ok")) {
                        let _ = candado().agregar_a_playlist(pid, &texto(&ev, "ruta"));
                    }
                    recargar = true;
                }
                "aviso" => {
                    let mut avisos: Vec<Value> = d["avisos"].as_array().cloned().unwrap_or_default();
                    avisos.push(json!(texto(&ev, "texto")));
                    let desde = avisos.len().saturating_sub(5);
                    d["avisos"] = json!(avisos[desde..]);
                    if verdadero(ev.get("fatal")) {
                        d["estado"] = json!("error");
                        d["actual"] = json!(texto(&ev, "texto"));
                    }
                }
                "fin" => {
                    let fallidas = ev["fallidas"].as_i64().unwrap_or(0);
                    d["fallidas"] = json!(fallidas);
                    let id = texto(d, "id");
                    if e.nube_pendientes.remove(&id) && d["hechas"].as_i64().unwrap_or(0) == 0 {
                        e.nube_fallidas.insert(id);
                    }
                    local = true; // lo nuevo sube a la nube y entra a sus playlists
                    if d["estado"] != "error" {
                        d["estado"] = json!("lista");
                        let mut t = format!("{} bajadas", ev["bajadas"].as_i64().unwrap_or(0));
                        if fallidas > 0 {
                            t.push_str(&format!(" · {fallidas} no se encontraron"));
                        }
                        d["actual"] = json!(t);
                        listo = Some(format!("Listo: {}", texto(d, "nombre")));
                    }
                    d["porcentaje"] = json!(100.0);
                    letras_ = true;
                }
                _ => {}
            }
        }
        if recargar {
            self.as_mut().recargar();
        }
        if local {
            self.as_mut().cambio_local();
        }
        self.as_mut().poner_descargas();
        if let Some(t) = listo {
            self.as_mut().avisar(&t);
        }
        if letras_ {
            self.esperar("letras", 20000, |o| o.bajar_letras()); // su letra, para verla sin internet
        }
    }

    pub fn parar_descargas(self: Pin<&mut Self>) {
        self.es().parar_descargas.store(true, Ordering::SeqCst);
        self.es()
            .cola_urls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .0
            .clear();
    }

    pub fn limpiar_descargas(mut self: Pin<&mut Self>) {
        if self.es().cola_urls.lock().unwrap_or_else(|e| e.into_inner()).1 {
            return;
        }
        self.as_mut().em().descargas.clear();
        self.poner_descargas();
    }

    // --- «Para ti» en Agregar música --------------------------------------------------------------------------

    pub fn recomendar_para_ti(mut self: Pin<&mut Self>, otras: bool) {
        let mut rng = rand::rng();
        let h: Vec<Value> = self
            .es()
            .sesion
            .get("historial")
            .and_then(|h| h.as_array())
            .cloned()
            .unwrap_or_default();
        let mut semillas: Vec<Value> = if otras {
            let mut diez: Vec<Value> = h.iter().take(10).cloned().collect();
            diez.shuffle(&mut rng);
            diez
        } else {
            h
        };
        semillas.truncate(3);
        let todas = self.bib().canciones("", Orden::Agregada).unwrap_or_default();
        if semillas.is_empty() {
            semillas = todas.choose_multiple(&mut rng, 3).map(semilla).collect();
        }
        if semillas.is_empty() {
            return;
        }
        let de: Vec<String> = semillas.iter().map(|x| texto(x, "titulo")).collect();
        if !otras && !self.es().para_ti.is_empty() && de == self.es().para_ti_de {
            return;
        }
        let mut tienes: Vec<Value> = todas.iter().map(semilla).collect();
        if otras {
            tienes.extend(self.es().para_ti.iter().map(ya_mostrada));
        }
        let vuelta = {
            let e = self.as_mut().em();
            e.para_ti_vuelta += 1;
            e.para_ti_cargando = true;
            e.para_ti_error.clear();
            e.para_ti_vuelta
        };
        self.as_mut().poner_para_ti();
        lanzar(
            self.qt_thread(),
            move || descargas::recomendar(&semillas, &tienes, 25, &descargas::pedir_json),
            move |mut obj, r| {
                if obj.es().para_ti_vuelta != vuelta {
                    return;
                }
                let (canciones, error) = match r {
                    Ok(l) => (l, String::new()),
                    Err(e) => (Vec::new(), claro(e)),
                };
                let e = obj.as_mut().em();
                if !canciones.is_empty() || e.para_ti.is_empty() {
                    e.para_ti = canciones;
                    e.para_ti_de = de;
                }
                e.para_ti_error = error;
                e.para_ti_cargando = false;
                obj.poner_para_ti();
            },
        );
    }

    pub fn bajar_para_ti(self: Pin<&mut Self>, i: i32) {
        let lista = self.sin_las_tuyas(&self.es().para_ti);
        if let Some(c) = usize::try_from(i).ok().and_then(|i| lista.get(i)) {
            let nombre = titulo_y(&texto(c, "titulo"), &texto(c, "artistas"));
            self.bajar_con(&texto(c, "url"), -1, &texto(c, "id"), &nombre);
        }
    }

    pub fn escuchar_para_ti(self: Pin<&mut Self>, i: i32) {
        let lista = self.sin_las_tuyas(&self.es().para_ti);
        if let Some(c) = usize::try_from(i).ok().and_then(|i| lista.get(i)).cloned() {
            self.escuchar(&c);
        }
    }

    // --- la radio sin fin -----------------------------------------------------------------------------------------

    pub fn poner_radio(mut self: Pin<&mut Self>, activar: bool) {
        crate::puente::guardar_ajuste("radio", json!(activar));
        self.as_mut().poner_ajustes();
        if activar {
            self.pedir_radio();
        }
    }

    /// ¿La conexión se cobra por lo que gastas (datos del cel)?
    fn con_datos_medidos() -> bool {
        match std::env::var("VORTEX_PLAYER_MEDIDA") {
            Ok(v) => v == "1",
            Err(_) => crate::arranque::ffi::datos_medidos(),
        }
    }

    /// Una de YouTube Music (de `descargas::buscar`/`radio`) como canción de la cola, que suena sin bajarla.
    pub fn de_la_radio(mut self: Pin<&mut Self>, c: &Value) -> Value {
        let url = texto(c, "url");
        self.as_mut()
            .em()
            .portadas
            .insert(url.clone(), texto(c, "portada"));
        json!({"ruta": url, "id": texto(c, "id"), "titulo": texto(c, "titulo"), "artista": texto(c, "artistas"),
               "album": texto(c, "album"), "duracion": numero(c, "duracion"), "portada": texto(c, "portada"),
               "radio": true})
    }

    /// Diez de tu biblioteca: primero de los mismos artistas, luego al azar.
    fn radio_local(&self, semillas: &[Value], en_cola: &HashSet<String>, todas: &[Cancion]) -> Vec<Value> {
        let artistas: HashSet<String> = semillas
            .iter()
            .map(|s| letras::limpiar_artista(&texto(s, "artista")).to_lowercase())
            .filter(|a| !a.is_empty())
            .collect();
        let mut libres: Vec<&Cancion> = todas.iter().filter(|c| !en_cola.contains(&c.ruta)).collect();
        libres.shuffle(&mut rand::rng());
        let (mismas, otras): (Vec<&Cancion>, Vec<&Cancion>) = libres
            .into_iter()
            .partition(|c| artistas.contains(&letras::limpiar_artista(&c.artista).to_lowercase()));
        mismas
            .into_iter()
            .chain(otras)
            .take(10)
            .map(|c| cancion_valor(c, self.es().portadas.get(&c.ruta).map_or("", String::as_str)))
            .collect()
    }

    /// Suena la última de la cola: se piden 10 parecidas a ella y a las dos de antes.
    pub fn pedir_radio(mut self: Pin<&mut Self>) {
        let (cola, i, origen, repetir) = leer(|r| {
            (
                r.cola(),
                r.indice(),
                r.origen().to_string(),
                r.repetir().to_string(),
            )
        });
        // La que pediste con «Iniciar radio» sigue aunque la radio sin fin esté apagada
        let a_mano = origen.starts_with("Radio de ");
        let Some(ultima) = cola.last() else { return };
        let ultima = texto(ultima, "ruta");
        let largo = i64::try_from(cola.len()).unwrap_or(i64::MAX);
        if !(a_mano || Biblioteca::ajustes().booleano("radio", true))
            || i + 1 < largo
            || repetir != "no"
            || self.es().radio_para == ultima
        {
            return;
        }
        let medida = Self::con_datos_medidos();
        if self.es().sin_red && !medida {
            return;
        }
        let i = usize::try_from(i.max(0)).unwrap_or(0);
        let semillas: Vec<Value> = cola[i.saturating_sub(2)..=i.min(cola.len() - 1)]
            .iter()
            .rev()
            .map(
                |c| json!({"id": id_o_ruta(c), "titulo": texto(c, "titulo"), "artista": texto(c, "artista")}),
            )
            .collect();
        let todas = self.bib().canciones("", Orden::Agregada).unwrap_or_default();
        if medida {
            // Con datos móviles la radio sigue con tu música (no gasta tu plan)
            let en_cola: HashSet<String> = cola.iter().map(|c| texto(c, "ruta")).collect();
            let lista = self.radio_local(&semillas, &en_cola, &todas);
            con_rep(|r| r.agregar_radio(&lista));
            if !std::mem::replace(&mut self.as_mut().em().aviso_datos, true) {
                self.avisar("Con datos móviles, la radio sigue con tu música");
            }
            return;
        }
        let mut tienes: Vec<Value> = todas.iter().map(semilla).collect();
        tienes.extend(cola.iter().map(
            |c| json!({"id": id_o_ruta(c), "titulo": texto(c, "titulo"), "artista": texto(c, "artista")}),
        ));
        let vuelta = {
            let e = self.as_mut().em();
            e.radio_vuelta += 1;
            e.radio_para = ultima;
            e.radio_vuelta
        };
        lanzar(
            self.qt_thread(),
            move || descargas::recomendar(&semillas, &tienes, 10, &descargas::pedir_json),
            move |obj, r| obj.fin_radio(vuelta, r),
        );
    }

    fn fin_radio(mut self: Pin<&mut Self>, vuelta: u64, r: Result<Vec<Value>, String>) {
        if vuelta != self.es().radio_vuelta {
            return;
        }
        let para = std::mem::take(&mut self.as_mut().em().radio_para);
        let (cola, origen, i, pausado, cancion) = leer(|r| {
            (
                r.cola(),
                r.origen().to_string(),
                r.indice(),
                r.pausado(),
                r.cancion(),
            )
        });
        if cola.last().map(|c| texto(c, "ruta")) != Some(para)
            || !(origen.starts_with("Radio de ") || Biblioteca::ajustes().booleano("radio", true))
        {
            return;
        }
        let canciones = match r {
            Ok(l) if !l.is_empty() => l,
            otro => {
                let largo = i64::try_from(cola.len()).unwrap_or(i64::MAX);
                if verdadero(Some(&cancion)) && i + 1 >= largo && pausado {
                    let t = match otro {
                        Err(_) => "Sin conexión: la radio no pudo seguir",
                        Ok(_) => "La radio no encontró más canciones",
                    };
                    self.avisar(t);
                }
                return;
            }
        };
        let lista: Vec<Value> = canciones.iter().map(|c| self.as_mut().de_la_radio(c)).collect();
        con_rep(|r| r.agregar_radio(&lista));
    }

    /// Toca esta canción y detrás, sin fin, parecidas a ella (como «Ir a la radio» de Spotify).
    pub fn iniciar_radio(mut self: Pin<&mut Self>, c: &QVariant) {
        let c = de_qml(c);
        let ruta = texto(&c, "ruta");
        if ruta.is_empty() {
            return;
        }
        let titulo = texto(&c, "titulo");
        let nombre = if titulo.is_empty() {
            tallo(&ruta)
        } else {
            titulo.clone()
        };
        con_rep(|r| {
            r.tocar(std::slice::from_ref(&c), 0);
            r.poner_origen(&format!("Radio de {nombre}"));
        });
        self.as_mut().em().radio_para.clear();
        self.as_mut().pedir_radio();
        let t = if titulo.is_empty() {
            "esta canción".to_string()
        } else {
            titulo
        };
        self.avisar(&format!("Radio de «{t}»"));
    }

    /// La de la radio que suena (o una de la cola), a tu biblioteca.
    pub fn bajar_de_la_radio(mut self: Pin<&mut Self>, ruta: &QString) {
        let ruta = ruta.to_string();
        let c = leer(|r| {
            r.cola()
                .into_iter()
                .find(|x| texto(x, "ruta") == ruta && verdadero(x.get("radio")))
        });
        let Some(c) = c else { return };
        if self.es().bajando_radio.contains(&ruta) {
            return;
        }
        self.as_mut().em().bajando_radio.push(ruta.clone());
        let nombre = titulo_y(&texto(&c, "titulo"), &texto(&c, "artista"));
        self.as_mut().bajar_con(&ruta, -1, &texto(&c, "id"), &nombre);
        self.avisar("Bajando a tu biblioteca");
    }

    // --- compartir ---------------------------------------------------------------------------------------------

    /// «copiar» el enlace de YouTube Music o mandarlo por «whatsapp». Las tuyas sin id se buscan primero.
    pub fn compartir(self: Pin<&mut Self>, c: &QVariant, como: &QString) {
        let c = de_qml(c);
        let como = como.to_string();
        let ruta = texto(&c, "ruta");
        if ruta.is_empty() {
            return;
        }
        let titulo = texto(&c, "titulo");
        let titulo = if titulo.is_empty() { tallo(&ruta) } else { titulo };
        let artista = texto(&c, "artista");
        let t = titulo_y(&titulo, &artista);
        let vid = id_o_ruta(&c);
        if !vid.is_empty() {
            self.fin_enlace(&como, &format!("https://music.youtube.com/watch?v={vid}"), &t);
            return;
        }
        lanzar(
            self.qt_thread(),
            move || {
                descargas::buscar(
                    &format!("{artista} {titulo}"),
                    "ytmusic",
                    1,
                    "yt-dlp",
                    &descargas::pedir_json,
                )
                .ok()
                .and_then(|r| r.first().map(|x| texto(x, "url")))
                .unwrap_or_default()
            },
            move |obj, url| obj.fin_enlace(&como, &url, &t),
        );
    }

    /// La playlist que estás viendo como enlace de YouTube («copiar» o «whatsapp»).
    pub fn compartir_playlist(mut self: Pin<&mut Self>, como: &QString) {
        let canciones = self.es().canciones.clone();
        let nombre = self.origen();
        let como = como.to_string();
        if canciones.is_empty() {
            return;
        }
        self.as_mut().avisar("Armando el enlace…");
        let n2 = nombre.clone();
        lanzar(
            self.qt_thread(),
            move || {
                let ids: Vec<String> = canciones
                    .iter()
                    .map(|c| {
                        let vid = id_o_ruta(c);
                        if !vid.is_empty() {
                            return vid;
                        }
                        let t = texto(c, "titulo");
                        let t = if t.is_empty() { tallo(&texto(c, "ruta")) } else { t };
                        descargas::buscar(
                            &format!("{} {t}", texto(c, "artista")),
                            "ytmusic",
                            1,
                            "yt-dlp",
                            &descargas::pedir_json,
                        )
                        .ok()
                        .and_then(|r| r.first().map(|x| texto(x, "id")))
                        .unwrap_or_default()
                    })
                    .collect();
                descargas::enlaces_playlist(&n2, &ids).join("\n")
            },
            move |obj, url| obj.fin_enlace(&como, &url, &nombre),
        );
    }

    fn fin_enlace(self: Pin<&mut Self>, como: &str, url: &str, t: &str) {
        if url.is_empty() {
            self.avisar("No encontré esta canción en YouTube Music");
        } else if como == "whatsapp" {
            abrir(&format!(
                "https://wa.me/?text={}",
                vortex_player::red::escapar_ruta(&format!("{t} {url}"))
            ));
        } else {
            crate::arranque::ffi::copiar_texto(url);
            let n = url.matches('\n').count();
            if n == 0 {
                self.avisar("Enlace copiado");
            } else {
                self.avisar(&format!("{} enlaces copiados", n + 1));
            }
        }
    }

    // --- tu mezcla del día --------------------------------------------------------------------------------------

    /// Una vez al día: lo que más escuchas (30 días) revuelto con 15 parecidas que no tienes.
    pub fn armar_mezcla(self: Pin<&mut Self>) {
        self.armar_mezcla_con(false);
    }

    pub fn armar_mezcla_con(mut self: Pin<&mut Self>, forzar: bool) {
        let hoy = hoy();
        let e = self.es();
        if e.mezcla_armando
            || (!forzar && e.mezcla.get("fecha").and_then(|f| f.as_str()) == Some(hoy.as_str()))
            || e.sin_red
        {
            return;
        }
        let mut rng = rand::rng();
        let (mut tuyas, todas) = {
            let b = self.bib();
            (
                b.mas_escuchadas(fecha::ahora() - 30.0 * 86400.0, 15)
                    .unwrap_or_default(),
                b.canciones("", Orden::Agregada).unwrap_or_default(),
            )
        };
        if tuyas.len() < 15 {
            let ya: HashSet<String> = tuyas.iter().map(|t| t.ruta.clone()).collect();
            let resto: Vec<&Cancion> = todas.iter().filter(|c| !ya.contains(&c.ruta)).collect();
            let faltan = 15 - tuyas.len();
            tuyas.extend(resto.choose_multiple(&mut rng, faltan).map(|c| (*c).clone()));
        }
        if tuyas.is_empty() {
            return;
        }
        let diez = &tuyas[..tuyas.len().min(10)];
        let semillas: Vec<Value> = diez
            .choose_multiple(&mut rng, 4.min(diez.len()))
            .map(semilla)
            .collect();
        let tienes: Vec<Value> = todas.iter().map(semilla).collect();
        let rutas_tuyas: Vec<String> = tuyas.iter().map(|c| c.ruta.clone()).collect();
        self.as_mut().em().mezcla_armando = true;
        lanzar(
            self.qt_thread(),
            move || descargas::recomendar(&semillas, &tienes, 15, &descargas::pedir_json).unwrap_or_default(),
            move |obj, nuevas| obj.fin_mezcla(hoy, rutas_tuyas, nuevas),
        );
    }

    fn fin_mezcla(mut self: Pin<&mut Self>, fecha_: String, mut tuyas: Vec<String>, nuevas: Vec<Value>) {
        self.as_mut().em().mezcla_armando = false;
        if nuevas.is_empty() {
            return; // sin internet: se intenta la próxima vez que abras la app
        }
        tuyas.shuffle(&mut rand::rng());
        let nuevas: Vec<Value> = nuevas.iter().map(|c| self.as_mut().de_la_radio(c)).collect();
        let mut canciones = Vec::new();
        for k in 0..tuyas.len().max(nuevas.len()) {
            if let Some(t) = tuyas.get(k) {
                canciones.push(json!({"ruta": t}));
            }
            if let Some(n) = nuevas.get(k) {
                canciones.push(n.clone());
            }
        }
        let mut m = Map::new();
        m.insert("fecha".into(), json!(fecha_));
        m.insert("canciones".into(), Value::Array(canciones));
        guardar_json("mezcla.json", &m);
        self.as_mut().em().mezcla = m;
        self.poner_mezcla();
    }

    pub fn canciones_mezcla(mut self: Pin<&mut Self>) -> Vec<Value> {
        let lista: Vec<Value> = self
            .es()
            .mezcla
            .get("canciones")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        let mut salida = Vec::new();
        for c in lista {
            let ruta = texto(&c, "ruta");
            if verdadero(c.get("radio")) {
                self.as_mut()
                    .em()
                    .portadas
                    .entry(ruta)
                    .or_insert_with(|| texto(&c, "portada"));
                salida.push(c);
            } else if let Some(t) = self.bib().cancion(&ruta).ok().flatten() {
                salida.push(cancion_valor(
                    &t,
                    self.es().portadas.get(&ruta).map_or("", String::as_str),
                ));
            }
        }
        salida
    }

    pub fn poner_mezcla(mut self: Pin<&mut Self>) {
        let lista = if self.es().mezcla.contains_key("canciones") {
            self.as_mut().canciones_mezcla()
        } else {
            Vec::new()
        };
        let fecha_ = self
            .es()
            .mezcla
            .get("fecha")
            .and_then(|f| f.as_str())
            .unwrap_or("")
            .to_string();
        let v = json!({
            "fecha": fecha_, "cuantas": lista.len(), "hoy": fecha_ == hoy(),
            "nuevas": lista.iter().filter(|c| verdadero(c.get("radio"))).count(),
            "portadas": lista.iter().take(4).map(|c| texto(c, "ruta")).collect::<Vec<_>>(),
            "armando": self.es().mezcla_armando,
        });
        self.set_mezcla(variante(&v));
    }

    pub fn tocar_mezcla(mut self: Pin<&mut Self>) {
        let lista = self.as_mut().canciones_mezcla();
        if !lista.is_empty() {
            con_rep(|r| {
                r.poner_origen("Tu mezcla del día");
                r.tocar(&lista, 0);
            });
        }
    }

    // --- lo nuevo de tus artistas ---------------------------------------------------------------------------------

    pub fn revisar_novedades(self: Pin<&mut Self>) {
        self.revisar_novedades_con(false);
    }

    /// Una vez al día: los álbumes y singles de este año de tus 5 artistas más escuchados.
    pub fn revisar_novedades_con(self: Pin<&mut Self>, forzar: bool) {
        let hoy = hoy();
        if (!forzar && self.es().novedades.get("fecha").and_then(|f| f.as_str()) == Some(hoy.as_str()))
            || self.es().sin_red
        {
            return;
        }
        let (artistas, claves) = {
            let b = self.bib();
            let artistas = b
                .artistas_mas_escuchados(fecha::ahora() - 60.0 * 86400.0, 5)
                .unwrap_or_default();
            let claves: HashSet<String> = b
                .canciones("", Orden::Agregada)
                .unwrap_or_default()
                .iter()
                .map(|c| descargas::clave(&c.titulo, &c.artista))
                .collect();
            (artistas, claves)
        };
        let mut ids: Map<String, Value> = self
            .es()
            .novedades
            .get("ids")
            .and_then(|i| i.as_object())
            .cloned()
            .unwrap_or_default();
        let l = fecha::local(fecha::ahora());
        let desde_anio = i64::from(if l.mes <= 2 { l.anio - 1 } else { l.anio });
        lanzar(
            self.qt_thread(),
            move || {
                let mut lista: Vec<Value> = Vec::new();
                for a in &artistas {
                    let canal = match ids.get(a).and_then(|c| c.as_str()).filter(|c| !c.is_empty()) {
                        Some(c) => c.to_string(),
                        None => descargas::artista_id(a, &descargas::pedir_json).ok()?, // sin internet: mañana
                    };
                    if canal.is_empty() {
                        continue;
                    }
                    ids.insert(a.clone(), json!(canal));
                    for x in descargas::lanzamientos(&canal, a, &descargas::pedir_json)
                        .ok()?
                        .into_iter()
                        .take(8)
                    {
                        if x["anio"].as_i64().unwrap_or(0) >= desde_anio
                            && !claves.contains(&descargas::clave(&texto(&x, "titulo"), a))
                        {
                            lista.push(x);
                        }
                    }
                }
                lista.sort_by_key(|x| -x["anio"].as_i64().unwrap_or(0));
                lista.truncate(12);
                Some((lista, ids))
            },
            |obj, r| {
                if let Some((lista, ids)) = r {
                    obj.fin_novedades(lista, ids);
                }
            },
        );
    }

    fn fin_novedades(mut self: Pin<&mut Self>, lista: Vec<Value>, ids: Map<String, Value>) {
        let mut vistos: HashSet<String> = self
            .es()
            .novedades
            .get("vistos")
            .and_then(|v| v.as_array())
            .map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let nuevas: Vec<Value> = lista
            .iter()
            .filter(|x| !vistos.contains(&texto(x, "id")))
            .cloned()
            .collect();
        vistos.extend(lista.iter().map(|x| texto(x, "id")));
        let mut vistos: Vec<String> = vistos.into_iter().collect();
        vistos.sort();
        let mut m = Map::new();
        m.insert("fecha".into(), json!(hoy()));
        m.insert("lista".into(), Value::Array(lista));
        m.insert("ids".into(), Value::Object(ids));
        m.insert("vistos".into(), json!(vistos));
        guardar_json("novedades.json", &m);
        self.as_mut().em().novedades = m;
        self.as_mut().poner_novedades();
        if let Some(x) = nuevas.first() {
            let t = if nuevas.len() == 1 {
                format!("Nuevo de {}: «{}»", texto(x, "artista"), texto(x, "titulo"))
            } else {
                format!(
                    "{} lanzamientos nuevos de tus artistas, como «{}» de {}",
                    nuevas.len(),
                    texto(x, "titulo"),
                    texto(x, "artista")
                )
            };
            self.as_mut().avisar(&t);
            if hay_programa("notify-send") && !self.es().prueba {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "Vortex Player",
                        "-i",
                        "vortex-player",
                        "Lo nuevo de tus artistas",
                        &t,
                    ])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
    }

    pub fn poner_novedades(self: Pin<&mut Self>) {
        let l = self.es().novedades.get("lista").cloned().unwrap_or(json!([]));
        self.set_novedades(variante(&l));
    }

    /// «tocar»: suena entero sin bajarlo; «bajar»: se baja entero a tu biblioteca.
    pub fn lanzamiento(self: Pin<&mut Self>, album: &QString, como: &QString) {
        let (album, como) = (album.to_string(), como.to_string());
        let a2 = album.clone();
        lanzar(
            self.qt_thread(),
            move || descargas::canciones_de_lanzamiento(&a2, &descargas::pedir_json).unwrap_or_default(),
            move |obj, (lista, canciones)| obj.fin_lanzamiento(&como, &album, &lista, canciones),
        );
    }

    fn fin_lanzamiento(
        mut self: Pin<&mut Self>,
        como: &str,
        album: &str,
        lista: &str,
        canciones: Vec<Value>,
    ) {
        let x = self
            .es()
            .novedades
            .get("lista")
            .and_then(|l| l.as_array())
            .and_then(|l| l.iter().find(|x| texto(x, "id") == album).cloned())
            .unwrap_or(json!({}));
        let Some(primera) = canciones.first() else {
            self.avisar("Sin conexión con YouTube Music");
            return;
        };
        if como == "bajar" {
            let varias = canciones.len() > 1;
            let url = if !lista.is_empty() && varias {
                format!("https://music.youtube.com/playlist?list={lista}")
            } else {
                format!("https://music.youtube.com/watch?v={}", texto(primera, "id"))
            };
            let vid = if varias {
                String::new()
            } else {
                texto(primera, "id")
            };
            let nombre = format!("{} · {}", texto(&x, "titulo"), texto(&x, "artista"));
            self.bajar_con(&url, -1, &vid, &nombre);
            return;
        }
        let cola: Vec<Value> = canciones
            .iter()
            .map(|c| {
                let id = texto(c, "id");
                self.as_mut().de_la_radio(&json!({
                    "url": format!("https://music.youtube.com/watch?v={id}"), "id": id, "titulo": texto(c, "titulo"),
                    "artistas": texto(&x, "artista"), "album": texto(&x, "titulo"), "portada": texto(&x, "portada"),
                }))
            })
            .collect();
        let t = texto(&x, "titulo");
        let origen = if t.is_empty() {
            "Lo nuevo de tus artistas".to_string()
        } else {
            t
        };
        con_rep(|r| {
            r.poner_origen(&origen);
            r.tocar(&cola, 0);
        });
    }

    // --- la letra ------------------------------------------------------------------------------------------------

    pub fn buscar_letra(mut self: Pin<&mut Self>, c: &Value) {
        let ruta = texto(c, "ruta");
        let vuelta = {
            let e = self.as_mut().em();
            e.letra_vuelta += 1;
            e.letra_ruta = ruta.clone();
            e.letra_lineas.clear();
            e.letra_sincronizada = false;
            e.letra_vuelta
        };
        self.as_mut().poner_letra(!ruta.is_empty(), "");
        if ruta.is_empty() {
            return;
        }
        let t = texto(c, "titulo");
        let titulo = if t.is_empty() { tallo(&ruta) } else { t };
        let artista = texto(c, "artista");
        let dur = numero(c, "duracion") as i64;
        lanzar(
            self.qt_thread(),
            move || {
                letras::de(
                    &rutas::cache().join("letras"),
                    &titulo,
                    &artista,
                    dur,
                    &id_de(&ruta),
                    Some(&letra_youtube),
                )
            },
            move |mut obj, r: Result<Option<Letra>, String>| {
                if vuelta != obj.es().letra_vuelta {
                    return;
                }
                let error = match r {
                    Ok(Some(l)) => {
                        let e = obj.as_mut().em();
                        e.letra_lineas = l.lineas;
                        e.letra_sincronizada = l.sincronizada;
                        ""
                    }
                    Ok(None) => "",
                    Err(_) => "Sin conexión: no pude buscar la letra",
                };
                obj.poner_letra(false, error);
            },
        );
    }

    fn poner_letra(self: Pin<&mut Self>, buscando: bool, error: &str) {
        let e = self.es();
        let v = json!({"ruta": e.letra_ruta, "lineas": e.letra_lineas, "sincronizada": e.letra_sincronizada,
                       "buscando": buscando, "error": error});
        self.set_letra(variante(&v));
    }

    pub fn linea_actual(&self, segundos: f64) -> i32 {
        let e = self.es();
        if !e.letra_sincronizada {
            return -1;
        }
        i32::try_from(letras::actual(&e.letra_lineas, (segundos * 1000.0) as i64)).unwrap_or(-1)
    }

    /// Busca (despacito, en un hilo) la letra de cada canción que todavía no tiene; se reanuda sola.
    pub fn bajar_letras(mut self: Pin<&mut Self>) {
        if verdadero(self.es().letras_estado.get("trabajando")) {
            return;
        }
        let todas = self.bib().canciones("", Orden::Agregada).unwrap_or_default();
        self.as_mut().em().letras_estado["trabajando"] = json!(true);
        let hilo = self.qt_thread();
        let vivo = self.es().vivo.clone();
        let sin_red = self.es().sin_red;
        std::thread::spawn(move || {
            let carpeta = rutas::cache().join("letras");
            let total = todas.len();
            let avisar = |con: usize, trabajando: bool| {
                let v = json!({"con": con, "total": total, "trabajando": trabajando});
                let _ = hilo.queue(move |o| o.en_letras(v));
            };
            let mut con = 0;
            let mut faltan = Vec::new();
            for c in &todas {
                let titulo = if c.titulo.is_empty() {
                    tallo(&c.ruta)
                } else {
                    c.titulo.clone()
                };
                let dur = c.duracion as i64;
                if letras::guardada(&carpeta, &titulo, &c.artista, dur) {
                    con += 1;
                } else {
                    faltan.push((titulo, c.artista.clone(), dur, id_de(&c.ruta)));
                }
            }
            avisar(con, !faltan.is_empty() && !sin_red);
            if !sin_red {
                for (titulo, artista, dur, vid) in faltan {
                    if !vivo.load(Ordering::Relaxed) {
                        break;
                    }
                    if letras::de(&carpeta, &titulo, &artista, dur, &vid, Some(&letra_youtube)).is_err() {
                        break; // sin internet (o LRCLIB caído): otro día
                    }
                    con += 1;
                    avisar(con, true);
                    std::thread::sleep(std::time::Duration::from_millis(1500)); // sin prisa: LRCLIB es de una persona
                }
            }
            avisar(con, false);
        });
    }

    pub fn en_letras(mut self: Pin<&mut Self>, v: Value) {
        self.as_mut().em().letras_estado = v.clone();
        self.set_letras_guardadas(variante(&v));
    }

    /// Lo que se borró en el otro aparato (lo llama la sincronización, ya con el candado).
    pub fn borrar_de_nube(b: &Biblioteca, ruta: &str) {
        a_la_papelera(ruta);
        let _ = b.quitar(ruta);
    }
}
