# Vortexine en Rust

La reescritura de las apps de Vortexine's OS en Rust, una por una (plan:
«Entregas/Jefe/2026-11-16 Plan de la reescritura en Rust»). **Las apps oficiales siguen siendo las de
Python** de `../apps` hasta que cada una en Rust las iguale.

- Para colaborar: [`CONTRIBUIR.md`](CONTRIBUIR.md).
- Pruebas: `./pruebas.sh`.
- Estado (16/11/2026, paso 0):
  - `vortex-core` (rutas, config, paleta y registro) está hecho, con pruebas. La paleta sale idéntica a
    la de `colores.py`.
  - `vortex-core-qt` y la demo están escritos, pero **sin compilar**, porque faltan las crates de cxx-qt.
  - Player y Local AI son esqueletos.
