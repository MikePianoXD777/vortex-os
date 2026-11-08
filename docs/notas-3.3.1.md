# Vortexine's OS 3.3.1

## Lo arreglado

### IA local en PCs con tarjeta de entrada

Con una gráfica como la RX 550, `vortex-ia modelos` decía que la IA local no valía la pena y Vortexine's Local AI
marcaba los modelos de chat como que no cabían. Ahora:

- **Una tarjeta de entrada o con menos de 3.3 GB de memoria se trata como si no hubiera gráfica:** la IA corre con tu
  procesador y tu RAM, y Local AI te lo avisa.
- **En esas PCs recomienda modelos que un procesador sí mueve.** Hay uno nuevo en el catálogo, **Qwen 3.5 4B**
  (`qwen3.5:4b`).
- **Local AI ahora sí usa los datos de `vortex-ia`** para saber qué gráfica tienes. Antes no los leía.
- **Sin gráfica, ya no recomienda modelos demasiado pesados** para un procesador.

Esto se probó con hardware simulado, no en una RX 550 real. Para ver cuánto da en tu PC:
`vortex-ia modelos --medir qwen3.5:4b`.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña). Local AI se actualiza si ya la tienes instalada; si no, está en
Vortexine's Shop.
