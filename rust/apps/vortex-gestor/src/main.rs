//! vortex-gestor-rs: el gestor de recursos de Vortexine's OS. Sin argumentos; lo arranca el servicio de
//! usuario `vortex-gestor.service` (a través de `bin/vortex-gestor`, que salta a este binario si existe).

//!
//! 4.5: `vortex-gestor-rs reabrir` vuelve a abrir la última app cerrada (Super+Shift+R, `vortex reabrir`).

fn main() {
    if std::env::args().nth(1).as_deref() == Some("reabrir") {
        let r = vortex_gestor::config::Rutas::del_entorno(vortex_gestor::sis::uid());
        std::process::exit(vortex_gestor::reabrir::correr(&r.estado_dir));
    }
    std::process::exit(vortex_gestor::gestor::arrancar());
}
