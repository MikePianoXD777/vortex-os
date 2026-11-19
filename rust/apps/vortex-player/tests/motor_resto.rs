//! El resto de `apps/vortex-player/pruebas/test_motor.py`: letras (elegir, la caché y el respaldo de YouTube
//! Music), «Tu resumen», lo más escuchado, lo nuevo de tus artistas y la clave para no recomendar dos veces.
//! Sin red y en carpetas temporales.

use serde_json::{Value, json};
use std::cell::RefCell;
use vortex_player::biblioteca::{self, Biblioteca};
use vortex_player::descargas::{self, Fallo};
use vortex_player::{fecha, letras};

const LRC: &str = "[00:01.00] Uno\n[00:02.50][00:06.00] Coro\n[00:04.2] Dos\n";

#[test]
fn limpiar_y_leer() {
    assert_eq!(letras::limpiar("YOASOBI - アイドル (Official Music Video)", "YOASOBI"), "アイドル");
    assert_eq!(letras::limpiar("Blinding Lights (Official Audio) ft. Nadie", "The Weeknd"), "Blinding Lights");
    assert_eq!(letras::limpiar_artista("Ado, Vaundy & otros"), "Ado");
    assert_eq!(letras::limpiar_artista("Aimer - Topic"), "Aimer");
    let lineas = letras::leer_lrc(LRC);
    let v: Vec<(i64, &str)> = lineas.iter().map(|l| (l.ms, l.texto.as_str())).collect();
    assert_eq!(v, [(1000, "Uno"), (2500, "Coro"), (4200, "Dos"), (6000, "Coro")]);
    let a: Vec<isize> = [0, 800, 2400, 5000, 9000].iter().map(|ms| letras::actual(&lineas, *ms)).collect();
    assert_eq!(a, [-1, 0, 1, 2, 3]);
}

#[test]
fn elegir() {
    let r = vec![
        json!({"duration": 300, "syncedLyrics": "[00:01.00] lejos"}),
        json!({"duration": 201, "plainLyrics": "plana"}),
        json!({"duration": 199, "syncedLyrics": LRC}),
    ];
    assert_eq!(letras::elegir(&r, 200, 4.0).unwrap()["duration"], 199); // la que dura lo mismo y sincronizada
    assert_eq!(letras::elegir(&r[..2], 200, 4.0).unwrap()["duration"], 201); // si no, la plana
    assert!(letras::elegir(&r[..1], 200, 4.0).is_none());
    assert!(letras::de_json(&r[2]).unwrap().sincronizada);
}

#[test]
fn de_guarda_y_respaldo() {
    let pedidos = RefCell::new(Vec::<String>::new());
    // None = LRCLIB no tiene nada; Err = sin internet
    let respuesta: RefCell<Result<Option<Value>, String>> =
        RefCell::new(Ok(Some(json!({"duration": 200, "syncedLyrics": LRC}))));
    let pedir = |url: &str| -> Result<Option<Value>, String> {
        pedidos.borrow_mut().push(url.to_string());
        match &*respuesta.borrow() {
            Err(e) => Err(e.clone()),
            Ok(None) => Ok(if url.contains("/get?") { None } else { Some(json!([])) }),
            Ok(Some(r)) => Ok(Some(if url.contains("/get?") { r.clone() } else { json!([r]) })),
        }
    };
    let buscar = |t: &str, a: &str, s: i64| letras::buscar_con(&pedir, t, a, s);
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("letras");
    let de = |titulo: &str, artista: &str, s: i64, vid: &str, yt: Option<letras::DeYoutube<'_>>| {
        letras::de_con(&buscar, &d, titulo, artista, s, vid, yt)
    };
    let l = de("Canción (Official Video)", "Alguien", 200, "", None).unwrap().unwrap();
    assert!(l.sincronizada);
    assert!(pedidos.borrow()[0].contains("track_name=Canci%C3%B3n"));
    let n = pedidos.borrow().len();
    assert_eq!(de("Canción (Official Video)", "Alguien", 200, "", None).unwrap(), Some(l)); // sin internet
    assert_eq!(pedidos.borrow().len(), n);
    // sin LRCLIB: la de YouTube Music (sin tiempos)
    *respuesta.borrow_mut() = Ok(None);
    let yt = |v: &str| Ok((v == "vid123").then(|| "línea 1\nlínea 2".to_string()));
    let l = de("Otra", "Alguien", 180, "vid123", Some(&yt)).unwrap().unwrap();
    let textos: Vec<&str> = l.lineas.iter().map(|x| x.texto.as_str()).collect();
    assert_eq!((l.sincronizada, textos), (false, vec!["línea 1", "línea 2"]));
    // no hay en ningún lado: se anota y no se vuelve a buscar
    assert_eq!(de("Nada", "Nadie", 100, "", None), Ok(None));
    let n = pedidos.borrow().len();
    assert_eq!(de("Nada", "Nadie", 100, "", None), Ok(None));
    assert_eq!(pedidos.borrow().len(), n);
    // sin internet: error y no se anota «nada»
    *respuesta.borrow_mut() = Err("sin red".into());
    assert!(de("Sin red", "Nadie", 100, "", None).is_err());
    *respuesta.borrow_mut() = Ok(Some(json!({"duration": 100, "plainLyrics": "ya hay"})));
    assert_eq!(de("Sin red", "Nadie", 100, "", None).unwrap().unwrap().lineas[0].texto, "ya hay");
    // el mismo archivo de caché que la de Python
    assert!(letras::guardada(&d, "Nada", "Nadie", 100));
    assert!(!letras::guardada(t.path(), "x", "y", 1));
}

