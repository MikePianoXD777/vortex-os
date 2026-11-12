# Vortexine's OS 3.3.5

## Lo arreglado

### Wi-Fi MediaTek que se cae o va lento (ASUS Vivobook Go y otras)

Algunas laptops traen una tarjeta de red **MediaTek** (MT7902, MT7921, MT7922; por ejemplo, las ASUS Vivobook Go 14 y 15)
y, en Linux, el Wi-Fi se corta o va muy lento por el ahorro de energía de la tarjeta.

Ahora, si tu compu tiene una tarjeta de red MediaTek, Vortexine's apaga ese ahorro de energía del Wi-Fi por ti, tanto
en instalaciones nuevas como al correr `vortex actualizar`. Si tu compu no tiene esa tarjeta, no se toca nada.

El kernel y el firmware de esa tarjeta se ponen al día solos con `vortex actualizar`.

**Si tienes una de estas laptops y el Wi-Fi no funciona:** conecta internet por cable o con tu celular por USB (módem USB),
corre `vortex actualizar` y reinicia.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
