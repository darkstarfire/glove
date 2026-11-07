//! Informações do Sistema (como o Perfil do Sistema da era): as seções à
//! esquerda e os detalhes de cada uma à direita, lidos de /proc, /sys e de
//! alguns comandos do sistema.

use std::process::Command;

use glove_kit::{paint::draw::*, run, App, Menu, MenuItem, Rect, Ui, WindowOptions};

const SECTIONS: [&str; 8] = ["Hardware", "Memória", "Gráficos", "Armazenamento", "Rede", "USB", "Áudio", "Software"];

fn read(p: &str) -> String {
    std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
}

fn cmd(c: &str, args: &[&str]) -> String {
    Command::new(c).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

fn gb(kb: u64) -> String {
    format!("{:.1} GB", kb as f64 / 1024.0 / 1024.0).replace('.', ",")
}

/// (rótulo, valor) de cada seção; linhas com rótulo vazio são títulos.
pub fn section(i: usize) -> Vec<(String, String)> {
    let kv = |k: &str, v: String| (k.to_string(), if v.is_empty() { "—".into() } else { v });
    match i {
        0 => {
            let cpu = read("/proc/cpuinfo");
            let model = cpu.lines().find_map(|l| l.strip_prefix("model name")).map(|v| v.trim_start_matches([' ', '\t', ':']).to_string()).unwrap_or_default();
            let threads = cpu.lines().filter(|l| l.starts_with("processor")).count();
            let cores = cpu.lines().find_map(|l| l.strip_prefix("cpu cores")).map(|v| v.trim_start_matches([' ', '\t', ':']).to_string()).unwrap_or_default();
            let mhz = read("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq").parse::<f64>().map(|k| format!("{:.2} GHz", k / 1e6).replace('.', ",")).unwrap_or_default();
            let model_name = format!("{} {}", read("/sys/class/dmi/id/sys_vendor"), read("/sys/class/dmi/id/product_name")).trim().to_string();
            vec![
                kv("Modelo", model_name),
                kv("Processador", model),
                kv("Velocidade máxima", mhz),
                kv("Núcleos", cores),
                kv("Processadores lógicos", threads.to_string()),
                kv("Firmware", format!("{} {}", read("/sys/class/dmi/id/bios_vendor"), read("/sys/class/dmi/id/bios_version")).trim().to_string()),
                kv("Número de série", read("/sys/class/dmi/id/product_serial")),
            ]
        }
        1 => {
            let m = read("/proc/meminfo");
            let get = |k: &str| m.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            vec![
                kv("Total", gb(get("MemTotal:"))),
                kv("Disponível", gb(get("MemAvailable:"))),
                kv("Em cache", gb(get("Cached:"))),
                kv("Swap", format!("{} ({} livres)", gb(get("SwapTotal:")), gb(get("SwapFree:")))),
            ]
        }
        2 => {
            let mut v: Vec<(String, String)> = cmd("lspci", &[]).lines().filter(|l| l.contains("VGA") || l.contains("3D controller") || l.contains("Display controller")).map(|l| kv("Placa", l.split_once(": ").map(|x| x.1.to_string()).unwrap_or_default())).collect();
            for e in std::fs::read_dir("/sys/class/drm").into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.contains('-') && read(&format!("/sys/class/drm/{name}/status")) == "connected" {
                    let modes = read(&format!("/sys/class/drm/{name}/modes"));
                    v.push(kv("Monitor", format!("{} ({})", name.split_once('-').map(|x| x.1).unwrap_or(&name), modes.lines().next().unwrap_or(""))));
                }
            }
            v
        }
        3 => cmd("lsblk", &["-d", "-n", "-o", "NAME,SIZE,MODEL,TRAN"])
            .lines()
            .filter(|l| !l.starts_with("loop") && !l.starts_with("zram") && !l.contains("boot"))
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                kv(f.first().unwrap_or(&""), f[1..].join(" "))
            })
            .collect(),
        4 => {
            let mut v = Vec::new();
            for e in std::fs::read_dir("/sys/class/net").into_iter().flatten().flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n == "lo" {
                    continue;
                }
                let mac = read(&format!("/sys/class/net/{n}/address"));
                let state = read(&format!("/sys/class/net/{n}/operstate"));
                let ip = cmd("ip", &["-4", "-o", "addr", "show", &n]).split_whitespace().skip_while(|w| *w != "inet").nth(1).unwrap_or("").to_string();
                v.push((String::new(), n.clone()));
                v.push(kv("Estado", if state == "up" { "conectado".into() } else { state }));
                v.push(kv("Endereço IP", ip));
                v.push(kv("Endereço MAC", mac));
            }
            v
        }
        5 => cmd("lsusb", &[])
            .lines()
            .map(|l| {
                let name = l.splitn(7, ' ').nth(6).unwrap_or(l).to_string();
                kv("Aparelho", name)
            })
            .collect(),
        6 => {
            let mut v: Vec<(String, String)> = read("/proc/asound/cards").lines().filter(|l| l.contains('[')).map(|l| kv("Placa de som", l.split_once(" - ").map(|x| x.1.trim().to_string()).unwrap_or_default())).collect();
            let s = cmd("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"]);
            if !s.is_empty() {
                v.push(kv("Volume", s.trim().replace("Volume: ", "").replace("[MUTED]", "(sem som)")));
            }
            v
        }
        _ => {
            let os = read("/etc/os-release");
            let pretty = os.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string()).unwrap_or_default();
            let up = read("/proc/uptime").split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as u64;
            vec![
                kv("Sistema", pretty),
                kv("Kernel", cmd("uname", &["-r"]).trim().to_string()),
                kv("Nome do computador", read("/proc/sys/kernel/hostname")),
                kv("Usuário", std::env::var("USER").unwrap_or_default()),
                kv("Ligado há", format!("{} h {} min", up / 3600, up / 60 % 60)),
                kv("Glove", env!("CARGO_PKG_VERSION").to_string()),
            ]
        }
    }
}

