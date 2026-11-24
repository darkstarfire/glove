//! O motor das cópias: cada cópia é uma pasta com a data
//! (`<destino>/Glove Backups/<computador>/2026-11-07-191500`) com a pasta pessoal
//! inteira. Arquivos iguais aos da cópia anterior (mesmo tamanho e data) viram
//! links fixos para ela: cada cópia parece completa, mas só o que mudou ocupa
//! espaço. Uma cópia só aparece depois de pronta (até lá tem o fim `.in-progress`).
//!
//! Ficam: todas as das últimas 24 horas, uma por dia no último mês e uma por
//! semana antes disso; com o disco quase cheio, as mais velhas vão embora.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Preferências (`~/.config/glove/backup.toml`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Prefs {
    /// Pasta do disco de cópias (None = ainda não escolhido).
    pub dest: Option<PathBuf>,
    /// Cópia automática a cada hora (o Glove chama `glove-backup --auto`).
    pub auto: bool,
    /// Fora da cópia: nomes de pasta (em qualquer lugar) ou caminhos dentro da pasta pessoal.
    pub exclude: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dest: None,
            auto: true,
            exclude: [".cache", "node_modules", ".npm", ".rustup", ".local/share/Trash", ".cargo/registry", ".local/share/Steam", ".var/app"].map(String::from).to_vec(),
        }
    }
}

impl Prefs {
    fn path() -> PathBuf {
        glove_config::dir().join("backup.toml")
    }
    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        if let Ok(t) = toml::to_string(self) {
            let _ = std::fs::create_dir_all(glove_config::dir());
            let _ = std::fs::write(Self::path(), t);
        }
    }
}

/// Gives today's name to the preferences an older version stored in `dir` (the Glove
/// configuration folder) as `copias.toml`. With nothing old there, nothing is touched.
fn migrate_old_names_in(dir: &Path) {
    glove_config::move_stored(&dir.join("copias.toml"), &dir.join("backup.toml"));
}

pub fn migrate_old_names() {
    migrate_old_names_in(&glove_config::dir());
}

/// End of the name of a backup still being made (`<stamp>.in-progress`), and the one an
/// older version gave it: what either left on the disk is a backup that did not finish.
const UNFINISHED: &str = ".in-progress";
const OLD_UNFINISHED: &str = ".andamento";

/// The lock of the hourly backup, and the name an older version gave it.
const LOCK: &str = ".in-progress.lock";
const OLD_LOCK: &str = ".andamento.lock";

/// A lock older than this was left behind by a backup that was cut short.
const STALE_LOCK: Duration = Duration::from_secs(6 * 3600);

/// Whether `name` is the folder of a backup that did not finish.
fn is_unfinished(name: &str) -> bool {
    name.ends_with(UNFINISHED) || name.ends_with(OLD_UNFINISHED)
}

/// Removes from `root` what a backup that was cut short left behind.
fn remove_unfinished(root: &Path) {
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            if is_unfinished(&e.file_name().to_string_lossy()) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

fn is_stale(lock: &Path) -> bool {
    lock.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e > STALE_LOCK)
}

/// Takes the lock of the hourly backup in `root` (one backup at a time) and says where
/// it is, to remove at the end. None when another backup holds it. A lock older than
/// `STALE_LOCK` was left behind and does not hold anything.
///
/// The lock of an older version counts the same way: while fresh, a backup of that
/// version may still be running, so this one waits for the next hour; once stale it is
/// removed, and so it cannot hold the backups back for good.
pub fn take_lock(root: &Path) -> Option<PathBuf> {
    let old = root.join(OLD_LOCK);
    if old.exists() {
        if !is_stale(&old) {
            return None;
        }
        let _ = std::fs::remove_file(&old);
    }
    let lock = root.join(LOCK);
    if std::fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() && !is_stale(&lock) {
        return None;
    }
    Some(lock)
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "glove".into())
}

/// Onde ficam as cópias deste computador.
pub fn root(dest: &Path) -> PathBuf {
    dest.join("Glove Backups").join(hostname())
}

