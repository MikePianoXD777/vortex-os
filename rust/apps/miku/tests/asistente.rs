//! El asistente de punta a punta, sin pantalla ni sonido: una IA de mentira, una voz de mentira (un guion
//! de Python con el mismo protocolo), un «altavoz» que es una carpeta y el socket de permisos de verdad.
//! Nada toca la cuenta, el micrófono ni la cámara de nadie.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_miku::asistente::{Arranque, Asistente, Entorno, Senal};
use vortex_miku::companera::Companera;
use vortex_miku::config::Config;
use vortex_miku::corazon::Corazon;
use vortex_miku::eventos::{AlEvento, Cerebro, Evento};
use vortex_miku::fecha::FechaHora;
use vortex_miku::ia::Ia;
use vortex_miku::oido::Oido;
use vortex_miku::rutas::Rutas;

/// La voz de mentira: contesta «listo» y, por cada frase, un WAV de verdad (vacío) en la carpeta indicada.
const VOZ_FALSA: &str = r#"
import json, sys
carpeta = sys.argv[1]
print(json.dumps({"listo": True, "voz": "supertonic-f1"}), flush=True)
n = 0
for l in sys.stdin:
    p = json.loads(l)
    if p.get("cmd") == "salir":
        break
    n += 1
    if "FALLA" in p["texto"]:
        print(json.dumps({"id": p["id"], "error": "no pude con esa frase"}), flush=True)
        continue
    ruta = f"{carpeta}/voz-{n}.wav"
    open(ruta, "wb").write(b"RIFF....WAVE")
    print(json.dumps({"id": p["id"], "wav": ruta, "seg": 0.4, "visemas": [[0, 0, 0, 0, 0], [50, 0, 0, 0, 0]]}), flush=True)
"#;

struct IaFalsa {
    al_evento: AlEvento,
    guion: Vec<Evento>,
    /// La pregunta no termina hasta que se suelte (para probar lo que pasa «mientras piensa»).
    retener: Arc<AtomicBool>,
    preguntas: Arc<Mutex<Vec<String>>>,
    estado: (String, String),
    ocupado: Arc<AtomicBool>,
    cancelaciones: Arc<Mutex<u32>>,
    olvidos: Arc<Mutex<u32>>,
}

impl Cerebro for IaFalsa {
    fn comprobar(&self) -> (String, String) {
        self.estado.clone()
    }
    fn preguntar(&self, texto: &str, etiqueta: u64) {
        self.preguntas.lock().unwrap().push(texto.into());
        for e in &self.guion {
            (self.al_evento)(etiqueta, e.clone());
        }
        let hasta = Instant::now() + Duration::from_secs(10);
        while self.retener.load(Ordering::SeqCst) && Instant::now() < hasta {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn cancelar(&self) {
        *self.cancelaciones.lock().unwrap() += 1;
    }
    fn olvidar(&self) {
        *self.olvidos.lock().unwrap() += 1;
    }
    fn poner_sentimientos(&self, _: &str) {}
    fn poner_autorizar(&self, _: &str) {}
    fn poner_internet(&self, _: bool) {}
    fn ocupado(&self) -> Arc<AtomicBool> {
        self.ocupado.clone()
    }
    fn agregar_persona(&self, _: &str) {}
}

struct Banco {
    dir: tempfile::TempDir,
    rutas: Rutas,
    senales: Arc<Mutex<Vec<Senal>>>,
    preguntas: Arc<Mutex<Vec<String>>>,
    retener: Arc<AtomicBool>,
    olvidos: Arc<Mutex<u32>>,
    asistente: Arc<Asistente>,
    companera: Arc<Companera>,
}

struct Opciones {
    cfg: Config,
    guion: Vec<Evento>,
    estado: (String, String),
    arranque: Arranque,
    retener: bool,
    voz: bool,
    ahora: &'static str,
    /// Qué tan rápido «suena» cada frase (50 = casi al instante, 0.005 = ~0,8 s).
    velocidad: f64,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            cfg: Config {
                seguir_escuchando: false,
                ..Config::default()
            },
            guion: vec![],
            estado: ("ok".into(), String::new()),
            arranque: Arranque::default(),
            retener: false,
            voz: true,
            ahora: "2026-11-19T15:00",
            velocidad: 50.0,
        }
    }
}

