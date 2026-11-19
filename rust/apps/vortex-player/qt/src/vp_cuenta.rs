//! Lo de `vp` con tu cuenta de Google y tu sesión: la biblioteca en la nube (la misma que en el teléfono),
//! seguir donde te quedaste, seguir en el otro aparato y «Tu resumen» de la semana.

use crate::motor::{con_rep, leer};
use crate::puente::qobject::Puente;
use crate::puente::{abrir, cancion_valor, guardar_ajuste, guardar_json, id_o_ruta, lanzar, numero, texto, variante};
use core::pin::Pin;
use cxx_qt::Threading;
use serde_json::{Value, json};
use std::time::Duration;
use vortex_player::biblioteca::{Biblioteca, id_de};
use vortex_player::cuenta_google::{ARCHIVO, Carpeta, ErrorGoogle};
use vortex_player::{fecha, sincronia};

/// Lo que dice Drive cuando la sesión ya no sirve (sincronia lo pasa a texto).
const SIN_SESION: &str = "Google ya no reconoce la sesión";

impl Puente {
    // --- la nube ---------------------------------------------------------------------------------------------

    pub fn cambio_local(self: Pin<&mut Self>) {
        if !self.nube_valor("correo").as_str().unwrap_or("").is_empty() {
            self.esperar("nube", 15000, |o| o.sincronizar());
        }
    }

    fn nube_valor(&self, clave: &str) -> Value {
        self.es().nube.get(clave).cloned().unwrap_or(Value::Null)
    }

    pub fn iniciar_sesion_google(mut self: Pin<&mut Self>) {
        if self.nube_valor("trabajando") == json!(true) {
            return;
        }
        self.as_mut().poner_nube(json!({"trabajando": true, "texto": "Elige tu cuenta en el navegador…", "error": ""}));
        let cuenta = self.es().cuenta.clone();
        lanzar(
            self.qt_thread(),
            move || cuenta.iniciar_sesion(&|url| abrir(url), Duration::from_secs(300)),
            |obj, r| match r {
                Ok(d) => obj.fin_nube(json!({"sesion": true, "correo": texto(&d, "correo"), "nombre": texto(&d, "nombre")})),
                Err(e) => obj.fin_nube(json!({"sesion": true, "error": e.to_string()})),
            },
        );
    }

    pub fn cerrar_sesion_google(mut self: Pin<&mut Self>) {
        let cuenta = self.es().cuenta.clone();
        std::thread::spawn(move || cuenta.cerrar_sesion());
        self.as_mut().poner_nube(json!({"correo": "", "nombre": "", "texto": "", "ultima": 0.0, "error": ""}));
        self.avisar("Sesión cerrada. Tus canciones se quedan en esta PC.");
    }

    pub fn sincronizar(mut self: Pin<&mut Self>) {
        if self.nube_valor("correo").as_str().unwrap_or("").is_empty() {
            return;
        }
        if self.nube_valor("trabajando") == json!(true) {
            self.as_mut().em().nube_otra_vez = true;
            return;
        }
        self.as_mut().poner_nube(json!({"trabajando": true, "texto": "Sincronizando con la nube…", "error": ""}));
        let (cuenta, bib, hilo) = (self.es().cuenta.clone(), self.es().bib.clone(), self.qt_thread());
        lanzar(
            self.qt_thread(),
            move || {
                // La borraste en el teléfono (ya se tiene el candado)
                let mut borrar = |b: &Biblioteca, _vid: &str, ruta: &str| {
                    Puente::borrar_de_nube(b, ruta);
                    let ruta = ruta.to_string();
                    let _ = hilo.queue(move |_| con_rep(|r| r.quitar_de_cola(&ruta)));
                };
                for intento in 0..2 {
                    let token = match cuenta.token() {
                        Ok(t) => t,
                        Err(ErrorGoogle::SinPermiso(e)) => return json!({"ok": false, "error": e, "salir": true}),
                        Err(e) => return json!({"ok": false, "error": format!("No se pudo sincronizar: {e}")}),
                    };
                    let drive = cuenta.drive(&token, ARCHIVO);
                    match sincronia::sincronizar(&bib, &drive, &mut borrar) {
                        Ok((_, plan)) => {
                            let bajar: Vec<Value> = plan
                                .bajar
                                .iter()
                                .map(|(vid, c)| {
                                    let nombre = [c.titulo.as_str(), c.artista.as_str()]
                                        .iter()
                                        .filter(|x| !x.is_empty())
                                        .copied()
                                        .collect::<Vec<_>>()
                                        .join(" · ");
                                    json!({"id": vid, "nombre": if nombre.is_empty() { vid.clone() } else { nombre }})
                                })
                                .collect();
                            return json!({"ok": true, "bajar": bajar, "borradas": plan.borrar.len()});
                        }
                        Err(e) if e == SIN_SESION => {
                            cuenta.olvidar_token();
                            if intento == 1 {
                                return json!({"ok": false, "error": e, "salir": true});
                            }
                        }
                        Err(e) => return json!({"ok": false, "error": format!("No se pudo sincronizar: {e}")}),
                    }
                }
                json!({"ok": false, "error": SIN_SESION, "salir": true})
            },
            |obj, r| obj.fin_nube(r),
        );
    }

