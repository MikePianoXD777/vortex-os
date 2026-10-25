# Vortexine's OS 1.6.3

Arregla el modo otaku: al apagarlo, sus fondos seguían saliendo.

## Lo nuevo

- **Al apagar el modo otaku, sus fondos ya no se quedan:**
  - Si una vez el modo se encendió a medias (por ejemplo, se cortó la descarga) o lo apagó una versión
    vieja, el sistema guardaba los ajustes del modo otaku como si fueran los tuyos. Desde ahí, cada vez que
    lo apagabas, el selector de fondos (Super+Shift+W) seguía en la carpeta «Fondos otaku», con su fuente y
    sus bordes.
  - Ahora esos ajustes se reconocen y se cambian por los normales: la carpeta «Fondos», y la fuente y los
    bordes por defecto.
  - Si eliges a mano la carpeta «Fondos otaku» en el modo normal, se respeta.
- **Se repara solo:** `vortex actualizar` arregla los sistemas que ya quedaron así. También lo hace
  `vortex otaku off` o `vortex otaku reparar`.

## Por dentro

- `pruebas/otaku.sh` prueba los ajustes que quedaron con los del modo otaku: apagar con el modo ya
  apagado, encender sobre ellos, un «antes» viejo, la carpeta elegida a mano y «reparar» con el modo
  encendido.

## Actualizar

```bash
vortex actualizar
```

## Requisitos

Los mismos de la 1.6.
