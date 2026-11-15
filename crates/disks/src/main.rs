//! Discos (como o Utilitário de Disco da era, com o que o gnome-disks faz): à
//! esquerda os discos (aparelhos e imagens), à direita o disco escolhido com o
//! mapa dos volumes (partições e espaço livre) e o volume escolhido.
//!
//! Tudo pelo udisks2 (`udisks.rs`); o que muda o disco pede a senha pelo polkit
//! (a janela do `glove-senha`). As operações rodam numa linha à parte, para a
//! janela não travar enquanto a senha é pedida.

mod gravar;
mod udisks;

use glove_config::tr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, icons::{self, Icon}, Color},
    run, AlertKind, App, Choice, FileBrowser, Menu, MenuItem, Rect, Ui, WindowOptions,
};
use udisks::{Model, Slot};

const LIST_W: f32 = 220.0;
const MAP_H: f32 = 64.0;

/// O pedaço escolhido no mapa: uma partição (pelo caminho no udisks) ou um espaço livre.
#[derive(Clone, Debug, PartialEq)]
enum Pick {
    Block(String),
    Free(u64),
}

/// Uma operação em andamento: o que aparece no rodapé e a resposta.
struct Job {
    what: String,
    rx: Receiver<Result<String, String>>,
}

struct Disks {
    conn: Option<zbus::blocking::Connection>,
    model: Model,
    /// O disco escolhido (caminho do bloco inteiro).
    disk: Option<String>,
    pick: Option<Pick>,
    message: Option<String>,
    job: Option<Job>,
    /// Criar disco inicializável: escolhendo a imagem, confirmando, gravando.
    iso_fb: Option<FileBrowser>,
    confirm: Option<PathBuf>,
    progress: Option<Arc<gravar::Progress>>,
}

/// Pontos de montagem do próprio sistema: um disco com algum deles não é apagado.
const SYSTEM_MOUNTS: [&str; 7] = ["/", "/boot", "/boot/efi", "/efi", "/home", "/usr", "/var"];

pub(crate) fn gb(n: u64) -> String {
    let g = n as f64 / 1e9;
    if g >= 100.0 {
        format!("{g:.0} GB")
    } else if g >= 1.0 {
        format!("{g:.1} GB").replace('.', glove_config::virgula())
    } else if n >= 1_000_000 {
        format!("{:.0} MB", n as f64 / 1e6)
    } else {
        format!("{:.0} KB", n as f64 / 1e3)
    }
}

fn sibling(name: &str) -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

fn statvfs(p: &Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).ok()?;
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        (libc::statvfs(c.as_ptr(), &mut st) == 0).then(|| (st.f_blocks as u64 * st.f_frsize as u64, st.f_bavail as u64 * st.f_frsize as u64))
    }
}

impl Disks {
    fn reload(&mut self) {
        if self.conn.is_none() {
            self.conn = zbus::blocking::Connection::system().ok();
        }
        let Some(conn) = &self.conn else {
            self.message = Some(tr!("Não foi possível falar com o serviço de discos (udisks2).").into());
            return;
        };
        match Model::read(conn) {
            Ok(m) => self.model = m,
            Err(e) => self.message = Some(tr!("O serviço de discos (udisks2) não respondeu: {}", e)),
        }
        // O disco escolhido sumiu (pendrive tirado): volta ao primeiro.
        let disks = self.model.disks();
        if !self.disk.as_ref().is_some_and(|d| disks.iter().any(|x| &self.model.blocks[x.block].path == d)) {
            self.disk = disks.first().map(|d| self.model.blocks[d.block].path.clone());
            self.pick = None;
        }
        if let Some(Pick::Block(p)) = &self.pick {
            if self.model.block(p).is_none() {
                self.pick = None;
            }
        }
    }

    fn disk_index(&self) -> Option<usize> {
        self.model.block(self.disk.as_deref()?)
    }

    fn drive_of(&self, bi: usize) -> Option<&udisks::Drive> {
        let d = &self.model.blocks[bi].drive;
        self.model.drives.iter().find(|x| &x.path == d)
    }

