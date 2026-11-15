//! O udisks2 (o serviço de discos do sistema) pelo D-Bus: lê os discos, as
//! partições e os sistemas de arquivos, e pede as operações. As que mudam o
//! disco passam pelo polkit (a janela de senha do Glove, `glove-senha`).

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const BUS: &str = "org.freedesktop.UDisks2";
const MIB: u64 = 1024 * 1024;

type Props = HashMap<String, OwnedValue>;
type Objects = HashMap<OwnedObjectPath, HashMap<String, Props>>;

fn get<T: TryFrom<OwnedValue>>(p: &Props, k: &str) -> Option<T> {
    p.get(k)?.try_clone().ok()?.try_into().ok()
}

/// Texto de um `ay` (caminhos e nomes vêm terminados em zero).
fn bytes(p: &Props, k: &str) -> String {
    let v: Vec<u8> = get(p, k).unwrap_or_default();
    String::from_utf8_lossy(v.split(|b| *b == 0).next().unwrap_or(&[])).into_owned()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drive {
    pub path: String,
    pub model: String,
    pub vendor: String,
    pub serial: String,
    pub size: u64,
    pub removable: bool,
    pub ejectable: bool,
    pub can_power_off: bool,
    /// "usb", "sdio", "ieee1394" ou vazio (interno).
    pub bus: String,
    /// 0 = SSD; > 0 = disco que gira (RPM); -1 = gira, velocidade desconhecida.
    pub rotation: i32,
    pub optical: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Part {
    pub number: u32,
    pub offset: u64,
    /// Tipo: GUID (GPT) ou "0x83" (MBR).
    pub ptype: String,
    pub name: String,
    pub table: String,
    pub flags: u64,
    /// A partição estendida (MBR) que guarda as lógicas.
    pub container: bool,
    pub contained: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub path: String,
    pub device: String,
    pub size: u64,
    pub drive: String,
    pub fstype: String,
    pub usage: String,
    pub label: String,
    pub uuid: String,
    pub read_only: bool,
    pub system: bool,
    pub ignore: bool,
    /// "gpt" ou "dos" quando o bloco tem tabela de partições.
    pub table: Option<String>,
    pub part: Option<Part>,
    /// Pontos de montagem (só quando tem sistema de arquivos).
    pub mounts: Option<Vec<String>>,
    /// Arquivo por trás de um disco de imagem (loop).
    pub loop_file: Option<String>,
}

impl Block {
    pub fn mounted(&self) -> Option<&str> {
        self.mounts.as_ref()?.first().map(String::as_str)
    }
    pub fn has_fs(&self) -> bool {
        self.mounts.is_some()
    }
}

/// Um pedaço do mapa de um disco: partição ou espaço livre.
#[derive(Clone, Debug, PartialEq)]
pub enum Slot {
    /// Índice em `Model::blocks`.
    Part(usize),
    Free { offset: u64, size: u64 },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub drives: Vec<Drive>,
    pub blocks: Vec<Block>,
}

/// Um disco da lista da esquerda: o bloco inteiro e o aparelho, se houver.
#[derive(Clone, Debug, PartialEq)]
pub struct Disk {
    pub block: usize,
    pub drive: Option<usize>,
}

impl Model {
    pub fn read(conn: &Connection) -> zbus::Result<Model> {
        let reply = conn.call_method(Some(BUS), "/org/freedesktop/UDisks2", Some("org.freedesktop.DBus.ObjectManager"), "GetManagedObjects", &())?;
        let objects: Objects = reply.body().deserialize()?;
        Ok(Self::from_objects(&objects))
    }

    fn from_objects(objects: &Objects) -> Model {
        let mut m = Model::default();
        for (path, ifaces) in objects {
            let path = path.as_str().to_string();
            if let Some(d) = ifaces.get("org.freedesktop.UDisks2.Drive") {
                m.drives.push(Drive {
                    path: path.clone(),
                    model: get(d, "Model").unwrap_or_default(),
                    vendor: get(d, "Vendor").unwrap_or_default(),
                    serial: get(d, "Serial").unwrap_or_default(),
                    size: get(d, "Size").unwrap_or(0),
                    removable: get(d, "Removable").unwrap_or(false) || get(d, "MediaRemovable").unwrap_or(false),
                    ejectable: get(d, "Ejectable").unwrap_or(false),
                    can_power_off: get(d, "CanPowerOff").unwrap_or(false),
                    bus: get(d, "ConnectionBus").unwrap_or_default(),
                    rotation: get(d, "RotationRate").unwrap_or(0),
                    optical: get::<Vec<String>>(d, "MediaCompatibility").unwrap_or_default().iter().any(|c| c.starts_with("optical")),
                });
            }
            let Some(b) = ifaces.get("org.freedesktop.UDisks2.Block") else { continue };
            let drive: OwnedObjectPath = get(b, "Drive").unwrap_or_else(|| OwnedObjectPath::try_from("/").unwrap());
            let mut block = Block {
                path: path.clone(),
                device: bytes(b, "Device"),
                size: get(b, "Size").unwrap_or(0),
                drive: if drive.as_str() == "/" { String::new() } else { drive.as_str().to_string() },
                fstype: get(b, "IdType").unwrap_or_default(),
                usage: get(b, "IdUsage").unwrap_or_default(),
                label: get(b, "IdLabel").unwrap_or_default(),
                uuid: get(b, "IdUUID").unwrap_or_default(),
                read_only: get(b, "ReadOnly").unwrap_or(false),
                system: get(b, "HintSystem").unwrap_or(false),
                ignore: get(b, "HintIgnore").unwrap_or(false),
                ..Default::default()
            };
            if let Some(t) = ifaces.get("org.freedesktop.UDisks2.PartitionTable") {
                block.table = Some(get(t, "Type").unwrap_or_default());
            }
            if let Some(p) = ifaces.get("org.freedesktop.UDisks2.Partition") {
                let table: OwnedObjectPath = get(p, "Table").unwrap_or_else(|| OwnedObjectPath::try_from("/").unwrap());
                block.part = Some(Part {
                    number: get(p, "Number").unwrap_or(0),
                    offset: get(p, "Offset").unwrap_or(0),
                    ptype: get(p, "Type").unwrap_or_default(),
                    name: get(p, "Name").unwrap_or_default(),
                    table: table.as_str().to_string(),
                    flags: get(p, "Flags").unwrap_or(0),
                    container: get(p, "IsContainer").unwrap_or(false),
                    contained: get(p, "IsContained").unwrap_or(false),
                });
            }
            if let Some(f) = ifaces.get("org.freedesktop.UDisks2.Filesystem") {
                let mounts: Vec<Vec<u8>> = get(f, "MountPoints").unwrap_or_default();
                block.mounts = Some(mounts.iter().map(|m| String::from_utf8_lossy(m.split(|b| *b == 0).next().unwrap_or(&[])).into_owned()).collect());
            }
            if let Some(l) = ifaces.get("org.freedesktop.UDisks2.Loop") {
                block.loop_file = Some(bytes(l, "BackingFile"));
            }
            m.blocks.push(block);
        }
        m.drives.sort_by(|a, b| (a.removable, &a.path).cmp(&(b.removable, &b.path)));
        m.blocks.sort_by(|a, b| natural(&a.device).cmp(&natural(&b.device)));
        m
    }

    /// Os discos da lista: os aparelhos (com mídia) e os discos de imagem em uso.
    pub fn disks(&self) -> Vec<Disk> {
        let mut out = Vec::new();
        for (di, d) in self.drives.iter().enumerate() {
            if let Some(bi) = self.blocks.iter().position(|b| b.drive == d.path && b.part.is_none() && !b.device.contains("boot") && b.size > 0) {
                out.push(Disk { block: bi, drive: Some(di) });
            } else if d.removable {
                // Leitor sem mídia (cartão, DVD): aparece, vazio.
                if let Some(bi) = self.blocks.iter().position(|b| b.drive == d.path && b.part.is_none()) {
                    out.push(Disk { block: bi, drive: Some(di) });
                }
            }
        }
        for (bi, b) in self.blocks.iter().enumerate() {
            let snap = b.loop_file.as_deref().is_some_and(|f| f.contains("/snap") || f.ends_with(".snap"));
            if b.loop_file.as_deref().is_some_and(|f| !f.is_empty()) && b.part.is_none() && !snap && b.size > 0 {
                out.push(Disk { block: bi, drive: None });
            }
        }
        out
    }

    pub fn block(&self, path: &str) -> Option<usize> {
        self.blocks.iter().position(|b| b.path == path)
    }

    /// As partições de um disco (pelo bloco inteiro), em ordem no disco.
    pub fn parts_of(&self, disk: usize) -> Vec<usize> {
        let path = &self.blocks[disk].path;
        let mut v: Vec<usize> = (0..self.blocks.len()).filter(|&i| self.blocks[i].part.as_ref().is_some_and(|p| &p.table == path)).collect();
        v.sort_by_key(|&i| self.blocks[i].part.as_ref().map(|p| p.offset).unwrap_or(0));
        v
    }

    /// O mapa do disco: partições (primárias e a estendida) e os espaços livres entre
    /// elas. Sem tabela, o disco inteiro é um pedaço só (sistema de arquivos ou vazio).
    pub fn layout(&self, disk: usize) -> Vec<Slot> {
        let b = &self.blocks[disk];
        if b.table.is_none() {
            return vec![Slot::Part(disk)];
        }
        let parts: Vec<usize> = self.parts_of(disk).into_iter().filter(|&i| !self.blocks[i].part.as_ref().unwrap().contained).collect();
        let spans: Vec<(u64, u64)> = parts.iter().map(|&i| (self.blocks[i].part.as_ref().unwrap().offset, self.blocks[i].size)).collect();
        let mut out = Vec::new();
        for (slot, (o, s)) in free_between(&spans, b.size) {
            match slot {
                Some(k) => out.push(Slot::Part(parts[k])),
                None => out.push(Slot::Free { offset: o, size: s }),
            }
        }
        out
    }

    /// As partições lógicas dentro da estendida, com os espaços livres dela.
    pub fn logical_layout(&self, ext: usize) -> Vec<Slot> {
        let Some(table) = self.blocks[ext].part.as_ref().map(|p| p.table.clone()) else { return Vec::new() };
        let start = self.blocks[ext].part.as_ref().unwrap().offset;
        let mut lg: Vec<usize> = (0..self.blocks.len()).filter(|&i| self.blocks[i].part.as_ref().is_some_and(|p| p.table == table && p.contained)).collect();
        lg.sort_by_key(|&i| self.blocks[i].part.as_ref().unwrap().offset);
        let spans: Vec<(u64, u64)> = lg.iter().map(|&i| (self.blocks[i].part.as_ref().unwrap().offset - start, self.blocks[i].size)).collect();
        free_between(&spans, self.blocks[ext].size)
            .into_iter()
            .map(|(slot, (o, s))| match slot {
                Some(k) => Slot::Part(lg[k]),
                None => Slot::Free { offset: start + o, size: s },
            })
            .collect()
    }
}

/// Junta pedaços ocupados (offset, tamanho) e os buracos entre eles. Buracos
/// menores que 1 MiB (alinhamento, cabeçalhos da tabela) não contam.
fn free_between(spans: &[(u64, u64)], total: u64) -> Vec<(Option<usize>, (u64, u64))> {
    let mut out = Vec::new();
    // GPT/MBR guardam o começo (1 MiB) e o GPT, o fim (cópia da tabela).
    let mut at = MIB;
    let end = total.saturating_sub(MIB);
    for (k, &(o, s)) in spans.iter().enumerate() {
        if o > at && o - at >= MIB {
            out.push((None, (at, o - at)));
        }
        out.push((Some(k), (o, s)));
        at = at.max(o + s);
    }
    if end > at && end - at >= MIB {
        out.push((None, (at, end - at)));
    }
    out
}

/// "sda10" depois de "sda9".
fn natural(s: &str) -> (String, u64) {
    let digits = s.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    let (a, n) = s.split_at(s.len() - digits);
    (a.to_string(), n.parse().unwrap_or(0))
}

// --- operações --------------------------------------------------------------------

/// Opções vazias (o udisks pede a{sv} em todas): a janela de senha é permitida.
pub fn no_options() -> HashMap<&'static str, Value<'static>> {
    HashMap::new()
}

/// Chama um método do udisks e devolve a resposta; o erro vem em texto para o usuário.
pub fn call<B>(path: &str, iface: &str, method: &str, body: &B) -> Result<zbus::message::Message, String>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    let conn = Connection::system().map_err(|e| e.to_string())?;
    conn.call_method(Some(BUS), path, Some(iface), method, body).map_err(|e| explain(&e))
}

/// Erro do udisks em português claro (o resto vem como o udisks escreveu).
pub fn explain(e: &zbus::Error) -> String {
    let t = e.to_string();
    if t.contains("NotAuthorized") || t.contains("not authorized") {
        return glove_config::tr!("A permissão de administrador não foi dada.").into();
    }
    if t.contains("DeviceBusy") || t.contains("target is busy") {
        return glove_config::tr!("O disco está em uso: feche os arquivos e programas que usam ele e tente de novo.").into();
    }
    // "org.freedesktop.UDisks2.Error.Failed: Error mounting …: mensagem"
    t.rsplit_once(": ").map(|(_, m)| m.to_string()).unwrap_or(t)
}

pub fn mount(block: &str) -> Result<String, String> {
    let m = call(block, "org.freedesktop.UDisks2.Filesystem", "Mount", &(no_options(),))?;
    m.body().deserialize::<String>().map_err(|e| e.to_string())
}

pub fn unmount(block: &str) -> Result<(), String> {
    call(block, "org.freedesktop.UDisks2.Filesystem", "Unmount", &(no_options(),)).map(|_| ())
}

pub fn power_off(drive: &str) -> Result<(), String> {
    call(drive, "org.freedesktop.UDisks2.Drive", "PowerOff", &(no_options(),)).map(|_| ())
}

pub fn eject(drive: &str) -> Result<(), String> {
    call(drive, "org.freedesktop.UDisks2.Drive", "Eject", &(no_options(),)).map(|_| ())
}

pub fn loop_delete(block: &str) -> Result<(), String> {
    call(block, "org.freedesktop.UDisks2.Loop", "Delete", &(no_options(),)).map(|_| ())
}

/// Abre o disco inteiro para gravar uma imagem por cima (pede a senha se precisar).
pub fn open_for_restore(block: &str) -> Result<std::fs::File, String> {
    let m = call(block, "org.freedesktop.UDisks2.Block", "OpenForRestore", &(no_options(),))?;
    let fd: zbus::zvariant::OwnedFd = m.body().deserialize().map_err(|e| e.to_string())?;
    Ok(std::fs::File::from(std::os::fd::OwnedFd::from(fd)))
}

/// Abre o disco inteiro para ler (conferir uma gravação).
pub fn open_for_backup(block: &str) -> Result<std::fs::File, String> {
    let m = call(block, "org.freedesktop.UDisks2.Block", "OpenForBackup", &(no_options(),))?;
    let fd: zbus::zvariant::OwnedFd = m.body().deserialize().map_err(|e| e.to_string())?;
    Ok(std::fs::File::from(std::os::fd::OwnedFd::from(fd)))
}

/// Formata o bloco: "gpt"/"dos" (tabela nova, apaga tudo), "vfat", "ext4", "exfat",
/// "ntfs", "btrfs" ou "empty". `label` vazio = sem nome.
pub fn format(block: &str, kind: &str, label: &str) -> Result<(), String> {
    let mut o: HashMap<&str, Value> = HashMap::new();
    if !label.is_empty() {
        o.insert("label", Value::from(label.to_string()));
    }
    // O dono do sistema de arquivos novo é quem pediu (pode gravar nele sem root).
    if !matches!(kind, "gpt" | "dos" | "empty" | "vfat" | "exfat" | "ntfs") {
        o.insert("take-ownership", Value::from(true));
    }
    call(block, "org.freedesktop.UDisks2.Block", "Format", &(kind, o)).map(|_| ())
}

/// Cria uma partição na tabela (offset e tamanho em bytes; 0 = o resto do disco) e a
/// formata; devolve o caminho dela no udisks.
pub fn create_partition(table: &str, offset: u64, size: u64, ptype: &str, fs: &str, label: &str) -> Result<String, String> {
    let mut fo: HashMap<&str, Value> = HashMap::new();
    if !label.is_empty() {
        fo.insert("label", Value::from(label.to_string()));
    }
    let m = call(table, "org.freedesktop.UDisks2.PartitionTable", "CreatePartitionAndFormat", &(offset, size, ptype, "", no_options(), fs, fo))?;
    let p: OwnedObjectPath = m.body().deserialize().map_err(|e| e.to_string())?;
    Ok(p.as_str().to_string())
}

/// Liga um arquivo de imagem como disco (só leitura); devolve o bloco dele.
pub fn loop_setup(file: &std::path::Path) -> Result<String, String> {
    let f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut o: HashMap<&str, Value> = HashMap::new();
    o.insert("read-only", Value::from(true));
    let fd = zbus::zvariant::Fd::from(&f);
    let m = call("/org/freedesktop/UDisks2/Manager", "org.freedesktop.UDisks2.Manager", "LoopSetup", &(fd, o))?;
    let p: OwnedObjectPath = m.body().deserialize().map_err(|e| e.to_string())?;
    Ok(p.as_str().to_string())
}

/// Espera o udisks mostrar o sistema de arquivos do bloco (logo depois de formatar
/// ou de ligar uma imagem, ele ainda está lendo).
pub fn wait_fs(block: &str) -> Result<(), String> {
    let conn = Connection::system().map_err(|e| e.to_string())?;
    for _ in 0..50 {
        let p = zbus::blocking::Proxy::new(&conn, BUS, block, "org.freedesktop.DBus.Properties").map_err(|e| e.to_string())?;
        let has: Result<OwnedValue, _> = p.call("Get", &("org.freedesktop.UDisks2.Filesystem", "MountPoints"));
        if has.is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Err(glove_config::tr!("O sistema de arquivos não apareceu a tempo.").into())
}

// --- nomes para gente ----------------------------------------------------------

/// Nome do tipo de partição (GPT ou MBR).
pub fn ptype_name(t: &str) -> String {
    use glove_config::tr;
    let t = t.to_ascii_lowercase();
    match t.as_str() {
        "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" | "0xef" => tr!("Sistema EFI").into(),
        "0fc63daf-8483-4772-8e79-3d69d8477de4" | "0x83" => "Linux".into(),
        "4f68bce3-e8cd-4db1-96e7-fbcaf984b709" => tr!("Linux (raiz)").into(),
        "933ac7e1-2eb4-4f13-b844-0e14e2aef915" => tr!("Linux (pessoal)").into(),
        "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f" | "0x82" => tr!("Linux (troca)").into(),
        "e6d6d379-f507-44c2-a23c-238f2a3df928" | "0x8e" => "Linux LVM".into(),
        "a19d880f-05fc-4d3b-a006-743f0f84911e" | "0xfd" => "Linux RAID".into(),
        "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" | "0x07" => tr!("Dados do Windows").into(),
        "0x0b" | "0x0c" => "FAT32".into(),
        "0x0e" | "0x06" => "FAT16".into(),
        "e3c9e316-0b5c-4db8-817d-f92df00215ae" => tr!("Reservada do Windows").into(),
        "de94bba4-06d1-4d40-a16a-bfd50179d6ac" | "0x27" => tr!("Recuperação do Windows").into(),
        "21686148-6449-6e6f-744e-656564454649" => tr!("Inicialização da BIOS").into(),
        "48465300-0000-11aa-aa11-00306543ecac" | "0xaf" => "Mac (HFS+)".into(),
        "7c3457ef-0000-11aa-aa11-00306543ecac" => "Mac (APFS)".into(),
        "0x05" | "0x0f" | "0x85" => tr!("Estendida").into(),
        _ => t,
    }
}

/// Nome do conteúdo (sistema de arquivos).
pub fn fs_name(t: &str) -> String {
    use glove_config::tr;
    match t {
        "ext4" => "Ext4 (Linux)".into(),
        "ext3" => "Ext3 (Linux)".into(),
        "ext2" => "Ext2 (Linux)".into(),
        "btrfs" => "Btrfs (Linux)".into(),
        "xfs" => "XFS (Linux)".into(),
        "f2fs" => "F2FS (Linux)".into(),
        "vfat" => "FAT".into(),
        "exfat" => "exFAT".into(),
        "ntfs" => "NTFS (Windows)".into(),
        "swap" => tr!("Troca (swap)").into(),
        "crypto_LUKS" => tr!("Criptografado (LUKS)").into(),
        "iso9660" => "ISO 9660 (CD/DVD)".into(),
        "udf" => "UDF (DVD)".into(),
        "hfsplus" => "HFS+ (Mac)".into(),
        "apfs" => "APFS (Mac)".into(),
        "LVM2_member" => "LVM".into(),
        "" => tr!("Desconhecido").into(),
        o => o.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn espacos_livres() {
        const G: u64 = 1024 * MIB;
        // Disco de 10 GiB: uma partição de 1 MiB a 2 GiB e outra de 5 a 8 GiB.
        let v = free_between(&[(MIB, 2 * G - MIB), (5 * G, 3 * G)], 10 * G);
        let kinds: Vec<Option<usize>> = v.iter().map(|x| x.0).collect();
        assert_eq!(kinds, vec![Some(0), None, Some(1), None]);
        assert_eq!(v[1].1, (2 * G, 3 * G));
        assert_eq!(v[3].1, (8 * G, 2 * G - MIB));
        // Disco vazio (só a tabela): tudo livre menos o começo e o fim.
        assert_eq!(free_between(&[], 10 * G), vec![(None, (MIB, 10 * G - 2 * MIB))]);
    }

    #[test]
    fn ordem_natural() {
        let mut v = vec!["/dev/sda10", "/dev/sda2", "/dev/sda1"];
        v.sort_by_key(|s| natural(s));
        assert_eq!(v, vec!["/dev/sda1", "/dev/sda2", "/dev/sda10"]);
    }
}
