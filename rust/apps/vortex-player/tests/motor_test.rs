//! Pruebas del motor de Vortex Player portadas desde `apps/vortex-player/pruebas/test_motor.py`
//! y prueba de compatibilidad con una `biblioteca.db` creada por la app de Python.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde_json::json;
use tempfile::TempDir;
use vortex_player::biblioteca::{Biblioteca, ME_GUSTA, Orden};
use vortex_player::calidad;
use vortex_player::cola::{AccionAnterior, Cola, Pista, Repetir};
use vortex_player::mpv::{ErrorMpv, EventoMpv, MpvIpc, OpcionesCarga, construir_filtros, validar_ruta_carga};

fn exigir_binario(nombre: &str) {
    let ok = Command::new("which")
        .arg(nombre)
        .output()
        .is_ok_and(|o| o.status.success());
    assert!(ok, "falta el binario requerido `{nombre}` en PATH");
}

fn generar_tono(ruta: &Path, opciones: &[&str], titulo: &str, artista: &str) {
    if let Some(padre) = ruta.parent() {
        fs::create_dir_all(padre).unwrap();
    }
    let mut args = vec![
        "-nostdin",
        "-v",
        "quiet",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=1",
    ];
    args.extend_from_slice(opciones);
    let meta_titulo = format!("title={titulo}");
    let meta_artista = format!("artist={artista}");
    if !titulo.is_empty() {
        args.push("-metadata");
        args.push(&meta_titulo);
    }
    if !artista.is_empty() {
        args.push("-metadata");
        args.push(&meta_artista);
    }
    let ruta_str = ruta.to_string_lossy().to_string();
    args.push("--");
    args.push(&ruta_str);

    let estado = Command::new("ffmpeg")
        .stdin(std::process::Stdio::null())
        .args(&args)
        .status()
        .expect("no se pudo ejecutar ffmpeg");
    assert!(estado.success(), "ffmpeg falló generando {}", ruta.display());
}

#[test]
fn calidad_etiquetas_y_niveles() {
    assert_eq!(calidad::etiqueta("flac", 24, 96000, 0), "Hi-Res 24/96");
    assert_eq!(calidad::etiqueta("flac", 24, 44100, 0), "Hi-Res 24/44,1");
    assert_eq!(calidad::etiqueta("flac", 16, 44100, 0), "CD 16/44,1");
    assert_eq!(calidad::etiqueta("pcm_s16le", 16, 48000, 1536), "CD 16/48");
    assert_eq!(calidad::etiqueta("alac", 16, 192000, 0), "Hi-Res 16/192");
    assert_eq!(calidad::etiqueta("opus", 0, 48000, 160), "Opus 160 kbps");
    assert_eq!(calidad::etiqueta("aac", 0, 44100, 256), "AAC 256 kbps");
    assert_eq!(calidad::nivel("pcm_s24le", 24, 96000), "hires");
    assert_eq!(calidad::nivel("flac", 16, 44100), "cd");
    assert_eq!(calidad::nivel("mp3", 0, 44100), "comprimida");
}

#[test]
fn calidad_desde_json_ffprobe() {
    let j = json!({
        "streams": [
            {
                "codec_type": "audio",
                "codec_name": "flac",
                "sample_rate": "96000",
                "bits_per_raw_sample": "24",
                "tags": {"TITLE": "Canción"}
            },
            {"codec_type": "video", "codec_name": "mjpeg"}
        ],
        "format": {
            "duration": "201.5",
            "tags": {"ARTIST": "Alguien", "track": "3/12"}
        }
    });

    let info = calidad::de_ffprobe("/m/x.flac", &j).expect("debería parsear audio");
    assert_eq!(info.titulo, "Canción");
    assert_eq!(info.artista, "Alguien");
    assert_eq!(info.pista, 3);
    assert_eq!(info.bits, 24);
    assert_eq!(info.hz, 96000);
    assert!(info.portada);
    assert_eq!(info.etiqueta(), "Hi-Res 24/96");

    let solo_video = json!({"streams": [{"codec_type": "video"}]});
    assert!(calidad::de_ffprobe("/m/x.jpg", &solo_video).is_none());
}

