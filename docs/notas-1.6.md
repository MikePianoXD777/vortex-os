# Vortexine's OS 1.6

Miku ya no necesita Claude: piensa con la IA que tengas, o con una IA local que corre en tu PC, sin cuenta
ni internet.

## Lo nuevo

- **Miku con cualquier IA:**
  - Además de Claude, puede pensar con **Gemini**, **OpenAI** o cualquier IA compatible con la API de
    OpenAI (OpenRouter, LM Studio, un servidor propio…).
  - Se elige con `vortex-miku ia`:
    - `vortex-miku ia gemini` o `vortex-miku ia openai [MODELO]`;
    - `vortex-miku ia otra URL MODELO`;
    - `vortex-miku ia claude` para volver a Claude.
  - La clave se pide ahí mismo y queda en tu llavero (o, si no tienes llavero, en un archivo que solo lees
    tú). Cámbiala con `vortex-miku ia clave`.
  - Con otra IA, Miku sigue viendo tu pantalla y usando sus acciones (abrir apps, volumen, grabar…), siempre
    con tu permiso en su ventana. La búsqueda en la web, por ahora, solo va con Claude.
- **IA local en tu PC, ya en la ISO:**
  - La ISO trae **Ollama** con aceleración por **Vulkan**. Funciona con tarjetas NVIDIA (el driver que pone
    el instalador ya trae Vulkan), AMD e Intel. Sin GPU corre en la CPU, más lenta.
  - El modelo es **Qwen 3.5 de 4B** (3,3 GB): entiende español, ve imágenes y usa herramientas. Cabe en una
    GPU de 4 GB, como una RTX 3050 Ti.
  - El modelo no viene en la ISO: se baja la primera vez (necesita internet esa vez) y luego funciona sin
    conexión.
  - Sin Claude, la ventana de Miku muestra el botón **Usar IA local (sin cuenta)**, que lo deja listo. También
    con `vortex-miku ia local`, y `vortex-miku ia bajar` lo vuelve a bajar.
  - Ollama solo escucha en tu PC (`127.0.0.1`) y arranca cuando Miku lo necesita. Miku sube la IA a la GPU en
    cuanto carga su voz (la primera respuesta ya no espera ~20 s) y la saca al cerrarla.
  - Si la voz natural y la IA no caben juntas en la GPU, la IA deja en el procesador lo que no cabe. Probado
    en una RTX 3080: voz 5,5 GB + IA 2,3 GB, respuestas en 0,5–3 s.
  - Contexto de 8K (con los 4K que trae Ollama, una plática corta ya borraba las instrucciones de Miku) y la
    temperatura que Qwen recomienda sin «pensar»: menos errores raros. Miku también sabe la fecha.
- **Mejores momentos con cualquier IA:** la elección de los momentos de una grabación usa la IA que elegiste,
  no solo Claude.
- **Ajustes → General:** la sección de Miku ya no avisa «Claude está apagado» si usas otra IA, y te dice cómo
  elegir la local.
- **La misma Miku en todas las PC: la voz natural se pone sola.**
  - La primera vez que abres a Miku, si tu PC aguanta, prepara su voz natural en segundo plano y te avisa con
    una notificación al empezar y al terminar. Mientras, habla con la voz de siempre.
  - Con una **NVIDIA serie 20 o más nueva de 6 GB o más**, va en la GPU (~7 GB, una sola vez).
  - Si no, con un **procesador Intel Core de 10.ª generación o más nuevo** (o un Ryzen 3000 o más nuevo) y
    8 GB de RAM, va en el procesador: lenta, pero funciona (~3 GB). Por ejemplo, un i9 de 11.ª generación.
  - Si tampoco, Miku se queda con la voz de siempre.
  - Con una GPU de 4 GB, como una RTX 3050 Ti, la IA local va en la GPU y la voz natural en el procesador.
  - No se instala sola si elegiste otra voz o si la quitaste (`vortex-miku natural quitar`).
    `vortex-miku natural estado` dice qué sirve en tu PC.

## Por dentro

- `apps/miku/pruebas/ia.py` prueba la otra IA, la IA local, las claves, los errores y el comando con una IA,
  un Ollama y un servidor MCP falsos, sin tocar tu llavero. Corre en GitHub en cada cambio.

## Actualizar

```bash
vortex actualizar
```

En un sistema ya instalado, esto también instala Ollama.

## Requisitos

Los mismos de la 1.0: PC x86-64 con 4 GB de RAM (8 GB recomendado), 40 GB de disco y una USB de 8 GB. Para
la IA local, 8 GB de RAM y unos 4 GB más de disco.
