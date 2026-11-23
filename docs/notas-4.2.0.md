# Vortexine's OS 4.2

Vortexine 4.2, porque a un pendejo le dibujaba el shell a 60 frames.

## Arreglado

### El shell ya no se queda dibujando con el procesador después de actualizar el driver de NVIDIA

Cuando `vortex actualizar` traía un driver de NVIDIA nuevo, reiniciaba el shell al terminar, pero la PC
seguía con el driver viejo cargado hasta reiniciarla. El shell nuevo ya no podía usar la tarjeta y se
dibujaba con el procesador: las animaciones iban a tirones, a unos 60 cuadros aunque tu monitor sea de
144 o 200 Hz, y abrir el calendario o cualquier panel se comía varios núcleos.

Ahora, si pasa eso, `vortex actualizar` deja el shell como está (sigue usando la tarjeta) y te avisa
que reinicies la PC.

### La pantalla de bloqueo ya no se cae si el shell se reinicia o truena

Si el shell se cerraba con la pantalla bloqueada, Hyprland enseñaba una pantalla roja («lockscreen app
died») y había que recuperar el bloqueo a mano. Ahora el shell se vuelve a abrir solo y retoma el
bloqueo en el momento, y te pide tu contraseña como siempre.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo (salvo que se haya actualizado el driver de NVIDIA: entonces te
pide reiniciar la PC).
