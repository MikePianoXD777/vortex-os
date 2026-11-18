//! Configuración en JSON, como `config.py` de las apps de Python.
//!
//! - Lo que falte toma su valor de fábrica (`Default` del tipo).
//! - Un JSON roto no truena: se usan los valores de fábrica.
//! - Un valor con el tipo equivocado («volumen»: "mucho») se ignora solo esa clave; las demás se respetan.
//! - Al guardar no se pierden las claves que la app no conoce (otra versión, otra app).
//! - Se guarda de forma atómica (un .tmp y luego se renombra) y en Linux solo lo lee tu usuario (600).

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// La ruta de un archivo de configuración de Vortexine (`~/.config/vortex/<nombre>`).
pub fn ruta(nombre: &str) -> PathBuf {
    crate::rutas::config().join(nombre)
}

/// El JSON tal cual está en disco (vacío si falta, no se puede leer o no es un objeto).
pub fn cargar_crudo(ruta: &Path) -> Map<String, Value> {
    fs::read_to_string(ruta)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

/// La configuración efectiva: cada clave con su valor de fábrica si falta o viene mal.
pub fn cargar<T: Serialize + DeserializeOwned + Default>(ruta: &Path) -> T {
    let Ok(Value::Object(mut mezcla)) = serde_json::to_value(T::default()) else {
        return T::default();
    };
    for (clave, valor) in cargar_crudo(ruta) {
        if !mezcla.contains_key(&clave) {
            continue; // una clave que este tipo no conoce: se queda en disco, no aquí
        }
        let anterior = mezcla.insert(clave.clone(), valor);
        if serde_json::from_value::<T>(Value::Object(mezcla.clone())).is_err()
            && let Some(a) = anterior
        {
            mezcla.insert(clave, a); // tipo equivocado: se queda el de fábrica
        }
    }
    serde_json::from_value(Value::Object(mezcla)).unwrap_or_default()
}

/// Pone las claves de `cambios` sin tocar las demás (crea la carpeta y el archivo si faltan).
pub fn guardar(ruta: &Path, cambios: &Map<String, Value>) -> io::Result<()> {
    let mut datos = cargar_crudo(ruta);
    for (k, v) in cambios {
        datos.insert(k.clone(), v.clone());
    }
    escribir_atomico(ruta, &serde_json::to_string_pretty(&Value::Object(datos))?)
}

/// Guarda un valor serializable completo (sus claves encima de las que ya hay en disco).
pub fn guardar_todo<T: Serialize>(ruta: &Path, valor: &T) -> io::Result<()> {
    match serde_json::to_value(valor)? {
        Value::Object(m) => guardar(ruta, &m),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "la configuración tiene que ser un objeto",
        )),
    }
}

/// Escribe en `<ruta>.tmp` (600 en Linux) y lo renombra encima: nunca queda un archivo a medias.
pub fn escribir_atomico(ruta: &Path, texto: &str) -> io::Result<()> {
    if let Some(carpeta) = ruta.parent() {
        fs::create_dir_all(carpeta)?;
    }
    let mut tmp = ruta.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, texto)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&tmp, ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(default)]
    struct Ajustes {
        volumen: u32,
        tema: String,
        aleatorio: bool,
    }
    impl Default for Ajustes {
        fn default() -> Self {
            Ajustes {
                volumen: 80,
                tema: "oscuro".into(),
                aleatorio: false,
            }
        }
    }

    fn carpeta() -> PathBuf {
        crate::carpeta_de_prueba("config")
    }

    #[test]
    fn sin_archivo_da_lo_de_fabrica() {
        assert_eq!(
            cargar::<Ajustes>(&carpeta().join("no-existe.json")),
            Ajustes::default()
        );
    }

    #[test]
    fn json_roto_no_truena() {
        let r = carpeta().join("roto.json");
        fs::write(&r, "{ esto no es json").unwrap();
        assert_eq!(cargar::<Ajustes>(&r), Ajustes::default());
        fs::write(&r, "[1, 2, 3]").unwrap();
        assert_eq!(cargar::<Ajustes>(&r), Ajustes::default());
    }

    #[test]
    fn una_clave_mala_no_tumba_a_las_demas() {
        let r = carpeta().join("mixto.json");
        fs::write(&r, r#"{"volumen": "mucho", "tema": "claro", "aleatorio": true}"#).unwrap();
        assert_eq!(
            cargar::<Ajustes>(&r),
            Ajustes {
                volumen: 80,
                tema: "claro".into(),
                aleatorio: true
            }
        );
    }

    #[test]
    fn guardar_no_pierde_claves_ajenas_y_es_privado() {
        let r = carpeta().join("sub/ajustes.json");
        fs::create_dir_all(r.parent().unwrap()).unwrap();
        fs::write(&r, r#"{"otra_app": {"x": 1}, "volumen": 10}"#).unwrap();
        let mut cambios = Map::new();
        cambios.insert("volumen".into(), json!(55));
        guardar(&r, &cambios).unwrap();
        let crudo = cargar_crudo(&r);
        assert_eq!(crudo["otra_app"], json!({"x": 1}));
        assert_eq!(crudo["volumen"], json!(55));
        assert!(!r.with_extension("json.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&r).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn guardar_todo_y_volver_a_leer() {
        let r = carpeta().join("todo.json");
        let a = Ajustes {
            volumen: 33,
            tema: "claro".into(),
            aleatorio: true,
        };
        guardar_todo(&r, &a).unwrap();
        assert_eq!(cargar::<Ajustes>(&r), a);
    }
}