impl Banco {
    fn nuevo(o: Opciones) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let rutas = Rutas::en(dir.path());
        std::fs::create_dir_all(&rutas.config_dir).unwrap();
        // El archivo de configuración que lee el asistente cuando consulta el disco.
        let mut m = serde_json::Map::new();
        for (k, v) in [
            ("camara", json!(o.cfg.camara)),
            ("camara_aviso", json!(o.cfg.camara_aviso)),
            ("companera", json!(o.cfg.companera)),
            ("companera_consentimiento", json!(o.cfg.companera_consentimiento)),
            ("companera_tono", json!(o.cfg.companera_tono)),
        ] {
            m.insert(k.into(), v);
        }
        Config::guardar(&rutas.config, &m).unwrap();
        let guion = dir.path().join("voz.py");
        std::fs::write(&guion, VOZ_FALSA).unwrap();
        let senales = Arc::new(Mutex::new(vec![]));
        let (preguntas, retener, olvidos, cancel) = (
            Arc::new(Mutex::new(vec![])),
            Arc::new(AtomicBool::new(o.retener)),
            Arc::new(Mutex::new(0u32)),
            Arc::new(Mutex::new(0u32)),
        );
        let fabrica: vortex_miku::asistente::FabricaCerebro = {
            let (guion_ia, retener, preguntas, estado, olvidos) = (
                o.guion.clone(),
                retener.clone(),
                preguntas.clone(),
                o.estado.clone(),
                olvidos.clone(),
            );
            Arc::new(
                move |_c: &Config, _a: bool, al_evento: AlEvento| -> Arc<dyn Cerebro> {
                    Arc::new(IaFalsa {
                        al_evento,
                        guion: guion_ia.clone(),
                        retener: retener.clone(),
                        preguntas: preguntas.clone(),
                        estado: estado.clone(),
                        ocupado: Arc::new(AtomicBool::new(false)),
                        cancelaciones: cancel.clone(),
                        olvidos: olvidos.clone(),
                    })
                },
            )
        };
        let mut corazon = Corazon::de_prueba(dir.path());
        corazon.ahora_fija = FechaHora::leer(o.ahora);
        let mut companera = Companera::de_prueba(dir.path());
        companera.ahora_fija = FechaHora::leer(o.ahora);
        let companera = Arc::new(companera);
        let mut oido = Oido::nuevo(1.0, 2.0);
        oido.fuente = Some(vec!["true".into()]); // sin micrófono: la fuente se acaba y «nadie habló»
        let mut ia = Ia::nueva(rutas.clone());
        ia.llavero = false;
        let env = Entorno {
            rutas: rutas.clone(),
            ia: Arc::new(ia),
            corazon: Arc::new(corazon),
            companera: companera.clone(),
            fabrica,
            factor_tiempo: 0.01,
            salida_prueba: Some((dir.path().join("altavoz"), o.velocidad)),
            // Sin voz: una que no carga (como sin modelo). No se deja el comando de verdad: en un equipo con
            // miku-voz y el modelo bajados, la voz sí cargaría.
            comando_voz: Some(if o.voz {
                (
                    which("python3"),
                    vec![
                        "-I".into(),
                        guion.to_string_lossy().into(),
                        rutas.temporales().to_string_lossy().into(),
                    ],
                    vec![],
                )
            } else {
                (
                    which("sh"),
                    vec![
                        "-c".into(),
                        r#"echo '{"error": "La voz no está instalada."}'"#.into(),
                    ],
                    vec![],
                )
            }),
            reproductor: None,
            oido: Some(Arc::new(oido)),
            transcribir: Some(Arc::new(|_| Ok("hola".into()))),
            sin_descargas: true,
            natural_auto: Some("0".into()),
            cli: None,
            notificar: false,
        };
        let s = senales.clone();
        let asistente = Asistente::nuevo(
            o.cfg,
            true,
            env,
            o.arranque,
            Arc::new(move |x| s.lock().unwrap().push(x)),
        );
        Self {
            dir,
            rutas,
            senales,
            preguntas,
            retener,
            olvidos,
            asistente,
            companera,
        }
    }

    fn esperar(&self, f: impl Fn(&[Senal]) -> bool, seg: u64) -> bool {
        let hasta = Instant::now() + Duration::from_secs(seg);
        while Instant::now() < hasta {
            if f(&self.senales.lock().unwrap()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn vistas(&self) -> Vec<Senal> {
        self.senales.lock().unwrap().clone()
    }

    fn socket(&self) -> PathBuf {
        self.rutas.temporales().join("autorizar.sock")
    }

    /// Una acción pide permiso: abre la conexión, manda el pedido y devuelve el lector para la respuesta.
    fn pedir(&self, pedido: Value) -> (UnixStream, BufReader<UnixStream>) {
        let mut s = UnixStream::connect(self.socket()).expect("el socket de permisos");
        writeln!(s, "{pedido}").unwrap();
        let r = BufReader::new(s.try_clone().unwrap());
        (s, r)
    }
}

fn which(p: &str) -> PathBuf {
    vortex_miku::sistema::cual(p).unwrap()
}

fn respuesta(r: &mut BufReader<UnixStream>) -> Value {
    r.get_mut()
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut l = String::new();
    r.read_line(&mut l).unwrap();
    serde_json::from_str(l.trim()).unwrap_or_else(|_| panic!("respuesta rara: {l:?}"))
}

fn hay_estado(v: &[Senal], e: &str) -> bool {
    v.iter().any(|s| matches!(s, Senal::Estado(x, _) if x == e))
}

fn texto_final(v: &[Senal]) -> Option<String> {
    v.iter().rev().find_map(|s| {
        if let Senal::Respuesta(t, true) = s {
            Some(t.clone())
        } else {
            None
        }
    })
}

fn hay_aviso(v: &[Senal], parte: &str) -> bool {
    v.iter()
        .any(|s| matches!(s, Senal::Aviso(t) if t.contains(parte)))
}

fn botones_ultimos(v: &[Senal]) -> Vec<String> {
    v.iter()
        .rev()
        .find_map(|s| {
            if let Senal::Acciones(b) = s {
                Some(b.iter().map(|(i, _)| i.clone()).collect())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn guion_hola() -> Vec<Evento> {
    vec![
        Evento::Inicio,
        Evento::Texto("Hola. ".into()),
        Evento::Texto("Soy Miku, ¡qué gusto!".into()),
        Evento::Listo("Hola. Soy Miku, ¡qué gusto!".into()),
    ]
}

fn carpeta_altavoz(b: &Banco) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(b.dir.path().join("altavoz"))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

// ---- Los casos ---------------------------------------------------------------------------------------

#[test]
fn una_pregunta_sale_de_punta_a_punta() {
    let b = Banco::nuevo(Opciones {
        guion: guion_hola(),
        ..Default::default()
    });
    b.asistente.iniciar();
    b.asistente.enviar("  ¿Cómo estás?  ");
    assert!(
        b.esperar(
            |v| texto_final(v).is_some_and(|t| t.starts_with("Hola. Soy Miku")),
            10
        ),
        "{:?}",
        b.vistas()
    );
    // Las dos frases pasaron por la voz y sonaron, con la boca moviéndose y la cara cambiando.
    assert!(
        b.esperar(
            |v| v.iter().filter(|s| matches!(s, Senal::BocaFin)).count() >= 2,
            10
        ),
        "{:?}",
        b.vistas()
    );
    let v = b.vistas();
    assert_eq!(b.preguntas.lock().unwrap().as_slice(), ["¿Cómo estás?"]);
    assert!(
        v.iter()
            .any(|s| matches!(s, Senal::Usuario(t) if t == "¿Cómo estás?"))
    );
    assert!(v.iter().any(|s| matches!(s, Senal::Boca(p, 30) if p.len() == 2)));
    assert!(
        v.iter()
            .any(|s| matches!(s, Senal::Emocion(e, _) if e == "happy")),
        "el «¡qué gusto!» la alegra: {v:?}"
    );
    assert!(hay_estado(&v, "pensando") && hay_estado(&v, "hablando"));
    assert!(
        v.iter().any(|s| matches!(s, Senal::Afecto(a, _) if *a > 0.0)),
        "sus sentimientos se ven"
    );
    assert_eq!(carpeta_altavoz(&b).len(), 2, "cada frase sonó una vez");
    // Los WAV temporales se borran al sonar.
    assert!(b.esperar(
        |_| {
            std::fs::read_dir(b.rutas.temporales())
                .map(|d| {
                    d.flatten()
                        .all(|e| !e.file_name().to_string_lossy().starts_with("voz-"))
                })
                .unwrap_or(true)
        },
        5
    ));
    b.asistente.cerrar_todo();
    assert_eq!(
        *b.olvidos.lock().unwrap(),
        1,
        "al cerrar, la IA olvida la conversación"
    );
}

#[test]
fn callarla_descarta_lo_que_falta() {
    let b = Banco::nuevo(Opciones {
        guion: vec![
            Evento::Texto("Una. Dos. Tres. Cuatro. Cinco.".into()),
            Evento::Listo("x".into()),
        ],
        velocidad: 0.005,
        ..Default::default()
    });
    b.asistente.iniciar();
    b.asistente.enviar("cuenta");
    assert!(b.esperar(|v| v.iter().any(|s| matches!(s, Senal::Boca(..))), 10));
    b.asistente.parar_de_hablar();
    assert!(b.esperar(|v| v.iter().any(|s| matches!(s, Senal::BocaFin)), 10));
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        carpeta_altavoz(&b).len() <= 2,
        "las frases de después ya no suenan: {:?}",
        carpeta_altavoz(&b)
    );
    b.asistente.cerrar_todo();
}

#[test]
fn si_falta_claude_sale_el_boton_y_se_dice() {
    let b = Banco::nuevo(Opciones {
        estado: (
            "activar".into(),
            "Para hablar conmigo necesito tu cuenta de Claude.".into(),
        ),
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(b.esperar(|v| texto_final(v).is_some(), 10), "{:?}", b.vistas());
    let v = b.vistas();
    assert_eq!(
        texto_final(&v).unwrap(),
        "Para hablar conmigo necesito tu cuenta de Claude."
    );
    assert_eq!(botones_ultimos(&v), ["activar", "usar_local"]);
    // Un botón que no está a la vista no hace nada.
    b.asistente.accion("autorizar");
    std::thread::sleep(Duration::from_millis(100));
    assert!(!hay_aviso(&b.vistas(), "Cancelado"));
    b.asistente.cerrar_todo();
}

#[test]
fn el_permiso_de_una_accion_autorizar_cancelar_y_todo() {
    let b = Banco::nuevo(Opciones {
        retener: true,
        guion: vec![Evento::Texto("Voy.".into())],
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(b.esperar(|_| b.socket().exists(), 5));
    // Autorizar.
    let (mut _s, mut r) = b.pedir(json!({"tipo": "pedir", "titulo": "Abrir «Firefox»", "detalle": "La app"}));
    assert!(
        b.esperar(
            |v| botones_ultimos(v) == ["autorizar", "autorizar_todo", "cancelar"],
            10
        ),
        "{:?}",
        b.vistas()
    );
    assert!(hay_aviso(&b.vistas(), "¿Lo hago?  Abrir «Firefox»"));
    b.asistente.accion("autorizar");
    assert_eq!(respuesta(&mut r), json!({"ok": true}));
    drop(_s);
    // Cancelar.
    assert!(b.esperar(|v| botones_ultimos(v).is_empty(), 5));
    let (mut s2, mut r2) = b.pedir(json!({"titulo": "Correr un comando", "detalle": "$ ls"}));
    assert!(b.esperar(|v| botones_ultimos(v).contains(&"cancelar".to_string()), 10));
    // Mientras una espera, otra acción se rechaza sola.
    let (_s3, mut r3) = b.pedir(json!({"titulo": "Otra", "detalle": "x"}));
    assert_eq!(respuesta(&mut r3), json!({"ok": false}));
    b.asistente.accion("cancelar");
    assert_eq!(respuesta(&mut r2), json!({"ok": false}));
    assert!(hay_aviso(&b.vistas(), "Cancelado: no hice nada."));
    let _ = s2.flush();
    drop(s2);
    // Autorizar todo: la siguiente pasa sin preguntar, pero se avisa.
    let (_s4, mut r4) = b.pedir(json!({"titulo": "Subir volumen", "detalle": "x"}));
    assert!(b.esperar(|v| botones_ultimos(v).contains(&"autorizar_todo".to_string()), 10));
    b.asistente.accion("autorizar_todo");
    assert_eq!(respuesta(&mut r4), json!({"ok": true}));
    drop(_s4);
    std::thread::sleep(Duration::from_millis(100));
    let (_s5, mut r5) = b.pedir(json!({"titulo": "Bajar brillo", "detalle": "x"}));
    assert_eq!(respuesta(&mut r5), json!({"ok": true}));
    assert!(hay_aviso(
        &b.vistas(),
        "Hecho sin preguntar (autorizaste todo):  Bajar brillo"
    ));
    // Un pedido mal formado se rechaza.
    let (_s6, mut r6) = b.pedir(json!({"sin": "titulo"}));
    assert_eq!(respuesta(&mut r6), json!({"ok": false}));
    b.retener.store(false, Ordering::SeqCst);
    b.asistente.cerrar_todo();
}

#[test]
fn una_pregunta_nueva_cancela_el_permiso_que_esperaba() {
    let b = Banco::nuevo(Opciones {
        retener: true,
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(b.esperar(|_| b.socket().exists(), 5));
    let (_s, mut r) = b.pedir(json!({"titulo": "Cosa", "detalle": "x"}));
    assert!(b.esperar(|v| botones_ultimos(v).contains(&"autorizar".to_string()), 10));
    b.asistente.enviar("mejor otra cosa");
    assert_eq!(respuesta(&mut r), json!({"ok": false}));
    b.retener.store(false, Ordering::SeqCst);
    b.asistente.cerrar_todo();
}

fn camara_lista(extra: impl FnOnce(&mut Config)) -> Banco {
    let mut cfg = Config {
        seguir_escuchando: false,
        camara: true,
        camara_aviso: true,
        ..Config::default()
    };
    extra(&mut cfg);
    Banco::nuevo(Opciones {
        cfg,
        retener: true,
        ..Default::default()
    })
}

#[test]
fn la_camara_solo_con_motivo_una_foto_y_su_indicador() {
    let b = camara_lista(|_| {});
    b.asistente.iniciar();
    assert!(b.esperar(|_| b.socket().exists(), 5));
    // Sin una pregunta en curso no hay foto.
    let (_s0, mut r0) = b.pedir(json!({"tipo": "camara"}));
    let n = respuesta(&mut r0);
    assert_eq!(n["ok"], false);
    assert!(
        n["mensaje"]
            .as_str()
            .unwrap()
            .contains("solo se usa cuando la persona te lo pide"),
        "{n}"
    );
    // Una pregunta que sí pide la cámara: pasa, con indicador, y se apaga al cerrar la conexión.
    b.asistente.enviar("Miku, mira el cuarto");
    assert!(b.esperar(|v| hay_estado(v, "pensando"), 5));
    let (s1, mut r1) = b.pedir(json!({"tipo": "camara"}));
    assert_eq!(respuesta(&mut r1), json!({"ok": true}));
    assert!(b.esperar(|v| v.contains(&Senal::Camara(true)), 5));
    assert!(hay_aviso(&b.vistas(), "una sola foto"));
    drop((s1, r1)); // la acción terminó: se cierra la conexión
    assert!(
        b.esperar(|v| v.contains(&Senal::Camara(false)), 5),
        "{:?}",
        b.vistas()
    );
    // Una sola foto por pregunta.
    let (_s2, mut r2) = b.pedir(json!({"tipo": "camara"}));
    assert!(
        respuesta(&mut r2)["mensaje"]
            .as_str()
            .unwrap()
            .contains("Ya tomé una foto")
    );
    // Queda anotada en la bitácora (sin imagen).
    assert!(
        b.companera
            .leer_bitacora()
            .iter()
            .any(|l| l.contains("Foto con la cámara (la pediste tú)"))
    );
    b.retener.store(false, Ordering::SeqCst);
    b.asistente.cerrar_todo();
}

#[test]
fn la_camara_apagada_dice_que_no_y_ofrece_activarla() {
    let b = camara_lista(|c| c.camara = false);
    b.asistente.iniciar();
    assert!(b.esperar(|_| b.socket().exists(), 5));
    b.asistente.enviar("mira el cuarto");
    assert!(b.esperar(|v| hay_estado(v, "pensando"), 5));
    let (_s, mut r) = b.pedir(json!({"tipo": "camara"}));
    let n = respuesta(&mut r);
    assert!(n["mensaje"].as_str().unwrap().contains("APAGADA"), "{n}");
    assert!(b.esperar(|v| botones_ultimos(v) == ["activar_camara"], 5));
    // Activarla pide confirmación y la guarda en el disco.
    b.asistente.accion("activar_camara");
    assert!(b.esperar(|v| botones_ultimos(v) == ["camara_si", "camara_no"], 5));
    b.asistente.accion("camara_si");
    assert!(b.esperar(|v| hay_aviso(v, "la cámara está activada"), 5));
    let disco = Config::cargar(&b.rutas.config, &b.rutas);
    assert!(disco.camara && disco.camara_aviso);
    b.retener.store(false, Ordering::SeqCst);
    b.asistente.cerrar_todo();
}

#[test]
fn si_no_suena_a_pedir_la_camara_se_pregunta_con_un_clic() {
    let b = camara_lista(|_| {});
    b.asistente.iniciar();
    assert!(b.esperar(|_| b.socket().exists(), 5));
    b.asistente.enviar("¿cómo ves todo por aquí?");
    assert!(b.esperar(|v| hay_estado(v, "pensando"), 5));
    let (_s, mut r) = b.pedir(json!({"tipo": "camara"}));
    assert!(
        b.esperar(|v| botones_ultimos(v) == ["autorizar", "cancelar"], 5),
        "{:?}",
        b.vistas()
    );
    // «Autorizar todo» nunca vale para la cámara: cada foto, su clic.
    b.asistente.accion("autorizar_todo");
    assert_eq!(respuesta(&mut r), json!({"ok": true}));
    b.retener.store(false, Ordering::SeqCst);
    b.asistente.cerrar_todo();
}

#[test]
fn la_asomada_del_modo_compania_cuenta_y_toma_la_foto() {
    let cfg = Config {
        seguir_escuchando: false,
        camara: true,
        camara_aviso: true,
        companera: true,
        companera_consentimiento: true,
        ..Config::default()
    };
    let b = Banco::nuevo(Opciones {
        cfg,
        retener: true,
        arranque: Arranque {
            companera: "asomarse".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(
        b.esperar(
            |v| botones_ultimos(v) == ["companera_cancelar", "companera_dia"],
            10
        ),
        "{:?}",
        b.vistas()
    );
    assert!(b.esperar(
        |v| texto_final(v).is_some_and(|t| t.contains("Me asomo a ver cómo vas")),
        5
    ));
    // Pasa la cuenta atrás y le pregunta a la IA con la orden oculta.
    assert!(
        b.esperar(|_| !b.preguntas.lock().unwrap().is_empty(), 20),
        "{:?}",
        b.vistas()
    );
    assert!(b.preguntas.lock().unwrap()[0].starts_with("[Asomada del modo compañera"));
    // Y esa pregunta (la de la asomada) puede tomar su foto aunque no la haya escrito la persona.
    let (_s, mut r) = b.pedir(json!({"tipo": "camara"}));
    assert_eq!(respuesta(&mut r), json!({"ok": true}));
    assert!(
        b.companera
            .leer_bitacora()
            .iter()
            .any(|l| l.contains("asomada del modo compañera"))
    );
    b.retener.store(false, Ordering::SeqCst);
    b.asistente.cerrar_todo();
}

#[test]
fn decir_no_en_la_cuenta_atras_cancela_y_la_cierra() {
    let cfg = Config {
        seguir_escuchando: false,
        camara: true,
        companera: true,
        companera_consentimiento: true,
        ..Config::default()
    };
    let b = Banco::nuevo(Opciones {
        cfg,
        arranque: Arranque {
            companera: "asomarse".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(b.esperar(
        |v| botones_ultimos(v).contains(&"companera_cancelar".to_string()),
        10
    ));
    b.asistente.accion("companera_cancelar");
    assert!(
        b.esperar(
            |v| texto_final(v).is_some_and(|t| t.starts_with("Va, no miro")),
            10
        ),
        "{:?}",
        b.vistas()
    );
    assert!(b.esperar(|v| v.contains(&Senal::Cerrar), 10), "se va sola");
    assert!(
        b.preguntas.lock().unwrap().is_empty(),
        "nada de foto ni de pregunta"
    );
    assert!(
        b.companera
            .leer_bitacora()
            .iter()
            .any(|l| l.contains("Asomada cancelada: no hubo foto"))
    );
    b.asistente.cerrar_todo();
}

#[test]
fn ya_no_me_molestes_la_calla_hasta_manana() {
    let cfg = Config {
        seguir_escuchando: false,
        companera: true,
        companera_consentimiento: true,
        camara: true,
        ..Config::default()
    };
    let b = Banco::nuevo(Opciones {
        cfg,
        ..Default::default()
    });
    b.asistente.iniciar();
    std::thread::sleep(Duration::from_millis(100));
    b.asistente.enviar("ya no me molestes");
    assert!(
        b.esperar(
            |v| texto_final(v).is_some_and(|t| t == "Va, no te molesto hasta mañana."),
            10
        ),
        "{:?}",
        b.vistas()
    );
    assert!(
        b.preguntas.lock().unwrap().is_empty(),
        "lo resolvió el código, sin la IA"
    );
    assert_eq!(b.companera.leer_estado()["callada_hasta"], "2026-11-20T09:00:00");
    b.asistente.cerrar_todo();
}

#[test]
fn apagar_el_modo_companera_lo_guarda_en_el_disco() {
    let cfg = Config {
        seguir_escuchando: false,
        companera: true,
        companera_consentimiento: true,
        camara: true,
        ..Config::default()
    };
    let b = Banco::nuevo(Opciones {
        cfg,
        ..Default::default()
    });
    b.asistente.iniciar();
    std::thread::sleep(Duration::from_millis(100));
    b.asistente.enviar("apaga el modo compañera");
    assert!(b.esperar(
        |v| texto_final(v).is_some_and(|t| t.starts_with("Listo, apagué el modo compañera")),
        10
    ));
    let d = Config::cargar(&b.rutas.config, &b.rutas);
    assert!(!d.companera && !d.companera_consentimiento);
    b.asistente.cerrar_todo();
}

#[test]
fn la_bienvenida_pide_el_consentimiento_y_lo_guarda() {
    let b = Banco::nuevo(Opciones {
        arranque: Arranque {
            companera: "consentimiento".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(
        b.esperar(|v| botones_ultimos(v).contains(&"companera_si".to_string()), 10),
        "{:?}",
        b.vistas()
    );
    assert!(hay_aviso(&b.vistas(), "Modo compañera: de vez en cuando"));
    b.asistente.accion("companera_si");
    assert!(b.esperar(|v| hay_aviso(v, "el modo compañera está activado"), 5));
    let d = Config::cargar(&b.rutas.config, &b.rutas);
    assert!(d.companera && d.companera_consentimiento && d.camara && d.camara_aviso);
    // Ya no hay botones: tocar uno viejo no hace nada.
    b.asistente.accion("companera_sin_cuenta");
    std::thread::sleep(Duration::from_millis(100));
    assert!(botones_ultimos(&b.vistas()).is_empty());
    b.asistente.cerrar_todo();
}

#[test]
fn una_frase_que_falla_en_la_voz_se_avisa_y_sigue() {
    let b = Banco::nuevo(Opciones {
        guion: vec![
            Evento::Texto("Esta FALLA. Esta sí.".into()),
            Evento::Listo("x".into()),
        ],
        ..Default::default()
    });
    b.asistente.iniciar();
    b.asistente.enviar("di algo");
    assert!(
        b.esperar(|v| hay_aviso(v, "La voz falló con una frase"), 10),
        "{:?}",
        b.vistas()
    );
    assert!(
        b.esperar(|v| v.iter().any(|s| matches!(s, Senal::Boca(..))), 10),
        "la otra frase suena"
    );
    b.asistente.cerrar_todo();
}

#[test]
fn sin_voz_responde_solo_por_texto() {
    let b = Banco::nuevo(Opciones {
        guion: guion_hola(),
        voz: false,
        ..Default::default()
    });
    b.asistente.iniciar();
    assert!(
        b.esperar(|v| hay_aviso(v, "Te respondo solo por texto"), 10),
        "{:?}",
        b.vistas()
    );
    b.asistente.enviar("hola");
    assert!(b.esperar(
        |v| texto_final(v).is_some_and(|t| t.starts_with("Hola. Soy Miku")),
        10
    ));
    b.asistente.cerrar_todo();
}

#[test]
fn escucha_transcribe_y_pregunta() {
    // El «micrófono» es un archivo con silencio, luego una voz y luego silencio otra vez.
    let dir = tempfile::tempdir().unwrap();
    let mut pcm: Vec<u8> = vec![];
    let mut sem = 5u64;
    let mut trozo = |seg: f64, amp: f64, tono: bool, pcm: &mut Vec<u8>| {
        for i in 0..(seg * 16000.0) as usize {
            sem = sem
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let ruido = ((sem >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0;
            let x = if tono {
                amp * (2.0 * std::f64::consts::PI * 700.0 * i as f64 / 16000.0).sin()
            } else {
                amp * ruido
            };
            pcm.extend((x as i16).to_le_bytes());
        }
    };
    trozo(0.6, 30.0, false, &mut pcm);
    trozo(1.0, 8000.0, true, &mut pcm);
    trozo(3.0, 30.0, false, &mut pcm);
    let f = dir.path().join("mic.raw");
    std::fs::write(&f, &pcm).unwrap();
    let b2 = banco_con_oido(&f, Arranque::default());
    b2.asistente.iniciar();
    assert!(
        b2.esperar(|v| hay_estado(v, "escuchando"), 10),
        "{:?}",
        b2.vistas()
    );
    assert!(
        b2.esperar(|v| v.iter().any(|s| matches!(s, Senal::Nivel(_))), 10),
        "el indicador del micrófono"
    );
    assert!(
        b2.esperar(
            |v| v.iter().any(|s| matches!(s, Senal::Usuario(t) if t == "hola")),
            10
        ),
        "{:?}",
        b2.vistas()
    );
    assert!(b2.esperar(|_| !b2.preguntas.lock().unwrap().is_empty(), 10));
    assert_eq!(b2.preguntas.lock().unwrap().as_slice(), ["hola"]);
    b2.asistente.cerrar_todo();
}

/// El chat mini (doble clic en la compañera de escritorio): con el oído listo, no abre el micrófono solo; lo
/// que escribes sí llega a la IA.
#[test]
fn en_el_chat_mini_no_abre_el_microfono_solo() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("mic.raw");
    std::fs::write(&f, vec![0u8; 32000]).unwrap();
    let b = banco_con_oido(&f, Arranque { chat: true, ..Default::default() });
    b.asistente.iniciar();
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert!(!hay_estado(&b.vistas(), "escuchando"), "{:?}", b.vistas());
    b.asistente.enviar("hola");
    assert!(b.esperar(|_| !b.preguntas.lock().unwrap().is_empty(), 10));
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert!(!hay_estado(&b.vistas(), "escuchando"), "ni después de contestar: {:?}", b.vistas());
    b.asistente.cerrar_todo();
}

fn banco_con_oido(f: &Path, arranque: Arranque) -> Banco {
    let o = Opciones {
        guion: guion_hola(),
        ..Default::default()
    };
    // Se arma igual que `Banco::nuevo`, cambiando solo el oído.
    let dir = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(dir.path());
    std::fs::create_dir_all(&rutas.config_dir).unwrap();
    let senales = Arc::new(Mutex::new(vec![]));
    let preguntas = Arc::new(Mutex::new(vec![]));
    let (retener, olvidos, cancel) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(0u32)),
        Arc::new(Mutex::new(0u32)),
    );
    let fabrica: vortex_miku::asistente::FabricaCerebro = {
        let (g, r, p, e, ol) = (
            o.guion.clone(),
            retener.clone(),
            preguntas.clone(),
            o.estado.clone(),
            olvidos.clone(),
        );
        Arc::new(move |_c: &Config, _a: bool, al: AlEvento| -> Arc<dyn Cerebro> {
            Arc::new(IaFalsa {
                al_evento: al,
                guion: g.clone(),
                retener: r.clone(),
                preguntas: p.clone(),
                estado: e.clone(),
                ocupado: Arc::new(AtomicBool::new(false)),
                cancelaciones: cancel.clone(),
                olvidos: ol.clone(),
            })
        })
    };
    let guion = dir.path().join("voz.py");
    std::fs::write(&guion, VOZ_FALSA).unwrap();
    let mut oido = Oido::nuevo(1.0, 5.0);
    oido.fuente = Some(vec!["cat".into(), f.to_string_lossy().into()]);
    let mut ia = Ia::nueva(rutas.clone());
    ia.llavero = false;
    let companera = Arc::new(Companera::de_prueba(dir.path()));
    let env = Entorno {
        rutas: rutas.clone(),
        ia: Arc::new(ia),
        corazon: Arc::new(Corazon::de_prueba(dir.path())),
        companera: companera.clone(),
        fabrica,
        factor_tiempo: 0.01,
        salida_prueba: Some((dir.path().join("altavoz"), 50.0)),
        comando_voz: Some((
            which("python3"),
            vec![
                "-I".into(),
                guion.to_string_lossy().into(),
                rutas.temporales().to_string_lossy().into(),
            ],
            vec![],
        )),
        reproductor: None,
        oido: Some(Arc::new(oido)),
        transcribir: Some(Arc::new(|pcm| {
            assert!(pcm.len() > 16000, "llegó la frase, no el silencio");
            Ok("hola".into())
        })),
        sin_descargas: true,
        natural_auto: Some("0".into()),
        cli: None,
        notificar: false,
    };
    let s = senales.clone();
    let asistente = Asistente::nuevo(
        Config {
            seguir_escuchando: false,
            ..Config::default()
        },
        true,
        env,
        arranque,
        Arc::new(move |x| s.lock().unwrap().push(x)),
    );
    Banco {
        dir,
        rutas,
        senales,
        preguntas,
        retener,
        olvidos,
        asistente,
        companera,
    }
}
