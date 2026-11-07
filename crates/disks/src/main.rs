//! Discos (como o Utilitário de Disco da era): à esquerda os discos e as
//! partições (lsblk), à direita as informações da escolhida, o espaço usado e
//! os botões Abrir, Montar e Ejetar (udisksctl, sem precisar de root).

use glove_config::tr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, icons::{self, Icon}, Color},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const LIST_W: f32 = 210.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dev {
    pub name: String,
    pub size: u64,
    /// "disk" ou "part".
    pub kind: String,
    pub mount: String,
    pub fstype: String,
    pub label: String,
    pub model: String,
    pub removable: bool,
    pub tran: String,
    pub parent: String,
}

/// Linhas KEY="valor" do `lsblk -P`.
pub fn parse_lsblk(text: &str) -> Vec<Dev> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut d = Dev::default();
        let mut rest = line.trim();
        while let Some(eq) = rest.find("=\"") {
            let key = rest[..eq].trim();
            let after = &rest[eq + 2..];
            let Some(end) = after.find('"') else { break };
            let val = after[..end].replace("\\x20", " ");
            match key {
                "NAME" => d.name = val,
                "SIZE" => d.size = val.parse().unwrap_or(0),
                "TYPE" => d.kind = val,
                "MOUNTPOINT" => d.mount = val,
                "FSTYPE" => d.fstype = val,
                "LABEL" => d.label = val,
                "MODEL" => d.model = val.trim().to_string(),
                "RM" => d.removable = val == "1",
                "TRAN" => d.tran = val,
                "PKNAME" => d.parent = val,
                _ => {}
            }
            rest = &after[end + 1..];
        }
        // Só discos de verdade e as partições deles (sem loop, zram nem as áreas de boot do eMMC).
        let skip = d.kind == "loop" || d.kind == "rom" && d.size == 0 || d.name.starts_with("zram") || d.name.contains("boot") && d.kind == "disk";
        if !d.name.is_empty() && !skip && (d.kind == "disk" || d.kind == "part" || d.kind == "crypt" || d.kind == "lvm") {
            out.push(d);
        }
    }
    out
}

