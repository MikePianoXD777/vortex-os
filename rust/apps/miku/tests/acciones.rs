//! Las acciones de Miku (`miku-aux mcp`): el servidor MCP de verdad como subproceso, un «Botón» de mentira que
//! contesta los permisos por el socket, y programas de mentira en el PATH (flatpak, xdg-open, wl-copy, wpctl…)
//! que anotan con qué los llamaron. Nada toca la cuenta, el micrófono, la cámara ni las apps de nadie.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// El botón: guarda lo que se le pide y contesta con lo que haya en la cola (por omisión, «sí»).
struct Boton {
    pedidos: Arc<Mutex<Vec<Value>>>,
    respuestas: Arc<Mutex<Vec<Value>>>,
}

fn boton(ruta: &Path) -> Boton {
    let l = UnixListener::bind(ruta).unwrap();
    let (pedidos, respuestas) = (Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(vec![])));
    let (p, r) = (pedidos.clone(), respuestas.clone());
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (p, r) = (p.clone(), r.clone());
            std::thread::spawn(move || {
                let mut lector = BufReader::new(s.try_clone().unwrap());
                let mut l = String::new();
                let _ = lector.read_line(&mut l);
                p.lock()
                    .unwrap()
                    .push(serde_json::from_str(l.trim()).unwrap_or_default());
                let resp = {
                    let mut r = r.lock().unwrap();
                    if r.is_empty() {
                        json!({"ok": true})
                    } else {
                        r.remove(0)
                    }
                };
                let mut s = s;
                let _ = writeln!(s, "{resp}");
                // La conexión sigue abierta mientras la acción corre: se cierra al irse el cliente.
                let mut resto = String::new();
                let _ = lector.read_line(&mut resto);
            });
        }
    });
    Boton { pedidos, respuestas }
}

struct Mcp {
    dir: tempfile::TempDir,
    hijo: Child,
    entrada: ChildStdin,
    salida: BufReader<ChildStdout>,
    boton: Boton,
    siguiente: u64,
}

fn programa(dir: &Path, nombre: &str, cuerpo: &str) {
    let r = dir.join("bin").join(nombre);
    std::fs::create_dir_all(r.parent().unwrap()).unwrap();
    std::fs::write(&r, format!("#!/bin/sh\n{cuerpo}\n")).unwrap();
    vortex_miku::sistema::permisos(&r, 0o755).unwrap();
}

impl Mcp {
    fn nuevo() -> Self {
        Self::con(true)
    }

    fn con(con_socket: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path();
        let reg = b.join("llamadas.log");
        let anotar = format!("echo \"$(basename \"$0\") $*\" >> {}", reg.display());
        // Los programas de mentira: anotan cómo los llamaron.
        programa(b, "xdg-open", &anotar);
        programa(b, "notify-send", &anotar);
        programa(b, "wpctl", &anotar);
        programa(b, "xdg-user-dir", "echo /tmp/miku-carpeta-falsa");
        programa(b, "xdg-mime", "exit 0");
        programa(
            b,
            "wl-copy",
            &format!("echo \"wl-copy $(cat)\" >> {}", reg.display()),
        );
        programa(
            b,
            "flatpak",
            &format!(
                "{anotar}\ncase \"$1\" in search) printf 'com.valvesoftware.Steam\\tSteam\\n';; list) printf 'org.gimp.GIMP\\tGIMP\\n';; esac"
            ),
        );
        programa(b, "gio", &anotar);
        let sock = b.join("autorizar.sock");
        let boton = boton(&sock);
        let mut c = Command::new(env!("CARGO_BIN_EXE_miku-aux"));
        c.arg("mcp")
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", b.join("bin").display()))
            .env("HOME", b)
            .env("XDG_RUNTIME_DIR", b)
            .env("XDG_DATA_HOME", b.join("datos"));
        if con_socket {
            c.env("VORTEX_MIKU_AUTORIZAR", &sock);
        }
        let mut hijo = c
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (entrada, salida) = (
            hijo.stdin.take().unwrap(),
            BufReader::new(hijo.stdout.take().unwrap()),
        );
        Self {
            dir,
            hijo,
            entrada,
            salida,
            boton,
            siguiente: 0,
        }
    }

