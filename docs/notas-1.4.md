# Vortexine's OS 1.4

Miku con internet, y `vortex actualizar` que por fin funciona en las instalaciones hechas desde la ISO.

## Lo nuevo

- **Miku con internet (opcional):** puede buscar en la web y leer páginas cuando le preguntas algo actual
  (documentación, un error, versiones, noticias).
  - Cada búsqueda y cada página te pide permiso en su ventana (**Autorizar** / **Cancelar**), mostrando la
    búsqueda o la dirección exacta. Así una página no puede hacer que Miku mande tus datos a otro sitio a
    escondidas.
  - Lo que lee en internet lo toma como información, nunca como órdenes, y no pone datos tuyos (lo que hay
    en tu pantalla, tus archivos) en una búsqueda.
  - Viene **apagado**. Se enciende en **Ajustes → General → «Miku con internet»**, con
    `vortex-miku internet on`, o marcando **«Miku con internet»** en el instalador (junto a Claude).
  - Usa la búsqueda y la lectura de páginas de Claude, así que necesita a Claude activado.
- **`vortex actualizar` en instalaciones desde la ISO:** hasta la 1.3 la ISO dejaba el código de Vortex
  en `/usr/share/vortex-os` sin git, y además de root, así que `vortex actualizar` no podía traer nada nuevo.
  Ahora:
  - la primera vez conecta esa carpeta con el repo de Vortexine's (clona aparte primero: sin internet no
    toca nada);
  - trae el código con `sudo git` y construye el shell, la tienda y Miku en una copia en tu `~/.cache`;
  - la ISO ya trae el repo con git.
- **Arreglo:** `vortex actualizar` ya no se corta al recargar Hyprland desde una terminal sin la variable de
  Hyprland.
- **Arreglo: el instalador abre en procesadores sin AVX2** (Intel anteriores a Haswell, muchos
  Celeron/Pentium/Atom, AMD A8/A10 viejos). En la 1.3 Calamares decía «CPU ISA level is lower than
  required» y no abría. Ahora se compila en un sistema Arch genérico y la ISO se revisa antes de salir, así
  que ya no hace falta el arreglo `iso/arreglos/calamares-cpu-vieja.sh` (sigue ahí para quien tenga la ISO
  1.3).

## Si vienes de la 1.3 instalada desde la ISO

El `vortex actualizar` de la 1.3 no sabe conectarse al repo. Una sola vez, en una terminal:

```bash
curl -fsSL https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/bin/vortex -o /tmp/vortex-nuevo && bash /tmp/vortex-nuevo actualizar
```

Después, `vortex actualizar` como siempre. Si instalaste con `install.sh` desde tu carpeta, basta con
`vortex actualizar`.

## Requisitos

Los mismos de la 1.0: PC x86-64 con 4 GB de RAM (8 GB recomendado), 40 GB de disco y una USB de 8 GB.