struct Info {
    sel: Option<usize>,
    cache: Vec<Option<Vec<(String, String)>>>,
}

impl App for Info {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        if ui.keys().iter().any(|k| k.combo == "Ctrl+W") || ui.menu_actions().iter().any(|a| a == "close") {
            ui.close = true;
        }
        if ui.keys().iter().any(|k| k.combo == "Ctrl+R") || ui.menu_actions().iter().any(|a| a == "refresh") {
            self.cache = vec![None; SECTIONS.len()];
        }
        let names: Vec<String> = SECTIONS.iter().map(|s| s.to_string()).collect();
        ui.area(Rect::new(10.0, 10.0, 170.0, h - 20.0), |ui| {
            ui.list(&mut self.sel, &names, 170.0, h - 20.0);
        });
        let i = self.sel.unwrap_or(0);
        let rows = self.cache[i].get_or_insert_with(|| section(i)).clone();
        let r = Rect::new(190.0, 10.0, w - 200.0, h - 20.0);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(255, 255, 255, 255)));
        ui.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
        ui.fonts.draw(ui.c, SECTIONS[i], r.x + 16.0, r.y + 26.0, 15.0, true, rgba(20, 20, 24, 255));
        let mut y = r.y + 52.0;
        let lw = 170.0;
        if rows.is_empty() {
            ui.fonts.draw(ui.c, "Nada encontrado.", r.x + 16.0, y, 12.0, false, rgba(130, 130, 136, 255));
        }
        for (k, v) in rows {
            if y > r.y + r.h - 8.0 {
                break;
            }
            if k.is_empty() {
                y += 6.0;
                ui.fonts.draw(ui.c, &v, r.x + 16.0, y, 12.0, true, rgba(40, 40, 46, 255));
                y += 20.0;
                continue;
            }
            let t = format!("{k}:");
            let tw = ui.text_width(&t, 11.0, false);
            ui.fonts.draw(ui.c, &t, r.x + lw - tw, y, 11.0, false, rgba(110, 110, 116, 255));
            let v = ui.fonts.ellipsize(&v, 12.0, false, r.w - lw - 30.0);
            ui.fonts.draw(ui.c, &v, r.x + lw + 10.0, y, 12.0, false, rgba(20, 20, 24, 255));
            y += 19.0;
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new("Arquivo", vec![MenuItem::new("refresh", "Atualizar", "Ctrl+R"), MenuItem::Separator, MenuItem::new("close", "Fechar", "Ctrl+W")])]
    }
}

pub fn main() {
    let opts = WindowOptions { title: "Informações do Sistema".into(), app_id: "glove.Info".into(), size: (680, 440), fixed: false, min_size: None };
    if let Err(e) = run(opts, Info { sel: Some(0), cache: vec![None; SECTIONS.len()] }) {
        eprintln!("glove-info: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn secoes_respondem() {
        // Toda seção devolve algo (ou nada) sem travar nem entrar em pânico.
        for i in 0..super::SECTIONS.len() {
            let _ = super::section(i);
        }
        assert!(!super::section(7).is_empty());
    }
}
