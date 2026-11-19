//! Los mejores momentos (`miku-aux momentos`): un video de 2 min hecho por ffmpeg con 3 «jugadas» (sonido fuerte y
//! pantalla roja) a los 30, 65 y 100 s, y un Claude de mentira que anota con qué lo llamaron. Sin red, sin
//! suscripción y sin tocar nada fuera de una carpeta temporal.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const JUGADAS: [i64; 3] = [30, 65, 100];

fn hay(p: &str) -> bool {
    Command::new(p)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn ffmpeg(args: &[&str]) {
    let r = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y"])
        .args(args)
        .stdin(Stdio::null())
        .status()
        .unwrap();
    assert!(r.success());
}

fn hacer_video(destino: &Path) {
    let rojo = JUGADAS
        .iter()
        .map(|t| format!("between(t,{},{})", t - 2, t + 2))
        .collect::<Vec<_>>()
        .join("+");
    let filtro = format!(
        "[0:v]drawbox=c=red:t=fill:enable='{rojo}'[v];[2:a]volume=volume='if({rojo},0.9,0)':eval=frame[s];[1:a][s]amix=inputs=2:normalize=0[a]"
    );
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=24:d=120",
        "-f",
        "lavfi",
        "-i",
        "anoisesrc=a=0.02:d=120",
        "-f",
        "lavfi",
        "-i",
        "sine=f=440:d=120",
        "-filter_complex",
        &filtro,
        "-map",
        "[v]",
        "-map",
        "[a]",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-c:a",
        "aac",
        "-shortest",
        destino.to_str().unwrap(),
    ]);
}

struct Mundo {
    dir: tempfile::TempDir,
    video: PathBuf,
}

impl Mundo {
    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }
    fn videos(&self) -> PathBuf {
        self.dir.path().join("videos")
    }
    fn claude(&self) -> PathBuf {
        self.home().join(".local/bin/claude")
    }
    fn llamada(&self) -> PathBuf {
        self.dir.path().join("llamada")
    }
    /// Un Claude de mentira: guarda los argumentos y la entrada, y contesta lo que haya en `respuesta.json`.
    fn poner_claude(&self, respuesta: &str) {
        let resultado = json!({"type": "result", "subtype": "success", "result": respuesta, "session_id": "00000000-0000-0000-0000-000000000000"});
        std::fs::write(self.dir.path().join("respuesta.json"), format!("{resultado}\n")).unwrap();
        let c = self.claude();
        std::fs::create_dir_all(c.parent().unwrap()).unwrap();
        let d = self.dir.path().display();
        std::fs::write(&c, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/argv'\ncat > '{d}/entrada'\ntouch '{d}/llamada'\ncat '{d}/respuesta.json'\n")).unwrap();
        vortex_miku::sistema::permisos(&c, 0o755).unwrap();
    }
    fn correr(&self, video: &Path) -> Output {
        let bin = self.dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let vortex = bin.join("vortex"); // «vortex claude activado» -> sí
        std::fs::write(&vortex, "#!/bin/sh\nexit 0\n").unwrap();
        vortex_miku::sistema::permisos(&vortex, 0o755).unwrap();
        let _ = std::fs::remove_file(self.llamada());
        Command::new(env!("CARGO_BIN_EXE_miku-aux"))
            .args(["momentos"])
            .arg(video)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HOME", self.home())
            .env("XDG_RUNTIME_DIR", self.dir.path().join("run"))
            .env("VORTEX_BIN", &vortex)
            .env("VORTEX_MIKU_VIDEOS", self.videos())
            .env("VORTEX_MIKU_TAREAS", self.dir.path().join("tareas"))
            .env("VORTEX_MIKU_SIN_AVISOS", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn proyecto(&self, nombre: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(self.videos().join(format!("{nombre} - mejores momentos.vortex")))
                .unwrap(),
        )
        .unwrap()
    }
}

fn pista<'a>(p: &'a Value, nombre: &str) -> &'a Vec<Value> {
    p["sequences"][0]["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == nombre)
        .unwrap()["clips"]
        .as_array()
        .unwrap()
}

