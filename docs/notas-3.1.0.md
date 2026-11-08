# Vortexine's OS 3.1

Vortexine's ahora está hecho para correr IA en tu PC, sin cuenta, sin internet y sin pagar.

## Lo nuevo

### Vortexine's Local AI

Una app nueva para platicar con modelos de IA que corren en tu PC, con un vault de notas como el de Obsidian.

- **Chat:** las respuestas salen mientras el modelo escribe, con lo que «pensó» plegado. Puedes detenerlo,
  regenerar, copiar y guardar el chat como nota.
- **Modelos:** un catálogo con los mejores modelos abiertos de hoy (Qwen 3.5, Gemma 4, Llama, DeepSeek, GPT-OSS…).
  Te dice cuáles **te caben** en tu tarjeta de video y cuál te recomienda, y los baja con una barra de progreso.
- **Vault:** notas en Markdown en tu carpeta, con `[[enlaces]]`, `#etiquetas`, retroenlaces y un grafo. Son archivos
  normales: también los abre Obsidian.
- **Preguntar a mi vault:** con «Usar mi vault», la IA lee tus notas para contestar y te dice de cuáles sacó la respuesta.
- Arranca Ollama sola si hace falta y, al cerrarla, lo apaga para soltar la memoria de video.

Llega con `vortex actualizar`. Si la quitas, no vuelve.

### Modo IA automático

Cuando una IA local está usando la tarjeta de video (Ollama, llama.cpp, ComfyUI, Stable Diffusion, PyTorch…),
Vortexine's se lo nota solo y le da la prioridad: apaga los efectos, pausa el fondo animado (que gasta memoria de
video) y detiene los clips. Te avisa una vez y se quita solo cuando la IA termina.

- Si abres un juego, el juego manda; al cerrarlo, el modo IA vuelve solo.
- La IA en el procesador no lo activa, ni Miku.
- A mano: `vortex modo-ia on`, `off` o `estado`.

### Ollama con CUDA en NVIDIA

En una NVIDIA (serie 16, 20 o más nueva), `vortex actualizar` te pregunta una sola vez si cambias Ollama a CUDA:
más palabras por segundo. Pesa unos 2.8 GB de descarga, por eso pregunta. Si dices que no, se queda con Vulkan,
que ya funciona en cualquier tarjeta. A mano: `vortex-ia motor`.

### ¿Qué modelo me cabe?

`vortex-ia modelos` mide tu tarjeta de video y tu RAM y te dice qué modelos caben enteros, cuáles solo en parte (van
lentos) y cuál te recomienda. Con `vortex-ia modelos --medir MODELO` mide cuántas palabras por segundo te da uno que
ya tienes.

### Aviso de modelo que no cabe

Si un modelo no cupo entero en la tarjeta de video y se fue en parte al procesador (por eso va lento), te avisa una
vez y te sugiere uno más chico.

### IA en la Shop

Una sección **IA** nueva, la primera de la Shop, con apps para usar IA en tu PC: Alpaca, Jan, LM Studio, GPT4All,
Reins, Speech Note, Upscayl y Upscaler.

### La tarjeta de video en el Task Manager

Una columna **Gráfica** con cuánta memoria de video usa cada app, y una tarjeta con el total en el resumen.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