fn read_devs() -> Vec<Dev> {
    std::process::Command::new("lsblk")
        .args(["-P", "-b", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE,LABEL,MODEL,RM,TRAN,PKNAME"])
        .output()
        .map(|o| parse_lsblk(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

fn statvfs(p: &Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).ok()?;
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        (libc::statvfs(c.as_ptr(), &mut st) == 0).then(|| (st.f_blocks as u64 * st.f_frsize as u64, st.f_bavail as u64 * st.f_frsize as u64))
    }
}

fn gb(n: u64) -> String {
    let g = n as f64 / 1e9;
    if g >= 100.0 { format!("{g:.0} GB") } else if g >= 1.0 { format!("{g:.1} GB").replace('.', glove_config::virgula()) } else { format!("{:.0} MB", n as f64 / 1e6) }
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

/// Nome para a lista: rótulo, modelo ou o "Sistema" do disco do /.
fn display(d: &Dev) -> String {
    if d.mount == "/" {
        return tr!("Sistema").into();
    }
    if !d.label.is_empty() {
        return d.label.clone();
    }
    if d.kind == "disk" {
        let size = gb(d.size);
        return if d.model.is_empty() { tr!("Disco de {}", size) } else { format!("{size} {}", d.model) };
    }
    if d.mount == "/boot/efi" {
        return tr!("Partição EFI").into();
    }
    d.name.clone()
}

fn connection(t: &str) -> &str {
    match t {
        "usb" => "USB",
        "sata" | "ata" => "SATA",
        "nvme" => "NVMe",
        "mmc" => tr!("Cartão / eMMC"),
        "" => tr!("Interna"),
        o => o,
    }
}

struct Disks {
    devs: Vec<Dev>,
    selected: Option<usize>,
    message: Option<String>,
}

impl Disks {
    fn current(&self) -> Option<Dev> {
        self.selected.and_then(|i| self.devs.get(i)).cloned()
    }

    fn udisks(&mut self, args: &[&str], ok: String) {
        match std::process::Command::new("udisksctl").args(args).output() {
            Ok(o) if o.status.success() => self.message = Some(ok),
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr);
                let e = e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
                self.message = Some(if e.contains("NotAuthorized") { "O sistema pediu permissão de administrador e ela não foi dada.".into() } else { tr!("Não deu certo: {}", e) });
            }
            Err(e) => self.message = Some(tr!("Não foi possível usar o udisksctl: {}", e)),
        }
        self.devs = read_devs();
    }

    fn mount(&mut self) {
        let Some(d) = self.current() else { return };
        self.udisks(&["mount", "-b", &format!("/dev/{}", d.name)], format!("“{}” montado.", display(&d)));
    }

    fn eject(&mut self) {
        let Some(d) = self.current() else { return };
        // Ejetar desmonta as partições do disco e desliga o aparelho.
        let disk = if d.kind == "part" { d.parent.clone() } else { d.name.clone() };
        let parts: Vec<Dev> = self.devs.iter().filter(|x| x.parent == disk && !x.mount.is_empty()).cloned().collect();
        // O aviso de "removido sem ejetar" não vale para este.
        glove_config::discos::mark_ejected(&format!("/dev/{disk}"));
        for p in parts {
            let _ = std::process::Command::new("udisksctl").args(["unmount", "-b", &format!("/dev/{}", p.name)]).output();
        }
        if d.kind == "part" && !d.mount.is_empty() {
            let _ = std::process::Command::new("udisksctl").args(["unmount", "-b", &format!("/dev/{}", d.name)]).output();
        }
        self.udisks(&["power-off", "-b", &format!("/dev/{disk}")], format!("“{}” pode ser removido com segurança.", display(&d)));
        self.selected = None;
    }

    fn open(&self) {
        if let Some(d) = self.current().filter(|d| !d.mount.is_empty()) {
            if let Ok(mut c) = std::process::Command::new(sibling("glove-files")).arg(&d.mount).spawn() {
                std::thread::spawn(move || c.wait());
            }
        }
    }

    /// Painel da direita: informações, espaço e botões.
    fn details(&mut self, ui: &mut Ui, r: Rect) {
        let Some(d) = self.current() else {
            ui.fonts.draw(ui.c, tr!("Escolha um disco ou uma partição à esquerda."), r.x + 20.0, r.y + 40.0, 13.0, false, rgba(110, 110, 116, 255));
            return;
        };
        let icon = if d.removable { Icon::Disk } else { Icon::Disk };
        icons::draw(ui.c, ui.fonts, icon, r.x + 16.0, r.y + 14.0, 56.0, None);
        ui.fonts.draw(ui.c, &display(&d), r.x + 84.0, r.y + 38.0, 15.0, true, rgba(20, 20, 24, 255));
        let sub = if d.kind == "disk" { tr!("Disco de {}", gb(d.size)) } else { tr!("Partição de {}", gb(d.size)) };
        ui.fonts.draw(ui.c, &sub, r.x + 84.0, r.y + 56.0, 11.0, false, rgba(90, 90, 96, 255));
        // Tabela "rótulo: valor".
        let mut rows: Vec<(&str, String)> = vec![(tr!("Dispositivo"), format!("/dev/{}", d.name)), (tr!("Capacidade"), gb(d.size))];
        if d.kind == "disk" {
            rows.push((tr!("Conexão"), connection(&d.tran).to_string()));
            if !d.model.is_empty() {
                rows.push((tr!("Modelo"), d.model.clone()));
            }
            rows.push((tr!("Removível"), if d.removable { tr!("Sim").into() } else { tr!("Não").into() }));
            let n = self.devs.iter().filter(|x| x.parent == d.name).count();
            rows.push((tr!("Partições"), n.to_string()));
        } else {
            rows.push((tr!("Formato"), if d.fstype.is_empty() { "—".into() } else { d.fstype.clone() }));
            rows.push((tr!("Montado em"), if d.mount.is_empty() { tr!("Não montado").into() } else { d.mount.clone() }));
        }
        let usage = (!d.mount.is_empty()).then(|| statvfs(Path::new(&d.mount))).flatten();
        if let Some((total, free)) = usage {
            rows.push((tr!("Disponível"), gb(free)));
            rows.push((tr!("Usado"), gb(total.saturating_sub(free))));
        }
        let mut y = r.y + 92.0;
        for (k, v) in &rows {
            let t = format!("{k}:");
            let tw = ui.text_width(&t, 12.0, false);
            ui.fonts.draw(ui.c, &t, r.x + 130.0 - tw, y, 12.0, false, rgba(80, 80, 86, 255));
            let v = ui.fonts.ellipsize(v, 12.0, false, r.w - 150.0);
            ui.fonts.draw(ui.c, &v, r.x + 140.0, y, 12.0, false, rgba(20, 20, 24, 255));
            y += 19.0;
        }
        // Barra do espaço.
        if let Some((total, free)) = usage {
            y += 6.0;
            let bar = Rect::new(r.x + 20.0, y, r.w - 40.0, 16.0);
            ui.c.fill(rrect(bar.x, bar.y, bar.w, bar.h, 8.0), &vgrad(bar.y, bar.y + bar.h, &[(0.0, rgba(160, 160, 166, 255)), (1.0, rgba(225, 225, 230, 255))]));
            let used = if total > 0 { 1.0 - free as f32 / total as f32 } else { 0.0 };
            if used > 0.02 {
                gel_rect(ui.c, bar.x, bar.y, (bar.w * used).max(bar.h), bar.h, 8.0, if used > 0.9 { rgba(230, 70, 60, 255) } else { ui.theme.accent });
            }
            let t = tr!("{} livres de {}", gb(free), gb(total));
            ui.fonts.draw(ui.c, &t, bar.x, bar.y + 32.0, 11.0, false, rgba(60, 60, 66, 255));
        }
        // Botões.
        let by = r.y + r.h - 34.0;
        let mounted = !d.mount.is_empty();
        let can_mount = d.kind == "part" && !mounted && !d.fstype.is_empty() && d.fstype != "swap";
        let can_eject = d.removable || self.devs.iter().any(|x| x.name == d.parent && x.removable);
        let (mut open, mut mount, mut eject) = (false, false, false);
        ui.area(Rect::new(r.x + 16.0, by, r.w - 32.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_active(mounted);
                open = ui.button(tr!("Abrir"));
                ui.set_active(can_mount);
                mount = ui.button("Montar");
                ui.set_active(can_eject);
                eject = ui.default_button(tr!("Ejetar"));
                ui.set_active(true);
            });
        });
        if open {
            self.open();
        }
        if mount {
            self.mount();
        }
        if eject {
            self.eject();
        }
    }
}