#[test]
fn calidad_archivos_de_verdad() {
    exigir_binario("ffmpeg");
    exigir_binario("ffprobe");
    let tmp = TempDir::new().unwrap();
    let flac = tmp.path().join("a.flac");
    let wav = tmp.path().join("b.wav");
    let opus = tmp.path().join("c.opus");

    generar_tono(
        &flac,
        &["-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"],
        "",
        "",
    );
    generar_tono(&wav, &["-ar", "44100", "-c:a", "pcm_s16le"], "", "");
    generar_tono(&opus, &["-c:a", "libopus", "-b:a", "160k"], "", "");

    let hires = calidad::leer(&flac, "ffprobe").unwrap();
    let cd = calidad::leer(&wav, "ffprobe").unwrap();
    let lossy = calidad::leer(&opus, "ffprobe").unwrap();

    assert_eq!(hires.etiqueta(), "Hi-Res 24/96");
    assert_eq!(cd.etiqueta(), "CD 16/44,1");
    assert!(lossy.etiqueta().starts_with("Opus"));
    assert_eq!(lossy.nivel(), "comprimida");
    assert!(calidad::leer(&tmp.path().join("no-existe.flac"), "ffprobe").is_none());
}

#[test]
fn biblioteca_escanear_y_playlists() {
    exigir_binario("ffmpeg");
    exigir_binario("ffprobe");
    let tmp = TempDir::new().unwrap();
    let m = tmp.path().join("Música");
    let db_path = tmp.path().join("datos").join("biblioteca.db");

    let a = m.join("Álbum").join("uno.flac");
    let b = m
        .join("Vortex Player")
        .join("Banda")
        .join("Dos [abc12345678].opus");

    generar_tono(
        &a,
        &["-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"],
        "Uno",
        "Banda",
    );
    generar_tono(&b, &["-c:a", "libopus"], "Dos", "Banda");

    fs::create_dir_all(m.join(".oculta")).unwrap();
    generar_tono(&m.join(".oculta").join("x.flac"), &[], "", "");
    fs::write(m.join("notas.txt"), "no es audio").unwrap();

    let bib = Biblioteca::abrir(Some(&db_path), "ffprobe").unwrap();
    let carpetas = vec![m.clone()];
    let r = bib.escanear_carpetas(&carpetas).unwrap();
    assert_eq!((r.nuevas, r.total), (2, 2));

    let canciones = bib.canciones("", Orden::Agregada).unwrap();
    let por_titulo: std::collections::HashMap<_, _> =
        canciones.iter().map(|c| (c.titulo.as_str(), c)).collect();
    assert_eq!(por_titulo["Uno"].etiqueta, "Hi-Res 24/96");
    assert_eq!(por_titulo["Uno"].origen, "local");
    assert_eq!(por_titulo["Dos"].titulo, "Dos");

    let por_calidad: Vec<_> = bib
        .canciones("", Orden::Calidad)
        .unwrap()
        .into_iter()
        .map(|c| c.titulo)
        .collect();
    assert_eq!(por_calidad, vec!["Uno", "Dos"]);

    let buscadas: Vec<_> = bib
        .canciones("dos", Orden::Agregada)
        .unwrap()
        .into_iter()
        .map(|c| c.titulo)
        .collect();
    assert_eq!(buscadas, vec!["Dos"]);

    assert_eq!(bib.escanear_carpetas(&carpetas).unwrap().nuevas, 0);

    let pid = bib
        .crear_playlist("Para correr", "https://open.spotify.com/playlist/X")
        .unwrap();
    assert_eq!(
        bib.crear_playlist("Otra vez", "https://open.spotify.com/playlist/X")
            .unwrap(),
        pid
    );

    let a_str = a.to_string_lossy().to_string();
    let b_str = b.to_string_lossy().to_string();
    bib.agregar_a_playlist(pid, &b_str).unwrap();
    bib.agregar_a_playlist(pid, &a_str).unwrap();
    bib.agregar_a_playlist(pid, &a_str).unwrap();

    let de_pl: Vec<_> = bib
        .canciones_de_playlist(pid)
        .unwrap()
        .into_iter()
        .map(|c| c.titulo)
        .collect();
    assert_eq!(de_pl, vec!["Dos", "Uno"]);
    assert_eq!(bib.playlists().unwrap()[0].canciones, 2);

    bib.quitar_de_playlist(pid, &b_str).unwrap();
    let de_pl_tras_quitar: Vec<_> = bib
        .canciones_de_playlist(pid)
        .unwrap()
        .into_iter()
        .map(|c| c.titulo)
        .collect();
    assert_eq!(de_pl_tras_quitar, vec!["Uno"]);

    fs::remove_file(&a).unwrap();
    assert_eq!(bib.escanear_carpetas(&carpetas).unwrap().borradas, 1);
    let restantes: Vec<_> = bib
        .canciones("", Orden::Agregada)
        .unwrap()
        .into_iter()
        .map(|c| c.titulo)
        .collect();
    assert_eq!(restantes, vec!["Dos"]);

    bib.borrar_playlist(pid, true).unwrap();
    assert!(bib.playlists().unwrap().is_empty());
}

