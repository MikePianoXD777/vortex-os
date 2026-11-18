//! Las pruebas de `pruebas/test_motor.py`, portadas, sin Wine ni Proton de verdad: un `umu-run` falso que
//! solo anota sus argumentos, carpetas de Proton falsas y una casa temporal. Los .exe se generan aquí
//! (un PE mínimo con información de versión e íconos), no hace falta el Proton de CachyOS.
//! Nunca se corre Proton, umu ni un .exe real, ni se toca la biblioteca ni los prefijos de la persona.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use vortex_runner::biblioteca::{self, Biblioteca};
use vortex_runner::estado::{Estado, Evento};
use vortex_runner::{Contexto, correr, pe, protones};

// ---- un PE de mentira ----------------------------------------------------------------------

fn utf16(s: &str, con_cero: bool) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    if con_cero {
        v.extend([0, 0]);
    }
    v
}

fn alinear(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

/// Un bloque de VS_VERSIONINFO: longitud, longitud del valor, tipo, clave, valor, hijos.
fn bloque(clave: &str, tipo: u16, largo_valor: u16, valor: &[u8], hijos: &[u8]) -> Vec<u8> {
    let mut b = vec![0, 0];
    b.extend(largo_valor.to_le_bytes());
    b.extend(tipo.to_le_bytes());
    b.extend(utf16(clave, true));
    alinear(&mut b);
    b.extend(valor);
    alinear(&mut b);
    b.extend(hijos);
    let n = b.len() as u16;
    b[0..2].copy_from_slice(&n.to_le_bytes());
    b
}

fn version(cadenas: &[(&str, &str)]) -> Vec<u8> {
    let mut strings = Vec::new();
    for (k, v) in cadenas {
        let mut s = bloque(k, 1, v.encode_utf16().count() as u16 + 1, &utf16(v, true), &[]);
        alinear(&mut s);
        strings.extend(s);
    }
    let tabla = bloque("040904B0", 1, 0, &[], &strings);
    let mut info = bloque("StringFileInfo", 1, 0, &[], &tabla);
    alinear(&mut info);
    let var = bloque("VarFileInfo", 1, 0, &[], &[]);
    info.extend(var);
    bloque("VS_VERSION_INFO", 0, 52, &[0u8; 52], &info)
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-cuerpo-de-mentira-";

/// (ancho, bits, datos): un ícono. Ancho 0 = 256.
type Icono = (u8, u16, Vec<u8>);

#[derive(Default)]
struct Recursos {
    version: Option<Vec<u8>>,
    iconos: Vec<Icono>,
}

fn dir(entradas: &[(u32, u32, bool)]) -> Vec<u8> {
    let mut d = vec![0u8; 12];
    d.extend((entradas.len() as u16).to_le_bytes()); // nombradas = 0 → con_id
    d.extend(0u16.to_le_bytes());
    d[12..14].copy_from_slice(&0u16.to_le_bytes());
    d[14..16].copy_from_slice(&(entradas.len() as u16).to_le_bytes());
    d.truncate(16);
    for (id, desp, es_dir) in entradas {
        d.extend(id.to_le_bytes());
        d.extend((desp | if *es_dir { 0x8000_0000 } else { 0 }).to_le_bytes());
    }
    d
}

/// El árbol de recursos de una sección .rsrc que empieza en `rva`.
/// (tipo, [(id, datos)])
type Tipos = Vec<(u32, Vec<(u32, Vec<u8>)>)>;

fn arbol(tipos: &Tipos, rva: u32) -> Vec<u8> {
    // Posiciones: raíz, carpetas de tipo, carpetas de id (idioma), entradas de datos, bloques de datos.
    let raiz_tam = 16 + 8 * tipos.len();
    let mut pos_tipo = Vec::new();
    let mut p = raiz_tam;
    for (_, ids) in tipos {
        pos_tipo.push(p);
        p += 16 + 8 * ids.len();
    }
    let mut pos_id = Vec::new(); // por (tipo, id)
    for (_, ids) in tipos {
        for _ in ids {
            pos_id.push(p);
            p += 16 + 8;
        }
    }
    let n_datos: usize = tipos.iter().map(|(_, i)| i.len()).sum();
    let pos_entradas = p;
    p += 16 * n_datos;
    let mut blobs = Vec::new();
    let mut pos_blob = Vec::new();
    for (_, ids) in tipos {
        for (_, datos) in ids {
            pos_blob.push(p + blobs.len());
            blobs.extend(datos);
            alinear(&mut blobs);
        }
    }
    let mut salida = dir(&tipos
        .iter()
        .enumerate()
        .map(|(i, (t, _))| (*t, pos_tipo[i] as u32, true))
        .collect::<Vec<_>>());
    let mut k = 0;
    for (_, ids) in tipos {
        let e: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(j, (id, _))| (*id, pos_id[k + j] as u32, true))
            .collect();
        salida.extend(dir(&e));
        k += ids.len();
    }
    for (i, _) in pos_id.iter().enumerate() {
        salida.extend(dir(&[(0x409, (pos_entradas + 16 * i) as u32, false)]));
    }
    for (i, (tam, _)) in tipos
        .iter()
        .flat_map(|(_, ids)| ids.iter().map(|(_, d)| (d.len(), ())))
        .enumerate()
    {
        salida.extend((rva + pos_blob[i] as u32).to_le_bytes());
        salida.extend((tam as u32).to_le_bytes());
        salida.extend([0u8; 8]);
    }
    assert_eq!(salida.len(), p);
    salida.extend(blobs);
    salida
}

/// Un .exe PE32+ mínimo con una sola sección de recursos.
fn pe_falso(r: &Recursos) -> Vec<u8> {
    let mut tipos: Tipos = Vec::new();
    if !r.iconos.is_empty() {
        let mut grupo = vec![0, 0, 1, 0];
        grupo.extend((r.iconos.len() as u16).to_le_bytes());
        let mut datos = Vec::new();
        for (i, (ancho, bits, d)) in r.iconos.iter().enumerate() {
            grupo.extend([*ancho, *ancho, 0, 0, 1, 0]);
            grupo.extend(bits.to_le_bytes());
            grupo.extend((d.len() as u32).to_le_bytes());
            grupo.extend((i as u16 + 1).to_le_bytes());
            datos.push((i as u32 + 1, d.clone()));
        }
        tipos.push((3, datos));
        tipos.push((14, vec![(1, grupo)]));
    }
    if let Some(v) = &r.version {
        tipos.push((16, vec![(1, v.clone())]));
    }
    let rva = 0x1000;
    let rsrc = arbol(&tipos, rva);
    let mut f = vec![0u8; 0x200];
    f[0..2].copy_from_slice(b"MZ");
    f[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    f[0x40..0x44].copy_from_slice(b"PE\0\0");
    f[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes()); // machine
    f[0x46..0x48].copy_from_slice(&1u16.to_le_bytes()); // secciones
    f[0x54..0x56].copy_from_slice(&0xF0u16.to_le_bytes()); // tamaño de la cabecera opcional
    let opcional = 0x58;
    f[opcional..opcional + 2].copy_from_slice(&0x20Bu16.to_le_bytes());
    let dir_recursos = opcional + 112 + 2 * 8;
    f[dir_recursos..dir_recursos + 4].copy_from_slice(&rva.to_le_bytes());
    f[dir_recursos + 4..dir_recursos + 8].copy_from_slice(&(rsrc.len() as u32).to_le_bytes());
    let tabla = opcional + 0xF0;
    f[tabla..tabla + 5].copy_from_slice(b".rsrc");
    f[tabla + 8..tabla + 12].copy_from_slice(&(rsrc.len() as u32).to_le_bytes());
    f[tabla + 12..tabla + 16].copy_from_slice(&rva.to_le_bytes());
    f[tabla + 16..tabla + 20].copy_from_slice(&(rsrc.len() as u32).to_le_bytes());
    f[tabla + 20..tabla + 24].copy_from_slice(&0x200u32.to_le_bytes());
    f.extend(rsrc);
    f
}

fn exe_con_nombre(cadenas: &[(&str, &str)]) -> Vec<u8> {
    pe_falso(&Recursos {
        version: Some(version(cadenas)),
        iconos: vec![],
    })
}

// ---- la casa de prueba ---------------------------------------------------------------------

const UMU_FALSO: &str = r#"#!/bin/sh
# Anota con qué lo llamaron y se queda «corriendo» un rato (con el WINEPREFIX en su entorno).
d=$(dirname "$0")
printf '%s\n' "$@" > "$d/umu-args"
env | grep -E '^(WINEPREFIX|PROTONPATH|GAMEID|STORE|VORTEX_RUNNER_ID)=' | sort > "$d/umu-env"
pwd > "$d/umu-pwd"
if [ -f "$d/instala" ]; then
  src=$(cat "$d/instala")
  mkdir -p "$WINEPREFIX/drive_c/Program Files/Juego" "$WINEPREFIX/drive_c/windows"
  cp "$src" "$WINEPREFIX/drive_c/Program Files/Juego/Juego.exe"
  cp "$src" "$WINEPREFIX/drive_c/Program Files/Juego/unins000.exe"
  cp "$src" "$WINEPREFIX/drive_c/windows/notepad.exe"
fi
dura=0
[ -f "$d/dura" ] && dura=$(cat "$d/dura")
exec sleep "$dura"
"#;

struct Casa {
    t: PathBuf,
    ctx: Contexto,
    bib: Biblioteca,
    juego: PathBuf,
}

impl Casa {
    fn new(nombre: &str) -> Casa {
        let t = vortex_core::carpeta_de_prueba(nombre).canonicalize().unwrap();
        let umu = t.join("umu-run");
        fs::write(&umu, UMU_FALSO).unwrap();
        fs::set_permissions(&umu, fs::Permissions::from_mode(0o755)).unwrap();
        for n in ["GE-Proton9-20", "proton-cachyos-slr", "GE-Proton10-3"] {
            let c = t.join("protones").join(n);
            fs::create_dir_all(&c).unwrap();
            fs::write(c.join("proton"), "#!/bin/sh\n").unwrap();
            fs::write(c.join("version"), format!("123 {n}\n")).unwrap();
        }
        fs::create_dir_all(t.join("protones/no-es-proton")).unwrap();
        let juego = t.join("Mis juegos").join("super_juego-x64.exe");
        fs::create_dir_all(juego.parent().unwrap()).unwrap();
        fs::write(&juego, [b"MZ".as_slice(), &[0u8; 200]].concat()).unwrap();
        let ctx = Contexto::de_prueba(&t, Some(umu));
        let mut bib = Biblioteca::new(ctx.rutas.clone());
        bib.comando = "vortex-runner".into();
        Casa { t, ctx, bib, juego }
    }

    fn estado(&self) -> Estado {
        Estado::new(self.ctx.clone())
    }

    fn dura(&self, s: u32) {
        fs::write(self.t.join("dura"), s.to_string()).unwrap();
    }

    fn sin_umu(&mut self) {
        self.ctx.umu = Some(self.t.join("no-existe"));
    }
}

impl Drop for Casa {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.t);
    }
}

