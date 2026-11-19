//! Las pruebas de `apps/vortex-local-ai/pruebas/test_motor.py`, portadas al motor de Rust (sin ventana, sin
//! red y sin Ollama de verdad). Un servidor HTTP falso hace de Ollama en 127.0.0.1 y todo lo demás va en
//! carpetas temporales: no se toca `~/.ollama`, ni tus chats, ni tu vault.

mod falso;

use falso::{ServidorFalso, puerto_cerrado};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vortex_local_ai::catalogo::{self, Medida, Vram};
use vortex_local_ai::conversaciones;
use vortex_local_ai::http::Cancelar;
use vortex_local_ai::memoria::{Memoria, fragmentar};
use vortex_local_ai::ollama::{ErrorOllama, Ollama, SeparadorPensar};
use vortex_local_ai::rutas::Rutas;
use vortex_local_ai::vault::{ErrorVault, Vault, enlaces_de, etiquetas_de, separar_frontmatter};

// ====================================================================== Ollama

fn ollama(s: &ServidorFalso) -> Ollama {
    Ollama::new(Some(&s.url), Some(false))
}

#[test]
fn disponible_y_modelos() {
    let s = ServidorFalso::new();
    let o = ollama(&s);
    assert!(o.disponible());
    assert!(o.asegurar_servidor(Duration::from_secs(1)));
    assert_eq!(o.version().as_deref(), Some("0.99.0"));
    let m = o.modelos().unwrap();
    assert_eq!(
        m.iter().map(|m| m.nombre.as_str()).collect::<Vec<_>>(),
        ["emb:latest", "qwen3:8b"]
    );
    let q = &m[1];
    assert_eq!(
        (
            q.bytes,
            q.familia.as_str(),
            q.parametros.as_str(),
            q.cuantizacion.as_str()
        ),
        (5_200_000_000, "qwen3", "8.2B", "Q4_K_M")
    );
    assert!(m[0].embeddings);
    assert!(!q.embeddings);
}

#[test]
fn cargados() {
    let s = ServidorFalso::new();
    let c = ollama(&s).cargados().unwrap();
    assert_eq!(c.len(), 1);
    let m = &c[0];
    assert_eq!(
        (m.nombre.as_str(), m.bytes, m.bytes_vram, m.en_gpu_pct),
        ("qwen3:8b", 6_000_000_000, 4_500_000_000, 75)
    );
}

#[test]
fn bajar_con_progreso() {
    let s = ServidorFalso::new();
    let mut eventos: Vec<(i64, i64, String)> = Vec::new();
    ollama(&s)
        .bajar(
            "falso:1b",
            Some(&mut |c, t, e| eventos.push((c, t, e.to_string()))),
            None,
        )
        .unwrap();
    assert_eq!(eventos[0].2, "Buscando el modelo");
    assert_eq!(eventos.last().unwrap(), &(1500, 1500, "Listo".to_string()));
    let estados: Vec<&str> = eventos.iter().map(|e| e.2.as_str()).collect();
    assert!(estados.contains(&"Bajando"));
    assert!(estados.contains(&"Revisando la descarga"));
    let completados: Vec<i64> = eventos.iter().map(|e| e.0).collect();
    let mut orden = completados.clone();
    orden.sort();
    assert_eq!(completados, orden, "el progreso nunca va para atrás");
}

#[test]
fn bajar_cancelado_a_medias() {
    let s = ServidorFalso::new();
    let cancelar = Cancelar::new();
    let c2 = cancelar.clone();
    let e = ollama(&s)
        .bajar(
            "falso:1b",
            Some(&mut |c, _, _| {
                if c >= 250 {
                    c2.set();
                }
            }),
            Some(&cancelar),
        )
        .unwrap_err();
    assert_eq!(e.clave, "cancelado");
}

#[test]
fn bajar_cancelado_con_el_servidor_callado() {
    // El servidor se queda callado 5 s: cancelar tiene que cortar ya, no esperar al siguiente trozo.
    let s = ServidorFalso::new();
    let cancelar = Cancelar::new();
    let c2 = cancelar.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        c2.set();
    });
    let inicio = Instant::now();
    let e = ollama(&s).bajar("lento:1b", None, Some(&cancelar)).unwrap_err();
    assert!(e.es_cancelado());
    assert!(
        inicio.elapsed() < Duration::from_millis(2500),
        "{:?}",
        inicio.elapsed()
    );
}

#[test]
fn bajar_modelo_que_no_existe() {
    let s = ServidorFalso::new();
    let e = ollama(&s).bajar("noexiste:1b", None, None).unwrap_err();
    assert_eq!(e.clave, "modelo");
    assert!(e.mensaje.contains("No encontré"));
    assert!(e.mensaje.contains("noexiste:1b"));
}

fn msj(t: &str) -> Vec<Value> {
    vec![json!({"role": "user", "content": t})]
}

