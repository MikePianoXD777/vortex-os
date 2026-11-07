# Vortexine's OS 2.9.2

## Lo nuevo

### Actualizar sin internet ya no llena la pantalla de errores

Si no hay internet (o el DNS no responde), `vortex actualizar` espera unos segundos por si la red viene llegando y, si no
llega, te lo dice claro y no toca nada: «No hay internet: no se actualizó nada». Antes seguía y pacman soltaba un error por
cada servidor del mundo, y el código de Vortex se quedaba en la versión de antes sin que se notara.

### Descargas desde servidores cerca de ti

La ISO dejaba prendidos todos los servidores de Arch del mundo, en orden de país: pacman probaba Uzbekistán o Vietnam antes
que los cercanos. Ahora `vortex actualizar` mide una vez al mes cuáles son los más rápidos desde donde estás (con
`cachyos-rate-mirrors`, el de CachyOS) y deja solo esos. Los servidores oficiales de CachyOS siguen primero. En la máquina
virtual pasó de 425 servidores a los 36 más rápidos, en unos 20 segundos.

La primera vez se instala el medidor; los servidores se miden al final de esa misma actualización y se notan desde la
siguiente.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
