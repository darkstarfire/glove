//! Dispositivos de som pelo `wpctl` (PipeWire): saídas, entradas, volume e mudo.

use std::process::Command;

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub default: bool,
    pub volume: f32,
    pub muted: bool,
}

/// Saídas e entradas de áudio do `wpctl status` (só a parte "Audio").
pub fn parse_status(text: &str) -> (Vec<Device>, Vec<Device>) {
    let (mut sinks, mut sources) = (Vec::new(), Vec::new());
    let mut in_audio = false;
    let mut section = "";
    for line in text.lines() {
        if !line.starts_with(' ') && !line.starts_with('│') && !line.starts_with('├') && !line.starts_with('└') {
            in_audio = line.trim() == "Audio";
            section = "";
            continue;
        }
        if !in_audio {
            continue;
        }
        let clean = line.trim_start_matches(|c: char| c == ' ' || c == '│' || c == '├' || c == '└' || c == '─');
        if clean.ends_with(':') {
            section = match clean.trim_end_matches(':') {
                "Sinks" => "sinks",
                "Sources" => "sources",
                _ => "",
            };
            continue;
        }
        if section.is_empty() {
            continue;
        }
        let (default, rest) = match clean.strip_prefix('*') {
            Some(r) => (true, r.trim_start()),
            None => (false, clean),
        };
        let Some((id, rest)) = rest.split_once('.') else { continue };
        let Ok(id) = id.trim().parse::<u32>() else { continue };
        let (name, vol) = match rest.split_once("[vol:") {
            Some((n, v)) => (n.trim().to_string(), v.trim_end_matches(']').trim().to_string()),
            None => (rest.trim().to_string(), String::new()),
        };
        let volume = vol.split_whitespace().next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
        let d = Device { id, name, default, volume, muted: vol.contains("MUTED") };
        if section == "sinks" {
            sinks.push(d);
        } else {
            sources.push(d);
        }
    }
    // Nomes iguais (comum em placas internas) ganham um número.
    for list in [&mut sinks, &mut sources] {
        let names: Vec<String> = list.iter().map(|d| d.name.clone()).collect();
        let mut seen = std::collections::HashMap::<String, u32>::new();
        for d in list.iter_mut() {
            if names.iter().filter(|n| **n == d.name).count() > 1 {
                let k = seen.entry(d.name.clone()).or_insert(0);
                *k += 1;
                d.name = format!("{} ({})", d.name, k);
            }
        }
    }
    (sinks, sources)
}

pub fn status() -> Option<(Vec<Device>, Vec<Device>)> {
    let out = Command::new("wpctl").arg("status").output().ok()?;
    out.status.success().then(|| parse_status(&String::from_utf8_lossy(&out.stdout)))
}

/// `status` numa thread (o `wpctl status` demora e travaria a janela).
pub fn status_async() -> std::sync::mpsc::Receiver<Option<(Vec<Device>, Vec<Device>)>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(status());
    });
    rx
}

/// Pedidos ao `wpctl`, feitos um de cada vez numa thread. Enquanto um roda, os
/// volumes pedidos se acumulam e só o último vale: arrastar o slider não abre
/// dezenas de processos nem fica para trás.
fn worker() -> &'static std::sync::mpsc::Sender<Vec<String>> {
    static TX: std::sync::OnceLock<std::sync::mpsc::Sender<Vec<String>>> = std::sync::OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = vec![first];
                batch.extend(rx.try_iter());
                // Volumes: só o último de cada alvo.
                let mut run: Vec<Vec<String>> = Vec::new();
                for args in batch {
                    if args.first().is_some_and(|a| a == "set-volume") {
                        run.retain(|r| !(r.first() == args.first() && r.get(3) == args.get(3)));
                    }
                    run.push(args);
                }
                for args in run {
                    let _ = Command::new("wpctl").args(&args).status();
                }
            }
        });
        tx
    })
}

fn wpctl(args: &[&str]) {
    let _ = worker().send(args.iter().map(|a| a.to_string()).collect());
}

pub fn set_default(id: u32) {
    wpctl(&["set-default", &id.to_string()]);
}

/// `target`: "@DEFAULT_AUDIO_SINK@" ou "@DEFAULT_AUDIO_SOURCE@".
pub fn set_volume(target: &str, v: f32) {
    wpctl(&["set-volume", "-l", "1.0", target, &format!("{:.3}", v.clamp(0.0, 1.0))]);
}

pub fn set_mute(target: &str, muted: bool) {
    wpctl(&["set-mute", target, if muted { "1" } else { "0" }]);
}

#[cfg(test)]
mod tests {
    const STATUS: &str = "PipeWire 'pipewire-0' [1.6.2]\n └─ Clients:\n        33. WirePlumber   [1.6.2]\n\nAudio\n ├─ Devices:\n │      48. Built-in Audio                      [alsa]\n │  \n ├─ Sinks:\n │  *   38. Built-in Audio Stereo               [vol: 0.59 MUTED]\n │      64. Built-in Audio Stereo               [vol: 0.92]\n │  \n ├─ Sources:\n │  *   62. Microfone                           [vol: 1.00]\n │  \n └─ Streams:\n\nVideo\n ├─ Sinks:\n │  *   73. Câmera   [vol: 1.00]\n";

    #[test]
    fn le_o_status() {
        let (sinks, sources) = super::parse_status(STATUS);
        assert_eq!(sinks.len(), 2);
        assert_eq!(sinks[0].id, 38);
        assert!(sinks[0].default && sinks[0].muted);
        assert!((sinks[0].volume - 0.59).abs() < 1e-6);
        assert_eq!(sinks[0].name, "Built-in Audio Stereo (1)");
        assert_eq!(sinks[1].name, "Built-in Audio Stereo (2)");
        assert!(!sinks[1].muted);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "Microfone");
    }
}