fn esperar(mut f: impl FnMut() -> bool, segundos: u64) -> bool {
    let limite = Instant::now() + Duration::from_secs(segundos);
    while Instant::now() < limite {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    f()
}

// ---- PE ------------------------------------------------------------------------------------

#[test]
fn pe_no_es_pe() {
    let c = vortex_core::carpeta_de_prueba("pe-no");
    let f = c.join("raro_juego-x64.exe");
    fs::write(&f, "hola").unwrap();
    assert_eq!(pe::guardar_icono(&f, &c.join("icono")), None);
    assert_eq!(pe::nombre(&f), pe::bonito("raro_juego-x64"));
    assert_eq!(pe::nombre(&c.join("no-existe.exe")), "No Existe");
    // Un PE cortado a la mitad tampoco truena
    let bueno = pe_falso(&Recursos {
        version: Some(version(&[("ProductName", "Algo")])),
        iconos: vec![(16, 32, vec![1; 40])],
    });
    for corte in [10, 0x41, 0x80, 0x150, 0x210, bueno.len() - 3] {
        fs::write(&f, &bueno[..corte]).unwrap();
        let _ = pe::nombre(&f);
        let _ = pe::guardar_icono(&f, &c.join("icono"));
    }
    let _ = fs::remove_dir_all(c);
}

#[test]
fn pe_nombre_bonito() {
    assert_eq!(pe::bonito("super_juego-x64"), "Super Juego");
    assert_eq!(pe::bonito("HollowKnight"), "Hollow Knight");
    assert_eq!(pe::bonito("Game.Shipping"), "Game");
    assert_eq!(pe::bonito("x64"), "x64"); // si no queda nada, el nombre tal cual
    assert_eq!(pe::bonito("MI JUEGO"), "MI JUEGO");
}

#[test]
fn pe_nombre_de_la_version() {
    let c = vortex_core::carpeta_de_prueba("pe-nombre");
    let casos: [(&[(&str, &str)], &str); 6] = [
        (
            &[("ProductName", "Mi Gran Juego"), ("FileDescription", "otra cosa")],
            "Mi Gran Juego",
        ),
        (
            &[("ProductName", "Microsoft"), ("FileDescription", "Editor Chulo")],
            "Editor Chulo",
        ),
        (&[("FileDescription", "Solo descripción")], "Solo descripción"),
        (
            &[("ProductName", "Windows"), ("FileDescription", "Setup")],
            "Mi Exe",
        ),
        (&[("ProductName", "ñandú ÁÉÍ — Ünico")], "ñandú ÁÉÍ — Ünico"),
        (&[("ProductName", &"x".repeat(61))], "Mi Exe"),
    ];
    for (i, (cadenas, esperado)) in casos.iter().enumerate() {
        let f = c.join(format!("c{i}")).join("mi_exe.exe");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, exe_con_nombre(cadenas)).unwrap();
        assert_eq!(pe::nombre(&f), *esperado, "caso {i}");
    }
    let _ = fs::remove_dir_all(c);
}