/// Data local de um instante: (ano, mês, dia, hora, minuto, segundo, dia da semana).
pub fn local(t: SystemTime) -> (i32, u32, u32, u32, u32, u32, u32) {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    (tm.tm_year + 1900, tm.tm_mon as u32 + 1, tm.tm_mday as u32, tm.tm_hour as u32, tm.tm_min as u32, tm.tm_sec as u32, tm.tm_wday as u32)
}

fn stamp(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s, _) = local(t);
    format!("{y:04}-{mo:02}-{d:02}-{h:02}{mi:02}{s:02}")
}

/// Uma cópia pronta.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub time: SystemTime,
}

/// Lê "2026-11-07-191500" como instante local.
fn parse_stamp(s: &str) -> Option<SystemTime> {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 4 || p[3].len() != 6 {
        return None;
    }
    let num = |x: &str| x.parse::<i32>().ok();
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = num(p[0])? - 1900;
    tm.tm_mon = num(p[1])? - 1;
    tm.tm_mday = num(p[2])?;
    tm.tm_hour = num(&p[3][0..2])?;
    tm.tm_min = num(&p[3][2..4])?;
    tm.tm_sec = num(&p[3][4..6])?;
    tm.tm_isdst = -1;
    let t = unsafe { libc::mktime(&mut tm) };
    (t >= 0).then(|| UNIX_EPOCH + Duration::from_secs(t as u64))
}

/// As cópias prontas, a mais nova primeiro.
pub fn snapshots(dest: &Path) -> Vec<Snapshot> {
    let Ok(rd) = std::fs::read_dir(root(dest)) else { return Vec::new() };
    let mut v: Vec<Snapshot> = rd.flatten().filter_map(|e| parse_stamp(&e.file_name().to_string_lossy()).map(|time| Snapshot { path: e.path(), time })).collect();
    v.sort_by(|a, b| b.time.cmp(&a.time));
    v
}

/// Andamento de uma cópia (lido pela janela enquanto a thread trabalha).
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub copied: AtomicU64,
    pub bytes: AtomicU64,
    pub skipped: AtomicU64,
    pub cancel: AtomicBool,
    pub done: AtomicBool,
    pub current: Mutex<String>,
    pub result: Mutex<Option<Result<PathBuf, String>>>,
}

struct Job<'a> {
    excl_names: HashSet<String>,
    excl_paths: Vec<PathBuf>,
    prev: Option<PathBuf>,
    p: &'a Progress,
}

