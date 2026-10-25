# Vortexine's OS 1.6.1

Una versión chica sobre la 1.6: el aviso de actualizaciones llega a todos, y el shell lleva el nombre y el logo
de Vortexine's.

## Lo nuevo

- **El aviso de actualizaciones llega a todos los sistemas:**
  - Antes solo se encendía la primera vez que corrías `vortex actualizar`, así que un sistema recién
    instalado desde la ISO nunca avisaba.
  - Ahora se enciende solo al entrar a tu sesión. Si lo apagaste (`vortex actualizaciones off`), se respeta.
  - En la sesión de prueba de la USB (el live) no se enciende.
- **Vortex Shell:** en Ajustes, la bienvenida y «Acerca de» dicen **Vortex Shell, basado en Serpantinum, por
  @ilyamiro**, con la espiral de Vortexine's en vez del logo de Serpantinum (pintada con el color de tu tema).

## Por dentro

- `pruebas/actualizaciones.sh` prueba que el aviso se encienda al entrar, que no se repita y que se respete
  «off», con un `systemctl` falso. Corre en GitHub en cada cambio.

## Actualizar

```bash
vortex actualizar
```

Si instalaste la 1.5 o la 1.6 desde la ISO, córrelo una vez a mano: con eso se enciende el aviso, y desde ahí
te avisa solo.

## Requisitos

Los mismos de la 1.6.
