//! Las pruebas `Descargas` y `Buscar` de `apps/vortex-player/pruebas/test_motor.py`: sin red, con yt-dlp, la
//! página de Spotify y las respuestas de YouTube Music de mentira, en carpetas temporales.

use serde_json::{Value, json};
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use vortex_player::descargas::{self, Fallo};

fn pagina_spotify() -> String {
    let datos = json!({"props": {"pageProps": {"state": {"data": {"entity": {
        "type": "playlist", "name": "Mis favoritas",
        "trackList": [{"title": "Kick Back", "subtitle": "Kenshi Yonezu", "duration": 193000},
                      {"title": "Idol", "subtitle": "YOASOBI, Otro", "duration": 213000},
                      {"title": "", "subtitle": "nadie"}]}}}}}});
    format!(r#"<html><script id="__NEXT_DATA__" type="application/json">{datos}</script></html>"#)
}

fn ejecutable(ruta: &Path, texto: &str) -> PathBuf {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(ruta, format!("#!/usr/bin/env bash\n{texto}")).unwrap();
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o755)).unwrap();
    ruta.to_path_buf()
}

fn t3(a: &str, b: &str, c: &str) -> Option<(String, String, String)> {
    Some((a.into(), b.into(), c.into()))
}

#[test]
fn tipo_de() {
    let t = descargas::tipo_de;
    assert_eq!(
        t("https://open.spotify.com/playlist/37i9dQZF1DX?si=1"),
        t3("spotify", "playlist", "37i9dQZF1DX")
    );
    assert_eq!(
        t("https://open.spotify.com/intl-es/track/abc123"),
        t3("spotify", "track", "abc123")
    );
    assert_eq!(t("spotify:album:XYZ"), t3("spotify", "album", "XYZ"));
    assert_eq!(
        t("https://music.youtube.com/watch?v=dQw4&list=RD1"),
        t3("youtube", "cancion", "dQw4")
    );
    assert_eq!(t("https://youtu.be/dQw4"), t3("youtube", "cancion", "dQw4"));
    assert_eq!(
        t("music.youtube.com/playlist?list=PL9"),
        t3("youtube", "playlist", "PL9")
    );
    assert_eq!(
        t("https://music.youtube.com/browse/MPREb_x"),
        t3("youtube", "playlist", "MPREb_x")
    );
    assert_eq!(t("https://example.com/cancion.mp3"), None);
    assert_eq!(t("hola"), None);
}

#[test]
fn enlaces_playlist() {
    let ids: Vec<String> = ["ZRtdQ81jPUQ", "", "dQw4w9WgXcQ", "ZRtdQ81jPUQ"]
        .map(String::from)
        .to_vec();
    let uno = descargas::enlaces_playlist("Para Ana & co", &ids);
    assert_eq!(
        uno,
        [
            "https://www.youtube.com/watch_videos?video_ids=ZRtdQ81jPUQ,dQw4w9WgXcQ&title=Para%20Ana%20%26%20co"
        ]
    );
    assert_eq!(
        descargas::tipo_de(&uno[0]),
        t3("youtube", "playlist", "ZRtdQ81jPUQ,dQw4w9WgXcQ")
    );
    let muchos =
        descargas::enlaces_playlist("Larga", &(0..120).map(|i| format!("{i:011}")).collect::<Vec<_>>());
    assert_eq!(muchos.len(), 3);
    assert!(muchos[0].ends_with("&title=Larga%20%281%2F3%29"));
    let cuantos: Vec<usize> = muchos
        .iter()
        .map(|e| descargas::tipo_de(e).unwrap().2.matches(',').count() + 1)
        .collect();
    assert_eq!(cuantos, [50, 50, 20]);
    assert!(descargas::enlaces_playlist("Vacía", &[String::new(), String::new()]).is_empty());
}

#[test]
fn spotify() {
    let (nombre, pistas) = descargas::leer_spotify(&pagina_spotify()).unwrap();
    assert_eq!(nombre, "Mis favoritas");
    assert_eq!(
        pistas,
        [
            json!({"titulo": "Kick Back", "artistas": "Kenshi Yonezu", "duracion": 193}),
            json!({"titulo": "Idol", "artistas": "YOASOBI, Otro", "duracion": 213})
        ]
    );
    assert_eq!(
        descargas::busqueda_ytmusic(&pistas[1]),
        "https://music.youtube.com/search?q=YOASOBI%20Idol#songs"
    );
    assert!(descargas::leer_spotify("<html></html>").is_err());
}

