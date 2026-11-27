//! "Sobre Este Computador", como nos Macs de hoje: a imagem do aparelho (notebook,
//! computador de mesa, tudo-em-um ou mini PC, pelo que o sistema diz do gabinete), o nome
//! do modelo, o processador, os gráficos, a memória e o sistema. "Mais Informações…" abre
//! o Monitor.

use glove_config::tr;
use glove_ui::{paint::draw::*, run, ui::Key, App, Rect, Ui, WindowOptions};

const W: f32 = 340.0;
const H: f32 = 470.0;

/// O tipo de aparelho (decide a imagem).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Notebook,
    Desktop,
    AllInOne,
    Mini,
}

impl Kind {
    /// Pelo tipo de gabinete do SMBIOS (/sys/class/dmi/id/chassis_type); sem ele, um
    /// aparelho com bateria é notebook.
    fn detect() -> Self {
        let chassis = std::fs::read_to_string("/sys/class/dmi/id/chassis_type").ok().and_then(|s| s.trim().parse::<u32>().ok());
        match chassis {
            Some(c) => Self::from_chassis(c),
            None if has_battery() => Self::Notebook,
            None => Self::Desktop,
        }
    }

    fn from_chassis(c: u32) -> Self {
        match c {
            // Portátil, notebook, laptop, subnotebook, conversível, destacável, tablet.
            8 | 9 | 10 | 14 | 30 | 31 | 32 => Self::Notebook,
            13 => Self::AllInOne,
            // Mini PC e computadores de mão / embarcados.
            35 | 36 | 33 | 34 => Self::Mini,
            _ => Self::Desktop,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Notebook => tr!("Notebook"),
            Self::Desktop => tr!("Computador de mesa"),
            Self::AllInOne => tr!("Computador tudo-em-um"),
            Self::Mini => tr!("Mini PC"),
        }
    }

    fn image(self) -> &'static [u8] {
        match self {
            Self::Notebook => include_bytes!("../assets/notebook.png"),
            Self::Desktop => include_bytes!("../assets/desktop.png"),
            Self::AllInOne => include_bytes!("../assets/allinone.png"),
            Self::Mini => include_bytes!("../assets/mini.png"),
        }
    }
}

fn has_battery() -> bool {
    std::fs::read_dir("/sys/class/power_supply").map(|d| d.flatten().any(|e| std::fs::read_to_string(e.path().join("type")).is_ok_and(|t| t.trim() == "Battery"))).unwrap_or(false)
}

fn dmi(field: &str) -> Option<String> {
    let v = std::fs::read_to_string(format!("/sys/class/dmi/id/{field}")).ok()?;
    let v = v.trim();
    // Valores de fábrica que não dizem nada.
    let vazio = ["", "To Be Filled By O.E.M.", "To be filled by O.E.M.", "Default string", "System Product Name", "System manufacturer", "Not Applicable", "None", "Type1ProductConfigId", "O.E.M."];
    (!vazio.contains(&v)).then(|| v.to_string())
}

/// O nome do modelo ("Aspire 5", "ThinkPad T14"): a família, ou o produto.
fn model_name() -> Option<String> {
    let vendor = dmi("sys_vendor");
    let name = dmi("product_family").filter(|f| f.chars().any(|c| c.is_ascii_digit()) || f.contains(' ')).or_else(|| dmi("product_name"))?;
    // Máquinas virtuais: o nome do programa já diz.
    if let Some(v) = &vendor {
        if ["QEMU", "innotek GmbH", "VMware, Inc.", "Xen", "Microsoft Corporation", "Parallels"].iter().any(|x| v.starts_with(x)) && !name.contains(' ') {
            return Some(name);
        }
    }
    Some(name)
}

/// O fabricante sem "Inc.", "Corporation" etc.
fn vendor_name() -> Option<String> {
    let v = dmi("sys_vendor")?;
    let v = v.replace(" Inc.", "").replace(", Inc.", "").replace(" Corporation", "").replace(" Co., Ltd.", "").replace(" Computer", "").replace(" COMPUTER", "");
    Some(match v.as_str() {
        "LENOVO" => "Lenovo".into(),
        "ASUSTeK" | "ASUSTeK INC." => "ASUS".into(),
        "Hewlett-Packard" | "HP" => "HP".into(),
        "Dell" => "Dell".into(),
        _ => v.trim_end_matches(',').to_string(),
    })
}