impl App for Disks {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
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
                "open" => self.open(),
                "mount" => self.mount(),
                "eject" => self.eject(),
                "close" => ui.close = true,
                _ => {}
            }
        }
        let foot = 24.0;
        // Lista da esquerda: discos com as partições recuadas embaixo.
        let list = Rect::new(0.0, 0.0, LIST_W, h - foot);
        ui.c.fill_rect(list.x, list.y, list.w, list.h, &solid(rgba(232, 236, 242, 255)));
        ui.c.vline(LIST_W - 0.5, 0.0, h - foot, rgba(150, 150, 156, 255));
        let mut y = 8.0;
        let disks: Vec<usize> = (0..self.devs.len()).filter(|&i| self.devs[i].kind == "disk").collect();
        let mut order = Vec::new();
        for &di in &disks {
            order.push((di, 0.0));
            let name = self.devs[di].name.clone();
            order.extend((0..self.devs.len()).filter(|&i| self.devs[i].parent == name && self.devs[i].kind != "disk").map(|i| (i, 18.0)));
        }
        let mut clicked = None;
        for (i, indent) in order {
            let d = &self.devs[i];
            let rr = Rect::new(0.0, y, LIST_W - 1.0, 36.0);
            let on = self.selected == Some(i);
            if on {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &vgrad(rr.y, rr.y + rr.h, &[(0.0, lighten(ui.theme.accent, 0.15)), (1.0, ui.theme.accent)]));
            }
            let ink = if on { Color::WHITE } else { rgba(20, 20, 24, 255) };
            icons::draw(ui.c, ui.fonts, Icon::Disk, 8.0 + indent, y + 4.0, 28.0, None);
            let name = ui.fonts.ellipsize(&display(d), 12.0, indent == 0.0, LIST_W - 50.0 - indent);
            ui.fonts.draw(ui.c, &name, 42.0 + indent, y + 16.0, 12.0, indent == 0.0, ink);
            let sub = if d.kind == "disk" { gb(d.size) } else if d.mount.is_empty() { tr!("{} · não montado", gb(d.size)) } else { gb(d.size) };
            ui.fonts.draw(ui.c, &sub, 42.0 + indent, y + 30.0, 10.0, false, if on { Color::WHITE } else { rgba(100, 100, 106, 255) });
            let resp = ui.hit(rr);
            if resp.pressed {
                clicked = Some(i);
            }
            if resp.double {
                clicked = Some(i);
                self.selected = Some(i);
                self.open();
            }
            y += 38.0;
        }
        if let Some(i) = clicked {
            self.selected = Some(i);
            self.message = None;
        }
        self.details(ui, Rect::new(LIST_W, 0.0, w - LIST_W, h - foot));
        // Rodapé com a mensagem do último pedido.
        let fy = h - foot;
        ui.c.fill_rect(0.0, fy, w, foot, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(216, 216, 220, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        if let Some(m) = &self.message {
            let m = ui.fonts.ellipsize(m, 11.0, false, w - 30.0);
            ui.fonts.draw(ui.c, &m, 12.0, fy + 16.0, 11.0, false, rgba(60, 60, 60, 255));
        }
    }

    fn tick(&self) -> Option<Duration> {
        Some(Duration::from_secs(3))
    }

    fn on_tick(&mut self) {
        let sel = self.current().map(|d| d.name);
        self.devs = read_devs();
        self.selected = sel.and_then(|n| self.devs.iter().position(|d| d.name == n));
    }

    fn menus(&self) -> Vec<Menu> {
        let d = self.current();
        vec![Menu::new(
            tr!("Disco"),
            vec![
                MenuItem::new("open", tr!("Abrir"), "Ctrl+O").enabled(d.as_ref().is_some_and(|d| !d.mount.is_empty())),
                MenuItem::new("mount", "Montar", "").enabled(d.as_ref().is_some_and(|d| d.kind == "part" && d.mount.is_empty())),
                MenuItem::new("eject", tr!("Ejetar"), "Ctrl+E").enabled(d.is_some()),
                MenuItem::Separator,
                MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
            ],
        )]
    }
}