#[test]
fn comando_sin_recomprimir() {
    let c = descargas::comando("https://youtu.be/x", Path::new("/m"), "firefox", true, "yt-dlp");
    assert!(c.contains(&"-x".to_string()));
    assert!(!c.contains(&"--audio-format".to_string()));
    let i = c.iter().position(|x| x == "--cookies-from-browser").unwrap();
    assert_eq!(c[i + 1], "firefox");
    assert!(c.contains(&"--no-playlist".to_string()));
    assert_eq!(c[c.len() - 2..], ["--", "https://youtu.be/x"]);
    assert!(
        !descargas::comando("u", Path::new("/m"), "", false, "yt-dlp")
            .contains(&"--cookies-from-browser".to_string())
    );
    assert!(
        descargas::comando(
            "https://music.youtube.com/search?q=a#songs",
            Path::new("/m"),
            "",
            false,
            "yt-dlp"
        )
        .contains(&"--playlist-items".to_string())
    );
    // La portada queda en un .jpg junto a la canción (de ahí la toma la ventana)
    assert!(c.windows(2).any(|w| w == ["--convert-thumbnails", "jpg"]));
}

#[test]
fn leer_linea() {
    assert_eq!(
        descargas::leer_linea("VORTEXA\t/m/a b.opus\n"),
        Some(json!({"tipo": "archivo", "ruta": "/m/a b.opus"}))
    );
    assert_eq!(
        descargas::leer_linea("VORTEXP\t 42.5%\tIdol"),
        Some(json!({"tipo": "progreso", "porcentaje": 42.5, "titulo": "Idol"}))
    );
    assert!(
        descargas::leer_linea("ERROR: [youtube] x: Private video").unwrap()["texto"]
            .as_str()
            .unwrap()
            .contains("privada")
    );
    assert_eq!(descargas::leer_linea("[download] Destination: x"), None);
}

/// Baja «algo» por cada búsqueda o URL; con «falla» en la búsqueda no encuentra nada.
fn yt_dlp_falso(t: &Path) -> PathBuf {
    let d = t.display();
    ejecutable(
        &t.join("bin/yt-dlp"),
        &format!(
            r#"echo "$@" >>"{d}/llamadas"
obj="${{@: -1}}"
if [[ " $* " == *" -J "* ]]; then echo '{{"title": "Album - Lo mejor", "entries": [{{}}, {{}}]}}'; exit 0; fi
[[ "$obj" == *falla* ]] && {{ echo "ERROR: [youtube] x: Video unavailable"; exit 1; }}
printf 'VORTEXP\t 50.0%%\tCancion\n'
n=$(wc -l <"{d}/llamadas")
printf 'VORTEXA\t/m/cancion%s.opus\n' "$n"
[[ "$*" == *--yes-playlist* && "$*" != *--playlist-items* ]] && printf 'VORTEXA\t/m/otra.opus\n'
exit 0
"#
        ),
    )
}

fn bajar_todo(url: &str, destino: &Path, yt: &str, pagina: Option<String>) -> Vec<Value> {
    let mut evs = Vec::new();
    let bajar = move |_: &str| pagina.clone().ok_or_else(|| "sin red".to_string());
    descargas::descargar(url, destino, "", yt, &bajar, &|| false, &mut |e| evs.push(e));
    evs
}

#[test]
fn descargar_spotify() {
    let t = tempfile::tempdir().unwrap();
    let yt = yt_dlp_falso(t.path());
    let pagina = pagina_spotify().replace("Idol", "Idol falla");
    let evs = bajar_todo(
        "https://open.spotify.com/playlist/AAA",
        &t.path().join("dest"),
        yt.to_str().unwrap(),
        Some(pagina),
    );
    assert_eq!(
        evs[0],
        json!({"tipo": "inicio", "nombre": "Mis favoritas", "total": 2, "es_lista": true})
    );
    let archivos: Vec<&Value> = evs
        .iter()
        .filter(|e| e["tipo"] == "archivo")
        .map(|e| &e["ruta"])
        .collect();
    assert_eq!(archivos, [&json!("/m/cancion1.opus")]);
    assert!(evs.iter().any(|e| {
        e["texto"]
            .as_str()
            .unwrap_or("")
            .contains("No encontré «Idol falla»")
    }));
    assert_eq!(
        evs.last().unwrap(),
        &json!({"tipo": "fin", "bajadas": 1, "fallidas": 1})
    );
    let llamadas = std::fs::read_to_string(t.path().join("llamadas")).unwrap();
    assert!(llamadas.contains("music.youtube.com/search?q=Kenshi%20Yonezu%20Kick%20Back#songs"));
}

