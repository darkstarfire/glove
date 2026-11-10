//! Serviços que costumam pesar e quase sempre podem ficar desligados: impressão,
//! Bluetooth, acesso remoto, servidores de desenvolvimento… Aparecem só os que
//! estão instalados. Os do sistema pedem a senha de administrador (pkexec); os
//! da conta (`systemctl --user`) mudam direto.

use glove_config::tr;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

/// Um grupo de unidades do systemd que liga e desliga junto.
pub struct Known {
    pub name: &'static str,
    pub desc: &'static str,
    pub units: &'static [&'static str],
    pub user: bool,
}

/// Os grupos conhecidos (nomes já traduzidos).
pub fn known() -> &'static [Known] {
    static K: std::sync::OnceLock<Vec<Known>> = std::sync::OnceLock::new();
    K.get_or_init(|| vec![
        Known { name: tr!("Impressão"), desc: tr!("Imprimir e achar impressoras na rede. Sem impressora, pode desligar."), units: &["cups.service", "cups.socket", "cups.path", "cups-browsed.service"], user: false },
        Known { name: tr!("Bluetooth"), desc: tr!("Fones, mouses e teclados sem fio."), units: &["bluetooth.service"], user: false },
        Known { name: tr!("Transferência por Bluetooth"), desc: tr!("Mandar e receber arquivos pelo Bluetooth."), units: &["obex.service"], user: true },
        Known { name: tr!("Modems 3G/4G"), desc: tr!("Internet por modem USB ou chip de celular."), units: &["ModemManager.service"], user: false },
        Known { name: tr!("Descoberta na rede (Avahi)"), desc: tr!("Acha impressoras e computadores na rede local."), units: &["avahi-daemon.service", "avahi-daemon.socket"], user: false },
        Known { name: tr!("AnyDesk"), desc: tr!("Acesso remoto pelo AnyDesk."), units: &["anydesk.service"], user: false },
        Known { name: tr!("TeamViewer"), desc: tr!("Acesso remoto pelo TeamViewer."), units: &["teamviewerd.service"], user: false },
        Known { name: tr!("Acesso por SSH"), desc: tr!("Entrar neste computador pela rede com SSH."), units: &["ssh.service", "ssh.socket"], user: false },
        Known { name: tr!("Docker"), desc: tr!("Contêineres de desenvolvimento."), units: &["docker.service", "docker.socket", "containerd.service"], user: false },
        Known { name: tr!("Máquinas virtuais (libvirt)"), desc: tr!("Máquinas virtuais do virt-manager e do GNOME Boxes."), units: &["libvirtd.service", "libvirtd.socket"], user: false },
        Known { name: tr!("Servidor web Apache"), desc: tr!("Servidor de páginas para desenvolvimento."), units: &["apache2.service"], user: false },
        Known { name: tr!("Servidor web nginx"), desc: tr!("Servidor de páginas para desenvolvimento."), units: &["nginx.service"], user: false },
        Known { name: tr!("Banco de dados PostgreSQL"), desc: tr!("Banco de dados para desenvolvimento."), units: &["postgresql.service"], user: false },
        Known { name: tr!("Banco de dados MySQL/MariaDB"), desc: tr!("Banco de dados para desenvolvimento."), units: &["mariadb.service", "mysql.service"], user: false },
        Known { name: tr!("Compartilhar com Windows (Samba)"), desc: tr!("Pastas deste computador visíveis na rede do Windows."), units: &["smbd.service", "nmbd.service"], user: false },
        Known { name: tr!("E-mail do sistema (Exim)"), desc: tr!("Entrega os e-mails internos do sistema; quase ninguém precisa."), units: &["exim4.service", "exim4-base.timer"], user: false },
        Known { name: tr!("Atualizações automáticas"), desc: tr!("Procura e baixa atualizações do sistema todo dia."), units: &["apt-daily.timer", "apt-daily-upgrade.timer", "unattended-upgrades.service"], user: false },
        Known { name: tr!("Snap"), desc: tr!("Programas instalados pela loja Snap."), units: &["snapd.service", "snapd.socket"], user: false },
        Known { name: tr!("Indexador de arquivos do GNOME"), desc: tr!("Lê o conteúdo dos arquivos para a busca do GNOME; o Glove não usa."), units: &["localsearch-3.service", "tracker-miner-fs-3.service"], user: true },
    ])
}

/// Estado de um grupo instalado.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub known: usize,
    /// Unidades que existem neste sistema.
    pub units: Vec<String>,
    pub on: bool,
    pub running: bool,
    /// Memória em uso (bytes), quando o systemd sabe.
    pub memory: Option<u64>,
}

/// Uma unidade como `systemctl show` descreve.
#[derive(Default, Debug, PartialEq)]
struct Unit {
    id: String,
    found: bool,
    enabled: bool,
    active: bool,
    memory: Option<u64>,
}

