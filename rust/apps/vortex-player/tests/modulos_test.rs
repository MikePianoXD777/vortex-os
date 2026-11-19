//! Pruebas de ecualizador, letras, descargas, nube, audio, paleta de `vortex-core`,
//! Discord y «Tus me gusta».

use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use serde_json::json;
use tempfile::TempDir;
use vortex_core::paleta;
use vortex_player::biblioteca::{Biblioteca, ME_GUSTA, Orden};
use vortex_player::ecualizador::{self, Banda};
use vortex_player::nube::{CancionNube, Local, PlaylistNube};
use vortex_player::{audio, discord, letras, nube};

fn exigir_binario(nombre: &str) {
    let ok = Command::new("which")
        .arg(nombre)
        .output()
        .is_ok_and(|o| o.status.success());
    assert!(ok, "falta el binario requerido `{nombre}` en PATH");
}

#[test]
fn ecualizador_perfiles_y_mpv() {
    let tmp = TempDir::new().unwrap();
    let eq_json = tmp.path().join("ecualizador.json");

    assert_eq!(
        ecualizador::filtro_mpv(&ecualizador::estado_en(&eq_json).bandas),
        ""
    );

    assert!(ecualizador::poner_preset_mpv_en(&eq_json, "Bass"));
    assert_eq!(ecualizador::estado_en(&eq_json).preset, "Bass");

    assert!(ecualizador::poner_bandas_mpv_en(
        &eq_json,
        &[Banda { f: 31.0, g: 3.0 }, Banda { f: 1000.0, g: -2.0 }]
    ));
    assert_eq!(
        ecualizador::filtro_mpv(&ecualizador::estado_en(&eq_json).bandas),
        "lavfi=[volume=volume=-3dB,equalizer=f=31:t=o:w=1:g=3,equalizer=f=1000:t=o:w=1:g=-2]"
    );
    assert!(!ecualizador::poner_bandas_mpv_en(
        &eq_json,
        &[Banda { f: 100.0, g: 0.0 }]
    ));

    let diez: Vec<Banda> = ecualizador::FRECUENCIAS
        .iter()
        .map(|&f| Banda { f: f as f64, g: 0.0 })
        .collect();
    for &(nombre, valores) in ecualizador::PERFILES {
        let ajustadas = ecualizador::bandas_de_perfil(nombre, &diez);
        let gs: Vec<f64> = ajustadas.iter().map(|b| b.g).collect();
        assert_eq!(gs, valores.to_vec(), "perfil {nombre}");
        assert_eq!(ecualizador::perfil_de(&ajustadas), nombre);
    }

    let tres = ecualizador::bandas_de_perfil(
        "Rock",
        &[
            Banda { f: 60.0, g: 0.0 },
            Banda { f: 1000.0, g: 0.0 },
            Banda { f: 12000.0, g: 0.0 },
        ],
    );
    assert_eq!(ecualizador::perfil_de(&tres), "Rock");
    assert_eq!(
        ecualizador::perfil_de(&[Banda { f: 60.0, g: 1.0 }, Banda { f: 1000.0, g: 9.0 }]),
        "Personalizado"
    );

    let viejo = json!({"b1": "5", "b2": "7", "b10": "2", "preset": "Bass"});
    let b = ecualizador::bandas_de(&viejo);
    assert_eq!(b.len(), 10);
    assert_eq!(b[0], Banda { f: 31.0, g: 5.0 });
    assert_eq!(b[1].g, 7.0);
    assert_eq!(b[9].g, 2.0);
    assert_eq!(b[4].g, 0.0);

    let nuevo = json!({"bandas": [{"f": 50, "g": 3}, {"f": 9000, "g": -20}]});
    assert_eq!(
        ecualizador::texto_bandas(&ecualizador::bandas_de(&nuevo)),
        "50:3,9000:-12"
    );
}