#[test]
fn descargar_youtube() {
    let t = tempfile::tempdir().unwrap();
    let yt = yt_dlp_falso(t.path());
    let yt = yt.to_str().unwrap();
    let evs = bajar_todo(
        "https://music.youtube.com/playlist?list=PL1",
        &t.path().join("dest"),
        yt,
        None,
    );
    assert_eq!(
        evs[0],
        json!({"tipo": "inicio", "nombre": "Lo mejor", "total": 2, "es_lista": true})
    );
    let is: Vec<&Value> = evs
        .iter()
        .filter(|e| e["tipo"] == "archivo")
        .map(|e| &e["i"])
        .collect();
    assert_eq!(is, [&json!(0), &json!(1)]);
    assert_eq!(evs.last().unwrap()["bajadas"], 2);
    let una = bajar_todo("https://youtu.be/abc", &t.path().join("dest"), yt, None);
    assert_eq!(una[0]["es_lista"], false);
    assert_eq!(
        una.last().unwrap(),
        &json!({"tipo": "fin", "bajadas": 1, "fallidas": 0})
    );
}

#[test]
fn sin_yt_dlp_y_enlace_raro() {
    let t = tempfile::tempdir().unwrap();
    let no = t.path().join("no-existe");
    let evs = bajar_todo(
        "https://youtu.be/abc",
        &t.path().join("d"),
        no.to_str().unwrap(),
        None,
    );
    assert!(
        evs.iter()
            .any(|e| e["fatal"] == true && e["texto"].as_str().unwrap().contains("pacman -S yt-dlp"))
    );
    let evs = bajar_todo("https://example.com", &t.path().join("d"), "yt-dlp", None);
    assert_eq!(evs[0]["fatal"], true);
    assert_eq!(evs.last().unwrap()["tipo"], "fin");
}

/// Una canción como la manda la búsqueda de YouTube Music.
fn fila_ytm(vid: &str, titulo: &str, segunda: &str) -> Value {
    let col = |t: Vec<&str>| json!({"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": t.iter().map(|x| json!({"text": x})).collect::<Vec<_>>()}}});
    json!({"musicResponsiveListItemRenderer": {
        "playlistItemData": {"videoId": vid}, "flexColumns": [col(vec![titulo]), col(segunda.split('|').collect())],
        "thumbnail": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [{"url": "https://x/a=w60-h60-l90"}]}}}}})
}

/// Una canción como la manda la radio («A continuación») de YouTube Music.
fn fila_radio(vid: &str, titulo: &str, linea: &str, dur: &str) -> Value {
    json!({"playlistPanelVideoRenderer": {"videoId": vid, "title": {"runs": [{"text": titulo}]},
        "longBylineText": {"runs": [{"text": linea}]}, "lengthText": {"runs": [{"text": dur}]},
        "thumbnail": {"thumbnails": [{"url": "https://x/b=w60-h60"}]}}})
}

#[test]
fn leer_busqueda_de_ytmusic() {
    let r = json!({"contents": [fila_ytm("aaaaaaaaaaa", "Idol", "YOASOBI| • |Idol| • |3:34"),
                                fila_ytm("bbbbbbbbbbb", "Sin álbum", "Canción| • |Alguien| • |1:02:03")]});
    let c = descargas::leer_ytmusic(&r);
    let resumen: Vec<Value> = c
        .iter()
        .map(|x| json!([x["titulo"], x["artistas"], x["album"], x["duracion"]]))
        .collect();
    assert_eq!(
        resumen,
        [
            json!(["Idol", "YOASOBI", "Idol", 214]),
            json!(["Sin álbum", "Alguien", "", 3723])
        ]
    );
    assert_eq!(c[0]["portada"], "https://x/a=w226-h226-l90");
    assert_eq!(c[0]["url"], "https://music.youtube.com/watch?v=aaaaaaaaaaa");
}