/// Lê a saída de `systemctl show -p Id,LoadState,UnitFileState,ActiveState,MemoryCurrent`.
fn parse_show(text: &str) -> Vec<Unit> {
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let mut u = Unit::default();
        for line in block.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "Id" => u.id = v.to_string(),
                "LoadState" => u.found = v != "not-found" && v != "masked",
                "UnitFileState" => u.enabled = matches!(v, "enabled" | "enabled-runtime"),
                "ActiveState" => u.active = v == "active" || v == "activating" || v == "reloading",
                // "[not set]" ou 2^64-1 quando não há contagem.
                "MemoryCurrent" => u.memory = v.parse::<u64>().ok().filter(|m| *m < u64::MAX / 2),
                _ => {}
            }
        }
        if !u.id.is_empty() {
            out.push(u);
        }
    }
    out
}

fn show(units: &[&str], user: bool) -> Vec<Unit> {
    if units.is_empty() {
        return Vec::new();
    }
    let mut c = Command::new("systemctl");
    if user {
        c.arg("--user");
    }
    c.args(["show", "--no-pager", "-p", "Id,LoadState,UnitFileState,ActiveState,MemoryCurrent"]).args(units);
    c.output().map(|o| parse_show(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
}

fn group(idx: usize, units: &[Unit]) -> Option<Service> {
    let k = &known()[idx];
    let mine: Vec<&Unit> = k.units.iter().filter_map(|n| units.iter().find(|u| u.id == *n && u.found)).collect();
    if mine.is_empty() {
        return None;
    }
    let memory = mine.iter().filter_map(|u| u.memory).reduce(|a, b| a + b);
    Some(Service { known: idx, units: mine.iter().map(|u| u.id.clone()).collect(), on: mine.iter().any(|u| u.enabled || u.active), running: mine.iter().any(|u| u.active), memory })
}

/// Os grupos instalados, na ordem da lista.
pub fn installed() -> Vec<Service> {
    let names = |user: bool| known().iter().filter(|k| k.user == user).flat_map(|k| k.units.iter().copied()).collect::<Vec<_>>();
    let (sys, usr) = (show(&names(false), false), show(&names(true), true));
    (0..known().len()).filter_map(|i| group(i, if known()[i].user { &usr } else { &sys })).collect()
}

/// O serviço é do sistema (precisa da senha de administrador para mudar)?
pub fn needs_admin(s: &Service) -> bool {
    !known()[s.known].user
}

/// Liga ou desliga (agora e nas próximas entradas). Roda numa linha à parte; o
/// resultado chega pelo canal. `password`: a de administrador, para os do sistema
/// (None quando o sudo não pede).
pub fn set(s: &Service, on: bool, password: Option<String>) -> Receiver<Result<(), String>> {
    let verb = if on { "enable" } else { "disable" };
    if needs_admin(s) {
        let mut args = vec!["systemctl".to_string(), verb.to_string(), "--now".to_string()];
        args.extend(s.units.iter().cloned());
        return crate::admin::run(args, password);
    }
    let (tx, rx) = channel();
    let units = s.units.clone();
    std::thread::spawn(move || {
        let r = match Command::new("systemctl").args(["--user", verb, "--now"]).args(&units).output() {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").trim().to_string()),
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(r);
    });
    rx
}

/// "45 MB", "1,2 GB".
pub fn human(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0).replace('.', ",")
    } else if mb >= 10.0 {
        format!("{} MB", mb.round() as u64)
    } else {
        format!("{:.1} MB", mb).replace('.', ",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_systemctl() {
        let t = "Id=cups.service\nLoadState=loaded\nUnitFileState=enabled\nActiveState=active\nMemoryCurrent=10485760\n\nId=cups.path\nLoadState=not-found\nUnitFileState=\nActiveState=inactive\nMemoryCurrent=[not set]\n\nId=cups-browsed.service\nLoadState=loaded\nUnitFileState=disabled\nActiveState=inactive\nMemoryCurrent=18446744073709551615\n";
        let u = parse_show(t);
        assert_eq!(u.len(), 3);
        assert!(u[0].found && u[0].enabled && u[0].active && u[0].memory == Some(10 << 20));
        assert!(!u[1].found && u[1].memory.is_none());
        assert!(u[2].found && !u[2].enabled && !u[2].active && u[2].memory.is_none());
        let g = group(0, &u).unwrap();
        assert_eq!(g.units, vec!["cups.service", "cups-browsed.service"]);
        assert!(g.on && g.running && g.memory == Some(10 << 20));
        // Nenhuma unidade instalada: o grupo não aparece.
        assert!(group(1, &u).is_none());
    }

    #[test]
    fn tamanhos() {
        assert_eq!(human(5 << 20), "5,0 MB");
        assert_eq!(human(45 << 20), "45 MB");
        assert_eq!(human(1536 << 20), "1,5 GB");
    }
}