    fn rpc(&mut self, metodo: &str, params: Value) -> Value {
        self.siguiente += 1;
        writeln!(
            self.entrada,
            "{}",
            json!({"jsonrpc": "2.0", "id": self.siguiente, "method": metodo, "params": params})
        )
        .unwrap();
        let mut l = String::new();
        self.salida.read_line(&mut l).unwrap();
        serde_json::from_str(l.trim()).unwrap_or_else(|_| panic!("respuesta rara: {l:?}"))
    }

    /// Llama una herramienta y devuelve (texto, ¿error?).
    fn herramienta(&mut self, nombre: &str, args: Value) -> (String, bool) {
        let r = self.rpc("tools/call", json!({"name": nombre, "arguments": args}));
        let res = &r["result"];
        let texto = res["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|x| x["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        (texto, res["isError"].as_bool().unwrap_or(false))
    }

    fn pedidos(&self) -> Vec<Value> {
        self.boton.pedidos.lock().unwrap().clone()
    }

    fn llamadas(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("llamadas.log")).unwrap_or_default()
    }

    /// Espera a que un programa lanzado aparte deje su huella.
    fn esperar_llamada(&self, parte: &str) -> String {
        let hasta = Instant::now() + Duration::from_secs(10);
        while Instant::now() < hasta {
            let l = self.llamadas();
            if l.contains(parte) {
                return l;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("no apareció «{parte}» en: {}", self.llamadas());
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

#[test]
fn se_presenta_y_lista_sus_herramientas() {
    let mut m = Mcp::nuevo();
    let r = m.rpc("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}));
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r["result"]["serverInfo"]["name"], "miku");
    let r = m.rpc("tools/list", json!({}));
    let mut nombres: Vec<&str> = r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    nombres.sort();
    assert_eq!(
        nombres,
        [
            "abrir",
            "ajuste",
            "clic",
            "correr_comando",
            "escribir",
            "grabar",
            "instalar_app",
            "mejores_momentos",
            "quitar_app",
            "redactar_correo",
            "redactar_mensaje",
            "ver_camara"
        ]
    );
    assert_eq!(m.rpc("nada", json!({}))["error"]["code"], -32601);
}

#[test]
fn correr_comando_pide_permiso_y_corre_con_el_codigo_y_la_salida() {
    let mut m = Mcp::nuevo();
    let (t, e) = m.herramienta(
        "correr_comando",
        json!({"comando": "echo hola; echo mal >&2; exit 3", "para_que": "probar"}),
    );
    assert!(!e);
    assert!(
        t.starts_with("Salió con código 3.\nhola") && t.contains("mal"),
        "{t}"
    );
    let p = &m.pedidos()[0];
    assert_eq!(p["titulo"], "Correr un comando");
    assert!(
        p["detalle"].as_str().unwrap().contains("probar")
            && p["detalle"].as_str().unwrap().contains("$ echo hola"),
        "{p}"
    );
    // Sin autorizar, no se corre.
    m.boton.respuestas.lock().unwrap().push(json!({"ok": false}));
    let (t, _) = m.herramienta(
        "correr_comando",
        json!({"comando": "touch /tmp/miku-no-debe-existir-xyz"}),
    );
    assert!(t.contains("NO lo autorizó"), "{t}");
    assert!(!Path::new("/tmp/miku-no-debe-existir-xyz").exists());
}

#[test]
fn nunca_sudo_ni_tocar_los_permisos_de_miku_y_ni_se_pregunta() {
    let mut m = Mcp::nuevo();
    for c in [
        "sudo rm -rf /",
        "ls && doas reboot",
        "echo hola | pkexec id",
        "vortex-miku camara on",
        "sed -i s/false/true/ ~/.config/vortex/miku.json",
        "rm ~/.local/state/vortex/miku-bitacora.log",
        "vortex claude reanudar",
    ] {
        let (t, e) = m.herramienta("correr_comando", json!({"comando": c}));
        assert!(e && (t.contains("sudo") || t.contains("Miku")), "{c}: {t}");
    }
    assert!(m.pedidos().is_empty(), "ni siquiera se le preguntó a la persona");
    // Un comando normal con «su» dentro de una palabra sí pasa.
    let (t, e) = m.herramienta("correr_comando", json!({"comando": "echo sudoku"}));
    assert!(!e && t.contains("sudoku"), "{t}");
}

#[test]
fn abrir_una_pagina_una_carpeta_y_lo_que_no_existe() {
    let mut m = Mcp::nuevo();
    let (t, e) = m.herramienta("abrir", json!({"que": "https://example.org/a b"}));
    assert!(!e && t.contains("example.org"), "{t}");
    assert_eq!(m.pedidos()[0]["titulo"], "Abrir una página web");
    m.esperar_llamada("xdg-open https://example.org/a b");
    // Una carpeta por su nombre en español.
    let carpeta = m.dir.path().join("Descargas");
    std::fs::create_dir_all(&carpeta).unwrap();
    programa(
        m.dir.path(),
        "xdg-user-dir",
        &format!("echo {}", carpeta.display()),
    );
    let (t, _) = m.herramienta("abrir", json!({"que": "Descargas"}));
    assert!(t.contains(&carpeta.to_string_lossy().to_string()), "{t}");
    m.esperar_llamada(&format!("xdg-open {}", carpeta.display()));
    let (t, e) = m.herramienta("abrir", json!({"que": "una-app-que-no-existe-jamas"}));
    assert!(e && t.contains("No encuentro una app, carpeta o archivo"), "{t}");
    assert_eq!(
        m.pedidos().len(),
        2,
        "lo que no existe no molesta a nadie con un permiso"
    );
}

#[test]
fn abrir_una_app_por_su_nombre_del_desktop() {
    let mut m = Mcp::nuevo();
    let apps = m.dir.path().join("datos/applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::write(
        apps.join("miapp.desktop"),
        "[Desktop Entry]\nType=Application\nName=Mi App Bonita\nName[es]=Mi App Linda\nExec=miapp\n",
    )
    .unwrap();
    std::fs::write(
        apps.join("oculta.desktop"),
        "[Desktop Entry]\nType=Application\nName=Mi App Oculta\nNoDisplay=true\nExec=x\n",
    )
    .unwrap();
    let (t, e) = m.herramienta("abrir", json!({"que": "mi app linda"}));
    assert!(!e && t == "Abrí Mi App Bonita.", "{t}");
    assert_eq!(m.pedidos()[0]["titulo"], "Abrir «Mi App Bonita»");
    m.esperar_llamada("gio launch");
    let (t, e) = m.herramienta("abrir", json!({"que": "oculta"}));
    assert!(e, "las ocultas no se abren: {t}");
}

#[test]
fn ajustes_con_sus_valores_y_su_permiso() {
    let mut m = Mcp::nuevo();
    let (t, e) = m.herramienta("ajuste", json!({"tipo": "volumen", "valor": "40"}));
    assert!(!e && t == "Listo: volumen al 40 %.", "{t}");
    assert!(
        m.llamadas()
            .contains("wpctl set-volume -l 1.5 @DEFAULT_AUDIO_SINK@ 40%")
    );
    let (t, _) = m.herramienta("ajuste", json!({"tipo": "volumen", "valor": "silencio"}));
    assert!(t.contains("silenciar el sonido"), "{t}");
    assert!(m.llamadas().contains("wpctl set-mute @DEFAULT_AUDIO_SINK@ 1"));
    // El volumen no pasa de 150 y el resto de ajustes pide valores válidos.
    m.herramienta("ajuste", json!({"tipo": "volumen", "valor": "999"}));
    assert!(m.llamadas().contains("150%"));
    let (t, e) = m.herramienta("ajuste", json!({"tipo": "wifi", "valor": "tal vez"}));
    assert!(e && t.contains("no es encender ni apagar"), "{t}");
    let (t, e) = m.herramienta("ajuste", json!({"tipo": "cafe", "valor": "1"}));
    assert!(e && t.contains("Ajustes que sé cambiar"), "{t}");
    let (t, e) = m.herramienta("ajuste", json!({"tipo": "fondo", "valor": "/no/hay/imagen.png"}));
    assert!(e && t.contains("No encuentro la imagen"), "{t}");
}

#[test]
fn instalar_y_quitar_apps_de_flathub_solo_del_usuario() {
    let mut m = Mcp::nuevo();
    let (t, e) = m.herramienta("instalar_app", json!({"nombre": "Steam"}));
    assert!(
        !e && t.contains("Empecé a instalar Steam (com.valvesoftware.Steam)"),
        "{t}"
    );
    let p = &m.pedidos()[0];
    assert_eq!(p["titulo"], "Instalar «Steam»");
    assert!(p["detalle"].as_str().unwrap().contains("sin contraseña"));
    let l = m.esperar_llamada("flatpak install --user -y --noninteractive flathub com.valvesoftware.Steam");
    assert!(l.contains("flatpak remote-add --user --if-not-exists flathub"));
    // Quitar una que sí está.
    let (t, e) = m.herramienta("quitar_app", json!({"nombre": "gimp"}));
    assert!(!e && t == "Listo, quité GIMP.", "{t}");
    assert!(
        m.llamadas()
            .contains("flatpak uninstall --user -y --noninteractive org.gimp.GIMP")
    );
    // Y una que no (las del sistema necesitan contraseña: eso lo hace la persona).
    let (t, e) = m.herramienta("quitar_app", json!({"nombre": "firefox"}));
    assert!(
        e && t.contains("No encuentro «firefox» entre tus apps de Flathub"),
        "{t}"
    );
}

#[test]
fn redactar_mensajes_y_correos_no_envian_nada() {
    let mut m = Mcp::nuevo();
    let (t, e) = m.herramienta(
        "redactar_mensaje",
        json!({"app": "whatsapp", "para": "Mamá", "texto": "Llego tarde"}),
    );
    assert!(
        !e && t.contains("Copié el mensaje") && t.contains("No se envió nada"),
        "{t}"
    );
    m.esperar_llamada("wl-copy Llegó tarde".replace("Llegó", "Llego").as_str());
    m.esperar_llamada("xdg-open https://web.whatsapp.com");
    let (t, e) = m.herramienta("redactar_mensaje", json!({"app": "whatsapp", "texto": ""}));
    assert!(e && t.contains("Falta el texto"), "{t}");
    // Sin app de correo, abre Gmail con todo bien escapado.
    let (t, e) = m.herramienta(
        "redactar_correo",
        json!({"para": "ana@ejemplo.com", "asunto": "Hola ñandú", "cuerpo": "Línea 1\nLínea 2"}),
    );
    assert!(
        !e && t.contains("Gmail en tu navegador") && t.contains("No se envió nada"),
        "{t}"
    );
    let l = m.esperar_llamada("mail.google.com");
    assert!(
        l.contains("to=ana%40ejemplo.com")
            && l.contains("su=Hola%20%C3%B1and%C3%BA")
            && l.contains("body=L%C3%ADnea%201%0AL%C3%ADnea%202"),
        "{l}"
    );
    assert!(
        m.pedidos()
            .iter()
            .any(|p| p["titulo"].as_str().unwrap().starts_with("Dejar listo un correo"))
    );
}

#[test]
fn la_camara_obedece_a_la_ventana_y_sin_ventana_no_hay_nada() {
    let mut m = Mcp::nuevo();
    m.boton
        .respuestas
        .lock()
        .unwrap()
        .push(json!({"ok": false, "mensaje": "La cámara está APAGADA en los ajustes de Miku."}));
    let (t, e) = m.herramienta("ver_camara", json!({"motivo": "cuarto"}));
    assert!(!e && t == "La cámara está APAGADA en los ajustes de Miku.", "{t}");
    assert_eq!(
        m.pedidos()[0],
        json!({"tipo": "camara", "titulo": "Tomar una foto con la cámara", "detalle": "Una sola foto; no se guarda y no se graba video.", "motivo": "cuarto"})
    );
    // Un motivo raro cuenta como «otro».
    m.boton.respuestas.lock().unwrap().push(json!({"ok": false}));
    m.herramienta("ver_camara", json!({"motivo": "espiar"}));
    assert_eq!(m.pedidos()[1]["motivo"], "otro");
    // Sin la ventana de Miku abierta, ninguna acción se hace.
    let mut sin = Mcp::con(false);
    let (t, e) = sin.herramienta("correr_comando", json!({"comando": "echo hola"}));
    assert!(
        e && t.contains("No puedo pedir permiso (Miku no está abierta): no hice nada."),
        "{t}"
    );
}

#[test]
fn grabar_pide_permiso_y_valida_la_accion() {
    let mut m = Mcp::nuevo();
    let (t, e) = m.herramienta("grabar", json!({"accion": "bailar"}));
    assert!(e && t.contains("tiene que ser empezar, parar o estado"), "{t}");
    let (t, e) = m.herramienta("grabar", json!({"accion": "parar"}));
    assert!(e && t.contains("No estoy grabando nada"), "{t}");
    let (t, _) = m.herramienta("grabar", json!({"accion": "estado"}));
    assert_eq!(t, "No estoy grabando.");
    assert!(m.pedidos().is_empty());
}
