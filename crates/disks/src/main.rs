//! Discos (como o Utilitário de Disco da era, com o que o gnome-disks faz): à
//! esquerda os discos (aparelhos e imagens), à direita o disco escolhido com o
//! mapa dos volumes (partições e espaço livre) e o volume escolhido.
//!
//! Tudo pelo udisks2 (`udisks.rs`); o que muda o disco pede a senha pelo polkit
//! (a janela do `glove-senha`). As operações rodam numa linha à parte, para a
//! janela não travar enquanto a senha é pedida.

mod dialogos;
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
    /// A folha da operação aberta (formatar, criar partição…).
    dialog: Option<dialogos::Dialog>,
    /// Apagar a partição: pedindo confirmação.
    delete_confirm: Option<String>,
    /// Espaço usado de cada volume (caminho no udisks → usado, total), medido montado.
    usage: Arc<std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>>>,
    /// Imagem do disco: onde salvar, qual restaurar e a confirmação.
    img_save_fb: Option<FileBrowser>,
    img_open_fb: Option<FileBrowser>,
    restore_confirm: Option<PathBuf>,
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

/// A saúde numa palavra.
fn health_word(h: &udisks::Health) -> String {
    if h.failing {
        tr!("Falhando — copie os seus arquivos já").into()
    } else if h.attrs_failing > 0 || h.bad_sectors.is_some_and(|n| n > 0) {
        tr!("Boa, com avisos").into()
    } else {
        tr!("Boa").into()
    }
}