#[test]
fn chat_con_think() {
    let s = ServidorFalso::new();
    let o = ollama(&s);
    let (mut trozos, mut pensado) = (Vec::<String>::new(), Vec::<String>::new());
    let r = o
        .chat(
            "pensador",
            &msj("hola"),
            Some(&mut |t| trozos.push(t.into())),
            None,
            None,
            Some(&mut |t| pensado.push(t.into())),
        )
        .unwrap();
    assert_eq!(r, "Hola mundo<");
    assert_eq!(trozos.concat(), "Hola mundo<");
    assert_eq!(pensado.concat(), "Hmm, pienso");
    assert!(!trozos.concat().contains("think"));
    let e = o.ultimas_estadisticas();
    assert_eq!(e.tokens, 10);
    assert_eq!(e.tokens_por_segundo, 10.0);
}

#[test]
fn chat_con_campo_thinking() {
    let s = ServidorFalso::new();
    let mut pensado = Vec::<String>::new();
    let r = ollama(&s)
        .chat(
            "campo",
            &msj("x"),
            None,
            None,
            None,
            Some(&mut |t| pensado.push(t.into())),
        )
        .unwrap();
    assert_eq!(r, "Respuesta");
    assert_eq!(pensado, ["Primero pienso"]);
}

#[test]
fn chat_eco_y_opciones() {
    let s = ServidorFalso::new();
    let r = ollama(&s)
        .chat(
            "qwen3:8b",
            &msj("¿qué tal?"),
            None,
            None,
            Some(&json!({"temperature": 0.2})),
            None,
        )
        .unwrap();
    assert_eq!(r, "Eco: ¿qué tal?");
    assert_eq!(s.estado.chats.lock().unwrap()[0]["options"]["temperature"], 0.2);
}

#[test]
fn chat_cancelado_regresa_lo_que_llevaba() {
    let s = ServidorFalso::new();
    let cancelar = Cancelar::new();
    let c2 = cancelar.clone();
    let inicio = Instant::now();
    let r = ollama(&s)
        .chat(
            "lento",
            &msj("x"),
            Some(&mut |_| c2.set()),
            Some(&cancelar),
            None,
            None,
        )
        .unwrap();
    assert_eq!(r, "Empiezo");
    assert!(inicio.elapsed() < Duration::from_millis(2500));
}

#[test]
fn chat_modelo_que_no_tienes() {
    let s = ServidorFalso::new();
    let e = ollama(&s)
        .chat("noexiste", &msj("x"), None, None, None, None)
        .unwrap_err();
    assert_eq!(e.clave, "modelo");
    assert!(e.mensaje.contains("Bájalo"));
}

#[test]
fn embeber_y_borrar() {
    let s = ServidorFalso::new();
    let o = ollama(&s);
    let v = o.embeber("emb", &["hola mundo".into(), "adiós".into()]).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].len(), 64);
    assert_eq!(o.embeber("emb", &["uno".into()]).unwrap().len(), 1);
    o.borrar("qwen3:8b").unwrap();
    assert!(
        s.estado
            .borrados
            .lock()
            .unwrap()
            .contains(&"qwen3:8b".to_string())
    );
    assert!(o.borrar("otro:1b").is_err());
}

#[test]
fn separador_de_think_con_etiquetas_partidas() {
    let mut s = SeparadorPensar::default();
    let mut salida = Vec::new();
    for t in ["a<", "th", "ink>b</", "think", ">c<t"] {
        salida.extend(s.meter(t));
    }
    salida.extend(s.terminar());
    let fuera: String = salida
        .iter()
        .filter(|(p, _)| !p)
        .map(|(_, t)| t.as_str())
        .collect();
    let dentro: String = salida
        .iter()
        .filter(|(p, _)| *p)
        .map(|(_, t)| t.as_str())
        .collect();
    assert_eq!(fuera, "ac<t");
    assert_eq!(dentro, "b");
}

#[test]
fn errores_traducidos() {
    use vortex_local_ai::ollama::traducir;
    assert_eq!(traducir("model 'x' not found", "x").clave, "modelo");
    assert!(
        traducir("no space left on device", "")
            .mensaje
            .contains("espacio")
    );
    assert_eq!(traducir("dial tcp: lookup registry", "x").clave, "red");
    assert!(
        traducir("qwen does not support thinking", "qwen")
            .mensaje
            .contains("«pensar»")
    );
    assert!(traducir("algo raro", "").mensaje.starts_with("Ollama dijo:"));
}

#[test]
fn servidor_caido_da_error_claro() {
    let o = Ollama::new(
        Some(&format!("http://127.0.0.1:{}", puerto_cerrado())),
        Some(false),
    );
    assert!(!o.disponible());
    let e = o.modelos().unwrap_err();
    assert!(["conexion", "no_instalado"].contains(&e.clave.as_str()), "{e:?}");
    assert!(e.mensaje.contains("No me pude conectar") || e.mensaje.contains("no está instalado"));
    assert!(o.chat("qwen3:8b", &msj("x"), None, None, None, None).is_err());
    assert!(o.bajar("qwen3:8b", None, None).is_err());
}

#[test]
fn no_arranca_nada() {
    let o = Ollama::new(
        Some(&format!("http://127.0.0.1:{}", puerto_cerrado())),
        Some(false),
    );
    let inicio = Instant::now();
    assert!(!o.asegurar_servidor(Duration::from_secs(1)));
    assert!(inicio.elapsed() < Duration::from_secs(3));
    assert!(!o.ultimo_error().is_empty());
    assert!(!o.arranco_algo());
}

