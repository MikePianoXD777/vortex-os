# Vortexine's OS 1.1

La actualización de la tienda. Todo lo de la 1.0, más **Vortexine's Shop**: instalar apps y juegos de
Flathub con un clic, sin terminal y sin contraseñas.

## Lo que trae

- **Vortexine's Shop**, la tienda de apps (Super+I, o «Vortexine's Shop» en el lanzador):
  - todo lo que hay en [Flathub](https://flathub.org), con búsqueda, categorías (juegos, audio y vídeo,
    gráficos, programación, oficina, internet y chat...), fichas con capturas, versión, licencia y tamaño;
  - recomendadas por Vortexine's (Steam, Prism Launcher, Sober, Heroic, Discord, Firefox, OBS...) y
    las más instaladas por la gente en el inicio;
  - **Instalar** con el progreso real de la descarga; si pides varias, esperan su turno de una en una;
  - **Abrir**, **Quitar** (con confirmación) y **Actualizaciones**, con el contador en la barra lateral;
  - una página «Instaladas» con lo que bajaste desde la tienda.
- **Sin contraseña:** las apps se instalan para tu usuario (`flatpak --user`), así que no piden `sudo` ni
  ventana de permisos. Las que vienen con el sistema (las del instalador) aparecen como instaladas, pero
  solo un administrador las quita.
- **No pesa en la ISO:** la ISO trae solo la tienda (unos 55 MB con PySide6); cada app se descarga de
  Flathub cuando la eliges. Flathub se agrega solo la primera vez.
- **Se ve como tu escritorio:** los colores de la tienda salen del fondo de pantalla, igual que el
  resto del shell, y cambian en vivo cuando cambias el fondo.
- **Rápida y sin bloqueos:** todo lo lento corre aparte; la ventana nunca se congela. Con la caché de
  `~/.cache/vortexines-shop/` (listas 1 h, fichas 24 h) sin internet sigue mostrando lo último que vio.
- Una sola ventana de la tienda por usuario: si la abres otra vez (Super+I), se trae la que ya estaba.

## Cómo se usa

1. **Super+I** (o busca «Vortexine's Shop» en el lanzador con Super).
2. Busca arriba, entra a una categoría o mira las recomendadas.
3. Entra a una app para ver sus capturas y descripción, y pulsa **Instalar**.
4. Cuando termine aparece **Abrir**; también la encuentras en el lanzador. **Quitar** la desinstala
   (tus datos en `~/.var/app` se conservan).

## Requisitos

Los de la 1.0: PC x86-64 con 4 GB de RAM (8 GB recomendado) y 40 GB de disco, USB de 8 GB. Internet para
usar la tienda y para las apps que elijas al instalar.

## Avisos

- La tienda necesita internet para mostrar el catálogo por primera vez; después usa la caché.
- Las apps salen de Flathub tal como las publican sus autores: Vortexine's no las revisa.
- Los mismos de la 1.0: algunos textos del instalador salen en inglés con «Español (México)» (la
  traducción de Calamares para es_MX está incompleta), y las apps de Flatpak elegidas al instalar se
  bajan en el primer arranque con internet.
