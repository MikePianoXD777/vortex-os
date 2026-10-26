# Vortexine's OS 1.8.0

Siete cosas nuevas para el día a día: tus juegos en un solo lugar, el contador de FPS, tu celular
conectado, copiar el texto de la pantalla, aviso de memorias USB, fondos que cambian solos y modo
laptop.

## Lo nuevo

- **Tus juegos (Super + Shift + G).** Una biblioteca con todos tus juegos y sus portadas: los de
  **Steam** (de todas tus bibliotecas, también las de otros discos), los de **Heroic** (Epic, GOG y
  Amazon) y las apps de juegos (Minecraft, Roblox con Sober, Prism…). Lo último que jugaste sale
  primero. Escribe para buscar y Enter abre el primero. El modo juego entra solo. También está en el
  lanzador: **«Tus juegos»**.
- **Contador de FPS en los juegos.** Ajustes → General → «Contador de FPS en los juegos» (o
  `vortex fps on`). Muestra los FPS, la GPU y la CPU en la esquina con MangoHud. **Shift derecho + F12**
  lo esconde dentro del juego. Solo sale en los juegos (Steam, Heroic, Lutris y los juegos de
  Flatpak), no en el resto del escritorio. Después de encenderlo, vuelve a abrir Steam.
- **Tu celular (Super + Shift + P).** Con KDE Connect: las notificaciones del celular llegan a la PC,
  ves su batería, le mandas archivos o lo que copiaste, y lo haces sonar cuando no lo encuentras.
  Viene apagado: el panel explica los tres pasos y lo enciende con un botón. Instala **KDE Connect** en
  tu celular (Play Store o F-Droid) y conéctalo al mismo Wi-Fi.
- **Copiar el texto de la pantalla (Super + Shift + T).** Marcas una zona y su texto queda copiado,
  listo para pegar: sirve con imágenes, videos pausados, PDFs escaneados o mensajes de error. Lee
  español e inglés.
- **Aviso de memorias USB.** Al conectar una memoria o una tarjeta SD, se monta sola y sale un aviso
  con **«Abrir»** (la abre en el gestor de archivos) y **«Expulsar»** (la saca con seguridad).
- **Fondos que cambian solos.** Ajustes → Tema → «Fondos que cambian solos»: cada 15 o 30 minutos,
  cada hora o cada 3 horas, un fondo al azar de tu carpeta de fondos. No cambia los fondos animados
  ni en modo juego.
- **Modo laptop.** Al desenchufar la laptop: perfil de ahorro, sin animaciones, brillo al 60 % y
  la pantalla a 60 Hz. Al enchufarla vuelve todo como estaba. Se puede apagar en Ajustes → Pantalla
  (solo aparece en equipos con batería).

La hoja de atajos (Super + F1) ya trae los atajos nuevos.

## Por dentro

- Comandos nuevos de `vortex`: `fps`, `celular`, `fondos-auto` y `bateria`. Programas nuevos:
  `vortex-usb` y `vortex-texto`.
- Paneles nuevos del shell: `juegos/` (con `juegos.sh`) y `celular/` (con `celular.sh`, que lee KDE
  Connect por D-Bus). Secciones nuevas en Ajustes: FPS (General), fondos automáticos (Tema) y modo
  laptop (Pantalla). vortex-shell 2.2.14-14.
- **`vortex actualizar` instala los paquetes nuevos que falten** (`udisks2`, `tesseract` con español e
  inglés, `kdeconnect` y `zenity`). Además pone al día los atajos de Hyprland y arranca el aviso de USB
  y los fondos automáticos sin cerrar la sesión.
- Pruebas nuevas: `pruebas/usb.sh`, `texto.sh`, `fondos-auto.sh`, `bateria.sh`, `celular.sh`, `fps.sh`
  y `juegos.sh`, todas con programas falsos y un HOME temporal.

## Actualizar

```bash
vortex actualizar
```

Pide tu contraseña una vez, para instalar los paquetes nuevos. Se instala con la ISO 1.6.3 y luego
`vortex actualizar`.

## Requisitos

Los mismos de la 1.7. Para ver el contador de FPS en los juegos de Flatpak, instala la capa de
MangoHud para Flatpak (`flatpak install flathub org.freedesktop.Platform.VulkanLayer.MangoHud`).
