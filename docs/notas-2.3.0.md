# Vortexine's OS 2.3.0

Llega **Vortexine Task Manager**: el administrador de tareas de Vortexine's, como el de Windows pero con
el diseño del sistema.

## Lo nuevo

### Vortexine Task Manager (Ctrl+Shift+Esc)

Se abre con **Ctrl+Shift+Esc**, como en Windows (antes abría btop en una terminal), o desde el lanzador
buscando «administrador de tareas». Toma los colores de tu fondo de pantalla, como el resto del escritorio.

- **Procesos**
  - Arriba, un resumen: si tu PC va bien o está trabajando duro, y qué app es la que más usa el
    procesador y la memoria en este momento.
  - Abajo, la tabla en tres partes: **Apps** (cada ventana abierta con todos sus procesos: Firefox con sus
    pestañas, la terminal con lo que corre adentro), **Segundo plano** (tus otros procesos) y **Del sistema**
    (los de root). Cada app se abre con la flechita para ver sus procesos uno por uno.
  - CPU, memoria y disco de cada uno; las celdas se pintan más fuerte mientras más usan. Clic en una
    columna para ordenar por ella. Buscar por nombre o por PID (Ctrl+F).
  - Clic derecho: **Cerrar ventana** (como la X, la app te deja guardar), **Finalizar tarea** (Supr),
    **Forzar cierre** (Mayús+Supr, para lo que se trabó), **Poner en pausa** / Reanudar, **Prioridad**
    (alta, normal, baja, mínima), **Abrir ubicación del archivo** y copiar el comando o el PID.
  - Los procesos del sistema (y la prioridad alta) te piden tu contraseña.
- **Rendimiento**: gráficas en vivo del último minuto de la **CPU** (total o por núcleo, con velocidad y
  temperatura), la **memoria** (cuánto está en uso, en caché y libre), cada **disco** (tiempo activo,
  lectura y escritura), la **red** (envío y recepción, IP) y la **tarjeta de video** (NVIDIA, AMD o
  Intel: uso, memoria de video, temperatura y consumo).
- **Servicios**: los tuyos y los del sistema. Los que fallaron salen primero, en rojo. Cada uno con su
  interruptor **Al iniciar** (si arranca solo al prender) y botones para iniciarlo, detenerlo o reiniciarlo.
- A la izquierda, siempre a la vista: CPU, memoria y GPU en vivo, cuántos procesos hay y cuánto lleva
  prendida la PC.

El panel de rendimiento del reloj también abre ahora el Task Manager.

## Para actualizar

```
vortex actualizar
```