#[test]
fn url_remota_no_se_arranca() {
    let o = Ollama::new(Some("http://192.0.2.1:11434"), Some(true));
    assert!(!o.asegurar_servidor(Duration::from_secs(1)));
    assert!(
        o.ultimo_error().contains("no está en esta PC"),
        "{}",
        o.ultimo_error()
    );
    assert!(!o.arranco_algo());
}

// ====================================================================== Vault

const INICIO: &str = "---
tags: [inicio]
---
# Inicio

Mira mis [[Ideas]] y sobre todo [[Ideas#Juegos|mis juegos]]. Algo pendiente: [[Fantasma]].
![[foto.png]]

#proyecto #2026 y una Canción bonita.

```python
print(\"[[NoEsEnlace]] #noetiqueta\")
```
Y `[[TampocoEsEnlace]]`.
";

const IDEAS: &str = "---
aliases: [Lluvia de ideas]
tags:
  - ideas
  - juegos/roblox
---
# Ideas

## Juegos
Un juego de carreras. Vuelve al [[Inicio]].
";

const SUB: &str = "Esto viene de la [[lluvia de ideas]] y del [[Inicio|principio]].\n";

fn vault_de_prueba() -> (tempfile::TempDir, Vault) {
    let t = tempfile::tempdir().unwrap();
    let r = t.path();
    fs::create_dir(r.join("Carpeta")).unwrap();
    fs::create_dir(r.join("Vacía")).unwrap();
    fs::write(r.join("Inicio.md"), INICIO).unwrap();
    fs::write(r.join("Ideas.md"), IDEAS).unwrap();
    fs::write(r.join("Carpeta/Sub.md"), SUB).unwrap();
    // Lo de Obsidian que no se debe tocar ni leer como notas
    fs::create_dir(r.join(".obsidian")).unwrap();
    fs::write(r.join(".obsidian/app.json"), "{\"x\": 1}").unwrap();
    fs::write(r.join(".obsidian/plantilla.md"), "[[Ideas]] #oculta").unwrap();
    let v = Vault::new(r).unwrap();
    (t, v)
}

#[test]
fn notas_ignora_obsidian() {
    let (_t, v) = vault_de_prueba();
    let mut rutas: Vec<String> = v.notas().into_iter().map(|n| n.ruta).collect();
    rutas.sort();
    assert_eq!(rutas, ["Carpeta/Sub.md", "Ideas.md", "Inicio.md"]);
    let ideas = v.notas().into_iter().find(|n| n.ruta == "Ideas.md").unwrap();
    assert_eq!(ideas.titulo, "Ideas");
    assert_eq!(ideas.alias, ["Lluvia de ideas"]);
    assert!(ideas.modificado > 1e9);
    assert!(!v.etiquetas().iter().any(|(t, _)| t == "oculta"));
}

#[test]
fn etiquetas() {
    let (_t, v) = vault_de_prueba();
    let e: Vec<(String, usize)> = v.etiquetas();
    assert_eq!(
        e,
        [
            ("ideas".to_string(), 1),
            ("inicio".into(), 1),
            ("juegos/roblox".into(), 1),
            ("proyecto".into(), 1)
        ]
    );
    assert_eq!(
        etiquetas_de("# Título\n#uno y #dos/tres, no #123 ni a#b ni http://x.mx/#ancla"),
        ["uno", "dos/tres"]
    );
}

#[test]
fn frontmatter() {
    use vortex_local_ai::vault::Yaml;
    let (frente, cuerpo) = separar_frontmatter(IDEAS);
    assert_eq!(
        frente["tags"],
        Yaml::Lista(vec!["ideas".into(), "juegos/roblox".into()])
    );
    assert_eq!(frente["aliases"], Yaml::Lista(vec!["Lluvia de ideas".into()]));
    assert!(cuerpo.trim_start().starts_with("# Ideas"));
    assert!(separar_frontmatter("sin nada").0.is_empty());
}

#[test]
fn enlaces_fuera_del_codigo() {
    let e = enlaces_de(INICIO);
    let nombres: Vec<&str> = e.iter().map(|e| e.nombre.as_str()).collect();
    assert_eq!(nombres, ["Ideas", "Ideas", "Fantasma", "foto.png"]);
    assert_eq!(
        (e[1].seccion.as_str(), e[1].texto.as_str()),
        ("Juegos", "mis juegos")
    );
    assert!(e[3].incrustado);
}

#[test]
fn resolver_como_obsidian() {
    let (_t, v) = vault_de_prueba();
    for n in [
        "Ideas",
        "ideas",
        "Ideas.md",
        "Ideas#Juegos",
        "Ideas|otra cosa",
        "Lluvia de ideas",
        "LLUVIA DE IDEAS",
    ] {
        assert_eq!(v.resolver(n, None).as_deref(), Some("Ideas.md"), "{n}");
    }
    assert_eq!(v.resolver("Carpeta/Sub", None).as_deref(), Some("Carpeta/Sub.md"));
    assert_eq!(v.resolver("Sub", None).as_deref(), Some("Carpeta/Sub.md"));
    assert_eq!(v.resolver("Fantasma", None), None);
    assert_eq!(v.resolver("plantilla", None), None); // está en .obsidian
}

#[test]
fn resolver_prefiere_la_misma_carpeta() {
    let (t, v) = vault_de_prueba();
    fs::write(t.path().join("Carpeta/Ideas.md"), "otra").unwrap();
    assert_eq!(v.resolver("Ideas", None).as_deref(), Some("Ideas.md"));
    assert_eq!(
        v.resolver("Ideas", Some("Carpeta/Sub.md")).as_deref(),
        Some("Carpeta/Ideas.md")
    );
}

#[test]
fn enlaces_y_retroenlaces() {
    let (_t, v) = vault_de_prueba();
    let enlaces = v.enlaces("Inicio.md").unwrap();
    let por = |n: &str| enlaces.iter().find(|e| e.nombre == n).unwrap().clone();
    assert_eq!(por("Ideas").ruta.as_deref(), Some("Ideas.md"));
    assert!(!por("Fantasma").existe);
    assert_eq!(por("Fantasma").ruta, None);
    let retro = v.retroenlaces("Ideas.md").unwrap();
    let mut rutas: Vec<&str> = retro.iter().map(|r| r.ruta.as_str()).collect();
    rutas.sort();
    assert_eq!(rutas, ["Carpeta/Sub.md", "Inicio.md"]); // Sub llega por el alias
    assert!(
        retro
            .iter()
            .find(|r| r.ruta == "Inicio.md")
            .unwrap()
            .fragmento
            .contains("[[Ideas]]")
    );
    let mut de_inicio: Vec<String> = v
        .retroenlaces("Inicio.md")
        .unwrap()
        .into_iter()
        .map(|r| r.ruta)
        .collect();
    de_inicio.sort();
    assert_eq!(de_inicio, ["Carpeta/Sub.md", "Ideas.md"]);
}

#[test]
fn grafo() {
    let (_t, v) = vault_de_prueba();
    let g = v.grafo();
    let mut ids: Vec<&str> = g.nodos.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, ["?Fantasma", "Carpeta/Sub.md", "Ideas.md", "Inicio.md"]);
    assert!(!g.nodos.iter().find(|n| n.id == "?Fantasma").unwrap().existe);
    let mut esperadas = vec![
        ("Inicio.md", "Ideas.md"),
        ("Inicio.md", "?Fantasma"),
        ("Ideas.md", "Inicio.md"),
        ("Carpeta/Sub.md", "Ideas.md"),
        ("Carpeta/Sub.md", "Inicio.md"),
    ];
    esperadas.sort();
    let reales: Vec<(&str, &str)> = g.aristas.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert_eq!(reales, esperadas);
    assert_eq!(g.nodos.iter().find(|n| n.id == "Ideas.md").unwrap().grado, 3);
}

#[test]
fn renombrar_actualiza_enlaces() {
    let (t, v) = vault_de_prueba();
    assert_eq!(v.renombrar("Ideas.md", "Proyectos").unwrap(), "Proyectos.md");
    assert!(!t.path().join("Ideas.md").exists());
    let inicio = v.leer("Inicio.md").unwrap();
    assert!(inicio.contains("[[Proyectos]]"));
    assert!(inicio.contains("[[Proyectos#Juegos|mis juegos]]"));
    assert!(!inicio.contains("[[Ideas"));
    assert!(inicio.contains("[[NoEsEnlace]]")); // lo del código no se toca
    assert!(inicio.contains("[[Fantasma]]"));
    assert!(v.leer("Carpeta/Sub.md").unwrap().contains("[[lluvia de ideas]]")); // el alias sigue sirviendo
    assert_eq!(
        v.resolver("lluvia de ideas", None).as_deref(),
        Some("Proyectos.md")
    );
    let mut r: Vec<String> = v
        .retroenlaces("Proyectos.md")
        .unwrap()
        .into_iter()
        .map(|r| r.ruta)
        .collect();
    r.sort();
    assert_eq!(r, ["Carpeta/Sub.md", "Inicio.md"]);
}

#[test]
fn mover_a_carpeta() {
    let (_t, v) = vault_de_prueba();
    assert_eq!(
        v.renombrar("Inicio.md", "Carpeta/Portada.md").unwrap(),
        "Carpeta/Portada.md"
    );
    assert!(v.leer("Ideas.md").unwrap().contains("[[Portada]]"));
    assert!(
        v.leer("Carpeta/Sub.md")
            .unwrap()
            .contains("[[Portada|principio]]")
    );
}

#[test]
fn renombrar_a_una_que_ya_existe() {
    let (t, v) = vault_de_prueba();
    assert!(matches!(
        v.renombrar("Ideas.md", "Inicio"),
        Err(ErrorVault::Existe(_))
    ));
    assert!(t.path().join("Ideas.md").exists());
}

#[test]
fn borrar_va_a_la_papelera() {
    let (t, v) = vault_de_prueba();
    assert_eq!(v.borrar("Ideas.md").unwrap(), ".trash/Ideas.md");
    assert!(t.path().join(".trash/Ideas.md").exists());
    assert!(!t.path().join("Ideas.md").exists());
    let rutas: Vec<String> = v.notas().into_iter().map(|n| n.ruta).collect();
    assert!(!rutas.contains(&"Ideas.md".to_string()));
    assert!(!rutas.contains(&".trash/Ideas.md".to_string()));
    // Otra con el mismo nombre no pisa la de la papelera
    let ruta = v.crear("Ideas", "").unwrap();
    assert_eq!(v.borrar(&ruta).unwrap(), ".trash/Ideas 1.md");
    assert_eq!(
        fs::read_to_string(t.path().join(".obsidian/app.json")).unwrap(),
        "{\"x\": 1}"
    );
}

fn hay_tmp(dir: &Path) -> bool {
    fs::read_dir(dir).unwrap().flatten().any(|e| {
        e.path().extension().is_some_and(|x| x == "tmp") || (e.path().is_dir() && hay_tmp(&e.path()))
    })
}

#[test]
fn crear_guardar_leer() {
    let (t, v) = vault_de_prueba();
    assert_eq!(v.crear("Inicio", "").unwrap(), "Inicio 1.md");
    assert_eq!(
        v.crear("Mi: nota/rara?", "Carpeta").unwrap(),
        "Carpeta/Mi nota rara.md"
    );
    assert_eq!(v.crear("", "").unwrap(), "Sin título.md");
    v.guardar("Nueva/Hoja.md", "hola #nueva").unwrap();
    assert_eq!(v.leer("Nueva/Hoja.md").unwrap(), "hola #nueva");
    assert!(v.etiquetas().iter().any(|(t, _)| t == "nueva"));
    assert!(!hay_tmp(t.path()), "no quedan temporales");
    assert_eq!(v.crear_carpeta("Otra/Más").unwrap(), "Otra/Más");
}

#[test]
fn rutas_que_se_salen() {
    let (_t, v) = vault_de_prueba();
    for mala in ["../fuera.md", "/etc/passwd", "Carpeta/../../x.md", ""] {
        assert!(matches!(v.leer(mala), Err(ErrorVault::Ruta(_))), "{mala}");
    }
    assert!(matches!(v.leer("NoExiste.md"), Err(ErrorVault::NoExiste(_))));
}

#[test]
fn buscar() {
    let (_t, v) = vault_de_prueba();
    let rutas = |q: &str| -> Vec<String> { v.buscar(q).unwrap().into_iter().map(|r| r.ruta).collect() };
    let res = v.buscar("cancion").unwrap();
    assert_eq!(
        res.iter().map(|r| r.ruta.as_str()).collect::<Vec<_>>(),
        ["Inicio.md"]
    );
    assert!(res[0].fragmento.contains("Canción"));
    assert_eq!(
        rutas("IDEAS")[0],
        "Ideas.md",
        "primero el que lo tiene en el título"
    );
    assert_eq!(rutas("#juegos"), ["Ideas.md"]);
    assert_eq!(rutas("juego carreras"), ["Ideas.md"]);
    assert!(rutas("zzzz").is_empty());
    assert!(rutas("").is_empty());
}

#[test]
fn arbol() {
    let (_t, v) = vault_de_prueba();
    let a = v.arbol();
    assert!(a.es_carpeta);
    assert_eq!(a.ruta, "");
    let nombres: Vec<&str> = a.hijos.iter().map(|h| h.nombre.as_str()).collect();
    assert_eq!(nombres, ["Carpeta", "Vacía", "Ideas.md", "Inicio.md"]);
    assert_eq!(a.hijos[0].hijos[0].ruta, "Carpeta/Sub.md");
    assert_eq!(a.hijos[0].hijos[0].titulo, "Sub");
    assert!(!nombres.contains(&".obsidian"));
}

// ====================================================================== Memoria

const NOTAS_MEMORIA: [(&str, &str); 3] = [
    (
        "Pizza.md",
        "# Pizza casera\n\nLa receta de pizza: harina, agua, levadura y sal. Hornear la pizza a 250 grados.\n",
    ),
    (
        "Roblox.md",
        "# Roblox\n\nMi juego de carreras en Roblox usa Lua y tiene autos rápidos.\n",
    ),
    (
        "Escuela/Mate.md",
        "# Matemáticas\n\nEl examen de fracciones es el lunes. Repasar fracciones y decimales.\n",
    ),
];

struct Memo {
    s: ServidorFalso,
    _raiz: tempfile::TempDir,
    cache: tempfile::TempDir,
    o: Arc<Ollama>,
}

fn memo() -> (Memo, Memoria) {
    let s = ServidorFalso::new();
    let raiz = tempfile::tempdir().unwrap();
    for (r, t) in NOTAS_MEMORIA {
        let p = raiz.path().join(r);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, t).unwrap();
    }
    let cache = tempfile::tempdir().unwrap();
    let o = Arc::new(ollama(&s));
    let m = Memoria::new(Vault::new(raiz.path()).unwrap(), o.clone(), None, cache.path());
    (
        Memo {
            s,
            _raiz: raiz,
            cache,
            o,
        },
        m,
    )
}