#[test]
fn titulos_y_recomendadas_sin_repetir() {
    let t = letras::titulo_limpio;
    assert_eq!(t("YOASOBI「アイドル」Official Music Video", "YOASOBI"), "アイドル");
    assert_eq!(t("Kick Back - Official Video", "米津玄師"), "Kick Back");
    assert_eq!(t("Bad Apple!! feat.nomico", "Alstroemeria Records"), "Bad Apple!!");
    assert_eq!(t("Song Name | Lyric Video", "X"), "Song Name");
    assert_eq!(t("Video", "X"), "Video"); // no se queda vacío
    assert_eq!(letras::artista_limpio("AdoVEVO"), "Ado");
    assert_eq!(letras::artista_limpio("Ado, Vaundy"), "Ado, Vaundy");
    // la misma canción subida como video y como canción de YouTube Music no se recomienda dos veces
    assert_eq!(
        descargas::clave("YOASOBI - アイドル (Official Music Video)", "YOASOBI - Topic"),
        descargas::clave("アイドル", "YOASOBI")
    );
}

#[test]
fn semana_e_id_de_la_radio() {
    let jueves = fecha::marca_local(2026, 10, 29, 15, 30);
    let (desde, hasta) = fecha::semana(0, Some(jueves));
    let dia = |t: f64| {
        let l = fecha::local(t);
        (l.anio, l.mes, l.dia, l.hora, l.minuto)
    };
    assert_eq!(dia(desde), (2026, 10, 26, 0, 0));
    assert_eq!(dia(hasta), (2026, 11, 2, 0, 0));
    assert_eq!(dia(fecha::semana(1, Some(jueves)).0), (2026, 10, 19, 0, 0));
    assert_eq!(biblioteca::id_de("https://music.youtube.com/watch?v=ZRtdQ81jPUQ"), "ZRtdQ81jPUQ");
    assert_eq!(biblioteca::id_de("/m/Idol [ZRtdQ81jPUQ].opus"), "ZRtdQ81jPUQ");
    assert_eq!(biblioteca::id_de("/m/otra.flac"), "");
}

#[test]
fn resumen() {
    let t = tempfile::tempdir().unwrap();
    let b = Biblioteca::abrir(Some(&t.path().join("b.db")), "ffprobe").unwrap();
    let (desde, hasta) = fecha::semana(0, None);
    for (titulo, artista, seg, dia) in [
        ("Idol", "YOASOBI - Topic", 200.0, 0.0),
        ("Idol", "YOASOBI", 100.0, 1.0),
        ("Racing", "Ado, Otro", 60.0, 1.0),
        ("Fuera", "Nadie", 999.0, 8.0),
    ] {
        b.anotar_escucha_json(
            &json!({"ruta": format!("/m/{titulo}.opus"), "titulo": titulo, "artista": artista}),
            seg,
            Some(desde + dia * 86400.0 + 60.0),
        )
        .unwrap();
    }
    let r = b.resumen(desde, hasta, 5).unwrap();
    assert_eq!(
        json!([r["escuchas"], r["minutos"], r["distintas"], r["dias"]]),
        json!([3, 6, 2, 2])
    );
    let c: Vec<Value> = r["canciones"].as_array().unwrap().iter().map(|c| json!([c["titulo"], c["veces"]])).collect();
    assert_eq!(c, [json!(["Idol", 2]), json!(["Racing", 1])]);
    let a: Vec<Value> = r["artistas"].as_array().unwrap().iter().map(|a| json!([a["artista"], a["minutos"]])).collect();
    assert_eq!(a, [json!(["YOASOBI", 5]), json!(["Ado", 1])]);
    assert_eq!(b.resumen(hasta, hasta + 1.0, 5).unwrap()["escuchas"], 0);
}