impl Job<'_> {
    fn excluded(&self, rel: &Path, name: &str) -> bool {
        self.excl_names.contains(name) || self.excl_paths.iter().any(|x| rel.starts_with(x))
    }

    fn walk(&self, src: &Path, dst: &Path, rel: &Path) {
        let Ok(rd) = std::fs::read_dir(src) else {
            self.p.skipped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        for e in rd.flatten() {
            if self.p.cancel.load(Ordering::Relaxed) {
                return;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let rel_child = rel.join(&name);
            if self.excluded(&rel_child, &name) {
                continue;
            }
            let (s, d) = (e.path(), dst.join(&name));
            let Ok(meta) = std::fs::symlink_metadata(&s) else { continue };
            let ft = meta.file_type();
            if ft.is_dir() {
                if std::fs::create_dir(&d).is_ok() {
                    self.walk(&s, &d, &rel_child);
                }
            } else if ft.is_symlink() {
                if let Ok(t) = std::fs::read_link(&s) {
                    let _ = std::os::unix::fs::symlink(t, &d);
                }
            } else if ft.is_file() {
                self.p.files.fetch_add(1, Ordering::Relaxed);
                if self.p.files.load(Ordering::Relaxed) % 64 == 0 {
                    *self.p.current.lock().unwrap() = rel_child.to_string_lossy().into_owned();
                }
                // Igual ao da cópia anterior: link fixo (não ocupa espaço de novo).
                let same = self.prev.as_ref().map(|p| p.join(&rel_child)).and_then(|p| std::fs::symlink_metadata(&p).ok().map(|m| (p, m))).filter(|(_, m)| m.is_file() && m.len() == meta.len() && m.modified().ok() == meta.modified().ok());
                if let Some((p, _)) = same {
                    if std::fs::hard_link(&p, &d).is_ok() {
                        continue;
                    }
                }
                match std::fs::copy(&s, &d) {
                    Ok(n) => {
                        self.p.copied.fetch_add(1, Ordering::Relaxed);
                        self.p.bytes.fetch_add(n, Ordering::Relaxed);
                        if let (Ok(t), Ok(f)) = (meta.modified(), std::fs::File::options().write(true).open(&d)) {
                            let _ = f.set_modified(t);
                        }
                    }
                    Err(_) => {
                        self.p.skipped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }
}

/// Espaço livre no disco de `p`, em bytes.
pub fn free_space(p: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| s.f_bavail as u64 * s.f_frsize as u64)
}

/// Faz uma cópia agora. Devolve a pasta da cópia nova.
pub fn backup(prefs: &Prefs, p: &Progress) -> Result<PathBuf, String> {
    let dest = prefs.dest.clone().ok_or_else(|| glove_config::tr!("Escolha o disco das cópias.").to_string())?;
    if !dest.is_dir() {
        return Err(glove_config::tr!("O disco das cópias não está conectado ({}).", dest.display()));
    }
    let root = root(&dest);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // Restos de uma cópia interrompida.
    remove_unfinished(&root);
    make_room(&dest, 1 << 30);
    let prev = snapshots(&dest).first().map(|s| s.path.clone());
    let name = stamp(SystemTime::now());
    let work = root.join(format!("{name}{UNFINISHED}"));
    std::fs::create_dir(&work).map_err(|e| e.to_string())?;
    let home = home();
    let mut excl_paths: Vec<PathBuf> = prefs.exclude.iter().filter(|x| x.contains('/')).map(PathBuf::from).collect();
    // O próprio destino, se estiver dentro da pasta pessoal.
    if let Ok(r) = dest.strip_prefix(&home) {
        excl_paths.push(r.to_path_buf());
    }
    let job = Job { excl_names: prefs.exclude.iter().filter(|x| !x.contains('/')).cloned().collect(), excl_paths, prev, p };
    job.walk(&home, &work, Path::new(""));
    if p.cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_dir_all(&work);
        return Err(glove_config::tr!("Cópia cancelada.").to_string());
    }
    let done = root.join(&name);
    std::fs::rename(&work, &done).map_err(|e| e.to_string())?;
    prune(&dest);
    Ok(done)
}

/// Quais cópias guardar (índices na lista da mais nova para a mais velha).
pub fn keep(times: &[SystemTime], now: SystemTime) -> Vec<bool> {
    let day = 86_400u64;
    let mut seen_days = HashSet::new();
    let mut seen_weeks = HashSet::new();
    times
        .iter()
        .map(|t| {
            let age = now.duration_since(*t).map(|d| d.as_secs()).unwrap_or(0);
            let days = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() / day).unwrap_or(0);
            if age < day {
                true
            } else if age < 30 * day {
                seen_days.insert(days)
            } else {
                seen_weeks.insert(days / 7)
            }
        })
        .collect()
}

pub fn prune(dest: &Path) {
    let snaps = snapshots(dest);
    let times: Vec<SystemTime> = snaps.iter().map(|s| s.time).collect();
    for (s, k) in snaps.iter().zip(keep(&times, SystemTime::now())) {
        if !k {
            let _ = std::fs::remove_dir_all(&s.path);
        }
    }
}

/// Disco quase cheio: apaga as cópias mais velhas (fica pelo menos uma).
fn make_room(dest: &Path, need: u64) {
    loop {
        let snaps = snapshots(dest);
        if snaps.len() < 2 || free_space(dest).is_none_or(|f| f >= need) {
            return;
        }
        let _ = std::fs::remove_dir_all(&snaps[snaps.len() - 1].path);
    }
}

/// Copia `src` (arquivo ou pasta) para `dst`.
pub fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(src)?;
    if meta.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)?.flatten() {
            copy_tree(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else if meta.file_type().is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(src)?, dst)
    } else {
        std::fs::copy(src, dst)?;
        if let (Ok(t), Ok(f)) = (meta.modified(), std::fs::File::options().write(true).open(dst)) {
            let _ = f.set_modified(t);
        }
        Ok(())
    }
}