#[test]
fn indice_en_cache() {
    let (c, m) = memo();
    assert!(m.ruta_indice.starts_with(c.cache.path()));
    // El nombre es «<vault>-<10 primeras de sha1 de la ruta>.sqlite», como el de Python.
    let n = m.ruta_indice.file_name().unwrap().to_string_lossy().to_string();
    assert!(n.ends_with(".sqlite") && n.rsplit_once('-').unwrap().1.len() == "0123456789.sqlite".len());
}

#[test]
fn incremental() {
    let (c, mut m) = memo();
    let mut progreso = Vec::new();
    let r = m
        .actualizar(
            Some(&mut |h, t, ruta| progreso.push((h, t, ruta.to_string()))),
            None,
        )
        .unwrap();
    assert_eq!(m.modelo_embeddings(), Some("emb:latest"));
    assert_eq!(m.modo(), "embeddings");
    assert_eq!(m.aviso(), "");
    assert_eq!((r.notas, r.nuevas, r.quitadas), (3, 3, 0));
    assert_eq!(r.fragmentos, 3);
    assert_eq!(progreso.last().map(|p| (p.0, p.1)), Some((3, 3)));

    // Sin cambios: no vuelve a embeber nada
    let embebidos = || {
        c.s.estado
            .textos_embebidos
            .load(std::sync::atomic::Ordering::SeqCst)
    };
    let antes = embebidos();
    assert_eq!(m.actualizar(None, None).unwrap().nuevas, 0);
    assert_eq!(embebidos(), antes);

    // Solo cambió la fecha (mismo contenido): el hash lo detecta y tampoco embebe
    let futuro = std::time::SystemTime::now() + Duration::from_secs(30);
    let f = fs::File::options()
        .write(true)
        .open(m.vault.raiz.join("Pizza.md"))
        .unwrap();
    f.set_modified(futuro).unwrap();
    assert_eq!(m.actualizar(None, None).unwrap().nuevas, 0);
    assert_eq!(embebidos(), antes);

    // Cambió una: solo esa
    m.vault
        .guardar(
            "Roblox.md",
            "# Roblox\n\nAhora el juego es de aviones y pizza voladora.\n",
        )
        .unwrap();
    assert_eq!(m.actualizar(None, None).unwrap().nuevas, 1);
    assert_eq!(embebidos(), antes + 1);
}

