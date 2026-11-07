//! Rede pelo NetworkManager (`nmcli`): interfaces, redes Wi-Fi, conectar e
//! TCP/IP. Usado pelo painel Rede e pelo menu Wi-Fi da barra.

use crate::tr;
use std::process::Command;

/// Campos de uma linha do `nmcli -t` (":" separa; "\:" e "\\" são escapes).
pub fn split_terse(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut esc = false;
    for c in line.chars() {
        if esc {
            out.last_mut().unwrap().push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ':' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    out
}

fn nmcli(args: &[&str]) -> Option<String> {
    let out = Command::new("nmcli").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Iface {
    pub device: String,
    /// "wifi", "ethernet"…
    pub kind: String,
    /// "connected", "disconnected", "unavailable", "connecting (…)"…
    pub state: String,
    pub connection: String,
}

impl Iface {
    pub fn connected(&self) -> bool {
        self.state == "connected"
    }

    /// Nome para mostrar ("Wi-Fi", "Ethernet").
    pub fn label(&self) -> String {
        match self.kind.as_str() {
            "wifi" => "Wi-Fi".into(),
            "ethernet" => tr!("Ethernet").into(),
            "bt" => tr!("Bluetooth").into(),
            "gsm" | "cdma" => tr!("Celular").into(),
            k => k.to_string(),
        }
    }
}

pub fn parse_devices(text: &str) -> Vec<Iface> {
    text.lines()
        .map(split_terse)
        .filter(|f| f.len() >= 4)
        .map(|f| Iface { device: f[0].clone(), kind: f[1].clone(), state: f[2].clone(), connection: f[3].clone() })
        .filter(|i| matches!(i.kind.as_str(), "wifi" | "ethernet" | "gsm" | "cdma"))
        .collect()
}

/// Interfaces de rede de verdade (sem loopback nem p2p). None = sem NetworkManager.
pub fn devices() -> Option<Vec<Iface>> {
    nmcli(&["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"]).map(|t| parse_devices(&t))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wifi {
    pub ssid: String,
    /// 0 a 100.
    pub signal: u8,
    pub secure: bool,
    pub in_use: bool,
}

/// Redes visíveis, sem nomes vazios nem repetidos (fica o sinal mais forte), da mais forte para a mais fraca.
pub fn parse_wifi(text: &str) -> Vec<Wifi> {
    let mut out: Vec<Wifi> = Vec::new();
    for f in text.lines().map(split_terse).filter(|f| f.len() >= 4) {
        let ssid = f[1].clone();
        if ssid.is_empty() {
            continue;
        }
        let w = Wifi { ssid, signal: f[2].parse().unwrap_or(0), secure: !f[3].trim().is_empty() && f[3] != "--", in_use: f[0] == "*" };
        match out.iter_mut().find(|o| o.ssid == w.ssid) {
            Some(o) => {
                o.in_use |= w.in_use;
                o.signal = o.signal.max(w.signal);
            }
            None => out.push(w),
        }
    }
    out.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.signal.cmp(&a.signal)));
    out
}

/// Redes Wi-Fi visíveis. `rescan` pede uma busca nova (mais lento).
pub fn wifi_list(rescan: bool) -> Vec<Wifi> {
    nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", if rescan { "yes" } else { "no" }]).map(|t| parse_wifi(&t)).unwrap_or_default()
}

pub fn wifi_enabled() -> Option<bool> {
    nmcli(&["radio", "wifi"]).map(|t| t.trim() == "enabled")
}

pub fn set_wifi(on: bool) -> Result<(), String> {
    run(&["radio", "wifi", if on { "on" } else { "off" }])
}

/// Nomes das conexões guardadas (para saber se uma rede já tem senha salva).
pub fn saved_connections() -> Vec<String> {
    nmcli(&["-t", "-f", "NAME", "connection", "show"]).map(|t| t.lines().map(|l| split_terse(l).remove(0)).collect()).unwrap_or_default()
}

/// Roda o nmcli e devolve a mensagem de erro dele, em uma linha.
fn run(args: &[&str]) -> Result<(), String> {
    let out = Command::new("nmcli").args(args).output().map_err(|e| format!("Não foi possível usar o nmcli: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    Err(err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(tr!("o NetworkManager recusou")).trim().trim_start_matches("Error: ").to_string())
}

/// Conecta a uma rede (bloqueia até o NetworkManager responder: use numa thread).
pub fn connect(ssid: &str, password: Option<&str>) -> Result<(), String> {
    if password.is_none() && saved_connections().iter().any(|c| c == ssid) {
        return run(&["connection", "up", "id", ssid]);
    }
    let mut args = vec!["device", "wifi", "connect", ssid];
    if let Some(p) = password {
        args.extend(["password", p]);
    }
    run(&args)
}

pub fn disconnect(device: &str) -> Result<(), String> {
    run(&["device", "disconnect", device])
}

/// Endereços de uma interface (como o painel TCP/IP do 10.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IpInfo {
    pub hwaddr: String,
    /// "192.168.0.5/24"
    pub address: String,
    pub gateway: String,
    pub dns: Vec<String>,
    /// Configurado à mão (ipv4.method manual) em vez de DHCP.
    pub manual: bool,
}

pub fn parse_ip(text: &str) -> IpInfo {
    let mut info = IpInfo::default();
    for f in text.lines().map(split_terse).filter(|f| f.len() >= 2) {
        let v = f[1..].join(":");
        match f[0].split('[').next().unwrap_or("") {
            "GENERAL.HWADDR" => info.hwaddr = v,
            "IP4.ADDRESS" if info.address.is_empty() => info.address = v,
            "IP4.GATEWAY" => info.gateway = v,
            "IP4.DNS" => info.dns.push(v),
            _ => {}
        }
    }
    info
}

pub fn ip_info(iface: &Iface) -> IpInfo {
    let mut info = nmcli(&["-t", "-f", "GENERAL.HWADDR,IP4.ADDRESS,IP4.GATEWAY,IP4.DNS", "device", "show", &iface.device]).map(|t| parse_ip(&t)).unwrap_or_default();
    if !iface.connection.is_empty() {
        info.manual = nmcli(&["-g", "ipv4.method", "connection", "show", &iface.connection]).is_some_and(|m| m.trim() == "manual");
    }
    info
}

/// Muda o IPv4 da conexão: None = DHCP; Some((endereço/prefixo, roteador, DNS)) = manual.
/// Reativa a conexão para valer na hora.
pub fn set_ipv4(connection: &str, manual: Option<(&str, &str, &str)>) -> Result<(), String> {
    match manual {
        None => run(&["connection", "modify", connection, "ipv4.method", "auto", "ipv4.addresses", "", "ipv4.gateway", "", "ipv4.dns", ""])?,
        Some((addr, gw, dns)) => run(&["connection", "modify", connection, "ipv4.method", "manual", "ipv4.addresses", addr, "ipv4.gateway", gw, "ipv4.dns", dns])?,
    }
    run(&["connection", "up", "id", connection])
}

/// Máscara "255.255.255.0" a partir do prefixo 24.
pub fn mask(prefix: u32) -> String {
    let m: u32 = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix.min(32)) };
    format!("{}.{}.{}.{}", m >> 24, (m >> 16) & 255, (m >> 8) & 255, m & 255)
}

/// Prefixo 24 a partir da máscara "255.255.255.0" (None se não for máscara válida).
pub fn prefix(mask: &str) -> Option<u32> {
    let parts: Vec<u8> = mask.split('.').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    if parts.len() != 4 {
        return None;
    }
    let m = u32::from_be_bytes([parts[0], parts[1], parts[2], parts[3]]);
    (m.leading_ones() + m.trailing_zeros() == 32 || m == 0).then(|| m.leading_ones())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campos_com_escape() {
        assert_eq!(split_terse(r"D4\:17\:61:bt:disconnected:"), ["D4:17:61", "bt", "disconnected", ""]);
        assert_eq!(split_terse(r"a\\b:c"), [r"a\b", "c"]);
    }

    #[test]
    fn interfaces_e_redes() {
        let d = parse_devices("wlp1s0:wifi:connected:Casa\nlo:loopback:connected (externally):lo\np2p-dev-wlp1s0:wifi-p2p:disconnected:\n");
        assert_eq!(d.len(), 1);
        assert!(d[0].connected());
        assert_eq!(d[0].label(), "Wi-Fi");
        let w = parse_wifi(" :Casa:100:WPA2\n ::89:WPA2\n :Aberta:40:\n*:Casa:69:WPA2\n :Vizinho:77:WPA1 WPA2\n");
        assert_eq!(w.iter().map(|w| w.ssid.as_str()).collect::<Vec<_>>(), ["Casa", "Vizinho", "Aberta"]);
        assert!(w[0].in_use && w[0].signal == 100);
        assert!(!w[2].secure);
    }

    #[test]
    fn enderecos() {
        let i = parse_ip("GENERAL.HWADDR:AA\\:BB\\:CC\nIP4.ADDRESS[1]:192.168.0.5/24\nIP4.GATEWAY:192.168.0.1\nIP4.DNS[1]:8.8.8.8\nIP4.DNS[2]:1.1.1.1\n");
        assert_eq!(i.hwaddr, "AA:BB:CC");
        assert_eq!(i.address, "192.168.0.5/24");
        assert_eq!(i.dns, ["8.8.8.8", "1.1.1.1"]);
        assert_eq!(mask(24), "255.255.255.0");
        assert_eq!(mask(20), "255.255.240.0");
        assert_eq!(prefix("255.255.255.0"), Some(24));
        assert_eq!(prefix("255.0.255.0"), None);
    }
}