    fn picked_block(&self) -> Option<usize> {
        match self.pick.as_ref()? {
            Pick::Block(p) => self.model.block(p),
            Pick::Free(_) => None,
        }
    }

    /// Nome do disco para a lista e o cabeçalho.
    fn disk_name(&self, bi: usize) -> String {
        let b = &self.model.blocks[bi];
        if let Some(f) = &b.loop_file {
            let f = Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return tr!("Imagem {}", f);
        }
        match self.drive_of(bi) {
            Some(d) => {
                let name = format!("{} {}", d.vendor.trim(), d.model.trim()).trim().to_string();
                if name.is_empty() { tr!("Disco de {}", gb(b.size)) } else { name }
            }
            None => b.device.clone(),
        }
    }

    fn disk_icon(&self, bi: usize) -> Icon {
        match self.drive_of(bi) {
            Some(d) if d.optical => Icon::Optical,
            Some(d) if d.removable || d.bus == "usb" => Icon::Removable,
            _ => Icon::Disk,
        }
    }

    /// Nome de um volume no mapa: rótulo, nome da partição, conteúdo.
    fn volume_name(&self, bi: usize) -> String {
        let b = &self.model.blocks[bi];
        if b.mounted() == Some("/") {
            return tr!("Sistema").into();
        }
        if !b.label.is_empty() {
            return b.label.clone();
        }
        if let Some(p) = &b.part {
            if p.container {
                return tr!("Estendida").into();
            }
            if !p.name.is_empty() {
                return p.name.clone();
            }
        }
        if b.fstype.is_empty() {
            return tr!("Partição {}", b.part.as_ref().map(|p| p.number).unwrap_or(0));
        }
        udisks::fs_name(&b.fstype)
    }

    /// Roda a operação numa linha à parte; a resposta chega no tique.
    fn start(&mut self, what: String, f: impl FnOnce() -> Result<String, String> + Send + 'static) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        self.message = Some(what.clone());
        self.job = Some(Job { what, rx });
    }

    fn mount(&mut self) {
        let Some(bi) = self.picked_block() else { return };
        let (path, name) = (self.model.blocks[bi].path.clone(), self.volume_name(bi));
        self.start(tr!("Montando “{}”…", name), move || udisks::mount(&path).map(|at| tr!("“{}” montado em {}.", name, at)));
    }

    fn unmount(&mut self) {
        let Some(bi) = self.picked_block() else { return };
        let (path, name) = (self.model.blocks[bi].path.clone(), self.volume_name(bi));
        self.start(tr!("Desmontando “{}”…", name), move || udisks::unmount(&path).map(|_| tr!("“{}” desmontado.", name)));
    }

    fn open(&self) {
        let Some(bi) = self.picked_block() else { return };
        if let Some(at) = self.model.blocks[bi].mounted() {
            if let Ok(mut c) = std::process::Command::new(sibling("glove-files")).arg(at).spawn() {
                std::thread::spawn(move || c.wait());
            }
        }
    }

    /// Ejetar: desmonta tudo do disco e ejeta a mídia ou desliga o aparelho.
    /// Disco de imagem: desconecta a imagem.
    fn eject(&mut self) {
        let Some(di) = self.disk_index() else { return };
        let name = self.disk_name(di);
        let b = self.model.blocks[di].clone();
        let mounted: Vec<String> = std::iter::once(di).chain(self.model.parts_of(di)).filter(|&i| self.model.blocks[i].mounted().is_some()).map(|i| self.model.blocks[i].path.clone()).collect();
        let drive = self.drive_of(di).cloned();
        glove_config::discos::mark_ejected(&b.device);
        self.start(tr!("Ejetando “{}”…", name), move || {
            for m in &mounted {
                udisks::unmount(m)?;
            }
            if b.loop_file.is_some() {
                udisks::loop_delete(&b.path)?;
            } else if let Some(d) = drive {
                if d.ejectable {
                    udisks::eject(&d.path)?;
                }
                if d.can_power_off {
                    udisks::power_off(&d.path)?;
                }
            }
            Ok(tr!("“{}” pode ser removido com segurança.", name))
        });
    }