#[test]
fn nota_borrada_sale_del_indice() {
    let (_c, mut m) = memo();
    m.actualizar(None, None).unwrap();
    m.vault.borrar("Pizza.md").unwrap();
    assert_eq!(m.actualizar(None, None).unwrap().quitadas, 1);
    assert_eq!(m.estado().unwrap().notas, 2);
    let ctx = m.contexto("receta de pizza", 6).unwrap();
    assert!(!ctx.iter().any(|c| c.ruta == "Pizza.md"));
}

#[test]
fn contexto_y_mensajes() {
    let (_c, mut m) = memo();
    m.actualizar(None, None).unwrap();
    let ctx = m.contexto("¿cuál es la receta de la pizza?", 2).unwrap();
    assert_eq!(ctx[0].ruta, "Pizza.md");
    assert_eq!(ctx[0].titulo, "Pizza");
    assert!(ctx[0].puntaje > ctx.last().unwrap().puntaje - 1e-9);
    assert_eq!(
        m.contexto("fracciones del examen", 1).unwrap()[0].ruta,
        "Escuela/Mate.md"
    );

    let historial = vec![
        json!({"role": "user", "content": "hola"}),
        json!({"role": "assistant", "content": "¡Hola!", "pensamiento": "x"}),
    ];
    let mensajes = m.mensajes_con_contexto("¿cómo hago pizza?", &historial).unwrap();
    assert_eq!(mensajes[0]["role"], "system");
    let sistema = mensajes[0]["content"].as_str().unwrap();
    assert!(sistema.contains("español"));
    assert!(sistema.contains("[[Pizza]]"));
    assert!(sistema.contains("harina"));
    assert_eq!(mensajes[1], json!({"role": "user", "content": "hola"}));
    assert_eq!(mensajes[2], json!({"role": "assistant", "content": "¡Hola!"}));
    assert_eq!(
        mensajes.last().unwrap(),
        &json!({"role": "user", "content": "¿cómo hago pizza?"})
    );
    assert_eq!(m.ultimo_contexto[0].ruta, "Pizza.md");
}

