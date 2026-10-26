# Vortexine's OS 1.7.3

El modo otaku puede sonar al encenderse.

## Lo nuevo

- **Un audio al encender el modo otaku:** al activarlo suenan 15 segundos del audio que elijas, mientras
  se bajan los fondos. Se pone una vez en cada PC:

  ```bash
  vortex otaku sonido ~/Descargas/mi-cancion.mp3
  ```

  - Se guardan solo 15 s (desde el principio, o desde el segundo que digas:
    `vortex otaku sonido ARCHIVO 30`), con un desvanecido al final.
  - Si el modo ya estaba encendido no vuelve a sonar, y al apagarlo tampoco suena.
  - `vortex otaku sonido off` lo quita.
  - Como los fondos otaku, el audio **no viene en Vortexine's** (tiene derechos de autor): cada quien pone el
    suyo en su PC.

## Por dentro

- `pruebas/otaku.sh` prueba el recorte de 15 s, el segundo inicial, que suene solo al encender y
  `sonido off`.
- La prueba de «ninguna imagen del personaje» ya no confunde el ícono de la app Miku (una nota musical
  original) con una imagen del personaje.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