/// As placas de vídeo pelo banco de dados do PCI ("Intel UHD Graphics").
fn graphics() -> String {
    let ids = std::fs::read_to_string("/usr/share/misc/pci.ids").or_else(|_| std::fs::read_to_string("/usr/share/hwdata/pci.ids")).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else { return "—".into() };
    let mut cards: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("card") && !n.to_string_lossy().contains('-'))).collect();
    cards.sort();
    for c in cards {
        let read = |f: &str| std::fs::read_to_string(c.join("device").join(f)).ok().map(|s| s.trim().trim_start_matches("0x").to_lowercase());
        let (Some(v), Some(d)) = (read("vendor"), read("device")) else { continue };
        let name = gpu_name(&ids, &v, &d);
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    if out.is_empty() { "—".into() } else { out.join(", ") }
}

/// O nome da placa no pci.ids: o fabricante curto e o modelo (o que está entre
/// colchetes, se houver: "Alder Lake-P GT1 [UHD Graphics]" → "UHD Graphics").
fn gpu_name(ids: &str, vendor: &str, device: &str) -> String {
    let mut vname = String::new();
    let mut dname = String::new();
    let mut in_vendor = false;
    for l in ids.lines() {
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if !l.starts_with('\t') {
            if in_vendor {
                break;
            }
            if l.len() > 6 && l[..4].eq_ignore_ascii_case(vendor) {
                in_vendor = true;
                vname = l[4..].trim().to_string();
            }
        } else if in_vendor && !l.starts_with("\t\t") {
            let t = l.trim_start();
            if t.len() > 6 && t[..4].eq_ignore_ascii_case(device) {
                dname = t[4..].trim().to_string();
                break;
            }
        }
    }
    let short = match vendor {
        "8086" => "Intel".to_string(),
        "1002" | "1022" => "AMD".to_string(),
        "10de" => "NVIDIA".to_string(),
        "1af4" | "1234" => "Virtual".to_string(),
        _ => vname.split([' ', ',']).next().unwrap_or("").to_string(),
    };
    let model = match (dname.find('['), dname.rfind(']')) {
        (Some(a), Some(b)) if b > a => dname[a + 1..b].to_string(),
        _ => dname,
    };
    let model = model.split(" / ").next().unwrap_or("").to_string();
    if model.is_empty() {
        short
    } else if model.starts_with(&short) {
        model
    } else {
        format!("{short} {model}")
    }
}

/// O sistema: "Glove OS 0.15.15" (ou o nome da distro e o Glove, em outra distro).
fn system_name() -> String {
    let version = env!("CARGO_PKG_VERSION");
    if glove_os() {
        return format!("Glove OS {version}");
    }
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let pretty = text.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string());
    match pretty {
        Some(p) => format!("{p} · Glove {version}"),
        None => format!("Glove {version}"),
    }
}