    /// O disco escolhido pode ser apagado (não é o do sistema em uso).
    fn can_overwrite(&self) -> bool {
        let Some(di) = self.disk_index() else { return false };
        let b = &self.model.blocks[di];
        if b.read_only || b.size == 0 || self.drive_of(di).is_some_and(|d| d.optical) {
            return false;
        }
        !std::iter::once(di).chain(self.model.parts_of(di)).any(|i| self.model.blocks[i].mounts.as_ref().is_some_and(|m| m.iter().any(|p| SYSTEM_MOUNTS.contains(&p.as_str()))))
    }

    fn ask_image(&mut self) {
        if !self.can_overwrite() || self.job.is_some() {
            self.message = Some(tr!("Escolha à esquerda o pendrive (ou outro disco que não seja o do sistema).").into());
            return;
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into());
        let start = ["Downloads", "Transferências"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or(home);
        self.iso_fb = Some(FileBrowser::open(Some(start), &["iso", "img"]));
    }

    fn start_bootable(&mut self, iso: PathBuf) {
        let Some(di) = self.disk_index() else { return };
        let b = self.model.blocks[di].clone();
        let mounted: Vec<String> = std::iter::once(di).chain(self.model.parts_of(di)).filter(|&i| self.model.blocks[i].mounted().is_some()).map(|i| self.model.blocks[i].path.clone()).collect();
        let t = gravar::Target { disk: b.path.clone(), device: b.device.clone(), size: b.size, mounted };
        let p = Arc::new(gravar::Progress::default());
        self.progress = Some(p.clone());
        let name = iso.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.start(tr!("Gravando {}…", name), move || gravar::write(iso, t, p));
    }

    fn can_eject(&self) -> bool {
        self.disk_index().is_some_and(|di| self.model.blocks[di].loop_file.is_some() || self.drive_of(di).is_some_and(|d| d.removable || d.ejectable || d.can_power_off))
    }

    // --- desenho -------------------------------------------------------------------

    fn list(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(232, 236, 242, 255)));
        ui.c.vline(r.x + r.w - 0.5, r.y, r.h, rgba(150, 150, 156, 255));
        let mut y = r.y + 8.0;
        let mut clicked = None;
        for d in self.model.disks() {
            let bi = d.block;
            let rr = Rect::new(r.x, y, r.w - 1.0, 44.0);
            let on = self.disk.as_deref() == Some(self.model.blocks[bi].path.as_str());
            if on {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &vgrad(rr.y, rr.y + rr.h, &[(0.0, lighten(ui.theme.accent, 0.15)), (1.0, ui.theme.accent)]));
            }
            let ink = if on { Color::WHITE } else { rgba(20, 20, 24, 255) };
            icons::draw(ui.c, ui.fonts, self.disk_icon(bi), r.x + 8.0, y + 6.0, 32.0, None);
            let name = ui.fonts.ellipsize(&self.disk_name(bi), 12.0, true, r.w - 56.0);
            ui.fonts.draw(ui.c, &name, r.x + 48.0, y + 19.0, 12.0, true, ink);
            let b = &self.model.blocks[bi];
            let sub = if b.size == 0 { tr!("Sem mídia").to_string() } else { format!("{} · {}", gb(b.size), b.device) };
            let sub = ui.fonts.ellipsize(&sub, 10.0, false, r.w - 56.0);
            ui.fonts.draw(ui.c, &sub, r.x + 48.0, y + 34.0, 10.0, false, if on { Color::WHITE } else { rgba(100, 100, 106, 255) });
            if ui.hit(rr).pressed {
                clicked = Some(self.model.blocks[bi].path.clone());
            }
            y += 46.0;
        }
        if let Some(p) = clicked {
            if self.disk.as_ref() != Some(&p) {
                self.pick = None;
            }
            self.disk = Some(p);
            self.message = None;
        }
    }

    /// Cabeçalho do disco: ícone, nome e a linha de informações.
    fn header(&mut self, ui: &mut Ui, r: Rect, di: usize) {
        icons::draw(ui.c, ui.fonts, self.disk_icon(di), r.x + 16.0, r.y + 12.0, 48.0, None);
        let name = ui.fonts.ellipsize(&self.disk_name(di), 15.0, true, r.w - 100.0);
        ui.fonts.draw(ui.c, &name, r.x + 76.0, r.y + 32.0, 15.0, true, rgba(20, 20, 24, 255));
        let b = &self.model.blocks[di];
        let mut info = vec![gb(b.size)];
        info.push(match b.table.as_deref() {
            Some("gpt") => tr!("Tabela GPT").to_string(),
            Some("dos") => tr!("Tabela MBR").to_string(),
            Some(o) => o.to_string(),
            None if b.fstype.is_empty() => tr!("Sem partições").to_string(),
            None => udisks::fs_name(&b.fstype),
        });
        if let Some(d) = self.drive_of(di) {
            info.push(match d.bus.as_str() {
                "usb" => "USB".to_string(),
                "sdio" => tr!("Cartão de memória").to_string(),
                _ if d.rotation == 0 && !d.optical => tr!("SSD interno").to_string(),
                _ if d.optical => tr!("Leitor de CD/DVD").to_string(),
                _ => tr!("Disco interno").to_string(),
            });
            if !d.serial.is_empty() {
                info.push(tr!("Série {}", d.serial));
            }
        }
        if let Some(f) = &b.loop_file {
            info.push(f.clone());
        }
        let info = ui.fonts.ellipsize(&info.join("  ·  "), 11.0, false, r.w - 100.0);
        ui.fonts.draw(ui.c, &info, r.x + 76.0, r.y + 50.0, 11.0, false, rgba(80, 80, 86, 255));
    }

    /// Uma fileira do mapa: pedaços proporcionais ao tamanho (com largura mínima).
    fn map_row(&mut self, ui: &mut Ui, r: Rect, slots: &[Slot], total: u64) -> Option<Pick> {
        let mut clicked = None;
        let min_w = 44.0;
        let n = slots.len() as f32;
        let free_w = (r.w - min_w * n).max(0.0);
        let sizes: Vec<u64> = slots.iter().map(|s| match s { Slot::Part(i) => self.model.blocks[*i].size, Slot::Free { size, .. } => *size }).collect();
        let sum: u64 = sizes.iter().sum::<u64>().max(1).max(total.min(sizes.iter().sum::<u64>().max(1)));
        let mut x = r.x;
        ui.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(200, 200, 206, 255)));
        for (k, slot) in slots.iter().enumerate() {
            let w = (min_w + free_w * sizes[k] as f32 / sum as f32).round();
            let w = if k + 1 == slots.len() { r.x + r.w - x } else { w };
            let seg = Rect::new(x, r.y, w, r.h);
            let (pick, title, sub, color) = match slot {
                Slot::Part(i) => {
                    let b = &self.model.blocks[*i];
                    let color = if b.part.as_ref().is_some_and(|p| p.container) {
                        rgba(150, 150, 160, 255)
                    } else if b.mounted().is_some() {
                        ui.theme.accent
                    } else {
                        lighten(ui.theme.accent, 0.35)
                    };
                    (Pick::Block(b.path.clone()), self.volume_name(*i), gb(b.size), color)
                }
                Slot::Free { offset, size } => (Pick::Free(*offset), tr!("Livre").to_string(), gb(*size), rgba(236, 236, 240, 255)),
            };
            let free = matches!(slot, Slot::Free { .. });
            if free {
                ui.c.fill_rect(seg.x + 1.0, seg.y + 1.0, seg.w - 2.0, seg.h - 2.0, &solid(color));
            } else {
                gel_rect(ui.c, seg.x + 1.0, seg.y + 1.0, seg.w - 2.0, seg.h - 2.0, 4.0, color);
            }
            if self.pick.as_ref() == Some(&pick) {
                ui.c.stroke(rrect(seg.x + 2.0, seg.y + 2.0, seg.w - 4.0, seg.h - 4.0, 4.0), &solid(rgba(20, 20, 30, 255)), 2.5);
            }
            let ink = if free { rgba(80, 80, 86, 255) } else { Color::WHITE };
            let t = ui.fonts.ellipsize(&title, 11.0, true, seg.w - 10.0);
            ui.fonts.draw(ui.c, &t, seg.x + 6.0, seg.y + seg.h / 2.0 - 2.0, 11.0, true, ink);
            let s = ui.fonts.ellipsize(&sub, 10.0, false, seg.w - 10.0);
            ui.fonts.draw(ui.c, &s, seg.x + 6.0, seg.y + seg.h / 2.0 + 13.0, 10.0, false, ink);
            if ui.hit(seg).pressed {
                clicked = Some(pick);
            }
            x += w;
        }
        clicked
    }

    /// Informações do volume escolhido (linhas "rótulo: valor") e a barra de uso.
    fn volume_info(&mut self, ui: &mut Ui, r: Rect) {
        let mut rows: Vec<(String, String)> = Vec::new();
        let mut usage = None;
        match self.pick.clone() {
            None => {
                ui.fonts.draw(ui.c, tr!("Escolha um volume no mapa."), r.x + 20.0, r.y + 20.0, 12.0, false, rgba(110, 110, 116, 255));
                return;
            }
            Some(Pick::Free(off)) => {
                let size = self.disk_index().and_then(|di| self.model.layout(di).into_iter().chain(self.ext_layout()).find_map(|s| match s { Slot::Free { offset, size } if offset == off => Some(size), _ => None }));
                rows.push((tr!("Espaço livre").into(), gb(size.unwrap_or(0))));
                rows.push((tr!("Começa em").into(), gb(off)));
            }
            Some(Pick::Block(_)) => {
                let Some(bi) = self.picked_block() else { return };
                let b = self.model.blocks[bi].clone();
                rows.push((tr!("Tamanho").into(), format!("{} ({} bytes)", gb(b.size), b.size)));
                let mut content = if b.usage == "filesystem" || !b.fstype.is_empty() { udisks::fs_name(&b.fstype) } else { tr!("Vazio ou desconhecido").into() };
                if !b.label.is_empty() {
                    content = format!("{content} — {}", b.label);
                }
                rows.push((tr!("Conteúdo").into(), content));
                rows.push((tr!("Dispositivo").into(), b.device.clone()));
                if let Some(p) = &b.part {
                    rows.push((tr!("Partição").into(), format!("{} — {}", p.number, udisks::ptype_name(&p.ptype))));
                }
                if !b.uuid.is_empty() {
                    rows.push(("UUID".into(), b.uuid.clone()));
                }
                if b.has_fs() {
                    rows.push((tr!("Montado em").into(), b.mounted().map(str::to_string).unwrap_or_else(|| tr!("Não montado").into())));
                    usage = b.mounted().and_then(|m| statvfs(Path::new(m)));
                }
            }
        }
        let mut y = r.y + 18.0;
        for (k, v) in &rows {
            let t = format!("{k}:");
            let tw = ui.text_width(&t, 12.0, false);
            ui.fonts.draw(ui.c, &t, r.x + 130.0 - tw, y, 12.0, false, rgba(80, 80, 86, 255));
            let v = ui.fonts.ellipsize(v, 12.0, false, r.w - 150.0);
            ui.fonts.draw(ui.c, &v, r.x + 140.0, y, 12.0, false, rgba(20, 20, 24, 255));
            y += 19.0;
        }
        if let Some((total, free)) = usage {
            y += 4.0;
            let bar = Rect::new(r.x + 140.0, y, (r.w - 330.0).clamp(80.0, 320.0), 12.0);
            ui.c.fill(rrect(bar.x, bar.y, bar.w, bar.h, 6.0), &vgrad(bar.y, bar.y + bar.h, &[(0.0, rgba(160, 160, 166, 255)), (1.0, rgba(225, 225, 230, 255))]));
            let used = if total > 0 { 1.0 - free as f32 / total as f32 } else { 0.0 };
            if used > 0.02 {
                gel_rect(ui.c, bar.x, bar.y, (bar.w * used).max(bar.h), bar.h, 6.0, if used > 0.9 { rgba(230, 70, 60, 255) } else { ui.theme.accent });
            }
            let t = tr!("{} livres de {}", gb(free), gb(total));
            ui.fonts.draw(ui.c, &t, bar.x + bar.w + 10.0, bar.y + 10.0, 11.0, false, rgba(60, 60, 66, 255));
        }
    }

    /// As lógicas da estendida escolhida (MBR), para a segunda fileira do mapa.
    fn ext_layout(&self) -> Vec<Slot> {
        let Some(di) = self.disk_index() else { return Vec::new() };
        let ext = self.model.parts_of(di).into_iter().find(|&i| self.model.blocks[i].part.as_ref().is_some_and(|p| p.container));
        ext.map(|e| self.model.logical_layout(e)).unwrap_or_default()
    }

    fn details(&mut self, ui: &mut Ui, r: Rect) {
        let Some(di) = self.disk_index() else {
            ui.fonts.draw(ui.c, tr!("Nenhum disco encontrado."), r.x + 20.0, r.y + 40.0, 13.0, false, rgba(110, 110, 116, 255));
            return;
        };
        self.header(ui, Rect::new(r.x, r.y, r.w, 70.0), di);
        ui.fonts.draw(ui.c, tr!("Volumes"), r.x + 20.0, r.y + 92.0, 12.0, true, rgba(40, 40, 46, 255));
        let mut y = r.y + 100.0;
        let layout = self.model.layout(di);
        if let Some(p) = self.map_row(ui, Rect::new(r.x + 20.0, y, r.w - 40.0, MAP_H), &layout, self.model.blocks[di].size) {
            self.pick = Some(p);
            self.message = None;
        }
        y += MAP_H + 6.0;
        let logical = self.ext_layout();
        if !logical.is_empty() {
            if let Some(p) = self.map_row(ui, Rect::new(r.x + 40.0, y, r.w - 60.0, MAP_H * 0.7), &logical, 0) {
                self.pick = Some(p);
                self.message = None;
            }
            y += MAP_H * 0.7 + 6.0;
        }
        // Ações do volume escolhido.
        let picked = self.picked_block().map(|bi| self.model.blocks[bi].clone());
        let mounted = picked.as_ref().is_some_and(|b| b.mounted().is_some());
        let can_mount = picked.as_ref().is_some_and(|b| b.has_fs() && b.mounted().is_none());
        let busy = self.job.is_some();
        let (mut mount, mut unmount, mut open, mut eject) = (false, false, false, false);
        ui.area(Rect::new(r.x + 18.0, y + 4.0, r.w - 36.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_active(can_mount && !busy);
                mount = ui.button(tr!("Montar"));
                ui.set_active(mounted && !busy);
                unmount = ui.button(tr!("Desmontar"));
                ui.set_active(mounted);
                open = ui.button(tr!("Abrir"));
                ui.set_active(true);
            });
        });
        let mut boot = false;
        ui.area(Rect::new(r.x + r.w - 290.0, r.y + 76.0, 274.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_active(self.can_overwrite() && !busy);
                boot = ui.button(tr!("Disco Inicializável…"));
                ui.set_active(self.can_eject() && !busy);
                eject = ui.button(tr!("Ejetar"));
                ui.set_active(true);
            });
        });
        if boot {
            self.ask_image();
        }
        y += 40.0;
        ui.c.hline(r.x + 20.0, y, r.w - 40.0, rgba(200, 200, 206, 255));
        self.volume_info(ui, Rect::new(r.x, y + 4.0, r.w, r.h - (y - r.y) - 4.0));
        if mount {
            self.mount();
        }
        if unmount {
            self.unmount();
        }
        if open {
            self.open();
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
                "unmount" => self.unmount(),
                "eject" => self.eject(),
                "bootable" => self.ask_image(),
                "close" => ui.close = true,
                _ => {}
            }
        }
        let foot = 24.0;
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(244, 244, 246, 255)));
        self.list(ui, Rect::new(0.0, 0.0, LIST_W, h - foot));
        self.details(ui, Rect::new(LIST_W, 0.0, w - LIST_W, h - foot));
        // Rodapé: a operação em andamento ou a resposta da última.
        let fy = h - foot;
        ui.c.fill_rect(0.0, fy, w, foot, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(216, 216, 220, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        let progress = self.progress.clone().filter(|_| self.job.is_some());
        if let Some(p) = progress {
            // Gravando: barra, a fase e o Cancelar.
            let frac = *p.fraction.lock().unwrap();
            let phase = p.phase.lock().unwrap().clone();
            let bar = Rect::new(12.0, fy + 7.0, 180.0, 10.0);
            ui.c.fill(rrect(bar.x, bar.y, bar.w, bar.h, 5.0), &vgrad(bar.y, bar.y + bar.h, &[(0.0, rgba(160, 160, 166, 255)), (1.0, rgba(225, 225, 230, 255))]));
            if frac > 0.01 {
                gel_rect(ui.c, bar.x, bar.y, (bar.w * frac).max(bar.h), bar.h, 5.0, ui.theme.accent);
            }
            let t = format!("{:.0}%  {}", frac * 100.0, phase);
            let t = ui.fonts.ellipsize(&t, 11.0, false, w - 320.0);
            ui.fonts.draw(ui.c, &t, 202.0, fy + 16.0, 11.0, false, rgba(60, 60, 60, 255));
            let mut cancel = false;
            ui.area(Rect::new(w - 100.0, fy + 1.0, 90.0, 22.0), |ui| cancel = ui.button(tr!("Cancelar")));
            if cancel {
                p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        } else if let Some(m) = &self.message {
            let m = ui.fonts.ellipsize(m, 11.0, false, w - 30.0);
            ui.fonts.draw(ui.c, &m, 12.0, fy + 16.0, 11.0, false, rgba(60, 60, 60, 255));
        }
        // Escolher a imagem e confirmar que o disco será apagado.
        if let Some(fb) = &mut self.iso_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.iso_fb = None;
                if let Choice::Picked(p) = c {
                    self.confirm = Some(p);
                }
                ui.redraw = true;
            }
        }
        if let Some(iso) = self.confirm.clone() {
            let di = self.disk_index();
            let disk = di.map(|d| format!("{} ({}, {})", self.disk_name(d), self.model.blocks[d].device, gb(self.model.blocks[d].size))).unwrap_or_default();
            let name = iso.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let text = tr!("Tudo o que está em {} será apagado, e o disco vai iniciar o computador com “{}” (UEFI). Imagens de Linux são gravadas inteiras; o instalador do Windows vira um pendrive FAT32.", disk, name);
            match ui.alert(AlertKind::Caution, tr!("Apagar o disco e criar o disco inicializável?"), &text, &[tr!("Cancelar"), tr!("Apagar e Gravar")]) {
                Some(1) => {
                    self.confirm = None;
                    self.start_bootable(iso);
                }
                Some(_) => self.confirm = None,
                None => {}
            }
        }
    }

    fn tick(&self) -> Option<Duration> {
        Some(if self.job.is_some() { Duration::from_millis(200) } else { Duration::from_secs(2) })
    }

    fn on_tick(&mut self) {
        if let Some(r) = self.job.as_ref().and_then(|j| j.rx.try_recv().ok()) {
            let what = self.job.take().map(|j| j.what).unwrap_or_default();
            self.message = Some(match r {
                Ok(m) => m,
                Err(e) => tr!("{} Não deu certo: {}", what.trim_end_matches('…'), e),
            });
        }
        self.reload();
    }

    fn menus(&self) -> Vec<Menu> {
        let b = self.picked_block().map(|bi| &self.model.blocks[bi]);
        let busy = self.job.is_some();
        vec![
            Menu::new(
                tr!("Disco"),
                vec![
                    MenuItem::new("bootable", tr!("Criar Disco Inicializável…"), "").enabled(self.can_overwrite() && !busy),
                    MenuItem::new("eject", tr!("Ejetar"), "Ctrl+E").enabled(self.can_eject() && !busy),
                    MenuItem::Separator,
                    MenuItem::new("close", tr!("Fechar"), "Ctrl+W"),
                ],
            ),
            Menu::new(
                tr!("Volume"),
                vec![
                    MenuItem::new("open", tr!("Abrir"), "Ctrl+O").enabled(b.is_some_and(|b| b.mounted().is_some())),
                    MenuItem::new("mount", tr!("Montar"), "").enabled(!busy && b.is_some_and(|b| b.has_fs() && b.mounted().is_none())),
                    MenuItem::new("unmount", tr!("Desmontar"), "").enabled(!busy && b.is_some_and(|b| b.mounted().is_some())),
                ],
            ),
        ]
    }
}

