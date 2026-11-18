//! Las diferencias entre la lista de filas de antes y la de ahora (lo que hacía `difflib` en `modelo.py`).
//!
//! Cada segundo llega una lista nueva. Si se la diéramos a QML como arreglo, rehacería todas las filas
//! (se pierde el desplazamiento y parpadea); aquí se compara por la clave de cada fila y solo se quitan o
//! meten las que cambiaron. El modelo de Qt (C++) aplica estas operaciones de atrás para adelante.

/// Una operación que no es «igual»: en la lista vieja se quitan `[i1, i2)` y en su lugar se meten las
/// nuevas `[j1, j2)` (si `i1 == i2` solo se insertan; si `j1 == j2` solo se quitan).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operacion {
    pub i1: usize,
    pub i2: usize,
    pub j1: usize,
    pub j2: usize,
}

/// Las operaciones (en orden de arriba hacia abajo) que convierten `viejas` en `nuevas`.
pub fn diferencias(viejas: &[String], nuevas: &[String]) -> Vec<Operacion> {
    // Lo igual al principio y al final no se compara (lo normal es que casi nada cambie).
    let prefijo = viejas.iter().zip(nuevas).take_while(|(a, b)| a == b).count();
    let sufijo = viejas[prefijo..]
        .iter()
        .rev()
        .zip(nuevas[prefijo..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let a = &viejas[prefijo..viejas.len() - sufijo];
    let b = &nuevas[prefijo..nuevas.len() - sufijo];
    if a.is_empty() && b.is_empty() {
        return Vec::new();
    }
    // Subsecuencia común más larga por programación dinámica.
    let (n, m) = (a.len(), b.len());
    let mut t = vec![0u32; (n + 1) * (m + 1)];
    let en = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[en(i, j)] = if a[i] == b[j] {
                t[en(i + 1, j + 1)] + 1
            } else {
                t[en(i + 1, j)].max(t[en(i, j + 1)])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut corrida: Option<(usize, usize)> = None; // dónde empezó lo que no es igual
    let mut cerrar = |corrida: &mut Option<(usize, usize)>, i: usize, j: usize| {
        if let Some((i1, j1)) = corrida.take() {
            ops.push(Operacion {
                i1: i1 + prefijo,
                i2: i + prefijo,
                j1: j1 + prefijo,
                j2: j + prefijo,
            });
        }
    };
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            cerrar(&mut corrida, i, j);
            i += 1;
            j += 1;
            continue;
        }
        corrida.get_or_insert((i, j));
        if j >= m || (i < n && t[en(i + 1, j)] >= t[en(i, j + 1)]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    cerrar(&mut corrida, i, j);
    ops
}

/// Aplica las operaciones a una lista de claves (como lo hace el modelo de Qt; sirve para probarlas).
pub fn aplicar(viejas: &[String], nuevas: &[String], ops: &[Operacion]) -> Vec<String> {
    let mut v = viejas.to_vec();
    for o in ops.iter().rev() {
        v.splice(o.i1..o.i2, nuevas[o.j1..o.j2].iter().cloned());
    }
    v
}
