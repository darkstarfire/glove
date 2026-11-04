//! Discos: discos montados, espaço usado/livre, abrir e ejetar.

use std::path::{Path, PathBuf};
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, icons::{self, Icon}},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

#[derive(Clone)]
struct Disk {
    device: String,
    mount: PathBuf,
    fstype: String,
    label: String,
    total: u64,
    free: u64,
    removable: bool,
}

fn unescape(s: &str) -> String {
    // /proc/mounts escreve espaços como \040.
    s.replace("\\040", " ").replace("\\011", "\t")
}

fn labels() -> Vec<(String, String)> {
    std::fs::read_dir("/dev/disk/by-label")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let target = std::fs::canonicalize(e.path()).ok()?;
            Some((target.to_string_lossy().into_owned(), unescape(&e.file_name().to_string_lossy().replace("\\x20", " "))))
        })
        .collect()
}

fn statvfs(p: &Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).ok()?;
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        (libc::statvfs(c.as_ptr(), &mut st) == 0).then(|| (st.f_blocks as u64 * st.f_frsize as u64, st.f_bavail as u64 * st.f_frsize as u64))
    }
}

fn read_disks() -> Vec<Disk> {
    let labels = labels();
    let mut out: Vec<Disk> = Vec::new();
    for line in std::fs::read_to_string("/proc/mounts").unwrap_or_default().lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 || !f[0].starts_with("/dev/") || f[0].starts_with("/dev/loop") {
            continue;
        }
        let mount = PathBuf::from(unescape(f[1]));
        let ms = mount.to_string_lossy();
        if ms.starts_with("/snap") || ms.starts_with("/boot/efi") && out.iter().any(|d| d.device == f[0]) {
            continue;
        }
        // Mesmo dispositivo montado em vários lugares: fica o primeiro.
        if out.iter().any(|d| d.device == f[0]) {
            continue;
        }
        let Some((total, free)) = statvfs(&mount) else { continue };
        let dev = std::fs::canonicalize(f[0]).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| f[0].to_string());
        let removable = ms.starts_with("/media/") || ms.starts_with("/run/media/");
        let label = labels
            .iter()
            .find(|(d, _)| *d == dev)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| if mount == Path::new("/") { "Sistema".into() } else { mount.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| ms.to_string()) });
        out.push(Disk { device: f[0].to_string(), mount, fstype: f[2].to_string(), label, total, free, removable });
    }
    out.sort_by_key(|d| (d.removable, d.mount != Path::new("/"), d.mount.clone()));
    out
}

