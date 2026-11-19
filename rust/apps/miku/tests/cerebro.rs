//! El cerebro con Claude (`cerebro.rs`) contra un `claude` de mentira (un script de shell): las capas de
//! seguridad del comando, las frases, los errores en español y una pregunta de principio a fin.

use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use vortex_miku::cerebro::{Cerebro, DivisorFrases, construir_comando, mensaje_error};
use vortex_miku::eventos::{Cerebro as _, Evento, Recolector};
use vortex_miku::rutas::Rutas;

fn guion(dir: &Path) -> std::path::PathBuf {
    let ruta = dir.join("claude");
    std::fs::write(
        &ruta,
        format!(
            r#"#!/bin/sh
echo "$@" >> "{d}/args.txt"
if [ "$1" = auth ]; then
  [ -f "{d}/sin-sesion" ] && exit 1
  exit 0
fi
modo=$(cat "{d}/modo")
case "$modo" in
  ok)
    printf '%s\n' '{{"type":"system","subtype":"init","session_id":"s1","tools":["x"],"mcp_servers":[]}}'
    printf '%s\n' '{{"type":"stream_event","event":{{"type":"content_block_start","content_block":{{"type":"tool_use","name":"mcp__vortex__ver_pantalla"}}}}}}'
    printf '%s\n' '{{"type":"stream_event","event":{{"type":"content_block_delta","delta":{{"type":"text_delta","text":"Hola. "}}}}}}'
    printf '%s\n' '{{"type":"stream_event","event":{{"type":"content_block_delta","delta":{{"type":"text_delta","text":"Qué tal."}}}}}}'
    printf '%s\n' '{{"type":"stream_event","event":{{"type":"content_block_stop"}}}}'
    printf '%s\n' '{{"type":"result","session_id":"s1","is_error":false,"result":"Hola. Qué tal."}}'
    ;;
  login)
    printf '%s\n' '{{"type":"result","is_error":true,"result":"Invalid authentication credentials · Please run /login"}}'
    exit 1
    ;;
  perdida)
    if grep -q -- "--resume" "{d}/args.txt" && [ ! -f "{d}/ya" ]; then
      touch "{d}/ya"
      printf '%s\n' '{{"type":"result","is_error":true,"result":"No conversation found with session ID: s1"}}'
      exit 1
    fi
    printf '%s\n' '{{"type":"system","subtype":"init","session_id":"s2"}}'
    printf '%s\n' '{{"type":"result","session_id":"s2","is_error":false,"result":"otra vez"}}'
    ;;
  lento)
    printf '%s\n' '{{"type":"system","subtype":"init","session_id":"s3"}}'
    exec sleep 30
    ;;
esac
"#,
            d = dir.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o755)).unwrap();
    ruta
}

fn montar(modo: &str) -> (tempfile::TempDir, Cerebro, Recolector) {
    let d = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(d.path());
    std::fs::create_dir_all(&rutas.personas).unwrap();
    std::fs::write(
        rutas.personas.join("asistente.txt"),
        "Eres la asistente de prueba.",
    )
    .unwrap();
    std::fs::write(d.path().join("modo"), modo).unwrap();
    let r = Recolector::default();
    let c = Cerebro::nuevo(&rutas, false, r.al_evento());
    *c.claude_fijo.lock().unwrap() = Some(guion(d.path()));
    c.revisar_activado.store(false, Ordering::SeqCst);
    (d, c, r)
}

