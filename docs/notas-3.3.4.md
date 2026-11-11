# Vortexine's OS 3.3.4

## Lo mejorado

### «Crear reporte» dice mejor qué pasó cuando se trabó todo

Si tu compu se queda congelada y tienes que reiniciarla, el reporte ahora también incluye:

- **Qué ventanas había abiertas** (el programa, el escritorio y si estaban flotantes, fijas o en pantalla completa; sin títulos, para no mostrar páginas ni archivos).
- **Si estabas en el modo «kde»**, si la barra de título de las ventanas se cargó bien.
- **La memoria y la swap** en ese momento.
- **Lo que pasó antes de reiniciar:** si faltó memoria o si el escritorio se cerró.

Con eso se puede saber si se acabó la memoria, si fue la tarjeta de video o si fue otra cosa.

Si te pasa, reinicia y abre el lanzador, busca **«Algo no funciona»** y dale a **Crear reporte** (o `vortex reporte` en una terminal). Luego mándalo. No lleva tu usuario, tus IP ni tus correos. Y la próxima vez que se trabe, espera 2 minutos antes de apagar: si fue la memoria, suele recuperarse sola.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