fn gb(n: u64) -> String {
    let g = n as f64 / 1e9;
    if g >= 100.0 { format!("{g:.0} GB") } else if g >= 1.0 { format!("{g:.1} GB").replace('.', ",") } else { format!("{:.0} MB", n as f64 / 1e6) }
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

struct Disks {
    disks: Vec<Disk>,
    selected: Option<usize>,
    message: Option<String>,
}

impl Disks {
    fn eject(&mut self) {
        let Some(d) = self.selected.and_then(|i| self.disks.get(i)).cloned() else { return };
        let out = std::process::Command::new("udisksctl").args(["unmount", "-b", &d.device]).output();
        match out {
            Ok(o) if o.status.success() => {
                let _ = std::process::Command::new("udisksctl").args(["power-off", "-b", &d.device]).output();
                self.message = Some(format!("“{}” pode ser removido com segurança.", d.label));
                self.selected = None;
            }
            Ok(o) => self.message = Some(format!("Não foi possível ejetar: {}", String::from_utf8_lossy(&o.stderr).trim())),
            Err(e) => self.message = Some(format!("Não foi possível ejetar (udisksctl): {e}")),
        }
        self.disks = read_disks();
    }
}

impl App for Disks {
    fn ui(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+O" => acts.push("open".into()),
                "Ctrl+E" => acts.push("eject".into()),
                "Ctrl+W" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "open" => {
                    if let Some(d) = self.selected.and_then(|i| self.disks.get(i)) {
                        let _ = std::process::Command::new(sibling("glove-files")).arg(&d.mount).spawn();
                    }
                }
                "eject" => self.eject(),
                "close" => ui.close = true,
                _ => {}
            }
        }

        let row = 74.0;
        let mut y = 12.0;
        for i in 0..self.disks.len() {
            let d = self.disks[i].clone();
            let r = Rect::new(10.0, y, w - 20.0, row - 6.0);
            let resp = ui.hit(r);
            if resp.pressed {
                self.selected = Some(i);
                if resp.double {
                    let _ = std::process::Command::new(sibling("glove-files")).arg(&d.mount).spawn();
                }
            }
            let on = self.selected == Some(i);
            ui.c.fill(rrect(r.x, r.y, r.w, r.h, 8.0), &solid(if on { rgba(0, 0, 0, 30) } else { rgba(255, 255, 255, 90) }));
            if on {
                ui.c.stroke(rrect(r.x, r.y, r.w, r.h, 8.0), &solid(ui.theme.accent), 2.0);
            }
            icons::draw(ui.c, ui.fonts, Icon::Disk, r.x + 8.0, r.y + 6.0, 54.0, None);
            let tx = r.x + 74.0;
            ui.fonts.draw(ui.c, &d.label, tx, r.y + 20.0, 14.0, true, rgba(20, 20, 20, 255));
            let info = format!("{}  ·  {}  ·  {}", d.mount.display(), d.device, d.fstype);
            let info = ui.fonts.ellipsize(&info, 11.0, false, r.w - 90.0);
            ui.fonts.draw(ui.c, &info, tx, r.y + 36.0, 11.0, false, rgba(90, 90, 90, 255));
            let bw = r.w - 90.0 - 170.0;
            let bar = Rect::new(tx, r.y + 44.0, bw.max(80.0), 14.0);
            ui.c.fill(rrect(bar.x, bar.y, bar.w, bar.h, 7.0), &vgrad(bar.y, bar.y + bar.h, &[(0.0, rgba(160, 160, 166, 255)), (1.0, rgba(225, 225, 230, 255))]));
            let used = if d.total > 0 { 1.0 - d.free as f32 / d.total as f32 } else { 0.0 };
            if used > 0.02 {
                gel_rect(ui.c, bar.x, bar.y, (bar.w * used).max(bar.h), bar.h, 7.0, if used > 0.9 { rgba(230, 70, 60, 255) } else { ui.theme.accent });
            }
            let txt = format!("{} livres de {}", gb(d.free), gb(d.total));
            ui.fonts.draw(ui.c, &txt, bar.x + bar.w + 10.0, bar.y + 11.0, 11.0, false, rgba(40, 40, 40, 255));
            y += row;
        }
        if self.disks.is_empty() {
            ui.fonts.draw(ui.c, "Nenhum disco montado encontrado.", 20.0, 40.0, 13.0, false, rgba(110, 110, 110, 255));
        }

        let h = ui.c.h;
        let fy = h - 40.0;
        ui.c.fill_rect(0.0, fy, w, 40.0, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(216, 216, 220, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        if let Some(m) = &self.message {
            let m = ui.fonts.ellipsize(m, 11.0, false, w - 260.0);
            ui.fonts.draw(ui.c, &m, 14.0, fy + 24.0, 11.0, false, rgba(60, 60, 60, 255));
        }
        let sel = self.selected.and_then(|i| self.disks.get(i)).cloned();
        if let Some(d) = sel {
            if ui.gel_button(Rect::new(w - 220.0, fy + 9.0, 96.0, 22.0), "Abrir", rgba(240, 240, 243, 255), 12.0, false) {
                let _ = std::process::Command::new(sibling("glove-files")).arg(&d.mount).spawn();
            }
            if d.removable && ui.gel_button(Rect::new(w - 116.0, fy + 9.0, 96.0, 22.0), "Ejetar", ui.theme.accent, 12.0, false) {
                self.eject();
            }
        }
    }

    fn tick(&self) -> Option<Duration> {
        Some(Duration::from_secs(3))
    }

    fn on_tick(&mut self) {
        self.disks = read_disks();
        if self.selected.is_some_and(|i| i >= self.disks.len()) {
            self.selected = None;
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.selected.and_then(|i| self.disks.get(i));
        vec![Menu::new(
            "Disco",
            vec![
                MenuItem::new("open", "Abrir", "Ctrl+O").enabled(sel.is_some()),
                MenuItem::new("eject", "Ejetar", "Ctrl+E").enabled(sel.is_some_and(|d| d.removable)),
                MenuItem::Separator,
                MenuItem::new("close", "Fechar", "Ctrl+W"),
            ],
        )]
    }
}

fn main() {
    let disks = read_disks();
    let h = (disks.len().max(1) as u32 * 74 + 64).clamp(200, 600);
    let opts = WindowOptions { title: "Discos".into(), app_id: "glove.Disks".into(), size: (560, h), fixed: false };
    if let Err(e) = run(opts, Disks { disks, selected: None, message: None }) {
        eprintln!("glove-disks: {e}");
    }
}