#[test]
fn pe_icono_el_mas_grande() {
    let c = vortex_core::carpeta_de_prueba("pe-icono");
    // 256 px en PNG gana a los chicos
    let f = c.join("a.exe");
    fs::write(
        &f,
        pe_falso(&Recursos {
            version: None,
            iconos: vec![
                (16, 32, vec![7; 30]),
                (0, 32, PNG.to_vec()),
                (48, 32, vec![8; 20]),
            ],
        }),
    )
    .unwrap();
    let ic = pe::guardar_icono(&f, &c.join("iconos/a")).unwrap();
    assert_eq!(ic, c.join("iconos/a.png"));
    assert_eq!(fs::read(&ic).unwrap(), PNG);
    // sin PNG: un .ico con el más grande (48) y su cabecera
    let g = c.join("b.exe");
    let dib = vec![9u8; 64];
    fs::write(
        &g,
        pe_falso(&Recursos {
            version: None,
            iconos: vec![(16, 32, vec![7; 30]), (48, 32, dib.clone())],
        }),
    )
    .unwrap();
    let ic = pe::guardar_icono(&g, &c.join("iconos/b")).unwrap();
    assert_eq!(ic, c.join("iconos/b.ico"));
    let b = fs::read(&ic).unwrap();
    assert_eq!(&b[..6], &[0, 0, 1, 0, 1, 0]);
    assert_eq!(&b[6..8], &[48, 48]);
    assert_eq!(u32::from_le_bytes(b[14..18].try_into().unwrap()), 64);
    assert_eq!(u32::from_le_bytes(b[18..22].try_into().unwrap()), 22);
    assert_eq!(&b[22..], dib.as_slice());
    // sin íconos
    let h = c.join("c.exe");
    fs::write(&h, exe_con_nombre(&[("ProductName", "X")])).unwrap();
    assert_eq!(pe::guardar_icono(&h, &c.join("iconos/c")), None);
    let _ = fs::remove_dir_all(c);
}

// ---- Proton --------------------------------------------------------------------------------

#[test]
fn protones_orden() {
    let casa = Casa::new("protones-orden");
    let l = protones::lista(&casa.ctx);
    let ids: Vec<_> = l.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "proton-cachyos-slr",
            "GE-Proton10-3",
            "GE-Proton9-20",
            "GE-Proton"
        ]
    );
    assert_eq!(l[0].nombre, "Proton CachyOS");
    assert_eq!(l[0].version, "proton-cachyos-slr");
    assert_eq!(protones::por_defecto(&l).id, "proton-cachyos-slr");
    assert_eq!(
        protones::buscar(Some("ya-no-existe"), &l).id,
        "proton-cachyos-slr"
    );
    assert_eq!(protones::buscar(Some("GE-Proton"), &l).ruta, "GE-Proton");
}

#[test]
fn protones_sin_cachyos() {
    let mut casa = Casa::new("protones-sin");
    fs::remove_dir_all(casa.t.join("protones/proton-cachyos-slr")).unwrap();
    let l = protones::lista(&casa.ctx);
    assert_eq!(protones::por_defecto(&l).id, "GE-Proton10-3");
    assert_eq!(correr::falta_motor(&casa.ctx), ["proton-cachyos-slr"]);
    casa.sin_umu();
    assert_eq!(
        correr::falta_motor(&casa.ctx),
        ["umu-launcher", "proton-cachyos-slr"]
    );
}

#[test]
fn protones_oficiales_de_steam_solo_los_que_se_llaman_proton() {
    let casa = Casa::new("protones-steam");
    let common = casa.t.join("steam/steamapps/common");
    for n in ["Proton 9.0", "Proton - Experimental", "Half-Life"] {
        fs::create_dir_all(common.join(n)).unwrap();
        fs::write(common.join(n).join("proton"), "x").unwrap();
    }
    let mut ctx = casa.ctx.clone();
    ctx.carpetas_proton = Some(vec![common, casa.t.join("protones")]);
    let ids: Vec<String> = protones::lista(&ctx).into_iter().map(|p| p.id).collect();
    assert!(ids.contains(&"Proton 9.0".to_string()) && ids.contains(&"Proton - Experimental".to_string()));
    assert!(!ids.contains(&"Half-Life".to_string()));
    // los repetidos (el mismo Proton por dos carpetas) salen una vez
    ctx.carpetas_proton = Some(vec![casa.t.join("protones"), casa.t.join("protones")]);
    assert_eq!(protones::lista(&ctx).len(), 4);
}

// ---- biblioteca ----------------------------------------------------------------------------

