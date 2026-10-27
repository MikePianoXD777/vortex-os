# Vortexine's OS 1.10.0

OneDrive como una carpeta más, y tres cosas para estudiar: metrónomo, afinador y un temporizador de
estudio.

## Lo nuevo

- **OneDrive.** Búscalo en el lanzador como **«OneDrive»** (o en la terminal: `vortex onedrive`).
  Un botón, **«Conectar OneDrive»**, abre el navegador para que inicies sesión con tu cuenta de
  Microsoft (personal, del trabajo o de la escuela). Desde ahí tus archivos están en la carpeta
  **OneDrive** de tu carpeta personal (y en Thunar, a la izquierda):
  - los abres, los editas y los guardas como cualquier archivo; lo que guardas se sube solo;
  - no se baja nada hasta que lo abres, así no se llena el disco;
  - se vuelve a conectar sola cada vez que entras.

  El panel muestra el espacio usado, abre la carpeta o la web, y explica cómo ver lo que te
  compartieron: en la web, en «Compartido», clic derecho → «Agregar acceso directo a Mis archivos».
  La primera vez instala lo que hace falta (te pide tu contraseña).
- **Metrónomo y afinador.** Búscalo en el lanzador como **«Metrónomo»**.
  - **Metrónomo:** de 20 a 300 pulsos por minuto, con el nombre del tempo (Adagio, Andante,
    Allegro…), compás de 2/4, 3/4, 4/4, 5/4 o 6/8 con el primer pulso acentuado, y **Tap** para
    marcar el tempo con clics. Sigue sonando aunque cierres el panel. Teclas: Espacio empieza y para;
    ↑↓ y ←→ cambian el tempo; T marca el pulso.
  - **Afinador:** canta o toca una nota cerca del micrófono y te dice cuál es, en do-re-mi y con la
    octava (La4 = 440 Hz), y cuántos cents le faltan o le sobran, con una aguja que se pone verde
    cuando estás afinado. Solo escucha mientras su pestaña está abierta.
- **Temporizador de estudio.** Búscalo en el lanzador como **«Temporizador de estudio»**. Bloques de
  estudio con descansos entre ellos (25/5, 50/10 o 15/3 minutos; cada cuatro bloques, un descanso
  largo).
  - **La cuenta se ve en la barra** mientras corre; con un clic abre el panel.
  - Al terminar cada bloque **suena y te avisa**, y el siguiente empieza solo hasta que lo paras.
  - Mientras estudias **silencia las notificaciones** («No molestar») y al descansar las deja como
    estaban. Se puede apagar.
  - Cuenta cuántos bloques llevas hoy y sigue contando aunque cierres el panel o se actualice el
    sistema.

## Por dentro

- `bin/vortex-onedrive` (`estado`, `conectar`, `montar`, `abrir`, `desconectar`) y el panel
  `shell/src/quickshell/onedrive/`. Usa rclone (se instala al conectar) con su propia configuración
  en `~/.config/vortex/onedrive/` (0600), sin tocar la de rclone de la persona, y la unidad de
  usuario `vortex-onedrive.service` (`rclone mount` con caché completa).
- `shell/src/quickshell/musica/musica.py`: el metrónomo hace su sonido y se lo pasa a `pw-cat`,
  contado en muestras, así los pulsos caen exactos; el afinador usa YIN en Python, sin paquetes
  nuevos.
- El temporizador de estudio es el singleton `Estudio` (`singletons/system/Estudio.qml`) y su panel
  `shell/src/quickshell/estudio/`. vortex-shell 2.2.14-16.
- Pruebas nuevas: `pruebas/onedrive.sh` y `pruebas/musica.sh` (sin cuenta de Microsoft, bocinas ni
  micrófono).

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
