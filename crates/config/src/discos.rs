//! Pendrives e discos externos: quais estão montados, Ejetar (desmonta as
//! partições e desliga o aparelho pelo udisksctl, sem root) e o registro dos
//! ejetados pelo Glove, para o aviso de "disco removido sem ejetar" só aparecer
//! quando o aparelho some sem ter sido ejetado.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// Nome mostrado (o da pasta de montagem: o rótulo do disco).
    pub name: String,
    pub path: PathBuf,
    /// Aparelho (ex.: "/dev/sdb1").
    pub dev: String,
}

/// Discos montados em /media/<usuário> ou /run/media/<usuário>.
pub fn removable_mounts() -> Vec<Mount> {
    let user = std::env::var("USER").unwrap_or_default();
    // Testes: GLOVE_MOUNTS_FILE aponta para uma lista no formato do /proc/mounts.
    let file = std::env::var_os("GLOVE_MOUNTS_FILE").map(PathBuf::from).unwrap_or_else(|| "/proc/mounts".into());
    parse_mounts(&std::fs::read_to_string(file).unwrap_or_default(), &user)
}

pub fn parse_mounts(text: &str, user: &str) -> Vec<Mount> {
    let prefixes = [format!("/media/{user}/"), format!("/run/media/{user}/")];
    let mut out: Vec<Mount> = Vec::new();
    for l in text.lines() {
        let mut f = l.split_whitespace();
        let (Some(dev), Some(mp)) = (f.next(), f.next()) else { continue };
        // Espaços vêm como \040 no /proc/mounts.
        let mp = mp.replace("\\040", " ");
        if prefixes.iter().any(|p| mp.starts_with(p.as_str())) {
            let path = PathBuf::from(&mp);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if !out.iter().any(|m| m.path == path) {
                out.push(Mount { name, path, dev: dev.to_string() });
            }
        }
    }
    out
}

/// Disco inteiro de uma partição ("/dev/sdb1" → "sdb"; "/dev/mmcblk0p1" → "mmcblk0").
fn parent_disk(dev: &str) -> String {
    let name = dev.trim_start_matches("/dev/");
    let sys = Path::new("/sys/class/block").join(name);
    if sys.join("partition").exists() {
        if let Ok(real) = std::fs::canonicalize(&sys) {
            if let Some(p) = real.parent().and_then(|p| p.file_name()) {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    name.to_string()
}

fn ejected_file() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("glove-ejetados")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Anota que o Glove ejetou este aparelho (não é remoção sem ejetar). Vale 2 minutos.
pub fn mark_ejected(dev: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ejected_file()) {
        let _ = writeln!(f, "{dev}\t{}", now_secs());
    }
}

/// O aparelho (ou o disco dele) foi ejetado pelo Glove há pouco? Limpa as anotações velhas.
pub fn take_ejected(dev: &str) -> bool {
    let p = ejected_file();
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let now = now_secs();
    let disk = parent_disk_name_only(dev);
    let fresh: Vec<(&str, u64)> = text
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter_map(|(d, t)| t.parse::<u64>().ok().map(|t| (d, t)))
        .filter(|(_, t)| now.saturating_sub(*t) < 120)
        .collect();
    let found = fresh.iter().any(|(d, _)| *d == dev || *d == disk);
    let rest: String = fresh.iter().map(|(d, t)| format!("{d}\t{t}\n")).collect();
    let _ = std::fs::write(&p, rest);
    found
}

/// "/dev/sdb1" → "/dev/sdb" pelo nome (o aparelho já pode ter sumido do /sys).
fn parent_disk_name_only(dev: &str) -> String {
    let name = dev.trim_start_matches("/dev/");
    let base = if name.starts_with("mmcblk") || name.starts_with("nvme") {
        name.rsplit_once('p').map(|(a, b)| if b.chars().all(|c| c.is_ascii_digit()) && !b.is_empty() { a } else { name }).unwrap_or(name)
    } else {
        name.trim_end_matches(|c: char| c.is_ascii_digit())
    };
    format!("/dev/{base}")
}

/// Ejetar o disco montado em `mount`: desmonta todas as partições dele e desliga
/// o aparelho. Devolve o nome do disco (para a mensagem) ou o motivo da falha.
pub fn eject(mount: &Path) -> Result<String, String> {
    let all = removable_mounts();
    let Some(m) = all.iter().find(|m| m.path == mount) else { return Err("Este disco não está montado.".into()) };
    let disk = parent_disk(&m.dev);
    let parts: Vec<&Mount> = all.iter().filter(|x| parent_disk(&x.dev) == disk).collect();
    mark_ejected(&format!("/dev/{disk}"));
    for p in &parts {
        mark_ejected(&p.dev);
        let o = std::process::Command::new("udisksctl").args(["unmount", "-b", &p.dev]).output().map_err(|e| format!("Não foi possível usar o udisksctl: {e}"))?;
        if !o.status.success() {
            let e = String::from_utf8_lossy(&o.stderr).to_lowercase();
            return Err(if e.contains("busy") || e.contains("ocupado") {
                format!("O disco “{}” está em uso. Feche os arquivos e os apps que usam o disco e tente de novo.", m.name)
            } else if e.contains("notauthorized") {
                "O sistema pediu permissão de administrador e ela não foi dada.".into()
            } else {
                format!("Não foi possível ejetar “{}”: {}", m.name, e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim())
            });
        }
    }
    // Desligar o aparelho (cartões SD e alguns discos não deixam: já estão desmontados, basta).
    let _ = std::process::Command::new("udisksctl").args(["power-off", "-b", &format!("/dev/{disk}")]).output();
    Ok(m.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pendrives() {
        let m = "/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /media/ana/MEU\\040PEN vfat rw 0 0\n/dev/sdc1 /run/media/ana/Fotos exfat rw 0 0\n/dev/sdd1 /media/beto/X vfat rw 0 0\n";
        let v = parse_mounts(m, "ana");
        assert_eq!(v.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["MEU PEN", "Fotos"]);
        assert_eq!(v[0].dev, "/dev/sdb1");
    }

    #[test]
    fn disco_da_particao() {
        assert_eq!(parent_disk_name_only("/dev/sdb1"), "/dev/sdb");
        assert_eq!(parent_disk_name_only("/dev/mmcblk0p2"), "/dev/mmcblk0");
        assert_eq!(parent_disk_name_only("/dev/nvme0n1p3"), "/dev/nvme0n1");
    }
}