struct About {
    kind: Kind,
    image: Option<glove_ui::paint::Pixmap>,
    title: String,
    subtitle: String,
    rows: Vec<(&'static str, String)>,
}

/// O sistema é o Glove OS (`NAME="Glove OS"` no os-release)?
fn glove_os() -> bool {
    let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    text.lines().any(|l| l.trim() == "NAME=\"Glove OS\"" || l.trim() == "NAME=Glove OS")
}

impl App for About {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let dark = glove_ui::paint::theme::dark();
        let (ink, dim) = if dark { (rgba(240, 240, 242, 255), rgba(160, 160, 166, 255)) } else { (rgba(20, 20, 22, 255), rgba(110, 110, 116, 255)) };
        // A imagem do aparelho, no alto.
        if let Some(img) = &self.image {
            let (bw, bh) = (w - 40.0, 190.0);
            let k = (bw / img.width() as f32).min(bh / img.height() as f32);
            let (dw, dh) = (img.width() as f32 * k, img.height() as f32 * k);
            let (x, y) = ((w - dw) / 2.0, 20.0 + (bh - dh) / 2.0);
            let s = ui.c.scale;
            let paint = glove_ui::paint::tiny_skia::PixmapPaint { quality: glove_ui::paint::tiny_skia::FilterQuality::Bicubic, ..Default::default() };
            ui.c.draw_pixmap_t(img.as_ref(), &paint, glove_ui::paint::tiny_skia::Transform::from_row(k * s, 0.0, 0.0, k * s, (x * s).round(), (y * s).round()));
        }
        let center = |ui: &mut Ui, t: &str, y: f32, px: f32, bold: bool, c| {
            let mut px = px;
            while px > 9.0 && ui.text_width(t, px, bold) > w - 28.0 {
                px -= 0.5;
            }
            let tw = ui.text_width(t, px, bold);
            ui.fonts.draw(ui.c, t, ((w - tw) / 2.0).round(), y, px, bold, c);
        };
        center(ui, &self.title.clone(), 244.0, 22.0, true, ink);
        center(ui, &self.subtitle.clone(), 264.0, 12.0, false, dim);
        // As linhas: o rótulo à direita do meio, o valor à esquerda dele.
        let mid = (w * 0.36).round();
        let mut y = 300.0;
        for (label, value) in self.rows.clone() {
            let lw = ui.text_width(label, 12.0, true);
            ui.fonts.draw(ui.c, label, mid - 6.0 - lw, y, 12.0, true, ink);
            let max = w - mid - 6.0 - 16.0;
            let lines = glove_ui::paint::chrome::wrap_two_lines(ui.fonts, &value, 12.0, false, max);
            for (k, l) in lines.iter().take(2).enumerate() {
                let l = ui.fonts.ellipsize(l, 12.0, false, max);
                ui.fonts.draw(ui.c, &l, mid + 6.0, y + k as f32 * 15.0, 12.0, false, dim);
            }
            y += 19.0 + 15.0 * (lines.len().min(2) as f32 - 1.0);
        }
        let label = tr!("Mais Informações…");
        let bw = (ui.text_width(label, 13.0, false) + 32.0).ceil();
        ui.area(Rect::new(((w - bw) / 2.0).round(), (y + 8.0).round(), bw, 24.0), |ui| {
            if ui.button(label) {
                let _ = std::process::Command::new(sibling("glove-monitor")).spawn();
            }
        });
        center(ui, tr!("Software livre · GPL-3.0 ou posterior"), H - 14.0, 10.0, false, dim);
        let _ = self.kind;
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
        return tr!("{} MB", mib);
    }
    // O sistema reserva uma parte; a partir de 4 GB mostra o valor inteiro da placa.
    if mib >= 4096 {
        return format!("{} GB", mib.div_ceil(1024));
    }
    let tenths = (mib * 10 + 512) / 1024;
    if tenths % 10 == 0 { format!("{} GB", tenths / 10) } else { format!("{}{}{} GB", tenths / 10, glove_config::virgula(), tenths % 10) }
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
    if tenths % 10 == 0 { format!("{} GHz", tenths / 10) } else { format!("{}{}{} GHz", tenths / 10, glove_config::virgula(), tenths % 10) }
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

/// "Sobre <app>" genérico dos apps do Glove (pedido pelo menu do app):
/// ícone grande, nome, versão e de onde vem.
struct AppAbout {
    icon: glove_ui::paint::icons::Icon,
    name: String,
}

impl App for AppAbout {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        glove_ui::paint::icons::draw(ui.c, ui.fonts, self.icon, (w - 64.0) / 2.0, 22.0, 64.0, None);
        let center = |ui: &mut Ui, t: &str, y: f32, px: f32, bold: bool, c| {
            let tw = ui.text_width(t, px, bold);
            ui.fonts.draw(ui.c, t, ((w - tw) / 2.0).max(8.0), y, px, bold, c);
        };
        center(ui, &self.name.clone(), 116.0, 14.0, true, rgba(20, 20, 20, 255));
        center(ui, concat!("Versão ", env!("CARGO_PKG_VERSION")), 136.0, 11.0, false, rgba(60, 60, 60, 255));
        center(ui, tr!("Parte do Glove"), 166.0, 11.0, false, rgba(60, 60, 60, 255));
        center(ui, tr!("Software livre · GPL-3.0 ou posterior"), 182.0, 10.0, false, rgba(90, 90, 90, 255));
        if ui.keys().iter().any(|k| matches!(k.key, Key::Escape | Key::Enter)) {
            ui.close = true;
        }
    }
}