#[test]
fn el_comando_va_cerrado_por_capas() {
    let aux = Path::new("/usr/lib/vortex/miku-aux");
    let a = construir_comando(
        "persona",
        "-hola",
        None,
        Some(Path::new("/usr/bin/vortex-mcp")),
        None,
        true,
        aux,
    );
    let pos = |x: &str| {
        a.iter()
            .position(|s| s == x)
            .unwrap_or_else(|| panic!("falta {x}: {a:?}"))
    };
    assert_eq!(a[pos("--tools") + 1], "", "sin herramientas integradas");
    assert!(a.contains(&"--strict-mcp-config".to_string()) && a.contains(&"--restricted".to_string()));
    assert_eq!(a[pos("--permission-mode") + 1], "dontAsk");
    let permitidas: Vec<&String> = a[pos("--allowedTools") + 1..pos("--disallowedTools")]
        .iter()
        .collect();
    assert_eq!(permitidas.len(), 4, "solo las 4 de vista: {permitidas:?}");
    assert!(
        !a.contains(&"--settings".to_string()),
        "internet sin la ventana de permisos no vale"
    );
    assert_eq!(&a[a.len() - 2..], ["--", "-hola"], "el texto va tras «--»");
    assert!(a[pos("--disallowedTools") + 1..].contains(&"Bash".to_string()));
    // Con la ventana de permisos: acciones y, con internet, el hook y WebSearch/WebFetch permitidas.
    let a = construir_comando("p", "t", Some("s1"), None, Some("/run/x.sock"), true, aux);
    let pos = |x: &str| a.iter().position(|s| s == x).unwrap();
    assert_eq!(a[pos("--tools") + 1], "WebSearch,WebFetch");
    let ajustes: serde_json::Value = serde_json::from_str(&a[pos("--settings") + 1]).unwrap();
    assert!(
        ajustes["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("miku-aux hook-web")
    );
    let mcp: serde_json::Value = serde_json::from_str(&a[pos("--mcp-config") + 1]).unwrap();
    assert_eq!(
        mcp["mcpServers"]["miku"]["env"]["VORTEX_MIKU_AUTORIZAR"],
        "/run/x.sock"
    );
    assert_eq!(mcp["mcpServers"]["miku"]["args"], json!(["mcp"]));
    assert!(a[pos("--allowedTools") + 1..pos("--disallowedTools")].contains(&"mcp__miku__clic".to_string()));
    assert!(
        !a[pos("--disallowedTools") + 1..].contains(&"WebFetch".to_string()),
        "con internet no se prohíbe lo web"
    );
    assert_eq!(a[pos("--resume") + 1], "s1");
}

#[test]
fn frases_en_trozos() {
    let mut d = DivisorFrases::default();
    let mut todas = vec![];
    for t in [
        "Hola, soy el Dr. Pérez. ",
        "¿Cómo estás? Bien",
        ".\nListo",
        ". Fin",
    ] {
        todas.extend(d.alimentar(t));
    }
    todas.push(d.vaciar());
    assert_eq!(
        todas,
        [
            "Hola, soy el Dr. Pérez.",
            "¿Cómo estás?",
            "Bien.",
            "Listo.",
            "Fin"
        ],
        "{todas:?}"
    );
}

#[test]
fn errores_en_espanol() {
    let (c, m) = mensaje_error("", Some(429), Some(&json!({"status": "rejected"})), "");
    assert_eq!(c, "cuota");
    assert!(m.starts_with("Ya usé todo el límite de Claude por ahora."));
    assert_eq!(mensaje_error("Please run /login", None, None, "").0, "sesion");
    assert_eq!(mensaje_error("", Some(401), None, "").0, "sesion");
    assert_eq!(
        mensaje_error("", None, None, "getaddrinfo ENOTFOUND api").0,
        "internet"
    );
    assert_eq!(mensaje_error("", Some(529), None, "").0, "saturado");
    assert_eq!(
        mensaje_error("unknown option '--restricted'", None, None, "").0,
        "version"
    );
    assert_eq!(mensaje_error("algo raro", None, None, "").0, "otro");
}

#[test]
fn una_pregunta_de_principio_a_fin() {
    let (d, c, r) = montar("ok");
    c.preguntar("hola", 7);
    let ev: Vec<_> = r
        .eventos()
        .into_iter()
        .map(|(e, ev)| {
            assert_eq!(e, 7);
            ev
        })
        .collect();
    assert_eq!(ev[0], Evento::Inicio);
    assert_eq!(ev[1], Evento::Herramienta("mcp__vortex__ver_pantalla".into()));
    assert_eq!(r.texto(), "Hola. Qué tal.");
    assert!(ev.contains(&Evento::BloqueFin));
    assert_eq!(ev.last().unwrap(), &Evento::Listo("Hola. Qué tal.".into()));
    assert_eq!(c.sesion().as_deref(), Some("s1"));
    assert_eq!(c.ultimo_init()["tools"], json!(["x"]));
    // La segunda pregunta sigue la conversación y la persona va en el comando.
    c.poner_sentimientos("\n\nSe siente bien.");
    c.preguntar("otra", 8);
    let args = std::fs::read_to_string(d.path().join("args.txt")).unwrap();
    let llamadas: Vec<&str> = args.split("-p --model").skip(1).collect();
    assert_eq!(llamadas.len(), 2);
    assert!(!llamadas[0].contains("--resume") && llamadas[1].contains("--resume s1"));
    assert!(llamadas[1].contains("Eres la asistente de prueba.") && llamadas[1].contains("Se siente bien."));
}

#[test]
fn sin_sesion_y_sin_claude() {
    let (d, c, r) = montar("ok");
    std::fs::write(d.path().join("sin-sesion"), "").unwrap();
    c.preguntar("hola", 1);
    assert!(
        matches!(&r.eventos()[0].1, Evento::Error { clave, .. } if clave == "sesion"),
        "{:?}",
        r.eventos()
    );
    // Sin el programa: pide activarlo.
    let d2 = tempfile::tempdir().unwrap();
    let r2 = Recolector::default();
    let c2 = Cerebro::nuevo(&Rutas::en(d2.path()), false, r2.al_evento());
    *c2.claude_fijo.lock().unwrap() = Some(d2.path().join("no-existe"));
    c2.revisar_activado.store(false, Ordering::SeqCst);
    // `claude_fijo` apunta a algo inexistente: aun así «existe» para `comprobar`; el error sale al lanzar.
    c2.preguntar("hola", 1);
    assert!(
        matches!(&r2.eventos().last().unwrap().1, Evento::Error { .. }),
        "{:?}",
        r2.eventos()
    );
}

#[test]
fn error_de_login_de_claude() {
    let (_d, c, r) = montar("login");
    c.preguntar("hola", 1);
    let e = r.eventos();
    assert!(
        matches!(&e.last().unwrap().1, Evento::Error { clave, mensaje } if clave == "sesion" && mensaje.contains("No tengo sesión")),
        "{e:?}"
    );
}

#[test]
fn la_conversacion_perdida_empieza_otra() {
    let (d, c, r) = montar("perdida");
    c.preguntar("uno", 1);
    assert_eq!(c.sesion().as_deref(), Some("s2"));
    // Se «pierde» la sesión s2 en el servidor: el siguiente --resume falla una vez y se reintenta sin él.
    std::fs::write(d.path().join("args.txt"), "--resume").unwrap();
    c.preguntar("dos", 2);
    assert!(
        matches!(&r.eventos().last().unwrap().1, Evento::Listo(t) if t == "otra vez"),
        "{:?}",
        r.eventos()
    );
}

#[test]
fn cancelar_corta_y_calla() {
    let (_d, c, r) = montar("lento");
    let c = Arc::new(c);
    let c2 = c.clone();
    let h = std::thread::spawn(move || c2.preguntar("hola", 1));
    for _ in 0..100 {
        if !r.eventos().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let t0 = std::time::Instant::now();
    c.cancelar();
    h.join().unwrap();
    assert!(t0.elapsed().as_secs() < 5, "cancelar no esperó los 30 s");
    let tipos = r.eventos();
    assert!(
        !tipos
            .iter()
            .any(|(_, e)| matches!(e, Evento::Error { .. } | Evento::Listo(_))),
        "tras cancelar ya no avisa: {tipos:?}"
    );
}
