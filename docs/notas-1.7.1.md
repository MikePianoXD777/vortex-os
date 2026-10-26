# Vortexine's OS 1.7.1

Un saludo propio en la terminal, Super+D vuelve a abrir tus chats, y arreglos del 1.7.

## Lo nuevo

- **El saludo de la terminal es de Vortex:**
  - Al abrir la terminal ya no sale el letrero de Caelestia: sale **«Vortexine's OS»** en morado, el
    logo de Vortex y, al lado, tu PC: sistema, kernel, Hyprland, procesador, gráfica, memoria, disco,
    shell y cuánto lleva encendida.
  - En kitty el logo es la imagen de verdad; en otras terminales, un logo de texto.
  - Funciona en bash y en fish. Si tenías un saludo propio en fish, se respeta; el de Caelestia se
    guarda como `fish_greeting.fish.caelestia`.
  - Para verlo cuando quieras: `vortex saludo`.
- **Super+D, el escritorio de los chats, como en Caelestia:** esconde lo que tienes y muestra Discord y
  WhatsApp; otra vez Super+D y vuelves.
  - Antes solo buscaba el Discord de Arch y no sabía abrir WhatsApp, así que casi siempre salía vacío.
  - Ahora abre Discord aunque sea su Flatpak (`com.discordapp.Discord`) o Vesktop, y el WhatsApp de
    Flatpak (`io.github.mimbrero.WhatsAppDesktop`). Si ya estaban abiertos en otro escritorio, los trae.
- **`vortex actualizar` siempre reinicia el shell al terminar:** lo nuevo se ve en seguida, sin cerrar
  sesión (antes solo pasaba si cambiaba el paquete del shell).
- **Miku tiene su ícono en la barra** en lugar del cuadrado de reserva.

## Arreglos

- **«Seleccionar fondo» del menú del escritorio (clic derecho) no hacía nada.** Tampoco el ecualizador de
  música, las apps del dock en las acciones rápidas ni la ventana activa. Fallaban si el shell estaba en
  una carpeta con espacios; ahora no.

## Por dentro

- `config/terminal/`: el saludo (`saludo.sh`, su `fastfetch.jsonc` y los logos) y `poner-saludo.sh`, que lo
  instala una sola vez por usuario (en la instalación y con `vortex actualizar`).
- `pruebas/atajos.lua` prueba que Super+D lance Discord con sus respaldos y WhatsApp, y que no lance nada
  si ya están abiertos.

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

Para Super+D necesitas Discord. Si no lo tienes:

```bash
flatpak install flathub com.discordapp.Discord
```

## Requisitos

Los mismos de la 1.7.