/// Restaura um item de uma cópia no lugar de origem. Se já existir algo lá,
/// volta com " (restaurado)" no nome. Devolve onde ficou.
pub fn restore(snapshot: &Path, item: &Path) -> std::io::Result<PathBuf> {
    let rel = item.strip_prefix(snapshot).map_err(|_| std::io::Error::other("fora da cópia"))?;
    let mut target = home().join(rel);
    if target.symlink_metadata().is_ok() {
        let stem = target.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = target.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let mut n = 1;
        loop {
            let extra = if n == 1 { String::new() } else { format!(" {n}") };
            let cand = target.with_file_name(format!("{stem} ({}{extra}){ext}", glove_config::tr!("restaurado")));
            if cand.symlink_metadata().is_err() {
                target = cand;
                break;
            }
            n += 1;
        }
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    copy_tree(item, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas() {
        let t = parse_stamp("2026-11-07-191500").unwrap();
        assert_eq!(stamp(t), "2026-11-07-191500");
        assert!(parse_stamp("2026-11-07-191500.in-progress").is_none());
        assert!(parse_stamp("2026-11-07-191500.andamento").is_none());
        assert!(parse_stamp("foto").is_none());
    }

    /// An empty folder for one test (never the real configuration, nor a real disk).
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("glove-backup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Makes a file look as if it was written `hours` ago.
    fn age(p: &Path, hours: u64) {
        let f = std::fs::File::options().write(true).open(p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(hours * 3600)).unwrap();
    }

    #[test]
    fn old_prefs_are_moved_once() {
        let dir = temp_dir("old-names");
        let prefs = Prefs { dest: Some(PathBuf::from("/media/disco")), auto: false, exclude: vec![".cache".into(), "Downloads/iso".into()] };
        let text = toml::to_string(&prefs).unwrap();
        std::fs::write(dir.join("copias.toml"), &text).unwrap();
        migrate_old_names_in(&dir);
        assert!(!dir.join("copias.toml").exists());
        assert_eq!(std::fs::read_to_string(dir.join("backup.toml")).unwrap(), text);
        assert_eq!(toml::from_str::<Prefs>(&text).unwrap(), prefs);
        // A second run finds nothing to do.
        migrate_old_names_in(&dir);
        assert_eq!(std::fs::read_to_string(dir.join("backup.toml")).unwrap(), text);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        // Something old beside the new: the new is the one in use, and nothing is touched.
        std::fs::write(dir.join("copias.toml"), "auto = true\n").unwrap();
        migrate_old_names_in(&dir);
        assert_eq!(std::fs::read_to_string(dir.join("backup.toml")).unwrap(), text);
        assert_eq!(std::fs::read_to_string(dir.join("copias.toml")).unwrap(), "auto = true\n");
        // Nothing stored at all: nothing is created.
        let empty = temp_dir("nothing-stored");
        migrate_old_names_in(&empty);
        assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn unfinished_backups_of_both_names_go_away() {
        assert!(is_unfinished("2026-11-07-191500.in-progress") && is_unfinished("2026-11-07-191500.andamento"));
        assert!(!is_unfinished("2026-11-07-191500") && !is_unfinished(LOCK) && !is_unfinished(OLD_LOCK));
        let dest = temp_dir("unfinished");
        let root = root(&dest);
        for name in ["2026-11-07-191500", "2026-11-08-101500.andamento", "2026-11-09-111500.in-progress"] {
            std::fs::create_dir_all(root.join(name).join("Docs")).unwrap();
            std::fs::write(root.join(name).join("Docs/a.txt"), "um").unwrap();
        }
        // Neither is listed as a finished backup.
        let done: Vec<PathBuf> = snapshots(&dest).into_iter().map(|s| s.path).collect();
        assert_eq!(done, vec![root.join("2026-11-07-191500")]);
        remove_unfinished(&root);
        let mut left: Vec<String> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, vec!["2026-11-07-191500".to_string()]);
        assert_eq!(std::fs::read_to_string(root.join("2026-11-07-191500/Docs/a.txt")).unwrap(), "um");
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn one_hourly_backup_at_a_time() {
        let root = temp_dir("lock");
        let lock = root.join(".in-progress.lock");
        // Free: taken. Taken a moment ago: another backup is running.
        assert_eq!(take_lock(&root), Some(lock.clone()));
        assert!(lock.exists());
        assert_eq!(take_lock(&root), None);
        // Left behind for more than six hours: it holds nothing.
        age(&lock, 5);
        assert_eq!(take_lock(&root), None);
        age(&lock, 7);
        assert_eq!(take_lock(&root), Some(lock.clone()));
        std::fs::remove_file(&lock).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lock_of_an_older_version() {
        let root = temp_dir("old-lock");
        let (lock, old) = (root.join(".in-progress.lock"), root.join(".andamento.lock"));
        // Fresh: a backup of the older version may be running, so nothing is taken or removed.
        std::fs::write(&old, "").unwrap();
        assert_eq!(take_lock(&root), None);
        assert!(old.exists() && !lock.exists());
        age(&old, 5);
        assert_eq!(take_lock(&root), None);
        assert!(old.exists() && !lock.exists());
        // Stale: it is a leftover; it goes away and the backup goes on under today's name.
        age(&old, 7);
        assert_eq!(take_lock(&root), Some(lock.clone()));
        assert!(!old.exists() && lock.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn quais_ficam() {
        let now = UNIX_EPOCH + Duration::from_secs(100 * 86_400);
        let h = |x: u64| now - Duration::from_secs(x * 3600);
        // Duas na última hora, duas no mesmo dia de 3 dias atrás, duas na mesma semana há 60 dias.
        let times = [h(0), h(1), h(70), h(71), h(60 * 24), h(60 * 24 + 25)];
        let k = keep(&times, now);
        assert_eq!(k, vec![true, true, true, false, true, false]);
    }

    #[test]
    fn copia_e_restaura() {
        let base = std::env::temp_dir().join(format!("glove-backup-teste-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (src, dst) = (base.join("casa"), base.join("disco"));
        std::fs::create_dir_all(src.join("Docs")).unwrap();
        std::fs::create_dir_all(src.join(".cache")).unwrap();
        std::fs::write(src.join("Docs/a.txt"), "um").unwrap();
        std::fs::write(src.join(".cache/lixo"), "x").unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        let prefs = Prefs { dest: Some(dst.clone()), auto: false, ..Default::default() };
        let p = Progress::default();
        let job = Job { excl_names: [".cache".to_string()].into(), excl_paths: vec![], prev: None, p: &p };
        let w1 = dst.join("s1");
        std::fs::create_dir_all(&w1).unwrap();
        job.walk(&src, &w1, Path::new(""));
        assert_eq!(std::fs::read_to_string(w1.join("Docs/a.txt")).unwrap(), "um");
        assert!(!w1.join(".cache").exists());
        // Segunda cópia: o arquivo igual vira link fixo para o da primeira.
        let job2 = Job { excl_names: HashSet::new(), excl_paths: vec![], prev: Some(w1.clone()), p: &p };
        let w2 = dst.join("s2");
        std::fs::create_dir_all(&w2).unwrap();
        job2.walk(&src, &w2, Path::new(""));
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(w1.join("Docs/a.txt")).unwrap().ino(), std::fs::metadata(w2.join("Docs/a.txt")).unwrap().ino());
        let _ = prefs;
        let _ = std::fs::remove_dir_all(&base);
    }
}
