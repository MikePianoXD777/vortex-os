# Vortexine's OS 3.2.0

## Lo nuevo

### Vortexine's Runner: programas y juegos de Windows (.exe)

Una app nueva para **jugar y usar programas de Windows** en Vortexine's OS, como Heroic Games Launcher. No viene de
fábrica: se instala con un clic desde **Vortexine's Shop → Apps de Vortexine's** (o con `vortex runner instalar`).

- **Arrastra un .exe** a la ventana, o usa «Agregar .exe». Sale en tu biblioteca con **su propio ícono y nombre**,
  sacados del mismo .exe.
- **¿Es un instalador?** (setup.exe, .msi) Usa «Instalar programa»: se instala en su propio espacio y, al terminar,
  Runner te pregunta cuál es el programa y lo agrega a tu biblioteca.
- **Cada programa en su propio espacio de Windows** (su «disco C:»), así no se estorban entre ellos.
- Corre con **el Proton de CachyOS** de serie (el mismo de Steam en CachyOS), con **umu-launcher**, el lanzador de
  Heroic y Lutris. Si algo no abre, en «Configurar» puedes elegir **GE-Proton** (se baja solo) o cualquier Proton que
  tengas en Steam.
- Botón **Jugar / Detener**, cuánto tiempo has jugado y cuándo lo abriste por última vez.
- En «…»: agregarlo al **lanzador de apps**, abrir su carpeta o su disco C:, la configuración de Wine, argumentos y
  quitarlo (con o sin borrar sus datos).
- La primera vez te pide instalar lo que corre los programas (umu-launcher y el Proton de CachyOS, unos 350 MB): un
  botón y tu contraseña, una sola vez.
- **Doble clic en un .exe** en Thunar: el instalador de apps de Vortexine's ahora ofrece primero «Abrir con
  Vortexine's Runner».

Quitar Runner no borra tu biblioteca ni las partidas guardadas de tus programas.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña). Luego abre Vortexine's Shop y busca **Vortexine's Runner** en «Apps
de Vortexine's».