#[test]
fn cancelar() {
    let (_c, mut m) = memo();
    let c = Cancelar::new();
    c.set();
    let r = m.actualizar(None, Some(&c)).unwrap();
    assert!(r.cancelado);
    assert_eq!(r.nuevas, 0);
    assert_eq!(m.actualizar(None, None).unwrap().nuevas, 3); // después sigue donde se quedó
}

#[test]
fn cambiar_de_modelo_rehace_todo() {
    let (c, mut m) = memo();
    m.actualizar(None, None).unwrap();
    let mut otra = Memoria::new(
        Vault::new(&m.vault.raiz).unwrap(),
        c.o.clone(),
        Some("emb".into()),
        c.cache.path(),
    );
    assert_eq!(otra.actualizar(None, None).unwrap().nuevas, 3);
}

#[test]
fn sin_embeddings_busca_por_palabras() {
    let (c, m0) = memo();
    let caido = Arc::new(Ollama::new(
        Some(&format!("http://127.0.0.1:{}", puerto_cerrado())),
        Some(false),
    ));
    let mut m = Memoria::new(Vault::new(&m0.vault.raiz).unwrap(), caido, None, c.cache.path());
    assert_eq!(m.actualizar(None, None).unwrap().nuevas, 0);
    assert_eq!(m.modo(), "palabras");
    assert!(m.aviso().contains("por palabras"));
    let ctx = m.contexto("¿qué tiene mi juego de Roblox?", 6).unwrap();
    assert_eq!(ctx[0].ruta, "Roblox.md");
    let mensajes = m
        .mensajes_con_contexto("¿qué tiene mi juego de Roblox?", &[])
        .unwrap();
    let sistema = mensajes[0]["content"].as_str().unwrap();
    assert!(sistema.contains("[[Roblox]]"));
    assert!(sistema.contains("buscando las palabras"));
    assert!(m.contexto("de la el", 6).unwrap().is_empty());
}

