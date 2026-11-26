//! Sound devices: the outputs and inputs by port come from `glove_config::audio`
//! (PipeWire); volume and mute go through `wpctl`.

use std::process::Command;

pub use glove_config::audio::{Direction, Port, Snapshot};

/// `audio::snapshot` in a thread (`pw-dump` takes a while and would freeze the window).
pub fn status_async() -> std::sync::mpsc::Receiver<Option<Snapshot>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(glove_config::audio::snapshot());
    });
    rx
}

/// Makes `port` the default in a thread (it may switch the card profile and wait for
/// the new sink); the receiver gets whether it worked.
pub fn select_async(port: Port, dir: Direction) -> std::sync::mpsc::Receiver<bool> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(glove_config::audio::select(&port, dir));
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

/// `target`: "@DEFAULT_AUDIO_SINK@" ou "@DEFAULT_AUDIO_SOURCE@".
pub fn set_volume(target: &str, v: f32) {
    wpctl(&["set-volume", "-l", "1.0", target, &format!("{:.3}", v.clamp(0.0, 1.0))]);
}

pub fn set_mute(target: &str, muted: bool) {
    wpctl(&["set-mute", target, if muted { "1" } else { "0" }]);
}
