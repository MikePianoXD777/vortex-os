# Vortexine's Local AI: la API del motor

El motor (`vortex_local_ai/`) no depende de Qt. La interfaz (`puente.py`, `qml/`) lo usa con esta API.
Lo de **«Además»** son agregados al contrato original: nada se quitó ni se renombró.

> **Hilos:** todo lo que habla con Ollama o lee el vault entero (`bajar`, `chat`, `embeber`,
> `Memoria.actualizar`, `Memoria.contexto`, `asegurar_servidor`) bloquea: córrelo en un hilo aparte,
> nunca en el hilo de Qt. Los callbacks (`al_progreso`, `al_trozo`, `al_pensar`) llegan **desde ese
> hilo**: pásalos a Qt con una señal.

```python
from vortex_local_ai import rutas, catalogo, conversaciones
from vortex_local_ai.ollama import Ollama, ErrorOllama, Cancelado
from vortex_local_ai.vault import Vault, ErrorVault
from vortex_local_ai.memoria import Memoria
```

## `__init__.py`
- `__version__ = "1.0.0"`. Además: `VERSION` (lo mismo), `NOMBRE = "Vortexine's Local AI"`, `APP_ID = "vortex-local-ai"`.

## `rutas.py`
- `DATOS` = `~/.local/share/vortex-local-ai` (respeta `XDG_DATA_HOME`).
- `CACHE` = `~/.cache/vortex-local-ai` (respeta `XDG_CACHE_HOME`).
- `VAULT_POR_DEFECTO` = `~/Documentos/Vortexine Local AI` (respeta `XDG_DOCUMENTS_DIR` / `user-dirs.dirs`).
- Con `VORTEX_LOCAL_AI_HOME=/tmp/x` todo va a `/tmp/x/datos`, `/tmp/x/cache` y `/tmp/x/vault`.
- Además: `recargar()` (recalcula las tres si cambiaste las variables después de importar),
  `chats()` → `DATOS/chats`, `config()` → `DATOS/config.json` (para la config de la interfaz).
- **Ojo:** usa `rutas.DATOS` (no `from rutas import DATOS`) para que `recargar()` surta efecto.

## `ollama.py`
`Ollama(url=None, *, puede_arrancar=None)`. URL: `VORTEX_LOCAL_AI_OLLAMA` o `http://127.0.0.1:11434`.
Solo biblioteca estándar.

| Método | Qué hace |
|---|---|
| `disponible() -> bool` | ¿Ollama contesta? (1,5 s como mucho) |
| `asegurar_servidor(timeout=20) -> bool` | Si no contesta, lo arranca y espera: primero `systemctl --user start vortex-ollama.service` (la unidad de Miku, mismo `~/.ollama`), si no `ollama serve` desacoplado. Solo si la URL es local. `False` si no se pudo (mira `ultimo_error`). |
| `modelos() -> list[dict]` | `/api/tags`: `nombre`, `bytes`, `familia`, `parametros` («8.2B»), `cuantizacion` («Q4_K_M»), `modificado` (texto ISO). Además: `embeddings` (bool, si parece de embeddings). |
| `cargados() -> list[dict]` | `/api/ps`: `nombre`, `bytes`, `bytes_vram`, `en_gpu_pct` (0-100). Además: `expira` (texto ISO). |
| `bajar(nombre, al_progreso=None, cancelar=None)` | `/api/pull` en streaming. `al_progreso(completado:int, total:int, estado:str)` con bytes sumados de todas las capas y `estado` en español («Buscando el modelo», «Bajando», «Revisando la descarga», «Guardando», «Listo»). `cancelar`: `threading.Event`. Si se cancela lanza `Cancelado` (subclase de `ErrorOllama`); lo bajado se retoma la próxima vez. |
| `borrar(nombre)` | `DELETE /api/delete`. |
| `chat(modelo, mensajes, al_trozo=None, cancelar=None, opciones=None, al_pensar=None) -> str` | `/api/chat` en streaming. `mensajes`: `[{"role": "system"\|"user"\|"assistant", "content": str}]` (puede llevar `"images": [base64]` para visión). `al_trozo(texto)` recibe cada trozo de la respuesta; `al_pensar(texto)` cada trozo del pensamiento (`<think>…</think>` o el campo `thinking`), que **no** va en `al_trozo` ni en lo que regresa. Regresa la respuesta completa sin el pensamiento. **Si se cancela, regresa lo que llevaba (no lanza).** `opciones` va como `options` de Ollama (`temperature`, `num_ctx`…). |
| `embeber(modelo, textos) -> list[list[float]]` | `/api/embed` (cae a `/api/embeddings` en Ollama viejo). `textos` puede ser un `str`. |

- `class ErrorOllama(Exception)`: `str(e)` es un mensaje claro en español para enseñárselo a la
  persona. Además: `e.clave` = `"conexion"` (no contesta), `"no_instalado"`, `"modelo"` (no existe /
  no lo tienes), `"red"` (sin internet al bajar), `"cancelado"`, `"otro"`.
