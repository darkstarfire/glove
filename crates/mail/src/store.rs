//! Contas (em ~/.config/glove/correio.toml, só o dono lê) e a cópia local
//! das pastas (cabeçalhos e mensagens já abertas), para ler sem rede.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::net::Security;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub name: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    #[serde(default)]
    pub imap_sec: Security,
    pub smtp_host: String,
    pub smtp_port: u16,
    #[serde(default = "starttls")]
    pub smtp_sec: Security,
    pub user: String,
    pub password: String,
}

fn starttls() -> Security {
    Security::StartTls
}

#[derive(Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    account: Vec<Account>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn config_file() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config")).join("glove/correio.toml")
}

pub fn cache_root() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share")).join("glove/correio")
}

pub fn load_accounts() -> Vec<Account> {
    std::fs::read_to_string(config_file()).ok().and_then(|t| toml::from_str::<File>(&t).ok()).map(|f| f.account).unwrap_or_default()
}

pub fn save_accounts(v: &[Account]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let p = config_file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    let text = toml::to_string(&File { account: v.to_vec() }).map_err(std::io::Error::other)?;
    // A senha fica no arquivo: só o dono pode ler.
    let tmp = p.with_extension("toml.tmp");
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    std::io::Write::write_all(&mut f, text.as_bytes())?;
    std::fs::rename(&tmp, &p)
}

/// Servidores prováveis pelo domínio do endereço.
pub fn guess(email: &str) -> Account {
    let domain = email.rsplit('@').next().unwrap_or("").to_lowercase();
    let (imap, smtp, smtp_port, smtp_sec) = match domain.as_str() {
        "gmail.com" | "googlemail.com" => ("imap.gmail.com".to_string(), "smtp.gmail.com".to_string(), 465, Security::Tls),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" | "outlook.com.br" | "hotmail.com.br" => ("outlook.office365.com".into(), "smtp.office365.com".into(), 587, Security::StartTls),
        "yahoo.com" | "yahoo.com.br" => ("imap.mail.yahoo.com".into(), "smtp.mail.yahoo.com".into(), 465, Security::Tls),
        "icloud.com" | "me.com" | "mac.com" => ("imap.mail.me.com".into(), "smtp.mail.me.com".into(), 587, Security::StartTls),
        "uol.com.br" => ("imap.uol.com.br".into(), "smtps.uol.com.br".into(), 587, Security::StartTls),
        "terra.com.br" => ("imap.terra.com.br".into(), "smtp.terra.com.br".into(), 587, Security::StartTls),
        "zoho.com" => ("imap.zoho.com".into(), "smtp.zoho.com".into(), 465, Security::Tls),
        "" => (String::new(), String::new(), 587, Security::StartTls),
        d => (format!("imap.{d}"), format!("smtp.{d}"), 587, Security::StartTls),
    };
    Account { name: String::new(), email: email.to_string(), imap_host: imap, imap_port: 993, imap_sec: Security::Tls, smtp_host: smtp, smtp_port, smtp_sec, user: email.to_string(), password: String::new() }
}

/// Uma mensagem na lista (o que se guarda da pasta).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Row {
    pub uid: u32,
    pub seen: bool,
    #[serde(default)]
    pub flagged: bool,
    #[serde(default)]
    pub answered: bool,
    pub size: u32,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub cc: String,
    pub subject: String,
    pub date: i64,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub references: String,
    /// Tem anexo (palpite pelo Content-Type).
    #[serde(default)]
    pub attach: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct FolderCache {
    validity: u32,
    #[serde(default)]
    row: Vec<Row>,
}

fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || matches!(c, '.' | '@' | '-' | '_') { c } else { '_' }).collect()
}

pub fn folder_dir(email: &str, folder: &str) -> PathBuf {
    cache_root().join(safe(email)).join(safe(folder))
}

pub fn load_rows(email: &str, folder: &str) -> (u32, Vec<Row>) {
    let p = folder_dir(email, folder).join("lista.toml");
    std::fs::read_to_string(p).ok().and_then(|t| toml::from_str::<FolderCache>(&t).ok()).map(|c| (c.validity, c.row)).unwrap_or_default()
}

pub fn save_rows(email: &str, folder: &str, validity: u32, rows: &[Row]) {
    let d = folder_dir(email, folder);
    if std::fs::create_dir_all(&d).is_err() {
        return;
    }
    if let Ok(t) = toml::to_string(&FolderCache { validity, row: rows.to_vec() }) {
        let _ = std::fs::write(d.join("lista.toml"), t);
    }
}