    fn fin_nube(mut self: Pin<&mut Self>, r: Value) {
        if r["sesion"] == json!(true) {
            // terminó «Iniciar sesión»
            let error = texto(&r, "error");
            if !error.is_empty() {
                self.as_mut().poner_nube(json!({"trabajando": false, "texto": "", "error": error}));
                return;
            }
            let correo = texto(&r, "correo");
            self.as_mut()
                .poner_nube(json!({"trabajando": false, "correo": correo, "nombre": texto(&r, "nombre")}));
            self.as_mut().avisar(&format!("Listo: tu música se sincroniza con {correo}"));
            self.sincronizar();
            return;
        }
        if r["ok"] != json!(true) {
            self.as_mut().poner_nube(json!({"trabajando": false, "texto": "", "error": texto(&r, "error")}));
            if r["salir"] == json!(true) {
                self.es().cuenta.cerrar_sesion();
                self.as_mut().poner_nube(json!({"correo": "", "nombre": ""}));
            }
            return;
        }
        let en_cola: std::collections::HashSet<String> = self
            .es()
            .descargas
            .iter()
            .filter(|d| matches!(d["estado"].as_str(), Some("espera" | "bajando")))
            .map(|d| texto(d, "id"))
            .collect();
        let nuevas: Vec<Value> = r["bajar"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|c| {
                let id = texto(c, "id");
                !en_cola.contains(&id) && !self.es().nube_fallidas.contains(&id)
            })
            .collect();
        for c in &nuevas {
            let id = texto(c, "id");
            self.as_mut().em().nube_pendientes.insert(id.clone());
            self.as_mut().bajar_con(&format!("https://music.youtube.com/watch?v={id}"), -1, &id, &texto(c, "nombre"));
        }
        let fallidas = self.es().nube_fallidas.len();
        let t = if !nuevas.is_empty() {
            format!("Bajando {} canciones de tu otro dispositivo", nuevas.len())
        } else if fallidas > 0 {
            format!("No se pudieron bajar {fallidas}; se reintentan al volver a abrir")
        } else {
            String::new()
        };
        self.as_mut().poner_nube(json!({"trabajando": false, "ultima": fecha::ahora(), "texto": t, "error": ""}));
        self.as_mut().recargar();
        if std::mem::take(&mut self.as_mut().em().nube_otra_vez) {
            self.esperar("nube", 15000, |o| o.sincronizar());
        }
    }

    // --- seguir donde te quedaste ------------------------------------------------------------------------------

    pub fn guardar_sesion(mut self: Pin<&mut Self>) {
        if !self.es().restaurada {
            return;
        }
        let (ruta, sesion) = leer(|r| (texto(&r.cancion(), "ruta"), r.sesion()));
        if !ruta.is_empty() {
            self.as_mut().em().sesion.insert("cola".into(), sesion);
        }
        guardar_json("sesion.json", &self.es().sesion);
    }

    /// Al abrir: la última canción, en pausa, en el segundo donde ibas, con su cola.
    pub fn restaurar(mut self: Pin<&mut Self>) {
        let s = self.es().sesion.get("cola").cloned().unwrap_or(json!({}));
        let rutas_: Vec<String> = s["rutas"].as_array().map(|l| l.iter().filter_map(|r| r.as_str().map(String::from)).collect()).unwrap_or_default();
        if texto(&leer(|r| r.cancion()), "ruta").is_empty() && !rutas_.is_empty() {
            let cola: Vec<Value> = {
                let b = self.bib();
                rutas_
                    .iter()
                    .filter_map(|r| b.cancion(r).ok().flatten())
                    .map(|c| cancion_valor(&c, self.es().portadas.get(&c.ruta).map_or("", String::as_str)))
                    .collect()
            };
            con_rep(|r| r.restaurar(&cola, &s));
        }
        self.as_mut().em().restaurada = true;
        self.cambio_cancion();
    }

    /// Las últimas 25 que escuchaste (sin repetir), para «Para ti».
    fn anotar_escuchada(mut self: Pin<&mut Self>, c: &Value) {
        let nueva = json!({"id": id_de(&texto(c, "ruta")), "titulo": texto(c, "titulo"), "artista": texto(c, "artista")});
        let clave = |x: &Value| (texto(x, "titulo"), texto(x, "artista"));
        let mut h: Vec<Value> = self
            .es()
            .sesion
            .get("historial")
            .and_then(|h| h.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|x| clave(x) != clave(&nueva))
            .collect();
        h.insert(0, nueva);
        h.truncate(25);
        self.as_mut().em().sesion.insert("historial".into(), Value::Array(h));
    }

