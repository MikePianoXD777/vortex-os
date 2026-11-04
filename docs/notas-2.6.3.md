# Vortexine's OS 2.6.3

Un parche: las apps ya no se cierran solas cuando la memoria se pone justa.

## Lo que se arregló

- **Discord y otras apps ya no se cierran de la nada.** Pasaba sobre todo con 8 GB de RAM y con varias
  cosas abiertas. Había tres causas, y las tres quedan corregidas:
  - **El sistema mataba procesos antes de tiempo**, aunque todavía quedara memoria comprimida libre. Un
    ajuste de memoria (`min_ttl_ms`) estaba muy agresivo. Medido en una máquina de 8 GB, con 12 GB de carga
    había de 1 a 18 procesos muertos en cada ronda; ahora, ninguno. El ajuste vuelve al valor de serie.
  - **Perder un proceso cerraba la app completa.** Discord trabaja con varios procesos y puede reponer el
    que se pierde, pero el sistema cerraba todos. Ahora la app sigue abierta.
  - **Un vigilante de memoria seguía prendido sin querer** (`systemd-oomd`). Se apagaba mal, y además el
    corte de «8 GB» dejaba fuera a las laptops de 8 GB, que reportan un poco menos. Ahora queda apagado de
    verdad en equipos con 8 GB o más.

Si la memoria se acaba de verdad, el sistema todavía tiene que cerrar algo, y antes de hacerlo puede
trabarse unos 3 segundos. Eso es lo esperado.

## Para actualizar

```
vortex actualizar
```

Te va a pedir tu contraseña. **Reinicia al terminar**, o por lo menos cierra y vuelve a abrir tus apps: las
que ya estaban abiertas siguen con el comportamiento anterior hasta que las cierres.

Si después de esto se te sigue cerrando una app, manda lo que sale con este comando:

```
journalctl -k -b --no-pager | grep -E "Out of memory|Killed process"; journalctl --user -b --no-pager | grep -E "oom-kill|OOM killer"; coredumpctl list --no-pager --since "-1 day" | tail -5
```
