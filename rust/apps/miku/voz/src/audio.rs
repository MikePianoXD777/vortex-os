//! Cómo suena cada frase y cómo se mueve la boca (lo de `audio.py`): tono, brillo, volumen, el WAV y los
//! visemas. Todo es por frase y sin numpy: una FFT propia (radix 2) y una DFT de las pocas bandas que miran
//! los visemas.

use std::f64::consts::PI;

pub fn centesimas_a_factor(centesimas: f64) -> f64 {
    2f64.powf(centesimas / 1200.0)
}

/// `np.interp(x, xp, fp)` para `xp` creciente: fuera del rango, el valor del borde.
fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    if x <= xp[0] {
        return fp[0];
    }
    let ultimo = xp.len() - 1;
    if x >= xp[ultimo] {
        return fp[ultimo];
    }
    let i = xp.partition_point(|&v| v <= x) - 1;
    let t = (x - xp[i]) / (xp[i + 1] - xp[i]);
    fp[i] + (fp[i + 1] - fp[i]) * t
}

/// Baja o sube el tono remuestreando (como frenar o acelerar una cinta): negativo = más grave y más lento.
pub fn cambiar_tono(x: &[f32], centesimas: f64) -> Vec<f32> {
    if centesimas == 0.0 || x.len() < 2 {
        return x.to_vec();
    }
    let r = centesimas_a_factor(centesimas);
    let n = (x.len() as f64 / r) as usize;
    let ultimo = x.len() - 1;
    (0..n)
        .map(|i| {
            let p = i as f64 * r;
            let j = p.floor() as usize;
            if j >= ultimo {
                return x[ultimo];
            }
            let t = (p - j as f64) as f32;
            x[j] + (x[j + 1] - x[j]) * t
        })
        .collect()
}

/// FFT compleja en el lugar (radix 2; `re.len()` potencia de 2). `inversa` divide entre N.
fn fft(re: &mut [f64], im: &mut [f64], inversa: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let signo = if inversa { 1.0 } else { -1.0 };
    let mut largo = 2;
    while largo <= n {
        let ang = signo * 2.0 * PI / largo as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for inicio in (0..n).step_by(largo) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..largo / 2 {
                let a = inicio + k;
                let b = a + largo / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                (cr, ci) = (cr * wr - ci * wi, cr * wi + ci * wr);
            }
        }
        largo <<= 1;
    }
    if inversa {
        for v in re.iter_mut().chain(im.iter_mut()) {
            *v /= n as f64;
        }
    }
}

/// Quita brillo: paso bajo suave (Butterworth de 3.er orden, fase cero) y un estante de agudos.
pub fn filtrar_agudos(x: &[f32], sr: u32, corte_hz: f64, estante_db: f64) -> Vec<f32> {
    const ESTANTE_HZ: f64 = 3500.0;
    if x.len() < 8 {
        return x.to_vec();
    }
    let n = x.len().next_power_of_two();
    let mut re: Vec<f64> = x
        .iter()
        .map(|&v| v as f64)
        .chain(std::iter::repeat_n(0.0, n - x.len()))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im, false);
    for k in 0..n {
        let bin = k.min(n - k);
        let f = bin as f64 * sr as f64 / n as f64;
        let mut g = 1.0 / (1.0 + (f / corte_hz).powi(6)).sqrt();
        if estante_db != 0.0 {
            let paso = 1.0 / (1.0 + (-(f - ESTANTE_HZ) / (ESTANTE_HZ * 0.25)).exp());
            g *= 10f64.powf(estante_db * paso / 20.0);
        }
        re[k] *= g;
        im[k] *= g;
    }
    fft(&mut re, &mut im, true);
    re[..x.len()].iter().map(|&v| v as f32).collect()
}

fn db(v: f64) -> f64 {
    20.0 * v.max(1e-9).log10()
}