#[test]
fn biblioteca_ajustes_y_otras_carpetas() {
    exigir_binario("ffmpeg");
    exigir_binario("ffprobe");
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config");
    let db_path = tmp.path().join("datos").join("biblioteca.db");
    let otra = tmp.path().join("Disco externo");
    generar_tono(
        &otra.join("tres.wav"),
        &["-c:a", "pcm_s24le", "-ar", "48000"],
        "",
        "",
    );

    // Una clave que no es de la biblioteca (la guarda el puente) no se pierde al guardar otra.
    Biblioteca::guardar_ajuste_en(&cfg, "discord", serde_json::json!(false)).unwrap();
    Biblioteca::guardar_ajuste_en(&cfg, "carpetas", serde_json::json!([otra.to_string_lossy()])).unwrap();
    let leidos = Biblioteca::ajustes_en(&cfg);
    assert_eq!(leidos.volumen(), 80);
    assert_eq!(leidos.valor("discord"), Some(&serde_json::json!(false)));

    let bib = Biblioteca::abrir(Some(&db_path), "ffprobe").unwrap();
    let carpetas: Vec<PathBuf> = leidos.carpetas().into_iter().map(PathBuf::from).collect();
    bib.escanear_carpetas(&carpetas).unwrap();
    assert_eq!(
        bib.canciones("", Orden::Agregada).unwrap()[0].etiqueta,
        "Hi-Res 24/48"
    );
}

