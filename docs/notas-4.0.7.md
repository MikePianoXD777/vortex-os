# Vortexine's OS 4.0.7

## Arreglado

### El shell ya no se redibuja entero cada 2 segundos

Los círculos de CPU, RAM y temperatura de la barra se animaban cada vez que llegaba una lectura,
cada 2 segundos. Mientras duraba esa animación, el shell volvía a dibujar todas sus ventanas (las
barras y el fondo de cada monitor) a la frecuencia del monitor: en uno de 200 Hz, cientos de cuadros
por segundo, un tercio del tiempo, todo el día, también mientras juegas.

- **Ahora solo se redibuja la barra**, y a 30 cuadros por segundo. Los círculos se mueven igual que antes.
- En reposo, el shell pasa de unos 630 dibujos por segundo a unos 6. Le quita trabajo al CPU y a la
  tarjeta de video, sobre todo con monitores rápidos.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
