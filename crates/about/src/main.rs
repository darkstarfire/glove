//! "Sobre o Glove": logotipo, versão, memória e processador, como o "Sobre este computador"
//! do 10.2/10.3. "Mais informações…" abre o Monitor.

use glove_kit::{paint::draw::*, run, ui::Key, App, Rect, Ui, WindowOptions};

const W: f32 = 300.0;

struct About {
    memory: String,
    processor: String,
}

impl App for About {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let accent = ui.theme.accent;
        gel_circle(ui.c, w / 2.0, 66.0, 44.0, accent);
        let center = |ui: &mut Ui, t: &str, y: f32, px: f32, bold: bool, c| {
            // Encolhe a fonte se o texto não couber (nomes de processador longos).
            let mut px = px;
            while px > 9.0 && ui.text_width(t, px, bold) > w - 24.0 {
                px -= 0.5;
            }
            let tw = ui.text_width(t, px, bold);
            ui.fonts.draw(ui.c, t, (w - tw) / 2.0, y, px, bold, c);
        };
        center(ui, "Glove", 150.0, 30.0, true, rgba(20, 20, 20, 255));
        center(ui, "Versão 0.2 — prévia", 170.0, 11.0, false, rgba(60, 60, 60, 255));
        center(ui, &format!("Memória: {}", self.memory), 202.0, 13.0, false, rgba(20, 20, 20, 255));
        center(ui, &format!("Processador: {}", self.processor), 221.0, 13.0, false, rgba(20, 20, 20, 255));

        let label = "Mais informações…";
        let bw = (ui.text_width(label, 13.0, false) + 32.0).ceil();
        ui.area(Rect::new(((w - bw) / 2.0).round(), 240.0, bw, 24.0), |ui| {
            if ui.button(label) {
                let _ = std::process::Command::new(sibling("glove-monitor")).spawn();
            }
        });
        center(ui, "Software livre · GPL-3.0 ou posterior", 292.0, 10.0, false, rgba(70, 70, 70, 255));
        if ui.keys().iter().any(|k| matches!(k.key, Key::Escape | Key::Enter)) {
            ui.close = true;
        }
    }
}

/// Programa irmão (mesma pasta do executável) ou pelo PATH.
fn sibling(name: &str) -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

/// Memória instalada em MiB: blocos de memória online (perto do valor da placa) ou MemTotal.
fn memory_mib() -> Option<u64> {
    let dir = std::path::Path::new("/sys/devices/system/memory");
    let block = std::fs::read_to_string(dir.join("block_size_bytes")).ok().and_then(|s| u64::from_str_radix(s.trim(), 16).ok());
    if let (Some(block), Ok(entries)) = (block, std::fs::read_dir(dir)) {
        let online = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("memory"))
            .filter(|e| std::fs::read_to_string(e.path().join("online")).map(|s| s.trim() == "1").unwrap_or(false))
            .count() as u64;
        if online > 0 {
            return Some(block * online / (1024 * 1024));
        }
    }
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = info.lines().find(|l| l.starts_with("MemTotal:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// "32 GB", "1,5 GB", "512 MB" (arredonda para cima, como a placa).
fn format_memory(mib: u64) -> String {
    let mib = mib.div_ceil(256) * 256;
    if mib < 1024 {
        return format!("{mib} MB");
    }
    // O sistema reserva uma parte; a partir de 4 GB mostra o valor inteiro da placa.
    if mib >= 4096 {
        return format!("{} GB", mib.div_ceil(1024));
    }
    let tenths = (mib * 10 + 512) / 1024;
    if tenths % 10 == 0 { format!("{} GB", tenths / 10) } else { format!("{},{} GB", tenths / 10, tenths % 10) }
}

/// Nome do processador sem as marcas registradas e sem a frequência (que vem à parte).
fn clean_cpu_name(raw: &str) -> String {
    let mut s = raw.replace("(R)", "").replace("(r)", "").replace("(TM)", "").replace("(tm)", "");
    for cut in [" @ ", " with ", " w/ "] {
        if let Some(i) = s.find(cut) {
            s.truncate(i);
        }
    }
    let words: Vec<&str> = s
        .split_whitespace()
        .filter(|w| !matches!(*w, "CPU" | "Processor" | "Gen" | "Intel®" | "Core™"))
        .filter(|w| !(w.ends_with("th") || w.ends_with("nd") || w.ends_with("rd") || w.ends_with("st")) || !w.starts_with(|c: char| c.is_ascii_digit()))
        .filter(|w| !w.ends_with("-Core"))
        .collect();
    words.join(" ")
}

/// Frequência em GHz com vírgula: "4,6 GHz"; abaixo de 1 GHz em MHz.
fn format_freq(khz: u64) -> String {
    let mhz = khz / 1000;
    if mhz < 1000 {
        return format!("{mhz} MHz");
    }
    let tenths = (mhz + 50) / 100;
    if tenths % 10 == 0 { format!("{} GHz", tenths / 10) } else { format!("{},{} GHz", tenths / 10, tenths % 10) }
}

fn processor() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let field = |key: &str| info.lines().find(|l| l.split(':').next().map(str::trim) == Some(key)).and_then(|l| l.splitn(2, ':').nth(1)).map(|v| v.trim().to_string());
    // x86: "model name"; ARM e outros: "Hardware"/"Model"/"cpu model".
    let name = field("model name").or_else(|| field("Hardware")).or_else(|| field("cpu model")).or_else(|| field("Model")).map(|n| clean_cpu_name(&n)).filter(|n| !n.is_empty());
    let name = name.unwrap_or_else(|| std::env::consts::ARCH.to_string());
    let khz = std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .or_else(|| field("cpu MHz").and_then(|m| m.parse::<f64>().ok()).map(|m| (m * 1000.0) as u64));
    match khz.filter(|k| *k > 0) {
        Some(k) => format!("{} {name}", format_freq(k)),
        None => name,
    }
}

fn main() {
    let about = About { memory: memory_mib().map(format_memory).unwrap_or_else(|| "—".into()), processor: processor() };
    let opts = WindowOptions { title: "Sobre o Glove".into(), app_id: "glove.About".into(), size: (W as u32, 310), fixed: true };
    if let Err(e) = run(opts, about) {
        eprintln!("glove-about: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memoria() {
        assert_eq!(format_memory(32640), "32 GB");
        assert_eq!(format_memory(31805), "32 GB");
        assert_eq!(format_memory(7820), "8 GB");
        assert_eq!(format_memory(1536), "1,5 GB");
        assert_eq!(format_memory(480), "512 MB");
    }

    #[test]
    fn processador() {
        assert_eq!(clean_cpu_name("12th Gen Intel(R) Core(TM) i7-12650H"), "Intel Core i7-12650H");
        assert_eq!(clean_cpu_name("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz"), "Intel Core i5-8250U");
        assert_eq!(clean_cpu_name("AMD Ryzen 7 5800H with Radeon Graphics"), "AMD Ryzen 7 5800H");
        assert_eq!(clean_cpu_name("AMD Ryzen 5 3600 6-Core Processor"), "AMD Ryzen 5 3600");
        assert_eq!(format_freq(4_600_000), "4,6 GHz");
        assert_eq!(format_freq(2_000_000), "2 GHz");
        assert_eq!(format_freq(700_000), "700 MHz");
    }
}
