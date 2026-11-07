# Vortexine's OS 2.9.1

## Lo nuevo

### El sistema usa menos RAM al prender

En una máquina virtual de 8 GB, sin Bluetooth, el sistema recién prendido baja de **1178 MB a unos 890 MB** (lo que marca
`free`). Con Bluetooth son unos 19 MB más. No se quitó ninguna función.

- El kernel reservaba de más antes de empezar a liberar memoria. Ahora deja más libre para tus programas y juegos, sin más
  tirones en las pruebas con poca memoria.
- El panel ya no levanta al prender los servicios de escritorio que solo usan algunas apps: arrancan cuando una los pide.
- Sin adaptador de Bluetooth, su agente no corre. Si conectas uno, vuelve al siguiente inicio de sesión.

Con NVIDIA el número será más alto, porque la máquina virtual no tiene gráfica.

### Vortex Player mucho más ligero (Vortex Player 1.7.1, Linux y Windows)

Con bibliotecas grandes, Vortex Player podía usar más de 1 GB de RAM y mucha CPU. Con 700 canciones, en la misma máquina
virtual, baja de **más de 5 GB a menos de 300 MB**, y mientras suena gasta 4 veces menos CPU. Se ve y funciona igual.

- Ya no copia toda tu biblioteca cada vez que necesita un total.
- La cola solo crea las filas que se ven.
- Se entera de que cambiaste de audífonos cuando pasa, en vez de preguntar cada 3 segundos.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña una vez para el ajuste del kernel). En Windows, el instalador nuevo de Vortex
Player.