pub fn body_path(email: &str, folder: &str, uid: u32) -> PathBuf {
    folder_dir(email, folder).join(format!("{uid}.eml"))
}

/// Apaga a cópia local de uma pasta (UIDVALIDITY mudou).
pub fn clear_folder(email: &str, folder: &str) {
    let _ = std::fs::remove_dir_all(folder_dir(email, folder));
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// "Tue, 5 Nov 2026 10:00:00 -0300" → segundos desde 1970 (UTC).
pub fn parse_date(s: &str) -> i64 {
    let s = s.split('(').next().unwrap_or(s);
    let s = s.split_once(',').map(|x| x.1).unwrap_or(s);
    let w: Vec<&str> = s.split_whitespace().collect();
    if w.len() < 4 {
        return 0;
    }
    let d: i64 = w[0].parse().unwrap_or(1);
    const M: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let m = M.iter().position(|x| w[1].to_lowercase().starts_with(x)).map(|i| i as i64 + 1).unwrap_or(1);
    let mut y: i64 = w[2].parse().unwrap_or(1970);
    if y < 100 {
        y += if y < 50 { 2000 } else { 1900 };
    }
    let t: Vec<i64> = w[3].split(':').map(|x| x.parse().unwrap_or(0)).collect();
    let secs = t.first().copied().unwrap_or(0) * 3600 + t.get(1).copied().unwrap_or(0) * 60 + t.get(2).copied().unwrap_or(0);
    let off = w.get(4).map(|z| {
        let z = z.trim();
        match z.as_bytes().first() {
            Some(b'+') | Some(b'-') if z.len() >= 5 => {
                let v: i64 = z[1..5].parse().unwrap_or(0);
                let o = (v / 100) * 3600 + (v % 100) * 60;
                if z.starts_with('-') { -o } else { o }
            }
            _ => match z.to_uppercase().as_str() {
                "EST" => -5 * 3600,
                "EDT" => -4 * 3600,
                "PST" => -8 * 3600,
                "PDT" => -7 * 3600,
                _ => 0,
            },
        }
    });
    days_from_civil(y, m, d) * 86400 + secs - off.unwrap_or(0)
}

/// Data curta no fuso local: "Hoje 10:05", "Ontem", "05/11/26".
pub fn short_date(t: i64, now: i64) -> String {
    if t == 0 {
        return String::new();
    }
    let lt = |t: i64| unsafe {
        let tt = t as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&tt, &mut tm);
        tm
    };
    let (a, b) = (lt(t), lt(now));
    let same_day = a.tm_year == b.tm_year && a.tm_yday == b.tm_yday;
    let yesterday = lt(now - 86400);
    if same_day {
        format!("Hoje {:02}:{:02}", a.tm_hour, a.tm_min)
    } else if a.tm_year == yesterday.tm_year && a.tm_yday == yesterday.tm_yday {
        format!("Ontem {:02}:{:02}", a.tm_hour, a.tm_min)
    } else {
        format!("{:02}/{:02}/{:02}", a.tm_mday, a.tm_mon + 1, (a.tm_year + 1900) % 100)
    }
}

/// Data completa no fuso local: "05/11/2026 10:05".
pub fn long_date(t: i64) -> String {
    let a = unsafe {
        let tt = t as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&tt, &mut tm);
        tm
    };
    format!("{:02}/{:02}/{} {:02}:{:02}", a.tm_mday, a.tm_mon + 1, a.tm_year + 1900, a.tm_hour, a.tm_min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas() {
        assert_eq!(parse_date("Thu, 1 Jan 1970 00:00:00 +0000"), 0);
        assert_eq!(parse_date("Thu, 05 Nov 2026 10:00:00 -0300"), parse_date("5 Nov 2026 13:00:00 +0000"));
        assert_eq!(parse_date("Sat, 14 Nov 2026 12:00:00 +0000 (UTC)"), 1_794_657_600);
    }

    #[test]
    fn palpite_de_servidor() {
        let a = guess("fulano@gmail.com");
        assert_eq!(a.imap_host, "imap.gmail.com");
        assert_eq!(a.smtp_port, 465);
        assert_eq!(guess("eu@empresa.com.br").smtp_host, "smtp.empresa.com.br");
    }
}
