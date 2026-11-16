# Vortexine's Local AI en Rust (ficha) — Fósil

La app oficial sigue siendo la de Python (`apps/vortex-local-ai`, 1.0.0). Su API completa está en
`apps/vortex-local-ai/API.md`: es la guía para portar el motor.

## Lo que hay que igualar (de la app de Python)

**Motor (sin Qt, va primero, con `cargo test`):**
- [ ] **Rutas** (`rutas.py`): DATOS `~/.local/share/vortex-local-ai`, CACHE `~/.cache/vortex-local-ai`,
      el vault por defecto `~/Documentos/Vortexine Local AI`; `VORTEX_LOCAL_AI_HOME` para las pruebas.
- [ ] **Cliente de Ollama** (`ollama.py`, por HTTP, con streaming): estado, modelos, bajar con avance,
      chat que llega mientras escribe y borrar. Si Ollama no corre, primero arranca
      `vortex-ollama.service` (el de Miku: los modelos de `~/.ollama` se comparten) y si no existe
      `ollama serve`. Errores en español claro. `VORTEX_LOCAL_AI_NO_ARRANCAR=1` para las pruebas.
- [ ] **Catálogo** (`catalogo.py`): los modelos recomendados, `vram_disponible()` (NVIDIA, AMD, Intel o CPU),
      `cabe()` («si», «justo» o «no») y `recomendado()`.
- [ ] **Vault** (`vault.py`): carpetas de notas Markdown como Obsidian: [[enlaces]] (con #sección, ^bloque,
      |alias y ![[incrustados]]), #etiquetas anidadas, frontmatter con `tags` y `aliases`, retroenlaces y
      el grafo. Abre cualquier carpeta (también un vault de Obsidian) sin romperla.
- [ ] **Memoria / «Preguntar a mi vault»** (`memoria.py`): fragmentos de ~800 caracteres, embeddings con
      Ollama, índice en SQLite en CACHE, actualización incremental (fecha y hash), búsqueda por similitud
      coseno, `contexto()` y `mensajes_con_contexto()`.
- [ ] **Chats** (`conversaciones.py`): un JSON por chat en `DATOS/chats/<id>.json`; lista, abrir, guardar,
      borrar, título automático y pasar el chat a una nota del vault.

**Interfaz (cxx-qt + los QML de `vortex_local_ai/qml`):** los cinco objetos que hoy ve QML (`ia`, `chat`,
`modelos`, `vault` y `memoria`, de `puente.py`) con los mismos nombres. Va después del motor y del piloto
del Player (que valida cxx-qt).

**Pruebas que hay que portar:** `test_motor.py` (54 casos) y `test_interfaz.py` (5), con un Ollama falso
(nunca el de verdad ni los modelos de la persona).

## «Listo para sustituir»
1. Todas las casillas, revisadas por el tester en Vortexine's OS.
2. Las pruebas portadas pasan.
3. Mismo aspecto (capturas en la VM con la misma paleta).
4. Rendimiento igual o mejor: arranque, RAM, y el tiempo de indexar un vault de prueba de ~1 000 notas.
5. Abre los chats, el índice y el vault de la de Python sin perder nada, y se puede volver.
