//! Criar disco inicializável (UEFI), como o Rufus, sem programas de fora:
//!
//! - ISO híbrida (Linux: Glove OS, Debian, Ubuntu, Fedora…): gravada bloco a bloco
//!   no pendrive; ela mesma já sabe iniciar em UEFI e em BIOS.
//! - Instalador do Windows (a ISO não é híbrida): pendrive GPT com uma partição FAT32
//!   (que todo UEFI lê, inclusive com o Secure Boot ligado) e os arquivos da ISO
//!   copiados. O `install.wim` maior que 4 GB (o limite do FAT32) é dividido em
//!   `install.swm`, `install2.swm`… pelo wimlib, partes que o instalador aceita.
//!
//! Tudo pelo udisks2: o pendrive é aberto, formatado e montado por ele (a senha,
//! quando precisa, é pedida pelo polkit), e a ISO é montada como imagem só de leitura.

use glove_config::tr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::udisks;

/// O limite de tamanho de arquivo do FAT32.
const FAT32_MAX: u64 = 4 * 1024 * 1024 * 1024 - 1;
/// Partes do install.wim dividido (MB), com folga abaixo do limite.
const SPLIT_MB: &str = "3800";
const CHUNK: usize = 4 * 1024 * 1024;
const GPT_BASIC_DATA: &str = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7";

/// O andamento, lido pela janela a cada tique.
#[derive(Default)]
pub struct Progress {
    pub fraction: Mutex<f32>,
    pub phase: Mutex<String>,
    pub cancel: AtomicBool,
}

impl Progress {
    fn set(&self, f: f32, phase: &str) {
        *self.fraction.lock().unwrap() = f.clamp(0.0, 1.0);
        *self.phase.lock().unwrap() = phase.to_string();
    }
    fn frac(&self, f: f32) {
        *self.fraction.lock().unwrap() = f.clamp(0.0, 1.0);
    }
    fn cancelled(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) { Err(tr!("Cancelado.").into()) } else { Ok(()) }
    }
}

/// O que a imagem é, pelo jeito de gravar.
#[derive(Debug, PartialEq)]
pub enum Kind {
    /// Tem a tabela de partições no começo (a assinatura 55 AA do MBR): grava inteira.
    Hybrid,
    /// Não é híbrida: se for o instalador do Windows, o caminho do FAT32.
    Other,
}

pub fn kind(iso: &Path) -> std::io::Result<Kind> {
    let mut f = std::fs::File::open(iso)?;
    let mut head = [0u8; 512];
    f.read_exact(&mut head)?;
    Ok(if head[510] == 0x55 && head[511] == 0xAA { Kind::Hybrid } else { Kind::Other })
}

/// O que a gravação vai fazer, para o pedido de confirmação.
pub struct Target {
    /// O disco inteiro (bloco no udisks) e o nome dele para a confirmação.
    pub disk: String,
    pub device: String,
    pub size: u64,
    /// Volumes montados do disco (desmontados antes de gravar).
    pub mounted: Vec<String>,
}

/// Grava a imagem no disco: escolhe o caminho pelo tipo da imagem.
pub fn write(iso: PathBuf, t: Target, p: Arc<Progress>) -> Result<String, String> {
    let size = std::fs::metadata(&iso).map_err(|e| e.to_string())?.len();
    p.set(0.0, tr!("Desmontando o disco…"));
    for m in &t.mounted {
        udisks::unmount(m)?;
    }
    match kind(&iso).map_err(|e| e.to_string())? {
        Kind::Hybrid => {
            if size > t.size {
                return Err(tr!("A imagem ({}) não cabe no disco ({}).", crate::gb(size), crate::gb(t.size)));
            }
            raw(&iso, size, &t, &p)
        }
        Kind::Other => windows(&iso, &t, &p),
    }
}

/// Bloco a bloco, com o disco aberto pelo udisks.
fn raw(iso: &Path, size: u64, t: &Target, p: &Progress) -> Result<String, String> {
    p.set(0.0, tr!("Gravando a imagem…"));
    let mut out = udisks::open_for_restore(&t.disk)?;
    let mut src = std::fs::File::open(iso).map_err(|e| e.to_string())?;
    copy_image(&mut src, &mut out, size, p)?;
    p.set(0.97, tr!("Terminando de gravar (pode levar um tempo)…"));
    out.sync_all().map_err(|e| e.to_string())?;
    drop(out);
    // Confere: lê de volta o que foi gravado e compara com a imagem.
    verify(iso, size, &t.disk, p)?;
    p.set(1.0, "");
    Ok(tr!("Pronto: {} já inicia o computador.", t.device))
}

/// Copia a imagem para o disco aberto, com o andamento (até 97%) e o Cancelar.
fn copy_image(src: &mut impl Read, out: &mut impl Write, size: u64, p: &Progress) -> Result<u64, String> {
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        p.cancelled()?;
        let n = src.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        p.frac(done as f32 / size.max(1) as f32 * 0.97);
    }
    Ok(done)
}