/// Volumen bajo y parejo: comprime suave lo fuerte (envolvente cada 5 ms, ataque 10 ms, suelta 120 ms, umbral
/// = el nivel medio de la frase), deja la frase en `rms_db` (sin contar silencios) y el pico bajo `pico_db`.
pub fn nivelar(x: &[f32], sr: u32, rms_db: f64, pico_db: f64, relacion: f64) -> Vec<f32> {
    if x.len() < (sr / 50) as usize {
        return x.to_vec();
    }
    let salto = ((sr / 200) as usize).max(1);
    let n = x.len() / salto;
    let env_db: Vec<f64> = (0..n)
        .map(|i| {
            let b = &x[i * salto..(i + 1) * salto];
            let m = b.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / salto as f64;
            20.0 * (m + 1e-12).sqrt().log10()
        })
        .collect();
    let maximo = env_db.iter().cloned().fold(f64::MIN, f64::max);
    let voz: Vec<f64> = env_db.iter().cloned().filter(|&e| e > maximo - 35.0).collect();
    let umbral = if voz.is_empty() {
        maximo
    } else {
        voz.iter().sum::<f64>() / voz.len() as f64
    };
    let (ataque, suelta) = (1.0 - (-5.0f64 / 10.0).exp(), 1.0 - (-5.0f64 / 120.0).exp());
    let mut g = 0.0;
    let suave: Vec<f64> = env_db
        .iter()
        .map(|&e| {
            let objetivo = if e > umbral {
                (e - umbral) / relacion + umbral
            } else {
                e
            };
            let r = (objetivo - e).min(0.0);
            g += (r - g) * if r < g { ataque } else { suelta };
            10f64.powf(g / 20.0)
        })
        .collect();
    let xp: Vec<f64> = (0..n).map(|i| (i as f64 + 0.5) * salto as f64).collect();
    let mut y: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, &v)| v as f64 * interp(i as f64, &xp, &suave))
        .collect();
    let pico_y = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let piso = 10f64.powf((db(pico_y) - 35.0) / 20.0);
    let activos: Vec<f64> = y.iter().cloned().filter(|v| v.abs() > piso).collect();
    let rms = if activos.is_empty() {
        (y.iter().map(|v| v * v).sum::<f64>() / y.len() as f64).sqrt()
    } else {
        (activos.iter().map(|v| v * v).sum::<f64>() / activos.len() as f64).sqrt()
    };
    let k = 10f64.powf(rms_db / 20.0) / rms.max(1e-9);
    y.iter_mut().for_each(|v| *v *= k);
    let pico = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let tope = 10f64.powf(pico_db / 20.0);
    if pico > tope {
        if pico > 2.0 * tope {
            let t09 = 0.9f64.tanh();
            y.iter_mut()
                .for_each(|v| *v = (*v / tope * 0.9).tanh() * tope / t09);
        } else {
            y.iter_mut().for_each(|v| *v *= tope / pico);
        }
    }
    y.into_iter().map(|v| v as f32).collect()
}

