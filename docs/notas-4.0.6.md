# Vortexine's OS 4.0.6

## Arreglado

### OneDrive se quedaba reintentando para siempre

Si la cuenta de OneDrive quedaba a medias (por ejemplo, después de una actualización de rclone),
la carpeta no podía montarse y el sistema lo volvía a intentar cada 20 segundos, todo el día, sin
decirte nada.

- **Ahora lo intenta 3 veces y para.** Un fallo suelto, como arrancar sin internet, no cuenta.
- **Te avisa**: el panel de OneDrive dice que hay que volver a conectarlo. Con «Conectar», vuelves a
  iniciar sesión y la carpeta se monta.
- Si ya tenías OneDrive con la cuenta rota, `vortex actualizar` lo detiene y te avisa.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
