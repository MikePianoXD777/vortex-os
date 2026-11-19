# Vortexine's OS 4.0.4

## Arreglado

### En Ajustes, «Fondos de pantalla» y «Pantalla de bloqueo» salían vacíos

Las dos pestañas no cargaban: se veía el panel en blanco. La vista previa del fondo, nueva en la 4.0,
usaba una opción que su imagen no tiene, y por eso fallaba la pestaña entera.

- **Las dos vuelven a cargar**, con su vista previa.

### Interruptores de Ajustes que no hacían nada

En algunas filas de Ajustes, los botones no respondían. Por ejemplo, las opciones de **Fondos animados**
(pausar cuando una ventana tapa el fondo, pausar con batería, optimizar) y **Tus sonidos** de las
notificaciones (escuchar, usar o borrar).

- **Ya responden.**

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
