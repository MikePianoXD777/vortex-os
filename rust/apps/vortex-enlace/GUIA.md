# Guía: vortex-enlace en Rust (la escribe Mike)

Vas a pasar `bin/vortex-enlace` (Python) a Rust. Hoy, **cada vez que copias algo**, arranca un Python
entero (~20 MB y un poco de CPU) solo para ver si es un enlace de video. En Rust va a ser instantáneo.

## Dónde estás

- Tu rama es `rust/enlace-mike` y tu carpeta `.claude/worktrees/rust-enlace-mike` (una copia aparte del
  repo: lo que hagas aquí no rompe nada más).
- Tu código: `rust/apps/vortex-enlace/src/main.rs`. Ábrelo al lado de `bin/vortex-enlace`.
- Cada función tiene un `todo!("paso N")`: es un hueco. Vas de uno en uno, en orden.

## Cómo compilar y probar

Desde la carpeta `rust/` de tu worktree (en la terminal):

```bash
cargo build -p vortex-enlace --offline
```

```bash
cargo test -p vortex-enlace --offline
```

Al principio las pruebas fallan con «not yet implemented: paso 1». Es normal: cuando llenes el paso 1,
`paso_1_sitio` pasa. Cuando llenes el 7, `paso_7_por_que`.

Al final, la prueba grande (la misma con la que se probó la de Python), contra la tuya:

```bash
VE="$PWD/rust/target/debug/vortex-enlace-rs" pruebas/enlace.sh
```

(esa se corre desde la raíz del worktree, no desde `rust/`). Usa programas falsos y HOME temporal:
no te manda avisos de verdad ni baja nada de internet.

## Lo básico de Rust que vas a usar

| Python | Rust |
|---|---|
| `texto.strip()` | `texto.trim()` |
| `len(texto) > 500` | `texto.len() > 500` |
| `any(c.isspace() for c in texto)` | `texto.chars().any(char::is_whitespace)` |
| `None` / devolver algo | `None` / `Some(algo)` |
| `ruta.exists()` | `ruta.exists()` (igual) |
| `ruta / "archivo"` | `ruta.join("archivo")` |
| `subprocess.run(["a", "b"])` | `Command::new("a").arg("b").status()` |
| `subprocess.run(..., capture_output=True).stdout` | `Command::new("a").output()` → `.stdout` |
| `try: ... except: pass` | `if let Ok(x) = algo() { ... }` |
| `json.loads(...)` | `serde_json::from_str::<serde_json::Value>(...)` |
| `time.time()` | `std::time::SystemTime::now().duration_since(UNIX_EPOCH)` |

- En Rust, si algo puede fallar devuelve un `Result`. Para empezar está bien ignorar el error con
  `let _ = algo();` o usar `if let Ok(...)`. **No uses `.unwrap()` en lo que puede fallar en la PC de
  alguien** (si falla, la app se cierra de golpe).
- `&str` es un texto prestado (como un parámetro), `String` es un texto tuyo (que puedes guardar o
  devolver). Si Rust se queja, casi siempre se arregla con `.to_string()` o con `&`.
- Si un error no lo entiendes, léelo completo: el compilador de Rust casi siempre te dice cómo
  arreglarlo («help: ...»).

## Cuando termines

1. Borra la línea `#![allow(unused)]` de arriba y arregla lo que marque.
2. `cargo fmt -p vortex-enlace` (acomoda el código solo) y `cargo clippy -p vortex-enlace --offline`
   (te da consejos).
3. Pasa la prueba grande (arriba).
4. Haz commit en tu rama y avísale a Claude: él la revisa, la mete al paquete `vortex-rust` y pone el
   salto en `bin/vortex-enlace`, como en las otras apps.

Pídele ayuda a Claude cuando te atores: dile en qué paso vas y pégale el error.