fn nombres(p: &Value) -> Vec<String> {
    pista(p, "V1")
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

fn mundo() -> Option<Mundo> {
    if !hay("ffmpeg") || !hay("ffprobe") {
        eprintln!("sin ffmpeg: no se prueba");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    for d in ["videos", "home", "run"] {
        std::fs::create_dir_all(dir.path().join(d)).unwrap();
    }
    let video = dir.path().join("videos/Partida prueba.mp4");
    hacer_video(&video);
    Some(Mundo { dir, video })
}

#[test]
fn con_claude_elige_sus_momentos_con_las_miniaturas_y_sin_herramientas() {
    let Some(m) = mundo() else { return };
    let respuesta = json!({"momentos": [{"candidato": 3, "titulo": "La tercera", "por_que": "Pantalla roja"}, {"candidato": 1, "titulo": "La primera", "por_que": "Sonido fuerte"}, {"candidato": 99, "titulo": "No existe"}]});
    m.poner_claude(&format!("Aquí van:\n{respuesta}"));
    let r = m.correr(&m.video);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(m.llamada().exists(), "se llamó a Claude");

    let tarea = std::fs::read_dir(m.dir.path().join("tareas"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let cands: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(tarea.join("candidatos.json")).unwrap()).unwrap();
    let picos: Vec<f64> = cands.iter().map(|c| c["pico"].as_f64().unwrap()).collect();
    for t in JUGADAS {
        assert!(
            picos.iter().any(|p| (p - t as f64).abs() <= 4.0),
            "falta la jugada de {t} s entre {picos:?}"
        );
    }
    assert!(
        cands
            .iter()
            .all(|c| Path::new(c["miniatura"].as_str().unwrap()).is_file()),
        "cada candidato trae su tira"
    );

    let argv = std::fs::read_to_string(m.dir.path().join("argv")).unwrap();
    let a: Vec<&str> = argv.lines().collect();
    let tras = |k: &str| a[a.iter().position(|x| *x == k).unwrap() + 1];
    assert_eq!(tras("--tools"), "", "sin herramientas");
    assert_eq!(tras("--model"), "sonnet");
    assert!(a.contains(&"--restricted") && a.contains(&"--strict-mcp-config"));
    let entrada: Value = serde_json::from_str(
        std::fs::read_to_string(m.dir.path().join("entrada"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    let imagenes = entrada["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["type"] == "image")
        .count();
    assert_eq!(imagenes, cands.len(), "le llegan las miniaturas como imágenes");

    let p = m.proyecto("Partida prueba");
    let seq = &p["sequences"][0];
    assert_eq!(p["formato"], 6);
    assert_eq!(p["activa"], seq["id"]);
    assert_eq!(
        nombres(&p),
        ["La primera", "La tercera"],
        "en orden y sin el que no existe"
    );
    let (v1, a1) = (pista(&p, "V1"), pista(&p, "A1"));
    assert_eq!(v1[0]["start"], 0.0);
    assert_eq!(v1[1]["start"], v1[0]["duration"], "uno tras otro");
    assert_eq!(v1[0]["source"], "Partida prueba.mp4");
    assert!(v1[0]["in_point"].as_f64() < v1[1]["in_point"].as_f64());
    assert_eq!(
        a1.iter().map(|c| &c["link"]).collect::<Vec<_>>(),
        v1.iter().map(|c| &c["link"]).collect::<Vec<_>>(),
        "el audio va enlazado"
    );
    assert!(v1.iter().all(|c| !c["link"].as_str().unwrap().is_empty()));
    let marcas: Vec<(&str, &str)> = seq["markers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| (k["name"].as_str().unwrap(), k["note"].as_str().unwrap()))
        .collect();
    assert_eq!(
        marcas,
        [("La primera", "Sonido fuerte"), ("La tercera", "Pantalla roja")]
    );
    assert_eq!(
        (seq["width"].as_i64(), seq["height"].as_i64(), seq["fps"].as_f64()),
        (Some(320), Some(180), Some(24.0))
    );
}

#[test]
fn sin_claude_o_con_una_respuesta_rota_quedan_los_picos() {
    let Some(m) = mundo() else { return };
    // Sin Claude.
    let r = m.correr(&m.video);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let n = nombres(&m.proyecto("Partida prueba"));
    assert!((3..=5).contains(&n.len()) && n[0] == "Momento 1", "{n:?}");
    // Claude que contesta algo que no es JSON.
    m.poner_claude("no sé");
    let r = m.correr(&m.video);
    assert!(r.status.success());
    assert!(m.llamada().exists());
    let n = nombres(&m.proyecto("Partida prueba"));
    assert!(!n.is_empty() && n[0] == "Momento 1", "{n:?}");
}

#[test]
fn un_video_corto_va_entero_y_sin_audio_si_no_lo_tiene() {
    let Some(m) = mundo() else { return };
    let corto = m.videos().join("corto.mp4");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=24:d=12",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        corto.to_str().unwrap(),
    ]);
    let r = m.correr(&corto);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let p = m.proyecto("corto");
    let v1 = pista(&p, "V1");
    assert_eq!(v1.len(), 1);
    assert!((v1[0]["duration"].as_f64().unwrap() - 12.0).abs() < 0.2);
    assert!(pista(&p, "A1").is_empty());
}

#[test]
fn sin_video_o_sin_argumento_falla_con_codigo() {
    let Some(m) = mundo() else { return };
    let r = m.correr(&m.dir.path().join("no-existe.mp4"));
    assert_eq!(r.status.code(), Some(1));
    let r = Command::new(env!("CARGO_BIN_EXE_miku-aux"))
        .arg("momentos")
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(2));
}