#[test]
fn buscar_cae_a_yt_dlp() {
    // YouTube Music contestó algo que no se entiende: se usa la búsqueda de yt-dlp
    let t = tempfile::tempdir().unwrap();
    let falso = ejecutable(
        &t.path().join("yt-dlp"),
        r#"echo '{"entries": [{"id": "ccccccccccc", "title": "T", "channel": "C", "duration": 61}, {"id": "UCcanal"}]}'"#,
    );
    let vacio = |_: &str, _: &Value| Ok(json!({}));
    let c = descargas::buscar("algo", "ytmusic", 20, falso.to_str().unwrap(), &vacio).unwrap();
    let r: Vec<Value> = c
        .iter()
        .map(|x| json!([x["id"], x["titulo"], x["artistas"], x["duracion"]]))
        .collect();
    assert_eq!(r, [json!(["ccccccccccc", "T", "C", 61])]);
    let c = descargas::buscar("algo", "youtube", 20, falso.to_str().unwrap(), &vacio).unwrap();
    assert!(
        c[0]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://www.youtube.com/watch?v=")
    );
    assert!(
        descargas::buscar("   ", "ytmusic", 20, "yt-dlp", &vacio)
            .unwrap()
            .is_empty()
    );
    // Sin conexión: el error llega a la persona (no se cae a yt-dlp)
    let sin_red = |_: &str, _: &Value| Err(Fallo::Red("Sin conexión".into()));
    assert_eq!(
        descargas::buscar("algo", "ytmusic", 20, "yt-dlp", &sin_red),
        Err("Sin conexión".into())
    );
}

#[test]
fn recomendar() {
    let radio_de = |vid: &str| -> Vec<Value> {
        match vid {
            "semilla0001" => vec![
                fila_radio("semilla0001", "Idol", "YOASOBI • Idol • 2023", "3:34"),
                fila_radio("r1111111111", "Monster", "YOASOBI • THE BOOK 2 • 2021", "3:26"),
                fila_radio("r2222222222", "Ya la tengo (Live)", "LiSA • X", "4:00"),
                fila_radio("r3333333333", "Comun", "Alguien • Y", "2:00"),
            ],
            _ => vec![
                fila_radio("r4444444444", "Gurenge", "LiSA • LEO-NiNE • 2020", "3:58"),
                fila_radio("r3333333333", "Comun", "Alguien • Y", "2:00"),
            ],
        }
    };
    let pedidos = RefCell::new(Vec::new());
    let pedir = |url: &str, cuerpo: &Value| -> Result<Value, Fallo> {
        pedidos.borrow_mut().push(
            url.rsplit('/')
                .next()
                .unwrap()
                .split('?')
                .next()
                .unwrap()
                .to_string(),
        );
        if url.contains("search") {
            // la semilla sin id (un archivo tuyo) se busca primero
            return Ok(json!({"x": [fila_ytm("semilla0002", "Homura", "LiSA| • |homura| • |4:35")]}));
        }
        Ok(json!({"contents": [radio_de(cuerpo["videoId"].as_str().unwrap())]}))
    };
    let c = descargas::recomendar(
        &[
            json!({"id": "semilla0001", "titulo": "Idol", "artista": "YOASOBI"}),
            json!({"titulo": "Homura", "artista": "LiSA"}),
        ],
        &[json!({"titulo": "Ya la tengo", "artista": "LiSA"})],
        20,
        &pedir,
    )
    .unwrap();
    // Intercaladas, sin la semilla, sin repetir y sin lo que ya tienes (aunque diga «(Live)»)
    let titulos: Vec<&Value> = c.iter().map(|x| &x["titulo"]).collect();
    assert_eq!(titulos, [&json!("Monster"), &json!("Gurenge"), &json!("Comun")]);
    assert_eq!(
        json!([c[0]["artistas"], c[0]["album"], c[0]["duracion"]]),
        json!(["YOASOBI", "THE BOOK 2", 206])
    );
    assert_eq!(*pedidos.borrow(), ["search", "next", "next"]);
    assert!(descargas::recomendar(&[], &[], 20, &pedir).unwrap().is_empty());
}