#[test]
fn fragmentar_largo() {
    let largo = format!(
        "# Uno\n\n{}\n\n## Dos\n\nCorto.\n",
        "Una oración bastante normal. ".repeat(80)
    );
    let trozos = fragmentar(&format!("---\ntags: [x]\n---\n{largo}"));
    assert!(trozos.iter().all(|(_, t)| t.chars().count() <= 820));
    assert_eq!(trozos[0].0, "Uno");
    assert_eq!(trozos.last().unwrap(), &("Dos".to_string(), "Corto.".to_string()));
    assert!(!trozos.iter().any(|(_, t)| t.contains("tags")));
}

// ====================================================================== Catálogo

fn gpu(gb: i64) -> Vram {
    Vram {
        nombre: "x".into(),
        total_mb: gb * 1024,
        libre_mb: gb * 1024,
        tipo: "nvidia".into(),
        ram_mb: 16384,
        de_entrada: false,
    }
}

fn cpu(gb: i64) -> Vram {
    Vram {
        nombre: "CPU".into(),
        total_mb: gb * 1024,
        libre_mb: gb * 1024,
        tipo: "cpu".into(),
        ram_mb: gb * 1024,
        de_entrada: false,
    }
}

#[test]
fn catalogo_completo() {
    let ids: Vec<&str> = catalogo::todos().map(|m| m.id).collect();
    let mut unicos = ids.clone();
    unicos.sort();
    unicos.dedup();
    assert_eq!(ids.len(), unicos.len());
    let sin_vault = catalogo::todos().filter(|m| m.para != "vault").count();
    assert!((12..=16).contains(&sin_vault));
    for m in catalogo::todos() {
        assert!(catalogo::nombre_de_tipo(m.para).is_some());
        assert!(m.vram_gb > m.gb * 0.9, "{}", m.id);
    }
    assert_eq!(catalogo::por_id(catalogo::EMBEDDINGS).unwrap().para, "vault");
    assert_eq!(
        catalogo::por_id("nomic-embed-text:latest").unwrap().id,
        "nomic-embed-text"
    );
    let tipos: std::collections::BTreeSet<&str> = catalogo::todos().map(|m| m.para).collect();
    assert_eq!(tipos, catalogo::TIPOS.iter().map(|t| t.0).collect());
}

#[test]
fn cabe() {
    let rtx3080 = Vram {
        nombre: "RTX 3080".into(),
        total_mb: 10240,
        libre_mb: 9000,
        tipo: "nvidia".into(),
        ram_mb: 32768,
        de_entrada: false,
    };
    let c = |id: &str, v: &Vram| catalogo::cabe(Medida::Id(id), v);
    assert_eq!(c("qwen3.5:9b", &rtx3080), "si");
    assert_eq!(c("gemma4:12b", &rtx3080), "justo");
    assert_eq!(c("qwen3.6:27b", &rtx3080), "no");
    assert_eq!(c("gpt-oss:20b", &rtx3080), "justo"); // moe: aguanta ir en parte en la CPU
    assert_eq!(
        catalogo::cabe(
            Medida::Entrada(catalogo::por_id("llama3.2:3b").unwrap()),
            &rtx3080
        ),
        "si"
    );
    assert_eq!(catalogo::cabe(Medida::Bytes(5_000_000_000), &rtx3080), "si");
    assert_eq!(c("no-existe:1b", &rtx3080), "no");
    let cpu16 = Vram {
        nombre: "CPU".into(),
        total_mb: 16384,
        libre_mb: 8000,
        tipo: "cpu".into(),
        ram_mb: 16384,
        de_entrada: false,
    };
    assert_eq!(c("llama3.2:3b", &cpu16), "si");
    assert_eq!(c("gemma4:12b", &cpu16), "justo");
    assert_eq!(c("gpt-oss:20b", &cpu16), "no");
}