/// WAV de 16 bits, mono.
pub fn a_wav(x: &[f32], sr: u32) -> Vec<u8> {
    let datos = (x.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + datos as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + datos).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&sr.to_le_bytes());
    b.extend_from_slice(&(sr * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&datos.to_le_bytes());
    for &v in x {
        b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    b
}

/// `np.percentile(v, p)` (interpolación lineal).
fn percentil(v: &[f64], p: f64) -> f64 {
    let mut o = v.to_vec();
    o.sort_by(|a, b| a.total_cmp(b));
    let pos = (o.len() - 1) as f64 * p / 100.0;
    let i = pos.floor() as usize;
    let j = (i + 1).min(o.len() - 1);
    o[i] + (o[j] - o[i]) * (pos - i as f64)
}

fn pertenencia(valor: f64, centro: f64, ancho: f64) -> f64 {
    (1.0 - (valor - centro).abs() / ancho).max(0.0)
}

/// Cuánto se abre la boca en cada cuadro (`hz` por segundo) y con qué vocal: `[aa, ih, ou, ee, oh]` de 0 a 100.
/// La abertura sale del volumen (contra el percentil 95 de la frase); la vocal, de la primera formante
/// (250-1000 Hz) y del centroide de la segunda (900-3500 Hz).
pub fn visemas(x: &[f32], sr: u32, hz: u32) -> Vec<[u8; 5]> {
    const APERTURA: [f64; 5] = [1.0, 0.55, 0.6, 0.7, 0.8]; // aa, ih, ou, ee, oh
    let paso = ((sr / hz) as usize).max(1);
    let n = x.len() / paso;
    if n == 0 {
        return vec![];
    }
    let m = 2 * paso;
    let ventana: Vec<f64> = (0..m)
        .map(|k| 0.5 - 0.5 * (2.0 * PI * k as f64 / (m - 1) as f64).cos())
        .collect();
    let freq = |k: usize| k as f64 * sr as f64 / m as f64;
    let bins: Vec<usize> = (0..=paso)
        .filter(|&k| (250.0..=3500.0).contains(&freq(k)))
        .collect();
    // Tabla de cos/sen de 2πj/m: la DFT de cada banda usa (k·j) mód m.
    let (cos, sin): (Vec<f64>, Vec<f64>) = (0..m)
        .map(|j| {
            let a = 2.0 * PI * j as f64 / m as f64;
            (a.cos(), a.sin())
        })
        .unzip();
    let mut relleno = vec![0f64; paso / 2];
    relleno.extend(x.iter().map(|&v| v as f64));
    relleno.extend(std::iter::repeat_n(0.0, m));
    let rms: Vec<f64> = (0..n)
        .map(|i| {
            let b = &x[i * paso..(i + 1) * paso];
            (b.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / paso as f64).sqrt()
        })
        .collect();
    let mut referencia = percentil(&rms, 95.0);
    if referencia == 0.0 {
        referencia = 1.0;
    }
    let mut salida = Vec::with_capacity(n);
    let mut cuadro = vec![0f64; m];
    for (i, &r) in rms.iter().enumerate() {
        let v = if r > 0.02 * referencia {
            (r / referencia).powf(0.8).min(1.0)
        } else {
            0.0
        };
        if v <= 0.02 {
            salida.push([0; 5]);
            continue;
        }
        for (j, c) in cuadro.iter_mut().enumerate() {
            *c = relleno[i * paso + j] * ventana[j];
        }
        let (mut mejor1, mut f1, mut suma1) = (f64::MIN, 500.0, 0.0);
        let (mut suma2, mut momento2) = (0.0, 0.0);
        for &k in &bins {
            let (mut a, mut b) = (0.0, 0.0);
            let mut idx = 0;
            for &c in &cuadro {
                a += c * cos[idx];
                b -= c * sin[idx];
                idx += k;
                if idx >= m {
                    idx -= m;
                }
            }
            let e = a * a + b * b;
            let f = freq(k);
            if (250.0..=1000.0).contains(&f) {
                suma1 += e;
                if e > mejor1 {
                    (mejor1, f1) = (e, f);
                }
            }
            if (900.0..=3500.0).contains(&f) {
                suma2 += e;
                momento2 += f * e;
            }
        }
        if suma1 <= 0.0 {
            f1 = 500.0;
        }
        let centroide = if suma2 > 0.0 { momento2 / suma2 } else { 1500.0 };
        let cerrada = pertenencia(f1, 330.0, 220.0);
        let media = pertenencia(f1, 520.0, 200.0);
        let abierta = ((f1 - 560.0) / 220.0).clamp(0.0, 1.0);
        let delante = ((centroide - 1500.0) / 800.0).clamp(0.0, 1.0);
        let atras = ((1900.0 - centroide) / 900.0).clamp(0.0, 1.0);
        let pesos = [
            abierta + 0.15,
            cerrada * delante,
            cerrada * atras,
            media * delante,
            media * atras,
        ];
        let total = match pesos.iter().sum::<f64>() {
            0.0 => 1.0,
            t => t,
        };
        let mut fila = [0u8; 5];
        for (k, p) in pesos.iter().enumerate() {
            fila[k] = (100.0 * v * APERTURA[k] * p / total).round_ties_even() as u8;
        }
        salida.push(fila);
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn seno(hz: f64, sr: u32, seg: f64, amp: f32) -> Vec<f32> {
        (0..(sr as f64 * seg) as usize)
            .map(|i| amp * (2.0 * PI * hz * i as f64 / sr as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn tono_grave_alarga() {
        let x = seno(200.0, 8000, 1.0, 0.5);
        assert_eq!(cambiar_tono(&x, 0.0).len(), x.len());
        let y = cambiar_tono(&x, -1200.0);
        assert_eq!(y.len(), 2 * x.len());
        assert!((y[2] - x[1]).abs() < 1e-6);
    }

    #[test]
    fn paso_bajo_quita_agudos() {
        let sr = 16000;
        let grave = seno(300.0, sr, 0.5, 0.3);
        let agudo = seno(6000.0, sr, 0.5, 0.3);
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        assert!((rms(&filtrar_agudos(&grave, sr, 2000.0, 0.0)) - rms(&grave)).abs() < 0.01);
        assert!(rms(&filtrar_agudos(&agudo, sr, 2000.0, 0.0)) < 0.01);
    }

    #[test]
    fn nivelar_deja_el_volumen() {
        let x = seno(220.0, 16000, 1.0, 0.9);
        let y = nivelar(&x, 16000, -20.0, -3.0, 2.2);
        let rms = (y.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / y.len() as f64).sqrt();
        assert!((20.0 * rms.log10() + 20.0).abs() < 0.6, "{}", 20.0 * rms.log10());
        assert!(y.iter().all(|v| v.abs() <= 10f32.powf(-3.0 / 20.0) + 1e-4));
    }

    #[test]
    fn wav_valido() {
        let w = a_wav(&[0.0, 1.0, -1.0, 2.0], 44100);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 8);
        assert_eq!(i16::from_le_bytes([w[46], w[47]]), 32767);
        assert_eq!(i16::from_le_bytes([w[48], w[49]]), -32767);
        assert_eq!(i16::from_le_bytes([w[50], w[51]]), 32767);
    }

    #[test]
    fn visemas_boca_abierta_con_voz() {
        let sr = 16000;
        let mut x = vec![0f32; sr as usize / 2];
        x.extend(seno(700.0, sr, 0.5, 0.4));
        let v = visemas(&x, sr, 30);
        assert_eq!(v.len(), x.len() / (sr as usize / 30));
        assert_eq!(v[2], [0; 5], "silencio: boca cerrada");
        let ultimo = v[v.len() - 3];
        assert!(ultimo[0] > 50, "700 Hz es una «a»: {ultimo:?}");
    }
}