#[test]
fn biblioteca_hecha_por_python() {
    exigir_binario("python3");
    exigir_binario("ffmpeg");
    exigir_binario("ffprobe");
    let raiz_player = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("apps/vortex-player");
    assert!(
        raiz_player.is_dir(),
        "no se encontró apps/vortex-player en {}",
        raiz_player.display()
    );

    let tmp = TempDir::new().unwrap();
    let musica = tmp.path().join("Música");
    let datos = tmp.path().join("datos");
    let config = tmp.path().join("config");
    let cache = tmp.path().join("cache");

    let flac = musica.join("Álbum").join("cancion_py.flac");
    generar_tono(
        &flac,
        &["-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"],
        "Desde Python",
        "YOASOBI - Topic",
    );
    let yt = musica
        .join("Vortex Player")
        .join("YOASOBI - アイドル (Official Music Video) [ZRtdQ81jPUQ].opus");
    generar_tono(
        &yt,
        &["-c:a", "libopus"],
        "YOASOBI - アイドル (Official Music Video)",
        "YOASOBI - Topic",
    );

    let script = r#"
from vortex_player.biblioteca import Biblioteca
import sys
bib = Biblioteca()
bib.escanear()
pid = bib.crear_playlist("Lista Py")
for c in bib.canciones():
    bib.agregar_a_playlist(pid, c["ruta"])
    if c["titulo"] == "Desde Python":
        bib.alternar_me_gusta(c["ruta"])
bib.cerrar()
"#;

    let out = Command::new("python3")
        .args(["-c", script])
        .env("PYTHONPATH", &raiz_player)
        .env("VORTEX_PLAYER_MUSICA", &musica)
        .env("VORTEX_PLAYER_DATOS", &datos)
        .env("VORTEX_PLAYER_CONFIG", &config)
        .env("VORTEX_PLAYER_CACHE", &cache)
        .output()
        .expect("falló ejecutando python3");
    assert!(
        out.status.success(),
        "Python falló: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let db_path = datos.join("biblioteca.db");
    assert!(db_path.is_file());

    let bib_rs = Biblioteca::abrir(Some(&db_path), "ffprobe").unwrap();
    let canciones = bib_rs.canciones("", Orden::Calidad).unwrap();
    assert_eq!(canciones.len(), 2);
    assert_eq!(canciones[0].titulo, "Desde Python");
    assert_eq!(canciones[0].etiqueta, "Hi-Res 24/96");
    assert_eq!(canciones[1].titulo, "アイドル");
    assert_eq!(canciones[1].artista, "YOASOBI");

    let pls = bib_rs.playlists().unwrap();
    assert_eq!(pls.len(), 2);
    assert_eq!(pls[0].nube, ME_GUSTA);
    assert!(pls[0].me_gusta);
    assert_eq!(pls[1].nombre, "Lista Py");
    assert_eq!(pls[1].canciones, 2);

    let me_gusta = bib_rs.me_gusta().unwrap();
    assert_eq!(me_gusta, vec![flac.to_string_lossy().to_string()]);
}

#[test]
fn cola_orden_despues_y_repeticion() {
    let p = |n: &str| Pista {
        ruta: format!("/m/{n}.flac"),
        titulo: n.to_string(),
        artista: "Artista".into(),
        album: "Álbum".into(),
        duracion: 180.0,
    };

    let mut cola = Cola::nueva();
    cola.tocar(vec![p("1"), p("2"), p("3")], 0);
    assert_eq!(cola.actual().unwrap().titulo, "1");

    cola.tocar_despues(p("A"));
    cola.tocar_despues(p("B"));
    cola.agregar(p("Z"));

    let titulos: Vec<_> = cola
        .lista_ordenada()
        .into_iter()
        .map(|x| x.titulo.as_str())
        .collect();
    assert_eq!(titulos, vec!["1", "A", "B", "2", "3", "Z"]);

    assert_eq!(cola.siguiente().unwrap().titulo, "A");
    assert_eq!(cola.anterior(4.0), AccionAnterior::ReiniciarPosicion);
    assert_eq!(cola.anterior(1.0), AccionAnterior::CargarPista);
    assert_eq!(cola.actual().unwrap().titulo, "1");

    cola.alternar_repetir();
    assert_eq!(cola.repetir(), Repetir::Todo);
    cola.alternar_repetir();
    assert_eq!(cola.repetir(), Repetir::Una);
    assert_eq!(cola.avanzar_automatico().unwrap().titulo, "1");

    assert!(cola.quitar_en(1));
    let sesion = cola.sesion(12.345);
    assert_eq!(sesion.posicion, 12.35);
    assert_eq!(sesion.repetir, "una");
}

#[test]
fn mpv_rechaza_esquemas_peligrosos() {
    assert!(validar_ruta_carga("/musica/cancion.flac").is_ok());
    assert!(validar_ruta_carga("https://music.youtube.com/watch?v=dQw4w9WgXcQ").is_ok());
    assert!(validar_ruta_carga("http://localhost:8080/stream.opus").is_ok());

    for mala in [
        "file:///etc/passwd",
        "ftp://servidor/cancion.mp3",
        "smb://servidor/compartida/a.flac",
        "edl://%0%no",
        "relativa/cancion.mp3",
        "",
    ] {
        assert!(
            matches!(validar_ruta_carga(mala), Err(ErrorMpv::RutaNoPermitida(_))),
            "debió rechazar {mala}"
        );
    }
}

#[test]
fn mpv_filtros_carpeta_privada_e_ipc_sin_sonido() {
    assert_eq!(construir_filtros("", 1.0, None), "");
    assert_eq!(
        construir_filtros("lavfi=[equalizer=f=63:t=o:w=1:g=5]", 1.25, Some(3.0)),
        "lavfi=[equalizer=f=63:t=o:w=1:g=5],scaletempo2,lavfi=[afade=t=in:d=3.0]"
    );

    exigir_binario("mpv");
    exigir_binario("ffmpeg");
    let tmp = TempDir::new().unwrap();
    let pista = tmp.path().join("prueba.wav");
    generar_tono(&pista, &["-ar", "44100", "-c:a", "pcm_s16le"], "Tono", "");

    let mut mpv = MpvIpc::iniciar_con(80, Some("null")).expect("mpv debería iniciar con --ao=null");

    assert!(matches!(
        mpv.cargar("file:///etc/passwd", OpcionesCarga::default()),
        Err(ErrorMpv::RutaNoPermitida(_))
    ));

    #[cfg(unix)]
    {
        let sock = PathBuf::from(mpv.ipc_ruta());
        let dir = sock.parent().unwrap();
        let modo = fs::metadata(dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o700, "la carpeta del socket debe ser 0700");
    }

    mpv.cargar(&pista.to_string_lossy(), OpcionesCarga::default())
        .unwrap();

    let mut vio_cambio = false;
    for _ in 0..20 {
        for ev in mpv.leer_eventos() {
            if matches!(ev, EventoMpv::CambioPropiedad { .. }) {
                vio_cambio = true;
            }
        }
        if vio_cambio {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(vio_cambio, "mpv debería emitir eventos por IPC");
    mpv.cerrar();
}