pub fn main() {
    let devs = read_devs();
    // Começa no disco do sistema.
    let selected = devs.iter().position(|d| d.mount == "/");
    let opts = WindowOptions { title: tr!("Discos").into(), app_id: "glove.Disks".into(), size: (640, 440), fixed: false, min_size: None };
    if let Err(e) = run(opts, Disks { devs, selected, message: None }) {
        eprintln!("glove-disks: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn le_o_lsblk() {
        let t = "NAME=\"loop0\" SIZE=\"1\" TYPE=\"loop\" MOUNTPOINT=\"/snap/x\" FSTYPE=\"squashfs\" LABEL=\"\" MODEL=\"\" RM=\"0\" TRAN=\"\" PKNAME=\"\"\nNAME=\"sda\" SIZE=\"536871960576\" TYPE=\"disk\" MOUNTPOINT=\"\" FSTYPE=\"\" LABEL=\"\" MODEL=\"STORAGE DEVICE\" RM=\"1\" TRAN=\"usb\" PKNAME=\"\"\nNAME=\"sda1\" SIZE=\"5\" TYPE=\"part\" MOUNTPOINT=\"\" FSTYPE=\"exfat\" LABEL=\"SD 512\" MODEL=\"\" RM=\"1\" TRAN=\"\" PKNAME=\"sda\"\nNAME=\"mmcblk0boot0\" SIZE=\"4\" TYPE=\"disk\" MOUNTPOINT=\"\" FSTYPE=\"\" LABEL=\"\" MODEL=\"\" RM=\"0\" TRAN=\"mmc\" PKNAME=\"\"\n";
        let d = super::parse_lsblk(t);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].model, "STORAGE DEVICE");
        assert!(d[0].removable && d[0].tran == "usb");
        assert_eq!(d[1].label, "SD 512");
        assert_eq!(d[1].parent, "sda");
        assert_eq!(super::display(&d[0]), "537 GB STORAGE DEVICE");
    }
}