#[test]
fn agregar_y_cambiar() {
    let casa = Casa::new("bib-agregar");
    let b = &casa.bib;
    let e = b.agregar(&casa.juego, None, None).unwrap();
    assert_eq!(e.nombre, "Super Juego");
    assert!(Path::new(&e.prefijo).is_dir());
    assert!(Path::new(&e.prefijo).starts_with(casa.ctx.rutas.prefijos()));
    assert_eq!(b.agregar(&casa.juego, None, None).unwrap().id, e.id); // no se repite
    assert_eq!(b.cargar().len(), 1);
    b.cambiar(
        &e.id,
        &[
            ("nombre", "  Mi Juego "),
            ("argumentos", "-dx11 -windowed"),
            ("hackeo", "no"),
        ],
    )
    .unwrap();
    let x = b.por_id(&e.id).unwrap();
    assert_eq!(
        (x.nombre.as_str(), x.argumentos.as_str()),
        ("Mi Juego", "-dx11 -windowed")
    );
    assert!(!x.otras.contains_key("hackeo"));
    b.cambiar(&e.id, &[("nombre", "   ")]).unwrap();
    assert_eq!(b.por_id(&e.id).unwrap().nombre, "Mi Juego");
}

#[test]
fn agregar_usa_el_nombre_y_el_icono_del_exe() {
    let casa = Casa::new("bib-pe");
    let exe = casa.t.join("Mis juegos/xyz.exe");
    let rec = Recursos {
        version: Some(version(&[("ProductName", "Súper Aventura")])),
        iconos: vec![(0, 32, PNG.to_vec())],
    };
    fs::write(&exe, pe_falso(&rec)).unwrap();
    let e = casa.bib.agregar(&exe, None, None).unwrap();
    assert_eq!(e.nombre, "Súper Aventura");
    assert!(e.id.starts_with("super-aventura-"), "{}", e.id);
    assert!(e.icono.ends_with(".png") && Path::new(&e.icono).is_file());
    // con nombre elegido manda el elegido
    let otro = casa.t.join("Mis juegos/otro.exe");
    fs::write(&otro, pe_falso(&rec)).unwrap();
    assert_eq!(
        casa.bib.agregar(&otro, Some("Mi Nombre"), None).unwrap().nombre,
        "Mi Nombre"
    );
}

#[test]
fn errores_y_biblioteca_rota() {
    let casa = Casa::new("bib-errores");
    let b = &casa.bib;
    let e = b.agregar(&casa.t.join("nada.exe"), None, None).unwrap_err();
    assert_eq!(e.to_string(), "No encuentro nada.exe.");
    let txt = casa.t.join("notas.txt");
    fs::write(&txt, "x").unwrap();
    assert_eq!(
        b.agregar(&txt, None, None).unwrap_err().to_string(),
        "notas.txt no es un programa de Windows (.exe, .msi o .bat)."
    );
    assert_eq!(
        b.cambiar("no-hay", &[]).unwrap_err().to_string(),
        "Ese programa ya no está en tu biblioteca."
    );
    let rota = casa.ctx.rutas.biblioteca();
    fs::create_dir_all(rota.parent().unwrap()).unwrap();
    for contenido in [
        "{roto",
        "[]",
        "{\"programas\": 3}",
        "{\"programas\": [1, {\"nombre\": \"sin id\"}]}",
    ] {
        fs::write(&rota, contenido).unwrap();
        assert!(b.cargar().is_empty(), "{contenido}");
    }
}

#[test]
fn lee_y_conserva_lo_que_escribio_python() {
    let casa = Casa::new("bib-python");
    let rota = casa.ctx.rutas.biblioteca();
    fs::create_dir_all(rota.parent().unwrap()).unwrap();
    // Lo que escribe biblioteca.py (ensure_ascii=False, indent=1), con una clave de una versión futura
    fs::write(
        &rota,
        r#"{
 "version": 1,
 "programas": [
  {
   "id": "otro-123abc",
   "nombre": "Ñandú",
   "exe": "/no/existe.exe",
   "prefijo": "/no/prefijo",
   "proton": "",
   "argumentos": "-x",
   "icono": "",
   "agregado": 1700000000,
   "ultima_vez": 1700000100,
   "minutos": 7,
   "lanzador": true,
   "clave_futura": {"a": [1, 2]}
  }
 ]
}"#,
    )
    .unwrap();
    let l = casa.bib.cargar();
    assert_eq!(l.len(), 1);
    assert_eq!(
        (l[0].nombre.as_str(), l[0].minutos, l[0].lanzador),
        ("Ñandú", 7, true)
    );
    casa.bib.sumar_tiempo("otro-123abc", 120.0, 1700000500.0);
    let texto = fs::read_to_string(&rota).unwrap();
    assert!(texto.contains("\"clave_futura\""), "{texto}");
    assert!(texto.contains("Ñandú") && !texto.contains("\\u"), "{texto}");
    assert!(
        texto.starts_with("{\n \"version\": 1,\n \"programas\": [\n  {\n   \"id\": \"otro-123abc\""),
        "{texto}"
    );
    let x = casa.bib.por_id("otro-123abc").unwrap();
    assert_eq!((x.minutos, x.ultima_vez), (9, 1700000500));
    assert!(!rota.with_extension("tmp").exists());
}

#[test]
fn lanzador_y_quitar() {
    let casa = Casa::new("bib-lanzador");
    let b = &casa.bib;
    let mut e = b.agregar(&casa.juego, None, None).unwrap();
    let d = b.crear_lanzador(&mut e).unwrap();
    let texto = fs::read_to_string(&d).unwrap();
    assert!(
        texto.contains(&format!("Exec=vortex-runner --correr {}\n", e.id)),
        "{texto}"
    );
    assert!(
        texto.contains("Name=Super Juego\n") && texto.contains("Icon=vortex-runner\n"),
        "{texto}"
    );
    assert!(
        texto.starts_with("[Desktop Entry]\nType=Application\n") && texto.ends_with("StartupNotify=false\n")
    );
    assert!(b.por_id(&e.id).unwrap().lanzador);
    b.cambiar(&e.id, &[("nombre", "Otro")]).unwrap();
    assert!(fs::read_to_string(&d).unwrap().contains("Name=Otro")); // el lanzador se pone al día
    fs::create_dir_all(Path::new(&e.prefijo).join("drive_c")).unwrap();
    b.quitar(&e.id, false);
    assert!(!d.exists());
    assert!(Path::new(&e.prefijo).is_dir()); // tus partidas se quedan
    let e2 = b.agregar(&casa.juego, None, None).unwrap();
    b.quitar(&e2.id, true);
    assert!(!Path::new(&e2.prefijo).exists());
    assert!(casa.juego.exists()); // el .exe tuyo nunca se borra
    assert!(b.cargar().is_empty());
}

