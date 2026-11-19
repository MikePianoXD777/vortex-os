# Vortexine's OS 4.0

La actualización más grande hasta ahora: un diseño nuevo de arriba abajo, las apps reescritas en Rust,
entrar directo al prender y un montón de funciones que trabajan solas.

## Diseño nuevo

- **Negro, blanco y grises**: barra en islas con botones redondos, ventanas sólidas con un borde blanco fino,
  tipografía Inter y Unbounded en títulos y relojes, y un fondo de fábrica negro con anillos.
- **El lanzador es un buscador grande al centro** y el panel del sistema usa fichas grandes.
- **Logo nuevo** y arranque con el logo (Plymouth).
- Todo sigue siendo tuyo: en Ajustes puedes volver a otro tema, a los colores del fondo o al liquid glass.

## Entrar y salir

- **Al prender entras directo a Vortex Shell**; la contraseña se pone en su pantalla de bloqueo.
- **Al cerrar sesión sale un inicio de sesión gráfico** con el mismo diseño.

## Las apps, en Rust

Las apps de Vortexine's se reescribieron en Rust: abren más rápido y gastan menos memoria. Se ven y funcionan igual.

- **Vortex Player, Vortexine's Camera, Local AI, la Shop, App Installer, Task Manager, Runner, Video,
  Connect y Shared.**
- **Miku**: su ventana, su oído, su cerebro, sus acciones y **su voz** (Supertonic) ya corren en Rust.
- Los servicios de fondo también: el gestor de recursos, las tareas, las memorias USB, el Bluetooth,
  los enlaces y la memoria.
- Local AI ahora se conecta también a un Ollama por **https**.

## Funciones nuevas

- **Detector de tirones** (`vortex tirones`): mide solo los tirones de tus partidas y te dice cuándo y por qué
  (memoria, disco, procesador o calor).
- **Juegos de Android** en la Shop, con Waydroid.
- **No interrumpe tus partidas**: las tareas automáticas esperan y los avisos no salen encima del juego.
- **Respaldo de partidas guardadas** al abrir un juego, con restauración.
- **Sesiones de trabajo**: guarda tus apps abiertas con su escritorio y vuelve a abrirlas.
- **Salud del disco**: aviso temprano si tu disco empieza a fallar (SMART).
- **«Algo no funciona»**: te dice qué programa ocupa un puerto y por qué la PC tarda en encender.
- **Traducir lo que tengas seleccionado** con Super+Alt+T, **limpieza semanal** y **respaldo a una memoria USB**.
- **Miku puede escribir por ti y hacer clic** (siempre con tu permiso).
- **Cerrar por completo** desde el dock: cierra la app entera, también lo que deja en la bandeja.
- El resumen de la partida trae tus clips, y la nota rápida se pasa a Obsidian con un clic.

## Más ligero

- El shell gasta ~75 MB menos, y con NVIDIA su memoria ya casi no crece con las horas (~95 % menos que antes).
- Las apps sueltan la memoria al quedarse quietas.

## Se fue

- «Tu celular» con KDE Connect (Super+Shift+P): se apaga en las PCs que lo tenían encendido.

## Para actualizar

```
vortex actualizar
```

Esta vez tarda más de lo normal: tu PC compila las apps nuevas en Rust (puede ser media hora o más en una PC modesta).
Déjala trabajar; al terminar, el shell se reinicia solo.