#[test]
fn letras_limpieza_y_lrc() {
    assert_eq!(
        letras::limpiar("YOASOBI - アイドル (Official Music Video)", "YOASOBI"),
        "アイドル"
    );
    assert_eq!(
        letras::limpiar("Blinding Lights (Official Audio) ft. Nadie", "The Weeknd"),
        "Blinding Lights"
    );
    assert_eq!(letras::limpiar_artista("Ado, Vaundy & otros"), "Ado");
    assert_eq!(letras::limpiar_artista("Aimer - Topic"), "Aimer");

    assert_eq!(
        letras::titulo_limpio("YOASOBI「アイドル」Official Music Video", "YOASOBI"),
        "アイドル"
    );
    assert_eq!(
        letras::titulo_limpio("Kick Back - Official Video", "米津玄師"),
        "Kick Back"
    );
    assert_eq!(
        letras::titulo_limpio("Bad Apple!! feat.nomico", "Alstroemeria Records"),
        "Bad Apple!!"
    );
    assert_eq!(letras::titulo_limpio("Song Name | Lyric Video", "X"), "Song Name");
    assert_eq!(letras::titulo_limpio("Video", "X"), "Video");
    assert_eq!(letras::artista_limpio("AdoVEVO"), "Ado");
    assert_eq!(letras::artista_limpio("Ado, Vaundy"), "Ado, Vaundy");

    let lrc = "[00:01.00] Uno\n[00:02.50][00:06.00] Coro\n[00:04.2] Dos\n";
    let lineas = letras::leer_lrc(lrc);
    let pares: Vec<(i64, &str)> = lineas.iter().map(|l| (l.ms, l.texto.as_str())).collect();
    assert_eq!(
        pares,
        vec![(1000, "Uno"), (2500, "Coro"), (4200, "Dos"), (6000, "Coro")]
    );
    let activos: Vec<isize> = [0, 800, 2400, 5000, 9000]
        .into_iter()
        .map(|ms| letras::actual(&lineas, ms))
        .collect();
    assert_eq!(activos, vec![-1, 0, 1, 2, 3]);
}

#[test]
fn nube_mezcla_y_plan_compatible_con_android() {
    let cancion = |vid: &str, t: i64| CancionNube {
        titulo: format!("T{vid}"),
        artista: "A".into(),
        album: String::new(),
        duracion: 200,
        agregada: t,
    };

    let mut pc_local = Local::default();
    pc_local.canciones.insert("aaaaaaaaaaa".into(), cancion("a", 10));
    pc_local.canciones.insert("bbbbbbbbbbb".into(), cancion("b", 20));
    let mut tel_local = Local::default();
    tel_local.canciones.insert("ccccccccccc".into(), cancion("c", 30));

    let pc = nube::desde(&pc_local);
    let tel = nube::desde(&tel_local);
    let m = nube::mezclar(&pc, &tel);
    assert_eq!(m.canciones.len(), 3);
    assert!(nube::iguales(&m, &nube::mezclar(&tel, &pc)));

    let mut quitadas = Local::default();
    quitadas.quitadas.insert("aaaaaaaaaaa".into(), 50);
    quitadas.quitadas.insert("bbbbbbbbbbb".into(), 50);
    let mut revive = Local::default();
    revive.canciones.insert("bbbbbbbbbbb".into(), cancion("b", 90));
    let m2 = nube::mezclar(
        &nube::mezclar(&pc, &nube::desde(&quitadas)),
        &nube::desde(&revive),
    );
    assert_eq!(
        m2.canciones.keys().cloned().collect::<Vec<_>>(),
        vec!["bbbbbbbbbbb"]
    );
    assert_eq!(m2.quitadas.get("aaaaaaaaaaa"), Some(&50));

    let tel_json = r#"{"formato":1,"canciones":{"aaaaaaaaaaa":{"titulo":"Idol","artista":"YOASOBI","album":"","duracion":213,"agregada":1793472533000}},"quitadas":{},"playlists":{"u1":{"nombre":"Anime","url":"","canciones":["aaaaaaaaaaa"],"modificada":1793472533000}},"borradas":{}}"#;
    let d = nube::leer(tel_json);
    assert_eq!(d.canciones["aaaaaaaaaaa"].artista, "YOASOBI");
    assert!(nube::iguales(&nube::leer("basura"), &nube::vacio()));
    assert!(nube::iguales(&nube::leer("[1, 2]"), &nube::vacio()));

    let mut doc_p = BTreeMap::new();
    doc_p.insert(
        "u1".into(),
        PlaylistNube {
            nombre: "Chill".into(),
            url: String::new(),
            canciones: vec!["aaaaaaaaaaa".into()],
            modificada: 10,
        },
    );
    let plan = nube::plan(
        &nube::DocNube {
            formato: 1,
            canciones: d.canciones.clone(),
            quitadas: BTreeMap::new(),
            playlists: doc_p,
            borradas: BTreeMap::new(),
        },
        &Local::default(),
    );
    assert_eq!(plan.bajar.len(), 1);
    assert_eq!(plan.playlists.len(), 1);
}

#[test]
fn audio_configs_y_eventos_pactl() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("xdg");

    assert!(audio::cambia_la_salida("Event 'change' on sink #48\n"));
    assert!(audio::cambia_la_salida("Evento «change» en server #0\n"));
    assert!(audio::cambia_la_salida("Event 'new' on card #71\n"));
    assert!(!audio::cambia_la_salida("Event 'change' on sink-input #120\n"));

    assert!(audio::config_hires_en(&cfg));
    assert!(!audio::config_hires_en(&cfg));
    assert!(
        fs::read_to_string(audio::archivo_hires_en(&cfg))
            .unwrap()
            .contains("96000")
    );
    assert!(audio::config_ldac_en(&cfg, true));
    assert!(
        fs::read_to_string(audio::archivo_ldac_en(&cfg))
            .unwrap()
            .contains("ldac.quality = \"hq\"")
    );
    assert!(audio::config_ldac_en(&cfg, false));
    assert!(!audio::archivo_ldac_en(&cfg).exists());
    assert!(!audio::config_ldac_en(&cfg, false));
}

