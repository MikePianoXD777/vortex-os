# Vortexine's OS 3.3.2

## Lo nuevo

### Logo nuevo: Rotor

Vortexine's OS estrena logo, una turbina de tres aspas. Ya está en el shell, en la terminal, en el instalador y en las
apps del sistema.

## Lo arreglado

### `vortex actualizar` ya no se detiene a preguntar por la IA

En una NVIDIA, `vortex actualizar` se quedaba hasta 2 minutos y medio esperando que contestaras una notificación sobre
el motor de la IA, y parecía trabado. Ya no revisa nada de IA.

Ahora la pregunta sale **solo al abrir Vortexine's Local AI**, en una ventanita, y una sola vez por máquina:

- **Cambiar ahora:** instala CUDA (te pide tu contraseña en una ventana). Ollama lo usa la próxima vez que arranque.
- **No, gracias:** te quedas con Vulkan y no vuelve a preguntar. Si cambias de idea: `vortex-ia motor` en una terminal.

Si cierras la ventanita o la de la contraseña sin contestar, te pregunta otra vez la próxima vez que abras Local AI.
Si no usas Local AI, nunca te pregunta nada.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