#[test]
fn el_comando_del_lanzador_con_espacios_va_entre_comillas() {
    let casa = Casa::new("bib-comando");
    let mut b = casa.bib.clone();
    b.comando = "\"/opt/mis apps/vortex-runner-rs\"".into();
    let mut e = b.agregar(&casa.juego, None, None).unwrap();
    let texto = fs::read_to_string(b.crear_lanzador(&mut e).unwrap()).unwrap();
    assert!(texto.contains(&format!(
        "Exec=\"/opt/mis apps/vortex-runner-rs\" --correr {}\n",
        e.id
    )));
}

#[test]
fn no_borra_prefijo_ajeno() {
    let casa = Casa::new("bib-ajeno");
    let ajeno = casa.t.join("mi-prefijo-de-lutris");
    fs::create_dir_all(&ajeno).unwrap();
    let e = casa.bib.agregar(&casa.juego, None, Some(&ajeno)).unwrap();
    casa.bib.quitar(&e.id, true);
    assert!(ajeno.is_dir());
}

#[test]
fn parece_instalador() {
    for n in [
        "setup.exe",
        "Steam_Install.exe",
        "programa-instalador.exe",
        "algo.msi",
    ] {
        assert!(biblioteca::parece_instalador(Path::new(n)), "{n}");
    }
    assert!(!biblioteca::parece_instalador(Path::new("Juego.exe")));
}

#[test]
fn prefijo_propio_y_descartar() {
    let casa = Casa::new("bib-prefijo");
    let b = &casa.bib;
    let pre = casa.ctx.rutas.prefijos().join("instalado-abc");
    let exe = pre.join("drive_c/Program Files/Juego/Juego.exe");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, [b"MZ".as_slice(), &[0u8; 100]].concat()).unwrap();
    assert_eq!(b.prefijo_de(&exe), Some(pre.clone()));
    assert_eq!(b.prefijo_de(&casa.juego), None);
    let e = b.agregar(&exe, None, None).unwrap();
    assert_eq!(e.prefijo, pre.to_string_lossy());
    assert!(!b.descartar_prefijo(&pre)); // lo usa un programa
    assert!(pre.is_dir());
    let otro = casa.ctx.rutas.prefijos().join("huerfano");
    fs::create_dir_all(&otro).unwrap();
    assert!(b.descartar_prefijo(&otro));
    assert!(!otro.exists());
    let fuera = casa.t.join("no-es-nuestro");
    fs::create_dir_all(&fuera).unwrap();
    assert!(!b.descartar_prefijo(&fuera)); // fuera de prefijos/: nunca
    assert!(fuera.is_dir());
}

#[test]
fn exes_nuevos_sin_los_de_serie() {
    let casa = Casa::new("bib-nuevos");
    let pre = casa.t.join("pre");
    let c = pre.join("drive_c");
    for rel in [
        "Program Files/Windows NT/Accessories/wordpad.exe",
        "Program Files (x86)/Internet Explorer/iexplore.exe",
        "Program Files/Proton Cosa/cosa.exe",
        "Program Files/Mi Juego/Juego.exe",
        "Program Files/Mi Juego/unins000.exe",
        "windows/system32/notepad.exe",
        "ProgramData/algo/x.exe",
    ] {
        fs::create_dir_all(c.join(rel).parent().unwrap()).unwrap();
        fs::write(c.join(rel), [b"MZ".as_slice(), &[0u8; 100]].concat()).unwrap();
    }
    fs::write(
        pre.join("tracked_files"),
        "drive_c/Program Files/Proton Cosa/cosa.exe\n",
    )
    .unwrap();
    let nuevos: Vec<_> = biblioteca::exes_nuevos(&pre, &BTreeSet::new())
        .into_iter()
        .map(|n| n.exe)
        .collect();
    assert_eq!(
        nuevos,
        [c.join("Program Files/Mi Juego/Juego.exe").to_string_lossy()]
    );
}

#[test]
fn exes_nuevos_ordena_el_mas_probable_primero() {
    let casa = Casa::new("bib-orden");
    let pre = casa.t.join("pre");
    let c = pre.join("drive_c");
    for (rel, tam) in [
        ("Games/Chico/a.exe", 10),
        ("Program Files/Grande/b.exe", 500),
        ("Program Files/Chico/c.exe", 50),
        ("Games/Enorme/d.exe", 9000),
    ] {
        fs::create_dir_all(c.join(rel).parent().unwrap()).unwrap();
        fs::write(c.join(rel), vec![b'M'; tam]).unwrap();
    }
    let antes: BTreeSet<String> = [c.join("Games/Chico/a.exe").to_string_lossy().into_owned()].into();
    let n: Vec<_> = biblioteca::exes_nuevos(&pre, &antes)
        .into_iter()
        .map(|n| n.nombre)
        .collect();
    assert_eq!(n, ["B", "C", "D"]); // primero «Program Files» (el más grande antes), luego los demás; sin los de «antes»
}

// ---- correr --------------------------------------------------------------------------------

#[test]
fn lanzar_con_umu() {
    let casa = Casa::new("correr-lanzar");
    let b = &casa.bib;
    let e = b.agregar(&casa.juego, None, None).unwrap();
    b.cambiar(&e.id, &[("argumentos", "-windowed 'con espacio'")])
        .unwrap();
    let e = b.por_id(&e.id).unwrap();
    correr::lanzar(&casa.ctx, &e, None, &[]).unwrap().wait().unwrap();
    let args = fs::read_to_string(casa.t.join("umu-args")).unwrap();
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        [casa.juego.to_str().unwrap(), "-windowed", "con espacio"]
    );
    let env = fs::read_to_string(casa.t.join("umu-env")).unwrap();
    let cada: Vec<&str> = env.lines().collect();
    assert!(cada.contains(&format!("WINEPREFIX={}", e.prefijo).as_str()));
    let ruta_proton = casa.t.join("protones/proton-cachyos-slr");
    assert!(
        cada.contains(&format!("PROTONPATH={}", ruta_proton.display()).as_str()),
        "{cada:?}"
    );
    assert!(cada.contains(&"GAMEID=umu-default") && cada.contains(&"STORE=none"));
    assert!(cada.contains(&format!("VORTEX_RUNNER_ID={}", e.id).as_str()));
    assert_eq!(
        fs::read_to_string(casa.t.join("umu-pwd")).unwrap().trim(),
        casa.juego.parent().unwrap().to_str().unwrap()
    );
    assert!(casa.ctx.rutas.registros().join(format!("{}.log", e.id)).is_file());
    b.cambiar(&e.id, &[("proton", "GE-Proton")]).unwrap();
    correr::lanzar(&casa.ctx, &b.por_id(&e.id).unwrap(), None, &[])
        .unwrap()
        .wait()
        .unwrap();
    assert!(
        fs::read_to_string(casa.t.join("umu-env"))
            .unwrap()
            .contains("PROTONPATH=GE-Proton\n")
    );
}