/// Lê de volta o que foi gravado (pelo udisks) e compara com a imagem.
fn verify(iso: &Path, size: u64, disk: &str, p: &Progress) -> Result<(), String> {
    let mut dev = udisks::open_for_backup(disk)?;
    let mut src = std::fs::File::open(iso).map_err(|e| e.to_string())?;
    p.set(0.0, tr!("Conferindo a gravação…"));
    let (mut a, mut b) = (vec![0u8; CHUNK], vec![0u8; CHUNK]);
    let mut done = 0u64;
    while done < size {
        p.cancelled()?;
        let n = src.read(&mut a).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        dev.read_exact(&mut b[..n]).map_err(|e| e.to_string())?;
        if a[..n] != b[..n] {
            return Err(tr!("A conferência achou diferenças: o pendrive pode estar com defeito.").into());
        }
        done += n as u64;
        p.frac(done as f32 / size.max(1) as f32);
    }
    Ok(())
}

/// O pendrive do Windows: GPT, FAT32, os arquivos e o install.wim dividido.
fn windows(iso: &Path, t: &Target, p: &Progress) -> Result<String, String> {
    p.set(0.0, tr!("Abrindo a imagem…"));
    let lp = udisks::loop_setup(iso)?;
    let res = (|| {
        udisks::wait_fs(&lp)?;
        let src = udisks::mount(&lp)?;
        let src = PathBuf::from(src);
        let r = windows_from(&src, t, p);
        let _ = udisks::unmount(&lp);
        r
    })();
    let _ = udisks::loop_delete(&lp);
    res
}

fn windows_from(src: &Path, t: &Target, p: &Progress) -> Result<String, String> {
    let sources = src.join("sources");
    let wim = sources.join("install.wim");
    let esd = sources.join("install.esd");
    if !wim.is_file() && !esd.is_file() && !src.join("bootmgr.efi").is_file() {
        return Err(tr!("Esta imagem não é híbrida nem um instalador do Windows: não dá para fazer um pendrive que inicie com ela.").into());
    }
    if !src.join("efi/boot/bootx64.efi").is_file() && !src.join("EFI/BOOT/BOOTX64.EFI").is_file() && !has_ci(src, "efi/boot/bootx64.efi") {
        return Err(tr!("Esta imagem do Windows não tem o inicializador UEFI (efi/boot/bootx64.efi).").into());
    }
    let big_wim = wim.is_file() && std::fs::metadata(&wim).map(|m| m.len() > FAT32_MAX).unwrap_or(false);
    if big_wim && !has_cmd("wimlib-imagex") {
        return Err(tr!("O install.wim desta imagem passa de 4 GB e precisa ser dividido: instale o pacote wimtools (sudo apt install wimtools).").into());
    }
    if std::fs::metadata(&esd).is_ok_and(|m| m.len() > FAT32_MAX) {
        return Err(tr!("O install.esd desta imagem passa de 4 GB, o limite do FAT32.").into());
    }
    let total = tree_size(src);
    if total + 64 * 1024 * 1024 > t.size {
        return Err(tr!("Os arquivos da imagem ({}) não cabem no disco ({}).", crate::gb(total), crate::gb(t.size)));
    }
    // O disco: tabela GPT nova e uma partição FAT32 ocupando tudo.
    p.set(0.0, tr!("Preparando o pendrive (GPT e FAT32)…"));
    udisks::format(&t.disk, "gpt", "")?;
    let part = udisks::create_partition(&t.disk, 0, 0, GPT_BASIC_DATA, "vfat", &volume_label(src))?;
    udisks::wait_fs(&part)?;
    let dst = PathBuf::from(udisks::mount(&part)?);
    let r = (|| -> Result<(), String> {
        let mut done = 0u64;
        copy_tree(src, &dst, &|rel: &Path| big_wim && rel == Path::new("sources/install.wim"), total, &mut done, p)?;
        if big_wim {
            split_wim(&wim, &dst.join("sources/install.swm"), total, done, p)?;
        }
        p.set(0.99, tr!("Terminando de gravar (pode levar um tempo)…"));
        sync_dir(&dst);
        Ok(())
    })();
    let un = udisks::unmount(&part);
    r?;
    un?;
    p.set(1.0, "");
    Ok(tr!("Pronto: {} instala o Windows em computadores com UEFI (inclusive com o Secure Boot ligado).", t.device))
}

/// Nome do pendrive: o da ISO (o udisks lê como rótulo), em até 11 letras do FAT.
fn volume_label(src: &Path) -> String {
    let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let clean: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(11).collect::<String>().to_uppercase();
    if clean.is_empty() { "WINDOWS".into() } else { clean }
}

