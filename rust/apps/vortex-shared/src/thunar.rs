//! «Enviar con Vortexine Shared» en el clic derecho de Thunar, sin tocar tus otras acciones.

use crate::Config;

pub const ID_ACCION: &str = "vortex-shared-enviar";

fn accion() -> String {
    format!(
        "<action>\n\t<icon>vortex-shared</icon>\n\t<name>Enviar con Vortexine Shared</name>\n\t<submenu></submenu>\n\t<unique-id>{ID_ACCION}</unique-id>\n\t<command>vortex-shared ventana %F</command>\n\t<description>Pásalo a otra compu con Vortexine's OS, cerca o lejos</description>\n\t<range>*</range>\n\t<patterns>*</patterns>\n\t<directories/>\n\t<audio-files/>\n\t<image-files/>\n\t<other-files/>\n\t<text-files/>\n\t<video-files/>\n</action>\n"
    )
}

pub fn poner_accion(cfg: &Config) {
    let uca = cfg.config.join("Thunar").join("uca.xml");
    let xml = std::fs::read_to_string(&uca).unwrap_or_else(|_| {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n".to_string()
    });
    if xml.contains(&format!("<unique-id>{ID_ACCION}</unique-id>")) {
        return; // ya está
    }
    let Some(i) = xml.rfind("</actions>") else {
        return; // no es un uca.xml que entendamos: mejor no tocarlo
    };
    if let Some(p) = uca.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = std::fs::write(&uca, format!("{}{}{}", &xml[..i], accion(), &xml[i..]));
}