#[test]
fn msi_y_otros_exe_en_su_prefijo() {
    let casa = Casa::new("correr-msi");
    let msi = casa.t.join("algo.msi");
    fs::write(&msi, "x").unwrap();
    let e = casa.bib.agregar(&msi, None, None).unwrap();
    assert_eq!(
        correr::orden(&casa.ctx, &e, None, &[])[1..],
        ["msiexec", "/i", msi.to_str().unwrap()]
    );
    // winecfg (otro .exe del mismo prefijo): sin los argumentos del programa, con los extra
    let mut j = casa.bib.agregar(&casa.juego, None, None).unwrap();
    j.argumentos = "-x".into();
    assert_eq!(
        correr::orden(&casa.ctx, &j, Some("winecfg"), &["/v".into()])[1..],
        ["winecfg", "/v"]
    );
    assert_eq!(
        correr::orden(&casa.ctx, &j, None, &[])[1..],
        [casa.juego.to_str().unwrap(), "-x"]
    );
}

#[test]
fn corriendo_y_detener() {
    let casa = Casa::new("correr-detener");
    let e = casa.bib.agregar(&casa.juego, None, None).unwrap();
    casa.dura(30);
    let mut hijo = correr::lanzar(&casa.ctx, &e, None, &[]).unwrap();
    assert!(esperar(|| correr::corriendo(&e), 5));
    assert!(correr::prefijos_abiertos().contains(&e.prefijo));
    correr::detener(&e, Duration::from_secs(3));
    let _ = hijo.wait();
    assert!(!correr::corriendo(&e));
}

#[test]
fn correr_y_esperar_suma_tiempo() {
    let casa = Casa::new("correr-esperar");
    let e = casa.bib.agregar(&casa.juego, None, None).unwrap();
    assert_eq!(correr::correr_y_esperar(&casa.ctx, &casa.bib, &e).unwrap(), 0);
    let x = casa.bib.por_id(&e.id).unwrap();
    assert!(x.ultima_vez > 0);
    assert_eq!(x.minutos, 0);
}

#[test]
fn sin_umu() {
    let mut casa = Casa::new("correr-sin-umu");
    let e = casa.bib.agregar(&casa.juego, None, None).unwrap();
    casa.sin_umu();
    let err = correr::lanzar(&casa.ctx, &e, None, &[]).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    assert!(err.to_string().contains("umu-run"));
}

#[test]
fn instalar_motor_con_pkexec_falso() {
    let mut casa = Casa::new("correr-pkexec");
    fs::remove_dir_all(casa.t.join("protones/proton-cachyos-slr")).unwrap();
    let pk = casa.t.join("pkexec");
    fs::write(
        &pk,
        format!("#!/bin/sh\necho \"$@\" > \"{}/pk\"\n", casa.t.display()),
    )
    .unwrap();
    fs::set_permissions(&pk, fs::Permissions::from_mode(0o755)).unwrap();
    casa.ctx.pkexec = pk.to_string_lossy().into_owned();
    assert_eq!(correr::instalar_motor(&casa.ctx).unwrap().codigo, 0);
    assert_eq!(
        fs::read_to_string(casa.t.join("pk")).unwrap().trim(),
        "pacman -S --needed --noconfirm proton-cachyos-slr"
    );
    // si no falta nada, no se llama a pkexec
    fs::remove_file(casa.t.join("pk")).unwrap();
    fs::create_dir_all(casa.t.join("protones/proton-cachyos-slr")).unwrap();
    fs::write(casa.t.join("protones/proton-cachyos-slr/proton"), "x").unwrap();
    assert_eq!(correr::instalar_motor(&casa.ctx).unwrap().codigo, 0);
    assert!(!casa.t.join("pk").exists());
    // pkexec que no existe: error, no pánico
    fs::remove_dir_all(casa.t.join("protones/proton-cachyos-slr")).unwrap();
    casa.ctx.pkexec = casa.t.join("no-hay-pkexec").to_string_lossy().into_owned();
    assert!(correr::instalar_motor(&casa.ctx).is_err());
}