- Además: `class Cancelado(ErrorOllama)` (`clave = "cancelado"`).
- Además, en la instancia: `url`, `ultimo_error` (texto del último fallo de `asegurar_servidor`),
  `ultimas_estadisticas` después de `chat`: `{"tokens", "segundos", "tokens_por_segundo"}`,
  `instalado() -> bool` (¿está el comando `ollama`?), `version() -> str|None`,
  `parar_propio()` (para el Ollama que arrancó esta instancia, si lo arrancó ella; si ya corría, no
  lo toca), `descargar_de_gpu(modelo)` (lo saca de la VRAM ya).
- `puede_arrancar=False` (o `VORTEX_LOCAL_AI_NO_ARRANCAR=1`) hace que `asegurar_servidor` nunca
  arranque nada: lo usan las pruebas.

## `catalogo.py`
- `CATALOGO: list[dict]`: `id` (nombre en Ollama, `qwen3:8b`), `nombre`, `descripcion`, `para`
  (`"chat"|"codigo"|"razonamiento"|"vision"|"vault"`), `gb` (descarga), `vram_gb` (para ir completo en
  la GPU). Además: `contexto` (tokens, texto como «128K»), `piensa` (bool: el modelo razona antes), `vision` (bool: ve
  imágenes), `moe` (bool: mezcla de expertos, aguanta ir en parte en la CPU).
- `vram_disponible() -> dict`: `{"nombre", "total_mb", "libre_mb", "tipo": "nvidia|amd|intel|cpu"}`.
  Orden: `vortex-ia modelos --json` (si existe) → `nvidia-smi` → `/sys/class/drm/*/device/mem_info_vram_*`
  → la RAM (`tipo="cpu"`). Además: `ram_mb` (RAM total), y si hay GPU intel/sin VRAM propia usa la RAM.
- `cabe(modelo, vram) -> "si"|"justo"|"no"`: `modelo` es un dict del catálogo, un `id` o un dict con
  `vram_gb`/`bytes`. `"justo"` = cabe con poco margen o una parte va a la CPU (más lento).
- `recomendado(vram) -> dict`: el mejor modelo de chat que te cabe completo (si ninguno, el más chico).
- Además: `por_id(id) -> dict|None`, `de_tipo(para) -> list[dict]`, `EMBEDDINGS` (el `id` del modelo
  de embeddings del vault), `TIPOS` (`{"chat": "Platicar", …}`, nombres bonitos de cada `para`),
  `FUENTE` (de dónde salen los tamaños).

## `vault.py`
`Vault(ruta)`: una carpeta de `.md` (se crea si no existe). Sirve un vault de Obsidian sin romperlo:
ignora las carpetas que empiezan con punto (`.obsidian/`, `.trash/`, `.git/`…). Todas las `ruta` son
relativas al vault, con `/`, y con `.md` (por ejemplo `Ideas/Juegos.md`). Una ruta que se sale del vault
lanza `ErrorVault` (subclase de `ValueError`).

| Método | Regresa |
|---|---|
| `notas()` | `[{"ruta", "titulo", "modificado" (float, epoch), "etiquetas": [str]}]`. Además: `alias`, `carpeta`, `bytes`. `titulo` = nombre del archivo sin `.md`, como Obsidian. |
| `arbol()` | `{"nombre", "ruta": "", "tipo": "carpeta", "hijos": [...]}`; cada hijo es `{"nombre", "ruta", "tipo": "carpeta", "hijos"}` o `{"nombre", "ruta", "tipo": "nota", "titulo", "modificado"}`. Carpetas primero, luego notas, por nombre. Las carpetas vacías salen. |
| `leer(ruta) -> str` | Texto completo (con frontmatter). `FileNotFoundError` si no existe. |
| `guardar(ruta, texto)` | Escritura atómica (temporal + `os.replace`). Crea carpetas. |
| `crear(titulo, carpeta="") -> ruta` | Nota vacía; si el nombre ya existe le pone « 1», « 2»… Limpia caracteres prohibidos (`\ / : * ? " < > \| # ^ [ ]`). Título vacío → «Sin título». |
| `renombrar(ruta, nuevo) -> ruta_nueva` | `nuevo` es un título («Otra») o una ruta («Carpeta/Otra.md», para mover). Actualiza los `[[enlaces]]` de las otras notas que apuntaban a ella, conservando `#sección` y `\|alias`. `FileExistsError` si ya hay una con ese nombre. |
| `borrar(ruta)` | Mueve a `.trash/` del vault (como Obsidian). Además: regresa la ruta dentro de `.trash`. |
| `buscar(texto)` | `[{"ruta", "titulo", "fragmento"}]`; sin importar mayúsculas ni acentos. `#etiqueta` busca por etiqueta. Primero las que lo tienen en el título. Además: `coincidencias` (int). |
| `enlaces(ruta)` | `[{"nombre", "ruta": str\|None, "existe": bool}]`: los `[[…]]` que salen de la nota (`ruta` resuelta o `None`). Además: `seccion`, `texto` (el alias tras `\|`). |
| `retroenlaces(ruta)` | `[{"ruta", "titulo", "fragmento"}]`: las notas que enlazan a esta, con la línea del enlace. |
| `grafo()` | `{"nodos": [{"id": ruta, "titulo", "etiquetas", "grado", "existe": True}], "aristas": [{"origen": ruta, "destino": ruta}]}`. Los enlaces a notas que no existen salen como nodos `{"id": "?nombre", "titulo": nombre, "existe": False, …}` (como los grises de Obsidian). Aristas sin repetir. |
| `resolver(nombre_enlace) -> ruta\|None` | Como Obsidian: acepta `Nota`, `Nota#sección`, `Nota\|alias`, `Carpeta/Nota`, con o sin `.md`; por nombre sin importar mayúsculas, luego por `aliases` del frontmatter. Si hay dos con el mismo nombre gana la de ruta más corta. Además: `resolver(nombre, desde=ruta)` prefiere la de la misma carpeta. |
| `etiquetas() -> dict[str, int]` | `{"juegos": 3, …}` (sin `#`, del texto y del frontmatter `tags`). |

