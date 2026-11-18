//! vortex-gestor-rs: el gestor de recursos de Vortexine's OS. Sin argumentos; lo arranca el servicio de
//! usuario `vortex-gestor.service` (a través de `bin/vortex-gestor`, que salta a este binario si existe).

fn main() {
    std::process::exit(vortex_gestor::gestor::arrancar());
}