#[test]
fn lo_que_deja_un_instalador() {
    let casa = Casa::new("correr-instalador");
    let b = &casa.bib;
    let modelo = casa.t.join("modelo.exe");
    fs::write(
        &modelo,
        pe_falso(&Recursos {
            version: Some(version(&[("ProductName", "Juego Instalado")])),
            iconos: vec![(0, 32, PNG.to_vec())],
        }),
    )
    .unwrap();
    fs::write(casa.t.join("instala"), modelo.to_str().unwrap()).unwrap();
    let setup = casa.t.join("setup.exe");
    fs::copy(&modelo, &setup).unwrap();
    let prefijo = b.nuevo_prefijo("Juego").unwrap();
    let antes = biblioteca::exes_en(&prefijo);
    let e = correr::temporal("instalando", &prefijo.to_string_lossy(), setup.to_str().unwrap());
    correr::lanzar(&casa.ctx, &e, None, &[]).unwrap().wait().unwrap();
    let nuevos = biblioteca::exes_nuevos(&prefijo, &antes);
    let nombres: Vec<_> = nuevos
        .iter()
        .map(|n| {
            Path::new(&n.exe)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(nombres, ["Juego.exe"]); // sin el desinstalador ni windows/
    assert_eq!(nuevos[0].nombre, "Juego Instalado");
    let x = b
        .agregar(Path::new(&nuevos[0].exe), None, Some(&prefijo))
        .unwrap();
    assert_eq!(x.prefijo, prefijo.to_string_lossy());
    assert!(!x.icono.is_empty());
}

// ---- una sola ventana ----------------------------------------------------------------------

#[test]
fn una_sola_ventana() {
    use vortex_runner::instancia::{Escucha, Mensaje, avisar_a_la_abierta, nombre};
    let t = vortex_core::carpeta_de_prueba("instancia");
    let sock = t.join(nombre());
    assert!(!avisar_a_la_abierta(&sock, &Mensaje::Mostrar)); // nadie escucha
    let escucha = Escucha::new(&sock).unwrap();
    assert!(avisar_a_la_abierta(&sock, &Mensaje::Mostrar));
    assert!(avisar_a_la_abierta(
        &sock,
        &Mensaje::Abrir("/x/mi juego.exe".into())
    ));
    let mut todos = Vec::new();
    assert!(esperar(
        || {
            todos.extend(escucha.revisar());
            todos.len() >= 2
        },
        3
    ));
    assert_eq!(
        todos,
        [Mensaje::Mostrar, Mensaje::Abrir("/x/mi juego.exe".into())]
    );
    assert!(nombre().starts_with("vortex-runner-"));
    drop(escucha);
    assert!(!sock.exists());
    let _ = fs::remove_dir_all(t);
}

// ---- lo que hacía puente.py ----------------------------------------------------------------

fn avisos(ev: &[Evento]) -> Vec<(String, String)> {
    ev.iter()
        .filter_map(|e| match e {
            Evento::Aviso(t, k) => Some((t.clone(), k.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn flujo_de_la_biblioteca() {
    let casa = Casa::new("estado-flujo");
    casa.dura(30);
    let otro = casa.t.join("Mis juegos/Otro Programa.exe");
    fs::write(&otro, [b"MZ".as_slice(), &[0u8; 200]].concat()).unwrap();
    let mut s = casa.estado();
    assert!(s.programas().is_empty());
    assert!(s.falta_motor().is_empty());
    let url = format!("file://{}", casa.juego.to_str().unwrap().replace(' ', "%20"));
    let id1 = s.agregar(&url);
    let id2 = s.agregar(otro.to_str().unwrap());
    assert!(!id1.is_empty() && !id2.is_empty());
    assert_eq!(s.agregar(otro.to_str().unwrap()), id2); // repetido: el mismo
    let mut nombres: Vec<_> = s.programas().iter().map(|p| p.nombre.clone()).collect();
    nombres.sort();
    assert_eq!(nombres, ["Otro Programa", "Super Juego"]);
    assert!(
        avisos(&s.tomar_eventos())
            .iter()
            .any(|(t, k)| t == "Agregué Super Juego." && k == "exito")
    );
    // «Configurar»
    s.guardar(&id1, "Mi Juego", "GE-Proton10-3", "-windowed");
    let p = s.programas().iter().find(|p| p.nombre == "Mi Juego").unwrap();
    assert_eq!(
        (p.proton.as_str(), p.argumentos.as_str()),
        ("GE-Proton10-3", "-windowed")
    );
    assert!(avisos(&s.tomar_eventos()).contains(&("Listo, guardado.".into(), "exito".into())));
    // jugar y ver que está abierto
    s.jugar(&id1);
    assert!(esperar(|| correr::corriendo(&casa.bib.por_id(&id1).unwrap()), 5));
    s.revisar(&correr::prefijos_abiertos());
    let p = s.programas().iter().find(|p| p.id == id1).unwrap();
    assert!(p.abierto);
    assert_eq!(p.subtitulo, "Abierto ahora");
    assert!(
        avisos(&s.tomar_eventos())
            .iter()
            .any(|(t, _)| t.starts_with("Abriendo Mi Juego…"))
    );
    s.jugar(&id1);
    assert!(avisos(&s.tomar_eventos()).contains(&("Mi Juego ya está abierto.".into(), "info".into())));
    // detener
    s.detener(&id1);
    assert!(esperar(|| !correr::corriendo(&casa.bib.por_id(&id1).unwrap()), 8));
    s.revisar(&correr::prefijos_abiertos());
    let p = s.programas().iter().find(|p| p.id == id1).unwrap();
    assert!(!p.abierto);
    assert!(
        p.subtitulo.starts_with("Abierto hace un momento"),
        "{}",
        p.subtitulo
    );
    assert!(casa.bib.por_id(&id1).unwrap().ultima_vez > 0);
    // los programas se ordenan por «última vez»
    assert_eq!(s.programas()[0].id, id1);
    // quitar
    s.quitar(&id1, true);
    assert_eq!(s.programas().len(), 1);
    assert!(avisos(&s.tomar_eventos()).contains(&("Quité Mi Juego de tu biblioteca.".into(), "info".into())));
}

#[test]
fn las_preguntas() {
    let casa = Casa::new("estado-preguntas");
    let setup = casa.t.join("Mis juegos/setup_cosa.exe");
    fs::write(&setup, [b"MZ".as_slice(), &[0u8; 200]].concat()).unwrap();
    let mut s = casa.estado();
    s.tomar_eventos();
    s.abrir_archivo(setup.to_str().unwrap());
    assert!(s.tomar_eventos().contains(&Evento::PreguntarArchivo {
        ruta: setup.to_str().unwrap().into(),
        nombre: "Setup Cosa".into(),
        instalador: true
    }));
    s.abrir_archivo("/no/existe.exe");
    assert!(avisos(&s.tomar_eventos()).contains(&("No encuentro ese archivo.".into(), "error".into())));
    // Un .exe que ya está en la biblioteca se abre directo (sin preguntar)
    let id = s.agregar(casa.juego.to_str().unwrap());
    s.tomar_eventos();
    s.abrir_archivo(casa.juego.to_str().unwrap());
    let ev = s.tomar_eventos();
    assert!(!ev.iter().any(|e| matches!(e, Evento::PreguntarArchivo { .. })));
    assert!(
        avisos(&ev)
            .iter()
            .any(|(t, _)| t.starts_with("Abriendo Super Juego")),
        "{ev:?} {id}"
    );
}

#[test]
fn instalar_y_elegir_el_programa() {
    let casa = Casa::new("estado-instalar");
    let modelo = casa.t.join("modelo.exe");
    fs::write(&modelo, exe_con_nombre(&[("ProductName", "Juego Instalado")])).unwrap();
    fs::write(casa.t.join("instala"), modelo.to_str().unwrap()).unwrap();
    casa.dura(1);
    let setup = casa.t.join("Mis juegos/setup_cosa.exe");
    fs::copy(&modelo, &setup).unwrap();
    let mut s = casa.estado();
    s.tomar_eventos();
    s.instalar(setup.to_str().unwrap());
    assert_eq!(s.instalando(), "setup_cosa");
    let ev = s.tomar_eventos();
    assert!(ev.contains(&Evento::InstalandoCambio));
    s.instalar(setup.to_str().unwrap());
    assert!(avisos(&s.tomar_eventos()).contains(&(
        "Espera a que termine de instalarse setup_cosa.".into(),
        "info".into()
    )));
    // cuando el instalador termina (umu falso: 1 s), la revisión pregunta cuál es el programa
    let mut elegir = None;
    assert!(esperar(
        || {
            s.revisar(&correr::prefijos_abiertos());
            for e in s.tomar_eventos() {
                if let Evento::ElegirInstalado(l) = e {
                    elegir = Some(l);
                }
            }
            elegir.is_some()
        },
        10
    ));
    let l = elegir.unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(
        (l[0].nombre.as_str(), l[0].carpeta.as_str()),
        ("Juego Instalado", "Juego")
    );
    assert_eq!(s.instalando(), "");
    s.agregar_instalado(&l[0].exe, &l[0].nombre);
    assert_eq!(s.programas().len(), 1);
    assert!(
        s.programas()[0].prefijo.contains("/prefijos/setup-cosa-"),
        "el programa corre en el prefijo donde se instaló"
    );
    assert!(
        avisos(&s.tomar_eventos())
            .contains(&("Juego Instalado quedó en tu biblioteca.".into(), "exito".into()))
    );
}

#[test]
fn un_instalador_que_no_deja_nada_se_descarta() {
    let casa = Casa::new("estado-instalar-nada");
    casa.dura(1);
    let setup = casa.t.join("Mis juegos/setup_cosa.exe");
    fs::write(&setup, "MZ").unwrap();
    let mut s = casa.estado();
    s.instalar(setup.to_str().unwrap());
    assert!(esperar(
        || {
            s.revisar(&correr::prefijos_abiertos());
            s.instalando().is_empty()
        },
        10
    ));
    let ev = s.tomar_eventos();
    assert!(!ev.iter().any(|e| matches!(e, Evento::ElegirInstalado(_))));
    assert!(
        avisos(&ev)
            .iter()
            .any(|(t, _)| t.starts_with("El instalador terminó, pero no dejó ningún programa nuevo"))
    );
    let n = fs::read_dir(casa.ctx.rutas.prefijos())
        .map(|r| r.count())
        .unwrap_or(0);
    assert_eq!(n, 0, "el espacio del instalador se borra");
}

#[test]
fn ninguno_borra_lo_instalado() {
    let casa = Casa::new("estado-ninguno");
    let modelo = casa.t.join("modelo.exe");
    fs::write(&modelo, exe_con_nombre(&[("ProductName", "X")])).unwrap();
    fs::write(casa.t.join("instala"), modelo.to_str().unwrap()).unwrap();
    casa.dura(1);
    let mut s = casa.estado();
    s.instalar(modelo.to_str().unwrap());
    assert!(esperar(
        || {
            s.revisar(&correr::prefijos_abiertos());
            s.instalando().is_empty()
        },
        10
    ));
    assert!(fs::read_dir(casa.ctx.rutas.prefijos()).unwrap().count() == 1);
    s.descartar_instalado();
    assert_eq!(fs::read_dir(casa.ctx.rutas.prefijos()).unwrap().count(), 0);
}

#[test]
fn sin_motor() {
    let mut casa = Casa::new("estado-sin-motor");
    casa.sin_umu();
    let mut s = casa.estado();
    assert_eq!(s.falta_motor(), ["umu-launcher"]);
    // Se puede agregar a la biblioteca sin el motor; lo que no se puede es jugar.
    let id = s.agregar(casa.juego.to_str().unwrap());
    assert!(!id.is_empty());
    s.tomar_eventos();
    s.jugar(&id);
    assert!(avisos(&s.tomar_eventos()).iter().any(|(_, k)| k == "error"));
    s.instalar(casa.juego.to_str().unwrap());
    assert!(avisos(&s.tomar_eventos()).iter().any(|(_, k)| k == "error"));
    // instalar el motor: el estado marca «instalando», y al terminar con la contraseña cancelada avisa
    assert!(s.iniciar_motor());
    assert!(!s.iniciar_motor());
    s.fin_motor(126, "");
    assert!(!s.instalando_motor());
    assert!(
        avisos(&s.tomar_eventos())
            .contains(&("No se instaló: cancelaste la contraseña.".into(), "info".into()))
    );
    s.fin_motor(1, "falló algo");
    assert!(
        avisos(&s.tomar_eventos())
            .contains(&("No se pudo instalar el motor: falló algo".into(), "error".into()))
    );
}

#[test]
fn archivo_que_ya_no_esta_y_selector_de_proton() {
    let casa = Casa::new("estado-sin-exe");
    let mut s = casa.estado();
    let id = s.agregar(casa.juego.to_str().unwrap());
    fs::remove_file(&casa.juego).unwrap();
    s.recargar();
    assert!(!s.programas()[0].existe);
    s.tomar_eventos();
    s.jugar(&id);
    assert!(avisos(&s.tomar_eventos()).contains(&(
        "Ya no encuentro el .exe de Super Juego: cámbialo en su configuración.".into(),
        "error".into()
    )));
    let p = s.protones_para_ui();
    assert_eq!(p[0], (String::new(), "El de serie (Proton CachyOS)".to_string()));
    assert_eq!(
        p[1],
        ("proton-cachyos-slr".to_string(), "Proton CachyOS".to_string())
    ); // su versión es igual a su id: sin « · »
    assert_eq!(
        p[2].1,
        "GE-Proton10-3 · GE-Proton10-3".replace(" · GE-Proton10-3", "")
    ); // idem
    assert_eq!(p.last().unwrap().0, "GE-Proton");
}

#[test]
fn correr_desde_un_acceso() {
    let casa = Casa::new("estado-correr");
    let e = casa.bib.agregar(&casa.juego, None, None).unwrap();
    assert_eq!(vortex_runner::estado::correr_programa(&casa.ctx, &e.id), 0);
    assert!(casa.bib.por_id(&e.id).unwrap().ultima_vez > 0);
    assert_eq!(vortex_runner::estado::correr_programa(&casa.ctx, "no-hay"), 1);
}