#[test]
fn recomendado() {
    let r = |v: &Vram| catalogo::recomendado(v).id;
    assert_eq!(r(&gpu(10)), "qwen3.5:9b");
    assert_eq!(r(&gpu(12)), "gemma4:12b");
    assert_eq!(r(&gpu(4)), "qwen3.5:2b");
    assert_eq!(r(&gpu(1)), "llama3.2:3b"); // ninguno cabe: el más chico
    assert_eq!(r(&gpu(32)), "qwen3.6:35b-a3b");
    assert_eq!(catalogo::recomendado(&gpu(10)).para, "chat");
}

#[test]
fn recomendado_sin_grafica() {
    let r = |v: &Vram| catalogo::recomendado(v).id;
    assert_eq!(r(&cpu(16)), "qwen3.5:4b"); // uno ligero, aunque quepa uno mayor
    assert_eq!(r(&cpu(64)), "qwen3.5:4b");
    assert_eq!(r(&cpu(8)), "qwen3.5:2b");
}

#[test]
fn vram_de_sysfs_amd() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("card1/device");
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("vendor"), "0x1002\n").unwrap();
    fs::write(
        d.join("mem_info_vram_total"),
        (8u64 * 1024 * 1024 * 1024).to_string(),
    )
    .unwrap();
    fs::write(d.join("mem_info_vram_used"), (1024u64 * 1024 * 1024).to_string()).unwrap();
    fs::create_dir(t.path().join("card1-DP-1")).unwrap();
    let v = catalogo::de_sysfs(16384, 8000, t.path()).unwrap();
    assert_eq!((v.tipo.as_str(), v.total_mb, v.libre_mb), ("amd", 8192, 7168));
}

// ====================================================================== Conversaciones y rutas

#[test]
fn rutas_de_prueba() {
    let r = Rutas {
        datos: "/x/datos".into(),
        cache: "/x/cache".into(),
        vault_por_defecto: "/x/vault".into(),
    };
    assert_eq!(r.chats(), Path::new("/x/datos/chats"));
    assert_eq!(r.config(), Path::new("/x/datos/config.json"));
    assert_eq!(vortex_local_ai::VERSION, "1.0.0");
}

#[test]
fn guardar_abrir_lista_borrar() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("chats");
    let mut conv = conversaciones::nueva("qwen3:8b", "");
    conv["mensajes"] = json!([
        {"role": "user", "content": "¿Cómo hago una pizza casera con queso y jamón?"},
        {"role": "assistant", "content": "Así…", "pensamiento": "La persona quiere\nuna receta", "citas": ["Recetas/Pizza.md"]},
    ]);
    let id = conversaciones::guardar(&dir, &mut conv).unwrap();
    assert!(dir.join(format!("{id}.json")).exists());
    let abierta = conversaciones::abrir(&dir, &id).unwrap();
    assert_eq!(abierta["mensajes"], conv["mensajes"]);
    assert!(
        abierta["titulo"]
            .as_str()
            .unwrap()
            .starts_with("Cómo hago una pizza")
    );
    let lista = conversaciones::lista(&dir);
    let resumen = lista.iter().find(|c| c["id"] == id.as_str()).unwrap();
    assert_eq!(resumen["n_mensajes"], 2);
    assert!(resumen.get("mensajes").is_none());

    let nota = conversaciones::a_nota(&abierta);
    assert!(nota.starts_with("---\ntags: [chat, vortex-local-ai]"));
    assert!(nota.contains("## Tú"));
    assert!(nota.contains("> [!note]- Pensamiento"));
    assert!(nota.contains("> una receta"));
    assert!(nota.contains("[[Pizza]]"));

    conversaciones::borrar(&dir, &id).unwrap();
    assert!(conversaciones::abrir(&dir, &id).is_none());
    conversaciones::borrar(&dir, &id).unwrap(); // dos veces no truena
    assert!(conversaciones::abrir(&dir, "../../etc/passwd").is_none());
    assert!(conversaciones::borrar(&dir, "../../etc/passwd").is_err());
}

#[test]
fn el_formato_es_el_de_python() {
    // Python guarda con indent=1 y ensure_ascii=False: los chats de una y otra se abren igual.
    let t = tempfile::tempdir().unwrap();
    let mut conv = json!({"id": "20261101-101010-abc123", "titulo": "Hola ñ", "mensajes": [], "creado": 1.0});
    conversaciones::guardar(t.path(), &mut conv).unwrap();
    let texto = fs::read_to_string(t.path().join("20261101-101010-abc123.json")).unwrap();
    assert!(texto.contains("\n \"titulo\": \"Hola ñ\""), "{texto}");
}

#[test]
fn titulo_automatico() {
    use conversaciones::titulo_automatico as t;
    assert_eq!(t(""), "Chat nuevo");
    assert_eq!(t("   "), "Chat nuevo");
    assert_eq!(t("hola"), "Hola");
    assert_eq!(
        t("Explícame [[Fracciones|las fracciones]] **ya**"),
        "Explícame las fracciones ya"
    );
    assert_eq!(
        t("uno dos tres cuatro cinco seis siete ocho"),
        "Uno dos tres cuatro cinco seis…"
    );
}

#[test]
fn errores_de_ollama_son_clonables() {
    let e: ErrorOllama = ErrorOllama::cancelado();
    assert_eq!(e.clone(), e);
}