/// Nome do volume: no FAT, até 11 letras maiúsculas (o limite dele).
fn fat_label(fs: &str, name: &str) -> String {
    if fs == "vfat" { name.to_uppercase().chars().filter(|c| c.is_ascii_alphanumeric() || " _-".contains(*c)).take(11).collect() } else { name.to_string() }
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
        // O uso dos volumes montados (os desmontados ficam com a última medida).
        if let Ok(mut u) = self.usage.lock() {
            for b in &self.model.blocks {
                if let Some(at) = b.mounted() {
                    if let Some((t, f)) = statvfs(Path::new(at)) {
                        u.insert(b.path.clone(), (t - f, t));
                    }
                }
            }
        }
        // O disco escolhido sumiu (pendrive tirado): volta ao primeiro.
        let disks = self.model.disks();
        if !self.disk.as_ref().is_some_and(|d| disks.iter().any(|x| &self.model.blocks[x.block].path == d)) {
            self.disk = disks.first().map(|d| self.model.blocks[d.block].path.clone());
            self.pick = None;
        }
        match self.pick.clone() {
            Some(Pick::Block(p)) if self.model.block(&p).is_none() => self.pick = None,
            // O espaço livre escolhido virou partição: escolhe a partição que começa ali.
            Some(Pick::Free(off)) => {
                if let Some(di) = self.disk_index() {
                    let slots: Vec<Slot> = self.model.layout(di).into_iter().chain(self.ext_layout()).collect();
                    if !slots.iter().any(|s| matches!(s, Slot::Free { offset, .. } if *offset == off)) {
                        self.pick = self.model.parts_of(di).into_iter().find(|&i| self.model.blocks[i].part.as_ref().is_some_and(|p| p.offset.abs_diff(off) < 2 * 1024 * 1024)).map(|i| Pick::Block(self.model.blocks[i].path.clone()));
                    }
                }
            }
            _ => {}
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
            return match &b.part {
                Some(p) => tr!("Partição {}", p.number),
                None => tr!("Vazio").into(),
            };
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

    /// O volume está em uso pelo sistema (/, /boot, /home…): não se mexe nele.
    fn is_system_volume(&self, bi: usize) -> bool {
        self.model.blocks[bi].mounts.as_ref().is_some_and(|m| m.iter().any(|p| SYSTEM_MOUNTS.contains(&p.as_str())))
    }

    /// O volume escolhido pode ser formatado, apagado ou redimensionado.
    fn can_change_volume(&self) -> Option<usize> {
        let bi = self.picked_block()?;
        let b = &self.model.blocks[bi];
        (!b.read_only && !self.is_system_volume(bi) && !b.part.as_ref().is_some_and(|p| p.container)).then_some(bi)
    }

    fn table_is_gpt(&self) -> bool {
        self.disk_index().is_some_and(|di| self.model.blocks[di].table.as_deref() == Some("gpt"))
    }

    fn open_new_part(&mut self) {
        let (Some(di), Some(Pick::Free(off))) = (self.disk_index(), self.pick.clone()) else { return };
        let all: Vec<Slot> = self.model.layout(di).into_iter().chain(self.ext_layout()).collect();
        let Some(max) = all.iter().find_map(|s| match s { Slot::Free { offset, size } if *offset == off => Some(*size), _ => None }) else { return };
        let gpt = self.table_is_gpt();
        self.dialog = Some(dialogos::Dialog::NewPart { offset: off, max, gpt, table: self.model.blocks[di].path.clone(), size: dialogos::gb_text(max), fs: 0, name: String::new() });
    }

    fn open_format(&mut self) {
        let Some(bi) = self.can_change_volume() else { return };
        let b = &self.model.blocks[bi];
        let fs = dialogos::FORMATS.iter().position(|f| f.0 == b.fstype).unwrap_or(0);
        self.dialog = Some(dialogos::Dialog::Format { block: b.path.clone(), what: self.volume_name(bi), fs, name: b.label.clone() });
    }

    fn open_format_disk(&mut self) {
        let Some(di) = self.disk_index().filter(|_| self.can_overwrite()) else { return };
        self.dialog = Some(dialogos::Dialog::FormatDisk { block: self.model.blocks[di].path.clone(), what: self.disk_name(di), scheme: 0, zero: false });
    }

    fn open_edit(&mut self) {
        let Some(bi) = self.picked_block() else { return };
        let b = &self.model.blocks[bi];
        let Some(p) = &b.part else { return };
        let gpt = self.table_is_gpt();
        let kind = dialogos::TYPES.iter().position(|t| if gpt { t.0 == p.ptype.to_ascii_lowercase() } else { t.1 == p.ptype.to_ascii_lowercase() }).unwrap_or(0);
        let name = if b.has_fs() { b.label.clone() } else { p.name.clone() };
        self.dialog = Some(dialogos::Dialog::Edit { block: b.path.clone(), gpt: gpt || b.has_fs(), kind, name });
    }

    /// Redimensionar: o máximo é a partição mais o espaço livre logo depois dela; o
    /// mínimo, o que os arquivos ocupam (medido montando só para leitura, se preciso).
    fn open_resize(&mut self) {
        let Some(bi) = self.can_change_volume() else { return };
        let b = self.model.blocks[bi].clone();
        let Some(p) = b.part.clone() else { return };
        let (grow_ok, shrink_ok, _) = if b.fstype.is_empty() { (true, true, true) } else { udisks::can_resize(&b.fstype) };
        if !grow_ok {
            self.message = Some(tr!("O Discos não sabe redimensionar {}.", udisks::fs_name(&b.fstype)));
            return;
        }
        let Some(di) = self.disk_index() else { return };
        let slots: Vec<Slot> = if p.contained { self.ext_layout() } else { self.model.layout(di) };
        let end = p.offset + b.size;
        let free_after = slots.iter().find_map(|s| match s { Slot::Free { offset, size } if offset.abs_diff(end) < 2 * 1024 * 1024 => Some(*size + (*offset - end.min(*offset))), _ => None }).unwrap_or(0);
        let max = b.size + free_after;
        // Montado: o mínimo é o que os arquivos ocupam (com folga). Desmontado, sem montar
        // só para medir (o Glove abriria a janela do volume): o programa de
        // redimensionar recusa, com segurança, um tamanho menor que os arquivos.
        let used = b.mounted().and_then(|m| statvfs(Path::new(m))).map(|(t, f)| t - f).unwrap_or(0);
        let min = if shrink_ok { ((used as f64 * 1.1) as u64 + 100_000_000).max(64_000_000).min(b.size) } else { b.size };
        if max <= min + 1_000_000 && !shrink_ok {
            self.message = Some(tr!("Não há espaço livre logo depois deste volume para aumentá-lo, e {} não pode ser diminuído.", udisks::fs_name(&b.fstype)));
            return;
        }
        self.dialog = Some(dialogos::Dialog::Resize { block: b.path.clone(), fs: b.fstype.clone(), min, max, size: dialogos::gb_text(b.size) });
    }

    /// Faz a operação confirmada numa linha à parte.
    fn run(&mut self, d: dialogos::Do) {
        use dialogos::Do;
        let unmount_all = |blocks: Vec<String>| -> Result<(), String> {
            for b in blocks {
                udisks::unmount(&b)?;
            }
            Ok(())
        };
        match d {
            Do::NewPart { table, offset, size, ptype, fs, name } => {
                let label = fat_label(&fs, &name);
                self.start(tr!("Criando a partição…").into(), move || udisks::create_partition(&table, offset, size, &ptype, &fs, &label).map(|_| tr!("Partição criada.").to_string()));
            }
            Do::Format { block, fs, name } => {
                let mounted: Vec<String> = self.model.block(&block).filter(|&i| self.model.blocks[i].mounted().is_some()).map(|_| block.clone()).into_iter().collect();
                let label = fat_label(&fs, &name);
                self.start(tr!("Formatando…").into(), move || {
                    unmount_all(mounted)?;
                    udisks::format(&block, &fs, &label).map(|_| tr!("Volume formatado.").to_string())
                });
            }
            Do::FormatDisk { block, scheme, zero } => {
                let Some(di) = self.model.block(&block) else { return };
                let mounted: Vec<String> = std::iter::once(di).chain(self.model.parts_of(di)).filter(|&i| self.model.blocks[i].mounted().is_some()).map(|i| self.model.blocks[i].path.clone()).collect();
                self.pick = None;
                self.start(tr!("Formatando o disco…").into(), move || {
                    unmount_all(mounted)?;
                    udisks::format_with(&block, &scheme, "", zero).map(|_| tr!("Disco formatado.").to_string())
                });
            }
            Do::Edit { block, ptype, name } => {
                let gpt = self.table_is_gpt();
                let b = self.model.block(&block).map(|i| self.model.blocks[i].clone());
                let has_fs = b.as_ref().is_some_and(|b| b.has_fs());
                let label = fat_label(b.as_ref().map(|b| b.fstype.as_str()).unwrap_or(""), &name);
                let changed_label = b.as_ref().is_some_and(|b| b.label != label);
                self.start(tr!("Mudando a partição…").into(), move || {
                    let (g, m) = ptype.split_once('|').unwrap_or((&ptype, &ptype));
                    udisks::set_partition_type(&block, if gpt { g } else { m })?;
                    // O nome: do volume (o que aparece na mesa), ou da partição GPT sem volume.
                    if has_fs && changed_label {
                        udisks::set_label(&block, &label)?;
                    } else if gpt && !has_fs {
                        udisks::set_partition_name(&block, &name)?;
                    }
                    Ok(tr!("Partição mudada.").to_string())
                });
            }
            Do::SelfTest { drive, nvme } => {
                self.start(tr!("Começando o teste rápido do disco…").into(), move || {
                    udisks::selftest(&drive, nvme)?;
                    udisks::smart_update(&drive, nvme);
                    Ok(tr!("Teste rápido começou (uns 2 minutos); o resultado aparece em Saúde do Disco.").to_string())
                });
            }
            Do::Resize { block, fs, size } => {
                let Some(bi) = self.model.block(&block) else { return };
                let old = self.model.blocks[bi].size;
                let mounted = self.model.blocks[bi].mounted().is_some();
                self.start(tr!("Redimensionando…").into(), move || {
                    if mounted {
                        udisks::unmount(&block)?;
                    }
                    const MIB: u64 = 1024 * 1024;
                    if size < old {
                        // Diminuir: confere o sistema de arquivos, encolhe ele um pouco
                        // abaixo do novo tamanho, encolhe a partição e ocupa o que sobrou.
                        if !fs.is_empty() {
                            if !udisks::check(&block, true)? {
                                return Err(tr!("O sistema de arquivos tem erros que não puderam ser consertados.").into());
                            }
                            udisks::resize_fs(&block, size - 4 * MIB)?;
                        }
                        udisks::resize_partition(&block, size)?;
                        if !fs.is_empty() {
                            udisks::resize_fs(&block, 0)?;
                        }
                    } else {
                        udisks::resize_partition(&block, size)?;
                        if !fs.is_empty() {
                            udisks::resize_fs(&block, 0)?;
                        }
                    }
                    Ok(tr!("Volume redimensionado.").to_string())
                });
            }
        }
    }

    /// Mede o espaço usado de um volume desmontado: monta só para leitura, em
    /// silêncio (sem ícone na mesa), mede e desmonta.
    fn measure(&mut self) {
        let Some(bi) = self.picked_block() else { return };
        let b = self.model.blocks[bi].clone();
        let usage = self.usage.clone();
        self.start(tr!("Medindo o espaço usado…").into(), move || {
            let at = udisks::mount_quiet(&b.path, &b.device)?;
            let r = statvfs(Path::new(&at));
            // Logo depois de montar, o udisks ainda pode estar ocupado: tenta de novo.
            let mut un = udisks::unmount(&b.path);
            for _ in 0..10 {
                if un.is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
                un = udisks::unmount(&b.path);
            }
            un?;
            let (t, f) = r.ok_or_else(|| tr!("Não deu para medir.").to_string())?;
            usage.lock().map_err(|e| e.to_string())?.insert(b.path.clone(), (t - f, t));
            Ok(tr!("{} usados de {}.", gb(t - f), gb(t)))
        });
    }

    /// Verificar o sistema de arquivos e, com erros, consertar.
    fn check(&mut self) {
        let Some(bi) = self.can_change_volume() else { return };
        let b = self.model.blocks[bi].clone();
        let name = self.volume_name(bi);
        self.start(tr!("Verificando “{}”…", name), move || {
            if b.mounted().is_some() {
                udisks::unmount(&b.path)?;
            }
            if udisks::check(&b.path, false)? {
                return Ok(tr!("“{}” está sem problemas.", name));
            }
            if udisks::check(&b.path, true)? {
                Ok(tr!("“{}” tinha erros e foi consertado.", name))
            } else {
                Err(tr!("“{}” tem erros que não puderam ser consertados. Copie os arquivos importantes.", name))
            }
        });
    }

    fn open_health(&mut self) {
        let Some(di) = self.disk_index() else { return };
        let Some(d) = self.drive_of(di).cloned() else { return };
        let Some(h) = d.health.clone() else { return };
        let mut rows: Vec<(String, String)> = vec![(tr!("Estado").into(), health_word(&h))];
        if let Some(t) = h.temp_c {
            rows.push((tr!("Temperatura").into(), format!("{t:.0} °C")));
        }
        if let Some(hrs) = h.power_on_hours {
            rows.push((tr!("Tempo ligado").into(), tr!("{} horas ({} dias)", hrs, hrs / 24)));
        }
        if let Some(n) = h.bad_sectors {
            rows.push((tr!("Setores ruins").into(), n.to_string()));
        }
        if h.nvme {
            if let Some((used, spare)) = udisks::nvme_wear(&d.path) {
                rows.push((tr!("Desgaste").into(), tr!("{}% da vida útil usada (reserva {}%)", used, spare)));
            }
        }
        for w in &h.warnings {
            rows.push((tr!("Aviso").into(), w.clone()));
        }
        let test = match (h.selftest.as_str(), h.selftest_remaining) {
            ("inprogress", r) if r >= 0 => tr!("em andamento ({}% faltando)", r),
            ("success", _) => tr!("passou").into(),
            ("", _) => tr!("nunca feito").into(),
            (o, _) => o.to_string(),
        };
        rows.push((tr!("Último teste").into(), test));
        self.dialog = Some(dialogos::Dialog::Health { drive: d.path.clone(), nvme: h.nvme, title: self.disk_name(di), rows });
    }

    fn ask_backup(&mut self) {
        let Some(di) = self.disk_index() else { return };
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into());
        let name = format!("{}.img", self.disk_name(di).replace(['/', ' '], "-"));
        self.img_save_fb = Some(FileBrowser::save(Some(home), &name));
    }

    fn start_backup(&mut self, out: PathBuf) {
        let Some(di) = self.disk_index() else { return };
        let b = self.model.blocks[di].clone();
        let p = Arc::new(gravar::Progress::default());
        self.progress = Some(p.clone());
        self.start(tr!("Criando a imagem de {}…", b.device), move || gravar::backup(b.path, b.size, out, p));
    }

    fn ask_restore(&mut self) {
        if !self.can_overwrite() {
            return;
        }
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| "/".into());
        self.img_open_fb = Some(FileBrowser::open(Some(home), &["img", "iso", "raw", "bin"]));
    }

    fn start_restore(&mut self, img: PathBuf) {
        let Some(di) = self.disk_index() else { return };
        let b = self.model.blocks[di].clone();
        let mounted: Vec<String> = std::iter::once(di).chain(self.model.parts_of(di)).filter(|&i| self.model.blocks[i].mounted().is_some()).map(|i| self.model.blocks[i].path.clone()).collect();
        let t = gravar::Target { disk: b.path.clone(), device: b.device.clone(), size: b.size, mounted };
        let p = Arc::new(gravar::Progress::default());
        self.progress = Some(p.clone());
        self.start(tr!("Restaurando a imagem em {}…", b.device), move || gravar::restore(img, t, p));
    }

    fn delete_part(&mut self, block: String) {
        let Some(bi) = self.model.block(&block) else { return };
        let mounted = self.model.blocks[bi].mounted().is_some();
        let name = self.volume_name(bi);
        self.pick = None;
        self.start(tr!("Apagando “{}”…", name), move || {
            if mounted {
                udisks::unmount(&block)?;
            }
            udisks::delete_partition(&block).map(|_| tr!("“{}” apagada.", name))
        });
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
        // Saúde do disco (SMART/NVMe), com o detalhe na folha "Saúde…".
        if let Some(h) = self.drive_of(di).and_then(|d| d.health.clone()) {
            let mut t = tr!("Saúde: {}", health_word(&h));
            if let Some(c) = h.temp_c {
                t += &format!("  ·  {c:.0} °C");
            }
            let color = if h.failing { rgba(190, 30, 30, 255) } else if h.attrs_failing > 0 || h.bad_sectors.is_some_and(|n| n > 0) { rgba(170, 110, 0, 255) } else { rgba(40, 120, 50, 255) };
            ui.fonts.draw(ui.c, &t, r.x + 76.0, r.y + 66.0, 11.0, true, color);
            let tw = ui.text_width(&t, 11.0, true);
            let mut open = false;
            ui.area(Rect::new(r.x + 84.0 + tw, r.y + 54.0, 90.0, 20.0), |ui| open = ui.small_button(tr!("Detalhes…")));
            if open {
                self.open_health();
            }
        }
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
            // Faixa embaixo do pedaço: quanto do volume está ocupado (quando se sabe).
            if let Slot::Part(i) = slot {
                let used = self.usage.lock().ok().and_then(|u| u.get(&self.model.blocks[*i].path).copied());
                if let Some((u, t)) = used.filter(|(_, t)| *t > 0) {
                    let f = (u as f32 / t as f32).clamp(0.0, 1.0);
                    ui.c.fill_rect(seg.x + 4.0, seg.y + seg.h - 8.0, seg.w - 8.0, 4.0, &solid(rgba(255, 255, 255, 110)));
                    ui.c.fill_rect(seg.x + 4.0, seg.y + seg.h - 8.0, (seg.w - 8.0) * f, 4.0, &solid(if f > 0.9 { rgba(230, 70, 60, 255) } else { rgba(30, 40, 80, 200) }));
                }
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
        let mut measure_btn = false;
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
                if let (Some(_), Some(di)) = (&b.part, self.disk_index()) {
                    let total = self.model.blocks[di].size.max(1);
                    rows.push((tr!("Do disco").into(), tr!("{}% do disco", format!("{:.1}", b.size as f64 * 100.0 / total as f64).replace('.', glove_config::virgula()))));
                }
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
                    usage = self.usage.lock().ok().and_then(|u| u.get(&b.path).copied()).map(|(u, t)| (t, t - u));
                    if usage.is_none() {
                        measure_btn = !b.part.as_ref().is_some_and(|p| p.container);
                    }
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
            let bar = Rect::new(r.x + 140.0, y, (r.w - 180.0).clamp(80.0, 320.0), 12.0);
            ui.c.fill(rrect(bar.x, bar.y, bar.w, bar.h, 6.0), &vgrad(bar.y, bar.y + bar.h, &[(0.0, rgba(160, 160, 166, 255)), (1.0, rgba(225, 225, 230, 255))]));
            let used = if total > 0 { 1.0 - free as f32 / total as f32 } else { 0.0 };
            if used > 0.02 {
                gel_rect(ui.c, bar.x, bar.y, (bar.w * used).max(bar.h), bar.h, 6.0, if used > 0.9 { rgba(230, 70, 60, 255) } else { ui.theme.accent });
            }
            let pct = if total > 0 { (total - free) as f64 * 100.0 / total as f64 } else { 0.0 };
            let t = tr!("{} usados ({}%), {} livres", gb(total - free), format!("{pct:.0}"), gb(free));
            ui.fonts.draw(ui.c, &t, bar.x, bar.y + 28.0, 11.0, false, rgba(60, 60, 66, 255));
        } else if measure_btn {
            let mut m = false;
            ui.area(Rect::new(r.x + 140.0, y, 160.0, 22.0), |ui| {
                ui.set_active(self.job.is_none());
                m = ui.small_button(tr!("Medir o Uso"));
                ui.set_active(true);
            });
            if m {
                self.measure();
            }
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
        let (mut mount, mut unmount, mut open, mut eject, mut chk) = (false, false, false, false, false);
        ui.area(Rect::new(r.x + 18.0, y + 4.0, r.w - 36.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_active(can_mount && !busy);
                mount = ui.button(tr!("Montar"));
                ui.set_active(mounted && !busy);
                unmount = ui.button(tr!("Desmontar"));
                ui.set_active(mounted);
                open = ui.button(tr!("Abrir"));
                ui.set_active(self.can_change_volume().is_some() && !busy && picked.as_ref().is_some_and(|b| b.has_fs()));
                chk = ui.button(tr!("Verificar"));
                ui.set_active(true);
            });
        });
        let (mut boot, mut fdisk) = (false, false);
        ui.area(Rect::new(r.x + r.w - 410.0, r.y + 76.0, 394.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                ui.set_active(self.can_overwrite() && !busy);
                fdisk = ui.button(tr!("Formatar Disco…"));
                boot = ui.button(tr!("Disco Inicializável…"));
                ui.set_active(self.can_eject() && !busy);
                eject = ui.button(tr!("Ejetar"));
                ui.set_active(true);
            });
        });
        if boot {
            self.ask_image();
        }
        if fdisk {
            self.open_format_disk();
        }
        // Segunda fileira: o que dá para fazer com o pedaço escolhido.
        let free = matches!(self.pick, Some(Pick::Free(_)));
        let changeable = self.can_change_volume().is_some();
        let is_part = picked.as_ref().is_some_and(|b| b.part.as_ref().is_some_and(|p| !p.container));
        let (mut newp, mut fmt, mut edit, mut resize, mut del) = (false, false, false, false, false);
        ui.area(Rect::new(r.x + 18.0, y + 34.0, r.w - 36.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                if free {
                    ui.set_active(!busy);
                    newp = ui.button(tr!("Criar Partição…"));
                } else {
                    ui.set_active(changeable && !busy);
                    fmt = ui.button(tr!("Formatar…"));
                    ui.set_active(is_part && changeable && !busy);
                    resize = ui.button(tr!("Redimensionar…"));
                    ui.set_active(is_part && !busy && picked.as_ref().is_some_and(|b| !self.model.block(&b.path).is_some_and(|i| self.is_system_volume(i))));
                    edit = ui.button(tr!("Editar…"));
                    ui.set_active(is_part && changeable && !busy);
                    del = ui.button(tr!("Apagar"));

                }
                ui.set_active(true);
            });
        });
        if newp {
            self.open_new_part();
        }
        if fmt {
            self.open_format();
        }
        if edit {
            self.open_edit();
        }
        if resize {
            self.open_resize();
        }
        if del {
            self.delete_confirm = self.picked_block().map(|i| self.model.blocks[i].path.clone());
        }
        if chk {
            self.check();
        }
        y += 74.0;
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
        // Folha ou aviso aberto: a janela de baixo não recebe cliques nem atalhos
        // (um clique no campo da folha não pode cair num botão atrás dela).
        let modal = self.dialog.is_some() || self.confirm.is_some() || self.delete_confirm.is_some() || self.iso_fb.is_some() || self.img_save_fb.is_some() || self.img_open_fb.is_some() || self.restore_confirm.is_some();
        let mut acts: Vec<String> = if modal { Vec::new() } else { ui.menu_actions().to_vec() };
        for k in ui.keys().iter().filter(|_| !modal) {
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
                "format-disk" => self.open_format_disk(),
                "check" => self.check(),
                "health" => self.open_health(),
                "backup" => self.ask_backup(),
                "restore" => self.ask_restore(),
                "format" => self.open_format(),
                "new-part" => self.open_new_part(),
                "edit-part" => self.open_edit(),
                "resize" => self.open_resize(),
                "delete-part" => self.delete_confirm = self.can_change_volume().map(|i| self.model.blocks[i].path.clone()),
                "close" => ui.close = true,
                _ => {}
            }
        }
        let foot = 24.0;
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(244, 244, 246, 255)));
        ui.set_enabled(!modal);
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
        ui.set_enabled(true);
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
        if let Some(d) = &mut self.dialog {
            match dialogos::sheet(ui, d) {
                dialogos::Answer::Waiting => {}
                dialogos::Answer::Cancel => self.dialog = None,
                dialogos::Answer::Go(g) => {
                    self.dialog = None;
                    self.run(g);
                }
            }
            ui.redraw = true;
        }
        if let Some(block) = self.delete_confirm.clone() {
            let name = self.model.block(&block).map(|i| self.volume_name(i)).unwrap_or_default();
            match ui.alert(AlertKind::Caution, &tr!("Apagar a partição “{}”?", name), tr!("Todos os arquivos dela serão perdidos. O espaço fica livre para uma partição nova."), &[tr!("Cancelar"), tr!("Apagar")]) {
                Some(1) => {
                    self.delete_confirm = None;
                    self.delete_part(block);
                }
                Some(_) => self.delete_confirm = None,
                None => {}
            }
        }
        if let Some(fb) = &mut self.img_save_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.img_save_fb = None;
                if let Choice::Picked(p) = c {
                    self.start_backup(p);
                }
                ui.redraw = true;
            }
        }
        if let Some(fb) = &mut self.img_open_fb {
            if let Some(c) = ui.file_sheet(fb) {
                self.img_open_fb = None;
                if let Choice::Picked(p) = c {
                    self.restore_confirm = Some(p);
                }
                ui.redraw = true;
            }
        }
        if let Some(img) = self.restore_confirm.clone() {
            let disk = self.disk_index().map(|d| format!("{} ({}, {})", self.disk_name(d), self.model.blocks[d].device, gb(self.model.blocks[d].size))).unwrap_or_default();
            let name = img.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match ui.alert(AlertKind::Caution, tr!("Apagar o disco e restaurar a imagem?"), &tr!("Tudo o que está em {} será trocado pelo conteúdo de “{}”, bloco a bloco.", disk, name), &[tr!("Cancelar"), tr!("Apagar e Restaurar")]) {
                Some(1) => {
                    self.restore_confirm = None;
                    self.start_restore(img);
                }
                Some(_) => self.restore_confirm = None,
                None => {}
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
                    MenuItem::new("format-disk", tr!("Formatar Disco…"), "").enabled(self.can_overwrite() && !busy),
                    MenuItem::new("bootable", tr!("Criar Disco Inicializável…"), "").enabled(self.can_overwrite() && !busy),
                    MenuItem::Separator,
                    MenuItem::new("backup", tr!("Criar Imagem do Disco…"), "").enabled(self.disk_index().is_some() && !busy),
                    MenuItem::new("restore", tr!("Restaurar Imagem no Disco…"), "").enabled(self.can_overwrite() && !busy),
                    MenuItem::new("health", tr!("Saúde do Disco…"), "").enabled(self.disk_index().and_then(|d| self.drive_of(d)).is_some_and(|d| d.health.is_some())),
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
                    MenuItem::Separator,
                    MenuItem::new("new-part", tr!("Criar Partição…"), "").enabled(!busy && matches!(self.pick, Some(Pick::Free(_)))),
                    MenuItem::new("format", tr!("Formatar…"), "").enabled(!busy && self.can_change_volume().is_some()),
                    MenuItem::new("edit-part", tr!("Editar…"), "").enabled(!busy && b.is_some_and(|b| b.part.is_some())),
                    MenuItem::new("resize", tr!("Redimensionar…"), "").enabled(!busy && self.can_change_volume().is_some_and(|i| self.model.blocks[i].part.is_some())),
                    MenuItem::new("check", tr!("Verificar e Consertar"), "").enabled(!busy && self.can_change_volume().is_some_and(|i| self.model.blocks[i].has_fs())),
                    MenuItem::new("delete-part", tr!("Apagar Partição"), "").enabled(!busy && self.can_change_volume().is_some_and(|i| self.model.blocks[i].part.is_some())),
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
    let mut app = Disks { conn: None, model, disk: None, pick: None, message: None, job: None, iso_fb: None, confirm: None, progress: None, dialog: None, delete_confirm: None, usage: Default::default(), img_save_fb: None, img_open_fb: None, restore_confirm: None };
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
    let mut app = Disks { conn: None, model: Model::default(), disk: None, pick: None, message: None, job: None, iso_fb: None, confirm: None, progress: None, dialog: None, delete_confirm: None, usage: Default::default(), img_save_fb: None, img_open_fb: None, restore_confirm: None };
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
