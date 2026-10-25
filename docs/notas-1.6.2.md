# Vortexine's OS 1.6.2

Arregla el instalador en PCs con 8 GB de RAM o más.

## Lo nuevo

- **El instalador ya no falla con «La configuración de unpackfs no es correcta»:**
  - Con una USB y suficiente RAM, el arranque copiaba todo el sistema a la memoria y soltaba la USB, y el
    instalador ya no encontraba de dónde copiar Vortexine's (`/run/archiso/bootmnt/arch/x86_64/airootfs.sfs`).
    Había que editar la línea de arranque a mano.
  - Ahora todas las entradas del menú de arranque llevan `copytoram=n`: el sistema se lee de la USB y el
    instalador funciona a la primera. No hay que tocar nada.
- **El botón «67»** en Ajustes → Bienvenida.
- **`vortex actualizar` reinicia el shell** cuando lo reconstruye, para que cargue la versión nueva.

## Por dentro

- `vm/vm.sh live` con `VORTEX_VM_USB=1` conecta la ISO como memoria USB en vez de CD. Desde un CD archiso
  nunca copia a la RAM, por eso las pruebas en la VM no veían el fallo.

## Actualizar

```bash
vortex actualizar
```

Lo instalado no cambia con este arreglo (era solo del instalador). Si vas a instalar, usa la ISO 1.6.2.

## Requisitos

Los mismos de la 1.6.