#[test]
fn mas_escuchadas_y_artistas() {
    let t = tempfile::tempdir().unwrap();
    let archivo = t.path().join("b.db");
    let b = Biblioteca::abrir(Some(&archivo), "ffprobe").unwrap();
    {
        let db = rusqlite::Connection::open(&archivo).unwrap();
        for (ruta, titulo, artista) in [("/m/a.opus", "A", "Ado"), ("/m/b.opus", "B", "YOASOBI - Topic"), ("/m/c.opus", "C", "Ado")] {
            db.execute(
                "INSERT INTO canciones (ruta, titulo, artista, album, duracion, agregada, mtime) VALUES (?1,?2,?3,'',200,0,0)",
                rusqlite::params![ruta, titulo, artista],
            )
            .unwrap();
        }
    }
    assert_eq!(b.artistas_mas_escuchados(0.0, 5).unwrap(), ["Ado", "YOASOBI"]); // sin escuchas: lo que más tienes
    let ahora = 1_800_000_000.0;
    for (ruta, titulo, artista, veces) in [
        ("/m/b.opus", "B", "YOASOBI - Topic", 3),
        ("/m/a.opus", "A", "Ado", 1),
        ("/m/borrada.opus", "X", "Nadie", 5),
    ] {
        for _ in 0..veces {
            b.anotar_escucha_json(&json!({"ruta": ruta, "titulo": titulo, "artista": artista}), 60.0, Some(ahora))
                .unwrap();
        }
    }
    let titulos: Vec<String> = b.mas_escuchadas(ahora - 10.0, 15).unwrap().into_iter().map(|c| c.titulo).collect();
    assert_eq!(titulos, ["B", "A"]); // la borrada no
    assert!(b.mas_escuchadas(ahora + 10.0, 15).unwrap().is_empty());
    assert_eq!(b.artistas_mas_escuchados(ahora - 10.0, 2).unwrap(), ["Nadie", "YOASOBI"]);
}

#[test]
fn lanzamientos_de_youtube_music() {
    let item = |bid: &str, titulo: &str, sub: &str| {
        json!({"musicTwoRowItemRenderer": {
            "title": {"runs": [{"text": titulo}]}, "subtitle": {"runs": [{"text": sub}]},
            "navigationEndpoint": {"browseEndpoint": {"browseId": bid}},
            "thumbnailRenderer": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [
                {"url": "https://lh3.googleusercontent.com/x=w120-h120-l90"}]}}}}})
    };
    let pedir = |url: &str, cuerpo: &Value| -> Result<Value, Fallo> {
        if url.contains("search") {
            return Ok(json!({"contents": [{"musicResponsiveListItemRenderer": {
                "navigationEndpoint": {"browseEndpoint": {"browseId": "UCado"}}}}]}));
        }
        if cuerpo["browseId"] == "UCado" {
            return Ok(json!({"contents": [{"musicCarouselShelfRenderer": {"contents": [
                item("MPREb_viejo", "Kyougen", "Álbum • 2022"), item("MPREb_nuevo", "Hello", "Single • 2026"),
                item("MPREb_ep", "Ep", "EP • 2025"), item("VLPLotra", "Una playlist", "Playlist")]}}]}));
        }
        Ok(json!({"playlistId": "OLAK5uy_abc", "contents": [{"musicResponsiveListItemRenderer": {
            "playlistItemData": {"videoId": "aaaaaaaaaaa"},
            "flexColumns": [{"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Hello"}]}}}]}}]}))
    };
    assert_eq!(descargas::artista_id("Ado", &pedir).unwrap(), "UCado");
    let l = descargas::lanzamientos("UCado", "Ado", &pedir).unwrap();
    let v: Vec<Value> = l.iter().map(|x| json!([x["id"], x["tipo"], x["anio"]])).collect();
    assert_eq!(
        v,
        [json!(["MPREb_nuevo", "Single", 2026]), json!(["MPREb_ep", "EP", 2025]), json!(["MPREb_viejo", "Álbum", 2022])]
    );
    assert!(l[0]["portada"].as_str().unwrap().ends_with("=w320-h320-l90"));
    assert_eq!(
        descargas::canciones_de_lanzamiento("MPREb_nuevo", &pedir).unwrap(),
        ("OLAK5uy_abc".to_string(), vec![json!({"id": "aaaaaaaaaaa", "titulo": "Hello"})])
    );
}
