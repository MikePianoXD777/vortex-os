# Vortexine's OS 1.9.0

Dos cosas para el día a día: un botón para cuando algo no funciona y una calculadora que entiende
porcentajes, unidades y divisas.

## Lo nuevo

- **¿Algo no funciona?** Búscalo en el lanzador como **«Algo no funciona»** (o en la terminal:
  `vortex arreglar`). Revisa tu equipo en un segundo:
  - **Sonido**: si está silenciado, en 0 o sin salida.
  - **Micrófono**: si está silenciado o en 0.
  - **Internet y Wi-Fi**: red apagada, Wi-Fi apagado, conectado pero sin internet, o una red que pide
    iniciar sesión (hotel, escuela, café).
  - **Bluetooth**: si está apagado.
  - **Disco**: si se está llenando.
  - **Fecha y hora**: si no se pone sola.
  - **Actualizaciones**: si una quedó a medias y no deja instalar nada más.
  - **Servicios**: los que fallaron al arrancar.

  Te dice qué pasa en palabras simples y, en lo que se puede, un botón **«Arreglar»**: reinicia el
  sonido, quita el silencio, reconecta la red, enciende el Wi-Fi o el Bluetooth, libera espacio
  (cachés y paquetes viejos, nunca tus archivos), pone la hora automática o destraba las
  actualizaciones. Lo que necesita permisos de administrador te pide tu contraseña. Después de
  arreglar vuelve a revisar, para que veas si quedó.
- **La calculadora del lanzador** ahora entiende:
  - **Porcentajes:** «15% de 230», «230 + 15%», «230 - 15%».
  - **Unidades:** «5 km a millas», «100 f a c», «1,5 kg a lb», «2 gb en mb», «90 km/h a mph»,
    «3 tazas a ml», «1 hora a minutos». Longitud, peso, volumen, temperatura, velocidad, datos,
    tiempo y área, en español o en inglés, con o sin acentos.
  - **Divisas:** «100 usd a mxn», «$100 a pesos», «50 euros a dólares». El tipo de cambio es el de
    hoy, de open.er-api.com (gratis y sin cuenta); se guarda 12 horas y, sin internet, usa el último
    que tenga.

  Enter copia el resultado.

## Por dentro

- `bin/vortex-arreglar` (`revisar` da la lista en JSON; `arreglar ID` arregla) y el panel
  `shell/src/quickshell/arreglar/`. Lee el Wi-Fi y el Bluetooth de `/sys/class/rfkill` y corre con
  `LC_ALL=C`, así funciona igual con el sistema en cualquier idioma.
- `shell/src/quickshell/launcher/Calculadora.js` y `launcher/divisas.sh`. vortex-shell 2.2.14-15.
- `vortex actualizar` instala `pacman-contrib` si falta (lo usa «Disco» para limpiar la caché de
  pacman).
- Pruebas nuevas: `pruebas/arreglar.sh` (36 casos con un equipo falso) y `pruebas/calculadora.sh`.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