pub fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--app") {
        let id = args.next().unwrap_or_default();
        let name = args.next().unwrap_or_else(|| "Aplicativo".into());
        // Painéis "Sobre" da época não têm título.
        let opts = WindowOptions { title: String::new(), app_id: "glove.About".into(), size: (260, 200), fixed: true, min_size: None };
        if let Err(e) = run(opts, AppAbout { icon: glove_ui::app_icon(&id), name }) {
            eprintln!("glove-about: {e}");
        }
        return;
    }
    let kind = Kind::detect();
    let model = model_name();
    let vendor = vendor_name();
    let title = model.clone().unwrap_or_else(|| kind.label().to_string());
    let subtitle = match (&vendor, &model) {
        (Some(v), Some(_)) => format!("{} {v}", kind.label()),
        _ => kind.label().to_string(),
    };
    let rows = vec![
        (tr!("Processador"), processor()),
        (tr!("Gráficos"), graphics()),
        (tr!("Memória"), memory_mib().map(format_memory).unwrap_or_else(|| "—".into())),
        (tr!("Sistema"), system_name()),
    ];
    let image = glove_ui::paint::image::decode_bytes(kind.image()).ok();
    let about = About { kind, image, title, subtitle, rows };
    let opts = WindowOptions { title: tr!("Sobre Este Computador").into(), app_id: "glove.About".into(), size: (W as u32, H as u32), fixed: true, min_size: None };
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
        assert_eq!(format_memory(1536), format!("1{}5 GB", glove_config::virgula()));
        assert_eq!(format_memory(480), "512 MB");
    }

    #[test]
    fn aparelho() {
        assert_eq!(Kind::from_chassis(10), Kind::Notebook);
        assert_eq!(Kind::from_chassis(3), Kind::Desktop);
        assert_eq!(Kind::from_chassis(13), Kind::AllInOne);
        assert_eq!(Kind::from_chassis(35), Kind::Mini);
        for k in [Kind::Notebook, Kind::Desktop, Kind::AllInOne, Kind::Mini] {
            assert!(glove_ui::paint::image::decode_bytes(k.image()).is_ok());
        }
    }

    #[test]
    fn placa_de_video() {
        let ids = "8086  Intel Corporation\n\t46a3  Alder Lake-P GT1 [UHD Graphics]\n\t\t1025 1234  sub\n10de  NVIDIA Corporation\n\t25a2  GA107M [GeForce RTX 3050 Mobile]\n1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t1638  Cezanne [Radeon Vega Series / Radeon Vega Mobile Series]\n";
        assert_eq!(gpu_name(ids, "8086", "46a3"), "Intel UHD Graphics");
        assert_eq!(gpu_name(ids, "10de", "25a2"), "NVIDIA GeForce RTX 3050 Mobile");
        assert_eq!(gpu_name(ids, "1002", "1638"), "AMD Radeon Vega Series");
    }

    #[test]
    fn processador() {
        assert_eq!(clean_cpu_name("12th Gen Intel(R) Core(TM) i7-12650H"), "Intel Core i7-12650H");
        assert_eq!(clean_cpu_name("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz"), "Intel Core i5-8250U");
        assert_eq!(clean_cpu_name("AMD Ryzen 7 5800H with Radeon Graphics"), "AMD Ryzen 7 5800H");
        assert_eq!(clean_cpu_name("AMD Ryzen 5 3600 6-Core Processor"), "AMD Ryzen 5 3600");
        assert_eq!(format_freq(4_600_000), format!("4{}6 GHz", glove_config::virgula()));
        assert_eq!(format_freq(2_000_000), "2 GHz");
        assert_eq!(format_freq(700_000), "700 MHz");
    }
}