    pub fn cambio_cancion(mut self: Pin<&mut Self>) {
        let (c, posicion) = leer(|r| (r.cancion(), numero(&r.sesion(), "posicion")));
        let ruta = texto(&c, "ruta");
        if ruta == self.es().letra_ruta || !self.es().restaurada {
            return;
        }
        if !ruta.is_empty() && posicion < 1.0 {
            // lo restaurado no cuenta como «escuchada» otra vez
            self.as_mut().anotar_escuchada(&c);
        }
        self.as_mut().guardar_sesion();
        self.buscar_letra(&c);
    }

    // --- seguir en el otro aparato (lo que suena va a la nube) ------------------------------------------------------

    fn aparato(&self) -> String {
        let a = Biblioteca::ajustes().texto("aparato");
        if !a.is_empty() {
            return a;
        }
        let hex: String = (0..12).map(|_| format!("{:x}", rand::random_range(0..16u8))).collect();
        let a = format!("pc-{hex}");
        guardar_ajuste("aparato", json!(a));
        a
    }

    /// Lo que suena y en qué segundo, para que el cel ofrezca seguir ahí. Solo las que tienen id de YouTube.
    /// Con `esperar` > 0 espera (hasta esos segundos) a que se suba: al salir.
    pub fn subir_sonando(mut self: Pin<&mut Self>, esperar: u64) {
        let (c, posicion, pausado) = leer(|r| (r.cancion(), r.posicion(), r.pausado()));
        let vid = id_o_ruta(&c);
        if self.nube_valor("correo").as_str().unwrap_or("").is_empty() || vid.is_empty() || self.es().sin_red {
            return;
        }
        let clave = (vid.clone(), (posicion as i64) / 10, pausado);
        if self.es().sonando_ultimo.as_ref() == Some(&clave) {
            return;
        }
        let cuando = (fecha::ahora() * 1000.0) as i64;
        {
            let e = self.as_mut().em();
            e.sonando_ultimo = Some(clave);
            e.sonando_subido = cuando;
        }
        let p = texto(&c, "portada");
        let portada = if p.is_empty() { self.es().portadas.get(&texto(&c, "ruta")).cloned().unwrap_or_default() } else { p };
        let datos = json!({
            "aparato": self.aparato(), "nombre": "tu PC", "id": vid, "titulo": texto(&c, "titulo"),
            "artista": texto(&c, "artista"), "album": texto(&c, "album"), "duracion": numero(&c, "duracion"),
            "portada": if portada.starts_with("http") { portada } else { String::new() },
            "posicion": (posicion * 10.0).round() / 10.0, "cuando": cuando,
        });
        let cuenta = self.es().cuenta.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // sin internet: ya se subirá la próxima
            if let Ok(token) = cuenta.token() {
                let d = cuenta.drive(&token, "sonando.json");
                let t = datos.to_string();
                match d.buscar() {
                    Ok(Some(fid)) => {
                        let _ = d.escribir(&fid, &t);
                    }
                    Ok(None) => {
                        let _ = d.crear(&t);
                    }
                    Err(_) => {}
                }
            }
            let _ = tx.send(());
        });
        if esperar > 0 {
            let _ = rx.recv_timeout(Duration::from_secs(esperar));
        }
    }

    pub fn al_salir(mut self: Pin<&mut Self>) {
        if leer(|r| r.posicion()) > 3.0 {
            self.as_mut().em().sonando_ultimo = None;
            self.subir_sonando(4);
        }
    }

    /// Al abrir la app, al volver a la ventana y cada 5 min: ¿sonaba algo en el cel?
    pub fn revisar_otro_aparato(mut self: Pin<&mut Self>) {
        let ahora = vortex_player::reproductor::monotonico();
        if self.nube_valor("correo").as_str().unwrap_or("").is_empty()
            || ahora - self.es().revisado_otro < 45.0
            || self.es().sin_red
        {
            return;
        }
        self.as_mut().em().revisado_otro = ahora;
        let cuenta = self.es().cuenta.clone();
        lanzar(
            self.qt_thread(),
            move || {
                let token = cuenta.token().ok()?;
                let d = cuenta.drive(&token, "sonando.json");
                match d.buscar().ok()? {
                    Some(fid) => serde_json::from_slice::<Value>(&d.leer(&fid).ok()?).ok(),
                    None => Some(json!({})),
                }
            },
            |obj, d| {
                if let Some(d) = d {
                    obj.fin_otro_aparato(d);
                }
            },
        );
    }

    fn fin_otro_aparato(mut self: Pin<&mut Self>, d: Value) {
        let ahora = fecha::ahora() * 1000.0;
        let cuando = numero(&d, "cuando");
        let visto = Biblioteca::ajustes().numero("seguir_visto", 0.0);
        if texto(&d, "id").is_empty()
            || texto(&d, "aparato") == self.aparato()
            || ahora - cuando > 12.0 * 3600.0 * 1000.0
            || cuando <= (self.es().sonando_subido as f64).max(visto)
        {
            return;
        }
        let (c, posicion) = leer(|r| (r.cancion(), r.posicion()));
        if id_o_ruta(&c) == texto(&d, "id") && (posicion - numero(&d, "posicion")).abs() < 30.0 {
            return;
        }
        self.as_mut().em().seguir = d.as_object().cloned().unwrap_or_default();
        self.poner_seguir();
    }

    pub fn seguir_aqui(mut self: Pin<&mut Self>) {
        let d = Value::Object(std::mem::take(&mut self.as_mut().em().seguir));
        self.as_mut().poner_seguir();
        if d.as_object().is_none_or(|m| m.is_empty()) {
            return;
        }
        guardar_ajuste("seguir_visto", d.get("cuando").cloned().unwrap_or(json!(0)));
        let id = texto(&d, "id");
        let c = {
            let b = self.bib();
            b.rutas_por_id()
                .ok()
                .and_then(|m| m.get(&id).cloned())
                .and_then(|r| b.cancion(&r).ok().flatten())
                .map(|c| cancion_valor(&c, self.es().portadas.get(&c.ruta).map_or("", String::as_str)))
        };
        // no la tienes aquí: suena de YouTube Music, como las de la radio
        let c = c.unwrap_or_else(|| {
            self.as_mut().de_la_radio(&json!({
                "url": format!("https://music.youtube.com/watch?v={id}"), "id": id, "titulo": texto(&d, "titulo"),
                "artistas": texto(&d, "artista"), "album": texto(&d, "album"), "duracion": numero(&d, "duracion"),
                "portada": texto(&d, "portada"),
            }))
        });
        let nombre = texto(&d, "nombre");
        let origen = format!("Desde {}", if nombre.is_empty() { "tu cel" } else { &nombre });
        let posicion = numero(&d, "posicion");
        con_rep(|r| {
            r.poner_origen(&origen);
            r.tocar_desde(std::slice::from_ref(&c), 0, posicion);
        });
    }

    pub fn descartar_seguir(mut self: Pin<&mut Self>) {
        if let Some(c) = self.es().seguir.get("cuando") {
            guardar_ajuste("seguir_visto", c.clone());
        }
        self.as_mut().em().seguir.clear();
        self.poner_seguir();
    }

    pub fn poner_seguir(self: Pin<&mut Self>) {
        let d = &self.es().seguir;
        let v = if d.is_empty() {
            json!({})
        } else {
            let mut m = d.clone();
            let minutos = ((fecha::ahora() * 1000.0 - numero(&Value::Object(d.clone()), "cuando")) / 60000.0) as i64;
            let hace = if minutos < 1 {
                "hace un momento".to_string()
            } else if minutos < 60 {
                format!("hace {minutos} min")
            } else {
                format!("hace {} h", minutos / 60)
            };
            m.insert("hace".into(), json!(hace));
            Value::Object(m)
        };
        self.set_seguir(variante(&v));
    }

    // --- «Tu resumen» de la semana ------------------------------------------------------------------------------

    pub fn poner_resumen(mut self: Pin<&mut Self>) {
        let esta = fecha::semana(0, None);
        if self.es().resumen.pointer("/semana/desde").and_then(Value::as_f64) != Some(esta.0) {
            // cambió la semana (o hubo una escucha): se vuelve a sacar
            let (semana, pasada) = {
                let b = self.bib();
                let (d1, h1) = fecha::semana(1, None);
                (b.resumen(esta.0, esta.1, 5).unwrap_or(json!({})), b.resumen(d1, h1, 5).unwrap_or(json!({})))
            };
            self.as_mut().em().resumen = json!({"semana": semana, "pasada": pasada});
        }
        let visto = Biblioteca::ajustes().numero("resumen_visto", 0.0);
        let mut r = self.es().resumen.clone();
        // el aviso sale una vez por semana, el lunes que abres la app, si la pasada escuchaste algo
        let escuchas = r.pointer("/pasada/escuchas").and_then(Value::as_f64).unwrap_or(0.0);
        r["aviso"] = json!(escuchas > 0.0 && visto < esta.0);
        self.set_resumen(variante(&r));
    }

    pub fn resumen_visto(self: Pin<&mut Self>) {
        guardar_ajuste("resumen_visto", json!(fecha::semana(0, None).0));
        self.poner_resumen();
    }
}