- Además: `ruta_absoluta(ruta) -> Path`, `existe(ruta) -> bool`, `crear_carpeta(ruta) -> ruta`,
  `frontmatter(ruta) -> dict`, `raiz` (`Path`), `nombre` (nombre de la carpeta).
- Además, funciones del módulo para el editor: `separar_frontmatter(texto) -> (dict, cuerpo)`,
  `etiquetas_de(texto) -> list[str]`, `enlaces_de(texto) -> list[dict]`,
  `limpiar_nombre(titulo) -> str`. Además: `memoria.fragmentar(texto) -> [(seccion, texto)]`.

## `memoria.py` («Preguntar a mi vault»)
`Memoria(vault, ollama, modelo_embeddings=None)`. Índice en SQLite en `CACHE/memoria/<vault>.sqlite`:
fragmentos de ~800 caracteres por sección/párrafo con sus vectores. Si `modelo_embeddings` es `None`
busca uno instalado (`catalogo.EMBEDDINGS` u otro con «embed» en el nombre).

- `actualizar(al_progreso=None, cancelar=None)`: incremental (fecha y hash; si cambia el modelo lo
  rehace todo); quita las notas borradas. `al_progreso(hechas:int, total:int, ruta:str)`. Además:
  regresa `{"notas", "nuevas", "quitadas", "fragmentos", "cancelado": bool}`. Cancelar deja guardado lo
  que ya iba.
- `contexto(pregunta, k=6) -> [{"ruta", "titulo", "fragmento", "puntaje"}]`, el mejor primero
  (similitud coseno; usa numpy solo si está). Además: `seccion` (el título # del fragmento). Como mucho 2
  fragmentos de la misma nota.
- `mensajes_con_contexto(pregunta, historial) -> list`: `[system, *historial, {"role": "user", …}]`,
  listo para `Ollama.chat`. El system prompt va en español con los fragmentos y pide citar con
  `[[titulo]]`.
- Sin modelo de embeddings (o con el índice vacío) busca por palabras con `vault.buscar` y lo dice.
- Además: `modo` (`"embeddings"` o `"palabras"`), `aviso` (texto para la interfaz, o `""`),
  `ultimo_contexto` (los fragmentos que usó `mensajes_con_contexto`, para enseñar las fuentes),
  `modelo_embeddings`, `ruta_indice`, `estado() -> {"notas", "fragmentos", "modelo", "modo"}`,
  `borrar_indice()`.

## `conversaciones.py`
Chats como JSON en `DATOS/chats/<id>.json`. Una conversación es:
```python
{"id": str, "titulo": str, "modelo": str, "creado": float, "modificado": float,
 "mensajes": [{"role": "user"|"assistant"|"system", "content": str,
               # opcionales: "pensamiento": str, "citas": [ruta], "modelo": str, "fecha": float}],
 "vault": bool}   # opcional: True si fue «Preguntar a mi vault»
```
- `lista() -> list[dict]`: sin `mensajes`, con `n_mensajes`; la más reciente primero.
- `abrir(id) -> dict|None` (también `None` si el id no es válido).
- `guardar(conv) -> id`: le pone `id`, `creado`, `titulo` (automático) si faltan, y `modificado`.
- `borrar(id)`.
- `titulo_automatico(primer_mensaje) -> str` (unas 6 palabras; «Chat nuevo» si está vacío).
- `a_nota(conv) -> str`: el chat en Markdown con frontmatter (`tags: [chat, vortex-local-ai]`), para
  `Vault.guardar`. El pensamiento va en un callout plegado de Obsidian.
- Además: `nueva(modelo="", titulo="") -> dict` (una conversación vacía, sin guardar).
