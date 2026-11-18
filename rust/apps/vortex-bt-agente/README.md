# vortex-bt-agente (Rust)

El agente de Bluetooth de Vortexine's OS, reescrito en Rust. Reemplaza a `bin/vortex-bt-agente` (Python + Gio);
la app de Python **sigue siendo la oficial** hasta que esta la iguale en el sistema.

## Qué hace (igual que el de Python)

- Se registra en el bus del **sistema** como `org.bluez.Agent1` en `/org/vortexine/bt_agente`, capacidad
  `DisplayYesNo`, y se pide como agente por defecto (`RegisterAgent` + `RequestDefaultAgent`).
- Se vuelve a registrar si BlueZ reinicia (`NameOwnerChanged` de `org.bluez`).
- Lo que pide otro aparato (PIN, confirmación de número, autorización, servicio) sale como aviso de
  `notify-send` con botones Aceptar/Rechazar, urgencia crítica y **50 s** de espera. Los textos, el PIN de siempre
  (`0000`) y el escape del nombre del aparato son los mismos. Sin respuesta, con Rechazar o al tocar la tarjeta, es
  que no (`org.bluez.Error.Rejected`); si BlueZ cancela, `org.bluez.Error.Canceled`. Sin `notify-send`, también no.
- `RequestPasskey` siempre se rechaza (no hay dónde escribir un número). `DisplayPin/Passkey` solo avisan.
- **Nuevo respecto a Python:** al registrarse deja `Pairable = true` en cada adaptador (`Adapter1`).
- No tiene argumentos de línea de comandos (el de Python tampoco).

## Memoria

Medida contra el mismo BlueZ falso, 3 s después de arrancar:

| | RssAnon | VmRSS |
|---|---|---|
| Python | ~15,5 MB | ~32,8 MB |
| Rust | ~0,35 MB | ~4,6 MB |

zbus 6.0.0 con su ejecutor propio (sin tokio); el aviso usa un hilo corto por pregunta.

## Cómo se prueba

```sh
rust/pruebas.sh                 # fmt, clippy -D warnings y cargo test de todo el workspace
# o solo este crate:
CARGO_BUILD_JOBS=4 systemd-run --user --scope -q -p MemoryMax=2G -p MemorySwapMax=0 -- \
  timeout 1200 cargo test --offline -p vortex-bt-agente
```

`tests/integracion.rs` levanta un `dbus-daemon --session` privado en una carpeta temporal, un BlueZ falso
(`AgentManager1`, `Adapter1`, `Device1`) y un `notify-send` falso; el bus real nunca se toca. La variable
`VORTEX_BT_AGENTE_ESPERA_S` solo existe para acortar la espera en la prueba del vencimiento.

## Qué falta para que reemplace al de Python

- Empaquetar: que el PKGBUILD instale el binario como `/usr/local/bin/vortex-bt-agente` (o ajustar `ExecStart`
  de la unidad que escribe `bt_agente_al_iniciar` en `bin/vortex`), y quitar `python-gobject` de las
  dependencias si ya nada más lo usa (`bin/vortex`, línea del 2.8.1).
- `install/30-escritorio.sh` y `COMANDOS_VORTEX` tratan `vortex-bt-agente` como script de `bin/`: habría que
  distinguirlo.
- Probarlo en la VM con un BlueZ y un adaptador reales (aquí solo con BlueZ falso).
- Decidir si `Pairable = true` permanente es lo que se quiere (BlueZ lo deja así hasta que se apague).
