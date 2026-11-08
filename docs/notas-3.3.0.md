# Vortexine's OS 3.3.0

## Lo nuevo

### Perfiles de rendimiento: Ultra, Alto, Medio y Bajo

Elige cuánto gasta tu PC en verse bonita, en **Ajustes → Pantalla → Rendimiento** o con `vortex rendimiento`. Se aplica
al momento y se queda aunque reinicies. De fábrica sigues en **Ultra**: si no tocas nada, nada cambia.

| Perfil | Qué quita |
| --- | --- |
| **Ultra** | Nada: efectos, animaciones y fondo de video |
| **Alto** | El desenfoque |
| **Medio** | Además, las animaciones de las ventanas |
| **Bajo** | Además, el video del fondo (se queda quieto en un cuadro), las acciones rápidas y los clips al jugar, y el shell se dibuja a 60 Hz |

**Cuánto se nota:** lo que más baja es el procesador. Con un fondo de video, Bajo lo deja quieto y el shell deja de
gastar CPU en él. En memoria el ahorro es modesto: en una máquina virtual con un video de 1080p, el shell pasó de
343 MB a 275 MB y todo el sistema bajó unos 50 MB. Sin fondo de video, Bajo ahorra menos.

**Lo que no cambia:** las animaciones internas de los paneles del shell siguen igual en todos los perfiles. El modo
juego, el modo IA y el de batería funcionan como siempre y, al terminar, vuelves a tu perfil.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña). Luego busca **Rendimiento** en Ajustes → Pantalla.