/// `glove-disks --gravar <imagem> <dispositivo>`: cria o disco inicializável sem a
/// janela (para scripts e testes), com o andamento no terminal.
fn write_cli(iso: &str, device: &str) -> i32 {
    let Ok(conn) = zbus::blocking::Connection::system() else {
        eprintln!("glove-disks: sem o D-Bus do sistema");
        return 1;
    };
    let Ok(model) = Model::read(&conn) else {
        eprintln!("glove-disks: o udisks2 não respondeu");
        return 1;
    };
    let Some(di) = model.blocks.iter().position(|b| b.device == device && b.part.is_none()) else {
        eprintln!("glove-disks: {device} não é um disco inteiro");
        return 1;
    };
    let mut app = Disks { conn: None, model, disk: None, pick: None, message: None, job: None, iso_fb: None, confirm: None, progress: None };
    app.disk = Some(app.model.blocks[di].path.clone());
    if !app.can_overwrite() {
        eprintln!("glove-disks: {device} é o disco do sistema (ou só de leitura): não será apagado");
        return 1;
    }
    let b = app.model.blocks[di].clone();
    let mounted: Vec<String> = std::iter::once(di).chain(app.model.parts_of(di)).filter(|&i| app.model.blocks[i].mounted().is_some()).map(|i| app.model.blocks[i].path.clone()).collect();
    let t = gravar::Target { disk: b.path, device: b.device, size: b.size, mounted };
    let p = Arc::new(gravar::Progress::default());
    let (tx, rx) = channel();
    let (iso, pc) = (PathBuf::from(iso), p.clone());
    std::thread::spawn(move || {
        let _ = tx.send(gravar::write(iso, t, pc));
    });
    loop {
        if let Ok(r) = rx.recv_timeout(Duration::from_secs(2)) {
            return match r {
                Ok(m) => {
                    println!("{m}");
                    0
                }
                Err(e) => {
                    eprintln!("glove-disks: {e}");
                    1
                }
            };
        }
        println!("{:3.0}%  {}", *p.fraction.lock().unwrap() * 100.0, p.phase.lock().unwrap());
    }
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--gravar") {
        let (Some(iso), Some(dev)) = (args.get(2), args.get(3)) else {
            eprintln!("uso: glove-disks --gravar <imagem.iso> <dispositivo>");
            std::process::exit(2);
        };
        std::process::exit(write_cli(iso, dev));
    }
    let mut app = Disks { conn: None, model: Model::default(), disk: None, pick: None, message: None, job: None, iso_fb: None, confirm: None, progress: None };
    app.reload();
    // Começa no disco do sistema, com o volume do sistema escolhido.
    if let Some(sys) = app.model.blocks.iter().find(|b| b.mounted() == Some("/")).cloned() {
        if let Some(t) = sys.part.as_ref().map(|p| p.table.clone()) {
            app.disk = Some(t);
            app.pick = Some(Pick::Block(sys.path.clone()));
        }
    }
    let opts = WindowOptions { title: tr!("Discos").into(), app_id: "glove.Disks".into(), size: (760, 520), fixed: false, min_size: Some((620, 420)) };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-disks: {e}");
    }
}
