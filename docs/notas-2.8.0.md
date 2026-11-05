# Vortexine's OS 2.8.0

## Lo nuevo

- **Vortexine Connect.** Internet para cuando viajas. Te muestra las redes wifi abiertas cerca y entras con un
  clic. Si la red pide aceptar algo, te abre su página y espera; cuando ya hay internet, cifra lo que mandas con
  Cloudflare WARP, gratis y sin cuenta. Solo entra a redes abiertas o con portal: no rompe contraseñas. La
  primera vez te pide aceptar las condiciones de Cloudflare.
- **Modo viaje.** Si lo prendes, cuando te quedas sin internet Connect entra sola a la red abierta con mejor
  señal y la cifra. Al volver a tu red, apaga la protección. Viene apagado porque una red abierta puede ser falsa.
- **También para Windows.** Connect tiene instalador y versión portátil en su propia Release; WARP se baja de
  Cloudflare con un botón de la app.

## Lo que debes saber

- **Qué protege:** la red donde estás ya no ve lo que mandas ni los nombres de los sitios que abres. Tu tráfico
  pasa por Cloudflare, así que no te vuelve anónimo.
- **Lo que escribas en la página de la red no va cifrado:** ahí todavía no hay protección. No pongas tus
  contraseñas de siempre.
- **Si la red bloquea el cifrado,** solo van protegidos los nombres de los sitios, y la app te lo dice. Evita
  entrar a tu banco ahí.
- **Mientras esté cifrado, Tailscale no conecta:** lo que lo usa, como Shared a distancia, espera hasta que
  quites la protección. La app te avisa si tienes Tailscale.
- **Ocupa** 45 MB de descarga y 216 MB en disco, porque sin internet ya no podrías bajar WARP. Cuando no estás
  protegido, su servicio está apagado.
- **Es la primera versión:** se probó con WARP de verdad en una red de prueba, pero todavía no en una laptop con
  wifi real ni en Windows. Si algo falla, cuéntanos.

## Para actualizar

```
vortex actualizar
```
