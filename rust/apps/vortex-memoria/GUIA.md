# Guía: vortex-memoria en Rust (la escribe Mike)

Tu segunda app. Vas a pasar `bin/vortex-memoria` (Python) a Rust. Es el aviso «Te estás quedando sin
memoria» (y el de «Tu juego va justo de memoria»). Lo lanza el gestor **justo cuando ya casi no hay
memoria**, y en ese momento arrancar un Python entero (~20 MB, y lento) es lo peor que se puede hacer.
En Rust arranca al instante y casi no gasta.

Es un escalón más que la de enlace: vas a leer `/proc` (donde Linux dice cuánto usa cada programa),
agrupar apps, ordenarlas y leer el JSON de Hyprland. Nada de ventanas.

## Dónde estás

- Tu rama es `rust/memoria-mike` y tu carpeta `.claude/worktrees/rust-memoria-mike`.
- Tu código: `rust/apps/vortex-memoria/src/main.rs`. Ábrelo al lado de `bin/vortex-memoria`.
- Son 16 pasos (`todo!("paso N")`). **Ve en orden**: los primeros son cortitos y los de más abajo usan
  los de arriba. Los pasos 1 a 5 tienen pruebas chiquitas.
- Arriba ya están hechas `proc_dir()`, `espera_s()`, `mi_uid()`, `home()` y `meminfo()` (esa usa tu
  paso 2). Y el `struct App`: lee su comentario, es como el diccionario del Python pero con campos fijos.

## Cómo compilar y probar

Desde la carpeta `rust/` de tu worktree:

```bash
cargo build -p vortex-memoria --offline
```

```bash
cargo test -p vortex-memoria --offline
```

Al principio fallan todas con «not yet implemented». Cada paso que llenes hace pasar su prueba
(`paso_1_gb`, `paso_2_leer_clave`, …).

Al final, la prueba grande (la misma del Python), desde la raíz del worktree (no desde `rust/`):

```bash
VM="$PWD/rust/target/debug/vortex-memoria-rs" pruebas/memoria.sh
```

Usa un `/proc`, un Hyprland y un notify-send **falsos**: no te manda avisos de verdad ni cierra nada.
Tiene que decir «Todas las pruebas pasaron.».

## Lo nuevo de Rust en esta (lo de la otra guía sigue valiendo)

| Python | Rust |
|---|---|
| `{"clase": c, "kb": 0}` (diccionario) | `App { clase: c, kb: 0, ..App::default() }` (struct) |
| `app["kb"]` | `app.kb` |
| `usos = {}` / `usos[g] = usos.get(g, 0) + n` | `let mut usos = HashMap::new();` / `*usos.entry(g).or_insert(0) += n;` |
| `for entrada in carpeta.iterdir():` | `for entrada in std::fs::read_dir(carpeta)?.flatten() {` |
| `lista.sort(key=..., reverse=True)` | `lista.sort_by(\|a, b\| b.kb.cmp(&a.kb));` |
| `lista[:3]` | `lista.truncate(3);` |
| `", ".join(nombres)` | `nombres.join(", ")` |
| `v.get("pid", 0)` (del JSON) | `v["pid"].as_u64().unwrap_or(0)` |
| `time.sleep(15)` | `std::thread::sleep(Duration::from_secs_f64(15.0))` |
| `x if cond else y` | `if cond { x } else { y }` |

- **`Option`**: «algo o nada». `Some(app)` o `None`. Se abre con `match` o con `if let Some(a) = ...`.
  El `?` dentro de una función que devuelve `Option` es «si es `None`, devuelve `None` ya».
- **`&` y `.clone()`**: si Rust dice «value moved» o «borrowed», casi siempre se arregla poniendo `&`
  delante (prestarlo) o `.clone()` (hacer una copia).
- **`u64` y `u32`**: números sin signo (nunca negativos). Para pasar de uno a otro: `x as u64`.
- El paso 12a tiene una trampa avisada en su comentario (los empates). Léela.

## Cuando termines

1. Borra la línea `#![allow(unused)]` de arriba y arregla lo que marque.
2. `cargo fmt -p vortex-memoria` y `cargo clippy -p vortex-memoria --offline -- -D warnings`.
3. Pasa la prueba grande (arriba).
4. Guárdalo en tu rama (commit). Desde la raíz del worktree:

```bash
git add rust/apps/vortex-memoria pruebas/memoria.sh
```

```bash
git commit -m "vortex-memoria en Rust: pasos 1 a 16"
```

   (No hace falta `git push`: se queda en tu PC y Claude lo une desde ahí.)
5. Avísale a Claude: la revisa, la mete al paquete `vortex-rust` y pone el salto en `bin/vortex-memoria`.

Si te atoras, dile a Claude en qué paso vas y pégale el error completo.
