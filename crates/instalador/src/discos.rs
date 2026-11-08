//! Os discos do computador, pelo `lsblk` (só leitura, sem precisar de root):
//! tamanho, modelo, partições e os espaços livres entre elas.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
struct Dev {
    name: String,
    path: String,
    size: u64,
    #[serde(rename = "type")]
    kind: String,
    model: Option<String>,
    tran: Option<String>,
    rm: bool,
    ro: bool,
    fstype: Option<String>,
    parttype: Option<String>,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
    label: Option<String>,
    pttype: Option<String>,
    start: Option<u64>,
    #[serde(rename = "log-sec")]
    log_sec: Option<u64>,
    #[serde(default)]
    children: Vec<Dev>,
}

#[derive(Deserialize)]
struct Lsblk {
    blockdevices: Vec<Dev>,
}

/// GUID da partição de sistema EFI.
pub const ESP_TYPE: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";

#[derive(Clone, Debug, PartialEq)]
pub struct Particao {
    pub path: String,
    pub numero: u32,
    pub tamanho: u64,
    pub fs: String,
    pub rotulo: String,
    pub esp: bool,
    pub inicio: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Disco {
    pub path: String,
    pub nome: String,
    pub tamanho: u64,
    pub removivel: bool,
    /// "gpt", "dos" ou vazio (sem tabela).
    pub tabela: String,
    pub setor: u64,
    pub particoes: Vec<Particao>,
    /// Maior espaço livre: (setor inicial, setores).
    pub livre: Option<(u64, u64)>,
    /// É o pendrive/DVD de onde o Glove ao vivo está rodando.
    pub ao_vivo: bool,
}

impl Disco {
    pub fn livre_bytes(&self) -> u64 {
        self.livre.map(|(_, n)| n * self.setor).unwrap_or(0)
    }
    pub fn esp(&self) -> Option<&Particao> {
        self.particoes.iter().find(|p| p.esp)
    }
}

pub fn part_path(disk: &str, n: u32) -> String {
    if disk.chars().last().is_some_and(|c| c.is_ascii_digit()) {
        format!("{disk}p{n}")
    } else {
        format!("{disk}{n}")
    }
}

fn numero(disk: &str, part: &str) -> u32 {
    part.strip_prefix(disk).map(|r| r.trim_start_matches('p')).and_then(|r| r.parse().ok()).unwrap_or(0)
}

/// "500 GB", "1,2 TB" (unidades de 1000, como nos discos).
pub fn tamanho(b: u64) -> String {
    let v = b as f64;
    let s = if v >= 1e12 { format!("{:.1} TB", v / 1e12) } else if v >= 1e9 { format!("{:.0} GB", v / 1e9) } else { format!("{:.0} MB", v / 1e6) };
    s.replace('.', glove_config::virgula())
}

fn monta(d: &Dev) -> Disco {
    let setor = d.log_sec.unwrap_or(512).max(512);
    let mut parts: Vec<Particao> = d
        .children
        .iter()
        .filter(|c| c.kind == "part")
        .map(|c| Particao {
            path: c.path.clone(),
            numero: numero(&d.path, &c.path),
            tamanho: c.size,
            fs: c.fstype.clone().unwrap_or_default(),
            rotulo: c.label.clone().unwrap_or_default(),
            esp: c.parttype.as_deref().is_some_and(|t| t.eq_ignore_ascii_case(ESP_TYPE)),
            inicio: c.start.unwrap_or(0),
        })
        .collect();
    parts.sort_by_key(|p| p.inicio);
    // Espaços livres: entre as partições (alinhados a 1 MiB; o fim do GPT guarda 34 setores).
    let total = d.size / setor;
    let align = (1 << 20) / setor;
    let mut livre: Option<(u64, u64)> = None;
    let mut pos = align;
    let mut gap = |a: u64, b: u64| {
        let a = a.div_ceil(align) * align;
        if b > a && livre.is_none_or(|(_, n)| b - a > n) {
            livre = Some((a, b - a));
        }
    };
    for p in &parts {
        gap(pos, p.inicio);
        pos = pos.max(p.inicio + p.tamanho / setor);
    }
    gap(pos, total.saturating_sub(64));
    let ao_vivo = d.children.iter().chain(std::iter::once(d)).any(|c| c.mountpoints.iter().flatten().any(|m| m.starts_with("/run/live")));
    let modelo = d.model.clone().unwrap_or_default().trim().to_string();
    let via = d.tran.clone().unwrap_or_default().to_uppercase();
    let nome = match (modelo.is_empty(), via.is_empty()) {
        (false, false) => format!("{modelo} ({via})"),
        (false, true) => modelo,
        _ => d.name.clone(),
    };
    Disco { path: d.path.clone(), nome, tamanho: d.size, removivel: d.rm, tabela: d.pttype.clone().unwrap_or_default(), setor, particoes: parts, livre, ao_vivo }
}

pub fn parse(json: &str) -> Vec<Disco> {
    let Ok(l) = serde_json::from_str::<Lsblk>(json) else { return Vec::new() };
    l.blockdevices.iter().filter(|d| d.kind == "disk" && !d.ro && d.size >= 8_000_000_000).map(monta).collect()
}

/// Os discos onde dá para instalar (sem o pendrive do Glove ao vivo).
pub fn listar() -> Vec<Disco> {
    let out = std::process::Command::new("lsblk").args(["-J", "-b", "-o", "NAME,PATH,SIZE,TYPE,MODEL,TRAN,RM,RO,FSTYPE,PARTTYPE,MOUNTPOINTS,LABEL,PTTYPE,START,LOG-SEC"]).output();
    let json = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    parse(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"blockdevices":[
      {"name":"sda","path":"/dev/sda","size":500107862016,"type":"disk","model":"Disco Teste","tran":"sata","rm":false,"ro":false,"fstype":null,"parttype":null,"mountpoints":[null],"label":null,"pttype":"gpt","start":null,"log-sec":512,
       "children":[
         {"name":"sda1","path":"/dev/sda1","size":536870912,"type":"part","model":null,"tran":null,"rm":false,"ro":false,"fstype":"vfat","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b","mountpoints":[null],"label":null,"pttype":"gpt","start":2048,"log-sec":512},
         {"name":"sda2","path":"/dev/sda2","size":200000000000,"type":"part","model":null,"tran":null,"rm":false,"ro":false,"fstype":"ntfs","parttype":"ebd0a0a2-b9e5-4433-87c0-68b6b72699c7","mountpoints":[null],"label":"Windows","pttype":"gpt","start":1050624,"log-sec":512}
       ]},
      {"name":"sdb","path":"/dev/sdb","size":16000000000,"type":"disk","model":"Pendrive","tran":"usb","rm":true,"ro":false,"fstype":"iso9660","parttype":null,"mountpoints":["/run/live/medium"],"label":"Glove","pttype":"dos","start":null,"log-sec":512},
      {"name":"loop0","path":"/dev/loop0","size":4096,"type":"loop","model":null,"tran":null,"rm":false,"ro":true,"fstype":"squashfs","parttype":null,"mountpoints":["/x"],"label":null,"pttype":null,"start":null,"log-sec":512}
    ]}"#;

    #[test]
    fn discos() {
        let d = parse(JSON);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].nome, "Disco Teste (SATA)");
        assert!(d[0].esp().is_some());
        assert_eq!(d[0].particoes[1].rotulo, "Windows");
        // Depois do Windows sobram uns 299 GB livres.
        let l = d[0].livre_bytes();
        assert!(l > 290_000_000_000 && l < 300_000_000_000, "{l}");
        assert!(d[1].ao_vivo);
        assert_eq!(part_path("/dev/nvme0n1", 2), "/dev/nvme0n1p2");
        assert_eq!(part_path("/dev/sda", 2), "/dev/sda2");
    }
}
