# Vortexine's OS 2.8.1

## Lo arreglado

- **Audífonos Bluetooth que se caían a los segundos.** Al sistema le faltaba el agente de Bluetooth, el programa que se
  encarga de emparejar. Sin él, un aparato nuevo podía conectarse sin quedar emparejado, y algunos audífonos cortaban
  a los pocos segundos. Ahora empareja de verdad y guarda la llave, sin que configures nada.

## Lo que debes saber

- **Si tus audífonos ya se caían:** ponlos en modo de emparejamiento y emparéjalos otra vez desde el panel de
  Bluetooth. Lo que ya funcionaba bien sigue igual.
- **Si otro aparato pide emparejarse contigo,** te sale un aviso con botones. Si no contestas, o solo tocas la tarjeta,
  se rechaza: así nadie cerca se cuela haciéndose pasar por un teclado. Los audífonos que conectas tú no preguntan nada.
- **Se probó con unos SOUNDPEATS H3:** quedaron emparejados y no se cayeron en más de 4 minutos, con una reconexión y
  con audio corriendo. Si a ti se te siguen cayendo, cuéntanos, porque puede ser otra cosa, como interferencia.

## Para actualizar

```
vortex actualizar
```