fn has_ci(root: &Path, rel: &str) -> bool {
    let mut cur = root.to_path_buf();
    for part in rel.split('/') {
        let Ok(rd) = std::fs::read_dir(&cur) else { return false };
        let Some(next) = rd.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part)) else { return false };
        cur = next.path();
    }
    true
}

fn has_cmd(c: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(c).is_file()))
}

fn tree_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten()
        .map(|e| {
            let m = e.metadata().ok();
            match m {
                Some(m) if m.is_dir() => tree_size(&e.path()),
                Some(m) => m.len(),
                None => 0,
            }
        })
        .sum()
}

/// Copia a árvore (menos o que `skip` disser, pelo caminho relativo).
fn copy_tree(src: &Path, dst: &Path, skip: &dyn Fn(&Path) -> bool, total: u64, done: &mut u64, p: &Progress) -> Result<(), String> {
    fn walk(root: &Path, dir: &Path, dst: &Path, skip: &dyn Fn(&Path) -> bool, total: u64, done: &mut u64, p: &Progress) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            p.cancelled()?;
            let path = e.path();
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let out = dst.join(&rel);
            let m = e.metadata().map_err(|e| e.to_string())?;
            if m.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                walk(root, &path, dst, skip, total, done, p)?;
                continue;
            }
            if skip(&rel) {
                continue;
            }
            *p.phase.lock().unwrap() = tr!("Copiando {}…", rel.display());
            let mut a = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut b = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", rel.display()))?;
            let mut buf = vec![0u8; CHUNK];
            loop {
                p.cancelled()?;
                let n = a.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                b.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", rel.display()))?;
                *done += n as u64;
                p.frac(*done as f32 / total.max(1) as f32 * 0.98);
            }
        }
        Ok(())
    }
    walk(src, src, dst, skip, total, done, p)
}

/// Divide o install.wim em partes de 3,8 GB com o wimlib (o andamento sai do tamanho
/// das partes já gravadas).
fn split_wim(wim: &Path, out: &Path, total: u64, before: u64, p: &Progress) -> Result<(), String> {
    *p.phase.lock().unwrap() = tr!("Dividindo o install.wim em partes (limite de 4 GB do FAT32)…").into();
    let mut c = std::process::Command::new("wimlib-imagex")
        .arg("split")
        .arg(wim)
        .arg(out)
        .arg(SPLIT_MB)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("wimlib-imagex: {e}"))?;
    let dir = out.parent().unwrap_or(Path::new("."));
    loop {
        if p.cancel.load(Ordering::Relaxed) {
            let _ = c.kill();
            let _ = c.wait();
            return Err(tr!("Cancelado.").into());
        }
        if let Ok(Some(st)) = c.try_wait() {
            if st.success() {
                return Ok(());
            }
            let mut err = String::new();
            if let Some(mut e) = c.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Err(format!("wimlib-imagex: {}", err.trim()));
        }
        let parts: u64 = std::fs::read_dir(dir).map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".swm")).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0);
        p.frac((before + parts) as f32 / total.max(1) as f32 * 0.98);
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

fn sync_dir(dir: &Path) {
    if let Ok(f) = std::fs::File::open(dir) {
        use std::os::fd::AsRawFd;
        // SAFETY: o descritor é do arquivo aberto acima.
        unsafe {
            libc::syncfs(f.as_raw_fd());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_a_imagem_inteira_e_cancela() {
        let img: Vec<u8> = (0..(CHUNK * 2 + 123)).map(|i| (i % 251) as u8).collect();
        let p = Progress::default();
        let mut out = Vec::new();
        let n = copy_image(&mut std::io::Cursor::new(&img), &mut out, img.len() as u64, &p).unwrap();
        assert_eq!((n as usize, out == img), (img.len(), true));
        assert!(*p.fraction.lock().unwrap() > 0.96);
        p.cancel.store(true, Ordering::Relaxed);
        assert!(copy_image(&mut std::io::Cursor::new(&img), &mut Vec::new(), 1, &p).is_err());
    }

    #[test]
    fn reconhece_iso_hibrida() {
        let dir = std::env::temp_dir().join(format!("glove-gravar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut img = vec![0u8; 4096];
        let f = dir.join("windows.iso");
        std::fs::write(&f, &img).unwrap();
        assert_eq!(kind(&f).unwrap(), Kind::Other);
        img[510] = 0x55;
        img[511] = 0xAA;
        let f = dir.join("linux.iso");
        std::fs::write(&f, &img).unwrap();
        assert_eq!(kind(&f).unwrap(), Kind::Hybrid);
        // O rótulo do FAT: até 11 letras, maiúsculas, sem espaço.
        assert_eq!(volume_label(Path::new("/media/x/CCCOMA_X64FRE_PT-BR_DV9")), "CCCOMA_X64F");
        assert_eq!(volume_label(Path::new("/")), "WINDOWS");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
