# Vortexine's OS 4.0.1

## Arreglado

### Algunas PCs se quedaban en la 3.5.2

Si un paquete del sistema (o uno de AUR) no se podía instalar, `vortex actualizar` se detenía ahí mismo:
Vortexine's se quedaba en la versión de antes y `vortex version` seguía diciendo 3.5.2.

- **Ahora te avisa qué paquete falló y sigue con la actualización de Vortexine's.**
- **La compilación de las apps en Rust ya no corre las pruebas en tu PC**: tarda menos, y no se cae por
  una prueba que depende de cómo está tu máquina.

### «Archivos en conflicto» al instalar Miku

Si en tu PC había un archivo suelto dentro de la carpeta de Miku (por ejemplo, de un cambio hecho a mano),
pacman no instalaba el paquete `vortex-miku` y decía «archivos en conflicto».

- **Ahora cada paquete de Vortexine's reemplaza lo que haya suelto en su propia carpeta** (`/usr/share/<paquete>`).
  Fuera de esa carpeta, pacman sigue revisando como siempre.

## Para actualizar

```
vortex actualizar
```

Si arriba sale que un paquete del sistema no se instaló, mándanos una captura de esas líneas.
