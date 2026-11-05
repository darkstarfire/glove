//! Sons do Glove, sintetizados aqui mesmo (nada copiado): os alertas que o
//! painel Som deixa escolher e o "pop" do volume. Cada som vira um WAV em
//! ~/.cache/glove/sons na primeira vez e é tocado pelo `pw-play` (ou `aplay`).

use std::f32::consts::TAU;
use std::path::PathBuf;

const RATE: u32 = 22_050;

/// Alertas que dá para escolher, na ordem do painel.
pub const ALERTS: [&str; 6] = ["Gota", "Sino", "Vidro", "Bolha", "Toque", "Sopro"];

/// Amostras (-1 a 1) de um som pelo nome.
fn synth(name: &str) -> Vec<f32> {
    let n = |secs: f32| (secs * RATE as f32) as usize;
    let t = |i: usize| i as f32 / RATE as f32;
    match name {
        // Gota: seno que desce depressa, com um estalo curto no começo.
        "Gota" => (0..n(0.16))
            .map(|i| {
                let x = t(i);
                let f = 1300.0 * (-x * 9.0).exp() + 500.0;
                (TAU * f * x).sin() * (-x * 26.0).exp()
            })
            .collect(),
        // Sino: fundamental e parciais inarmônicos que somem devagar.
        "Sino" => (0..n(0.9))
            .map(|i| {
                let x = t(i);
                let e = (-x * 4.5).exp();
                ((TAU * 880.0 * x).sin() + 0.5 * (TAU * 2210.0 * x).sin() * (-x * 3.0).exp() + 0.25 * (TAU * 3520.0 * x).sin()) * e * 0.55
            })
            .collect(),
        // Vidro: dois agudos curtinhos, como um toque numa taça.
        "Vidro" => (0..n(0.45))
            .map(|i| {
                let x = t(i);
                ((TAU * 1975.0 * x).sin() * 0.6 + (TAU * 2960.0 * x).sin() * 0.4) * (-x * 11.0).exp() * (1.0 - (-x * 400.0).exp())
            })
            .collect(),
        // Bolha: sobe rápido, como uma bolha estourando.
        "Bolha" => (0..n(0.14))
            .map(|i| {
                let x = t(i);
                let phase = TAU * (320.0 * x + 2600.0 * x * x);
                phase.sin() * (x / 0.14 * std::f32::consts::PI).sin()
            })
            .collect(),
        // Toque: duas batidas graves e redondas.
        "Toque" => (0..n(0.26))
            .map(|i| {
                let x = t(i);
                let k = if x < 0.11 { x } else { x - 0.13 };
                if k < 0.0 {
                    0.0
                } else {
                    (TAU * 520.0 * k).sin() * (-k * 40.0).exp()
                }
            })
            .collect(),
        // Sopro: ruído filtrado que cresce e some.
        "Sopro" => {
            let mut seed = 0x1234_5678u32;
            let mut low = 0.0f32;
            (0..n(0.35))
                .map(|i| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                    low += (white - low) * 0.18;
                    let x = t(i) / 0.35;
                    low * (x * std::f32::consts::PI).sin() * 2.2
                })
                .collect()
        }
        // Pop do volume: um clique curto e macio.
        _ => (0..n(0.07))
            .map(|i| {
                let x = t(i);
                (TAU * 760.0 * x).sin() * (-x * 60.0).exp()
            })
            .collect(),
    }
}

/// WAV PCM de 16 bits, mono.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 0.8 * i16::MAX as f32) as i16).to_le_bytes());
    }
    v
}

fn cache_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dir = base.join("glove/sons");
    let file = dir.join(format!("{name}.wav"));
    if !file.is_file() {
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&file, wav(&synth(name))).ok()?;
    }
    Some(file)
}

/// Toca um som (volume de 0 a 1) sem esperar. Sem PipeWire nem ALSA, não faz nada.
pub fn play(name: &str, volume: f32) {
    if volume <= 0.0 {
        return;
    }
    let Some(file) = cache_file(name) else { return };
    let spawned = std::process::Command::new("pw-play")
        .arg(format!("--volume={:.2}", volume.clamp(0.0, 1.0)))
        .arg(&file)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .or_else(|_| std::process::Command::new("aplay").arg("-q").arg(&file).stderr(std::process::Stdio::null()).spawn());
    // Recolhe o processo quando terminar (sem zumbis).
    if let Ok(mut child) = spawned {
        std::thread::spawn(move || child.wait());
    }
}

/// O alerta escolhido no painel Som, no volume escolhido.
pub fn alert() {
    let c = crate::Config::load();
    if c.sound.alert_volume > 0.0 {
        play(&c.sound.alert, c.sound.alert_volume);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sons_validos() {
        for name in super::ALERTS.iter().chain(["Volume"].iter()) {
            let s = super::synth(name);
            assert!(s.len() > 1000, "{name} curto demais");
            assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 2.5), "{name} fora da faixa");
            let w = super::wav(&s);
            assert_eq!(&w[..4], b"RIFF");
            assert_eq!(w.len(), 44 + s.len() * 2);
        }
    }
}