#[test]
fn colores_desde_vortex_core() {
    let resp = paleta::respaldo();
    assert_eq!(resp["fondo"], "#0b0d1a");

    let tmp = TempDir::new().unwrap();
    let inexistente = tmp.path().join("qs_colors.json");
    assert_eq!(paleta::leer(&inexistente), resp);
}

#[test]
fn discord_marcos_y_actividad() {
    let mut datos = discord::marco(1, &json!({"a": "ñandú"}));
    datos.extend_from_slice(&discord::marco(3, &json!({"b": 2})));
    let (marcos, resto) = discord::leer_marcos(&datos);
    assert_eq!(marcos, vec![(1, json!({"a": "ñandú"})), (3, json!({"b": 2}))]);
    assert!(resto.is_empty());

    let (m1, r1) = discord::leer_marcos(&datos[..datos.len() - 2]);
    assert_eq!(m1, vec![(1, json!({"a": "ñandú"}))]);
    let mut unido = r1;
    unido.extend_from_slice(&datos[datos.len() - 2..]);
    let (m2, _) = discord::leer_marcos(&unido);
    assert_eq!(m2, vec![(3, json!({"b": 2}))]);

    let act = discord::actividad(
        &json!({"ruta": "/música/Bohemia [dQw4w9WgXcQ].opus", "titulo": "Bohemia", "artista": "Queen", "album": "Opera"}),
        10.0,
        100.0,
        1.0,
        1000.0,
    );
    assert_eq!(act["type"], 2);
    assert_eq!(act["details"], "Bohemia");
    assert_eq!(act["state"], "Queen");
    assert_eq!(
        act["assets"]["large_image"],
        "https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg"
    );
}

#[test]
fn biblioteca_me_gusta_y_titulos_limpios() {
    exigir_binario("ffmpeg");
    exigir_binario("ffprobe");
    let tmp = TempDir::new().unwrap();
    let m = tmp.path().join("Música");
    let db_path = tmp.path().join("datos").join("biblioteca.db");
    fs::create_dir_all(m.join("Vortex Player")).unwrap();

    let a = m.join("uno.flac");
    let b = m
        .join("Vortex Player")
        .join("YOASOBI - アイドル (Official Music Video) [ZRtdQ81jPUQ].opus");
    for (ruta, codec, tit, art) in [
        (&a, "flac", "Uno", "Banda"),
        (
            &b,
            "libopus",
            "YOASOBI - アイドル (Official Music Video)",
            "YOASOBI - Topic",
        ),
    ] {
        let ok = Command::new("ffmpeg")
            .stdin(std::process::Stdio::null())
            .args([
                "-nostdin",
                "-v",
                "quiet",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:a",
                codec,
                "-metadata",
                &format!("title={tit}"),
                "-metadata",
                &format!("artist={art}"),
                "--",
            ])
            .arg(ruta)
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }

    let bib = Biblioteca::abrir(Some(&db_path), "ffprobe").unwrap();
    bib.escanear_carpetas(&[m]).unwrap();
    let mut pares: Vec<_> = bib
        .canciones("", Orden::Agregada)
        .unwrap()
        .into_iter()
        .map(|c| (c.titulo, c.artista))
        .collect();
    pares.sort();
    assert_eq!(
        pares,
        vec![
            ("Uno".to_string(), "Banda".to_string()),
            ("アイドル".to_string(), "YOASOBI".to_string())
        ]
    );

    assert_eq!(bib.id_me_gusta(false).unwrap(), -1);
    let otra = bib.crear_playlist("Otra", "").unwrap();
    let a_s = a.to_string_lossy().to_string();
    let b_s = b.to_string_lossy().to_string();
    assert!(bib.alternar_me_gusta(&a_s).unwrap());
    assert!(bib.alternar_me_gusta(&b_s).unwrap());
    assert_eq!(bib.me_gusta().unwrap(), vec![b_s.clone(), a_s.clone()]);
    assert!(!bib.alternar_me_gusta(&a_s).unwrap());
    assert_eq!(bib.me_gusta().unwrap(), vec![b_s]);
    assert!(bib.estado_nube().unwrap().playlists.contains_key(ME_GUSTA));
    assert_ne!(otra, bib.id_me_gusta(false).unwrap());
}
