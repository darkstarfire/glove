//! Conversa com o Secret Service (org.freedesktop.secrets) pelo D-Bus: chaveiros,
//! itens, segredos (sessão "plain", local), apagar, trancar/destrancar e mudar a
//! senha do chaveiro (a interface do gnome-keyring para isso).

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

const DEST: &str = "org.freedesktop.secrets";
const ROOT: &str = "/org/freedesktop/secrets";
const SERVICE: &str = "org.freedesktop.Secret.Service";
const COLLECTION: &str = "org.freedesktop.Secret.Collection";
const ITEM: &str = "org.freedesktop.Secret.Item";
const PROPS: &str = "org.freedesktop.DBus.Properties";

#[derive(Clone, Debug)]
pub struct Item {
    pub path: String,
    pub label: String,
    pub kind: String,
    pub account: String,
    pub place: String,
    pub modified: u64,
}

#[derive(Clone, Debug)]
pub struct Keyring {
    pub path: String,
    pub label: String,
    pub locked: bool,
    pub items: Vec<Item>,
}

pub enum Backend {
    Dbus { conn: Option<Connection>, session: Option<OwnedObjectPath>, error: Option<String> },
    /// Itens de mentira (GLOVE_CHAVES_TESTE=1).
    Fake(Vec<Keyring>, HashMap<String, String>),
}

/// "05/11/2026 18:42".
pub fn date(secs: u64) -> String {
    if secs == 0 {
        return "—".into();
    }
    unsafe {
        let t = secs as libc_time;
        let mut tm: Tm = std::mem::zeroed();
        localtime_r(&t, &mut tm);
        format!("{:02}/{:02}/{} {:02}:{:02}", tm.tm_mday, tm.tm_mon + 1, tm.tm_year + 1900, tm.tm_hour, tm.tm_min)
    }
}

#[allow(non_camel_case_types)]
type libc_time = i64;
#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const u8,
}
unsafe extern "C" {
    fn localtime_r(t: *const libc_time, tm: *mut Tm) -> *mut Tm;
}

/// Tipo e onde/conta a partir dos atributos (como o Keychain mostra "senha de rede", "senha de app").
fn describe(attrs: &HashMap<String, String>) -> (String, String, String) {
    let schema = attrs.get("xdg:schema").map(String::as_str).unwrap_or("");
    let kind = if schema.contains("NetworkManager") {
        "Senha de rede (Wi-Fi/VPN)"
    } else if schema.contains("NetworkPassword") || attrs.contains_key("server") || attrs.contains_key("host") {
        "Senha de rede"
    } else if schema.contains("chrome") || schema.contains("Chrome") || attrs.contains_key("application") {
        "Senha de app"
    } else {
        "Senha"
    };
    let get = |keys: &[&str]| keys.iter().find_map(|k| attrs.get(*k)).cloned().unwrap_or_default();
    let account = get(&["user", "username", "account", "login", "email"]);
    let place = get(&["server", "host", "domain", "ssid", "connection-id", "service", "application", "origin", "uri", "setting-name"]);
    (kind.into(), account, place)
}

fn string(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        Value::Value(b) => string(b),
        _ => String::new(),
    }
}

fn boolean(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Value(b) => boolean(b),
        _ => false,
    }
}

fn uint(v: &Value) -> u64 {
    match v {
        Value::U64(n) => *n,
        Value::U32(n) => *n as u64,
        Value::Value(b) => uint(b),
        _ => 0,
    }
}

fn paths(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter_map(|x| if let Value::ObjectPath(p) = x { Some(p.to_string()) } else { None }).collect(),
        Value::Value(b) => paths(b),
        _ => Vec::new(),
    }
}

fn dict(v: &Value) -> HashMap<String, String> {
    match v {
        Value::Dict(d) => d.iter().map(|(k, v)| (string(k), string(v))).collect(),
        Value::Value(b) => dict(b),
        _ => HashMap::new(),
    }
}

impl Backend {
    pub fn new() -> Self {
        if std::env::var_os("GLOVE_CHAVES_TESTE").is_some() {
            return fake();
        }
        match Connection::session() {
            Ok(c) => Backend::Dbus { conn: Some(c), session: None, error: None },
            Err(e) => Backend::Dbus { conn: None, session: None, error: Some(format!("Sem o serviço de senhas: {e}")) },
        }
    }

    fn conn(&self) -> Result<&Connection, String> {
        match self {
            Backend::Dbus { conn: Some(c), .. } => Ok(c),
            Backend::Dbus { error, .. } => Err(error.clone().unwrap_or_else(|| "Sem o serviço de senhas.".into())),
            Backend::Fake(..) => Err("teste".into()),
        }
    }

    fn props(&self, path: &str, iface: &str) -> Result<HashMap<String, OwnedValue>, String> {
        let c = self.conn()?;
        let r = c.call_method(Some(DEST), path, Some(PROPS), "GetAll", &(iface,)).map_err(|e| e.to_string())?;
        r.body().deserialize::<HashMap<String, OwnedValue>>().map_err(|e| e.to_string())
    }

    pub fn keyrings(&mut self) -> Result<Vec<Keyring>, String> {
        if let Backend::Fake(r, _) = self {
            return Ok(r.clone());
        }
        let svc = self.props(ROOT, SERVICE)?;
        let cols = svc.get("Collections").map(|v| paths(v)).unwrap_or_default();
        let mut out = Vec::new();
        for c in cols {
            let p = self.props(&c, COLLECTION)?;
            let label = p.get("Label").map(|v| string(v)).filter(|l| !l.is_empty()).unwrap_or_else(|| c.rsplit('/').next().unwrap_or("").to_string());
            let locked = p.get("Locked").is_some_and(|v| boolean(v));
            let mut items = Vec::new();
            for ip in p.get("Items").map(|v| paths(v)).unwrap_or_default() {
                let Ok(ipr) = self.props(&ip, ITEM) else { continue };
                let attrs = ipr.get("Attributes").map(|v| dict(v)).unwrap_or_default();
                let (kind, account, place) = describe(&attrs);
                items.push(Item {
                    label: ipr.get("Label").map(|v| string(v)).unwrap_or_default(),
                    modified: ipr.get("Modified").map(|v| uint(v)).unwrap_or(0),
                    path: ip,
                    kind,
                    account,
                    place,
                });
            }
            out.push(Keyring { path: c, label, locked, items });
        }
        Ok(out)
    }

    fn session(&mut self) -> Result<OwnedObjectPath, String> {
        if let Backend::Dbus { session: Some(s), .. } = self {
            return Ok(s.clone());
        }
        let c = self.conn()?;
        let r = c.call_method(Some(DEST), ROOT, Some(SERVICE), "OpenSession", &("plain", Value::from(""))).map_err(|e| e.to_string())?;
        let (_, s): (OwnedValue, OwnedObjectPath) = r.body().deserialize().map_err(|e| e.to_string())?;
        if let Backend::Dbus { session, .. } = self {
            *session = Some(s.clone());
        }
        Ok(s)
    }

    pub fn secret(&mut self, item: &str) -> Result<String, String> {
        if let Backend::Fake(_, s) = self {
            return s.get(item).cloned().ok_or_else(|| "Sem senha.".into());
        }
        let s = self.session()?;
        let c = self.conn()?;
        let r = c.call_method(Some(DEST), item, Some(ITEM), "GetSecret", &(s,)).map_err(|e| e.to_string())?;
        let (_, _, value, _): (OwnedObjectPath, Vec<u8>, Vec<u8>, String) = r.body().deserialize().map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&value).into_owned())
    }

    pub fn delete(&mut self, item: &str) -> Result<(), String> {
        if let Backend::Fake(r, _) = self {
            for k in r.iter_mut() {
                k.items.retain(|i| i.path != item);
            }
            return Ok(());
        }
        let c = self.conn()?;
        let r = c.call_method(Some(DEST), item, Some(ITEM), "Delete", &()).map_err(|e| e.to_string())?;
        let prompt: OwnedObjectPath = r.body().deserialize().map_err(|e| e.to_string())?;
        self.prompt(&prompt)
    }

    /// Alguns pedidos voltam com um "prompt" (o serviço pergunta a senha na janela dele).
    fn prompt(&self, p: &OwnedObjectPath) -> Result<(), String> {
        if p.as_str() == "/" {
            return Ok(());
        }
        let c = self.conn()?;
        c.call_method(Some(DEST), p.as_str(), Some("org.freedesktop.Secret.Prompt"), "Prompt", &("",)).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn lock_call(&mut self, path: &str, method: &str) -> Result<(), String> {
        if let Backend::Fake(r, _) = self {
            if let Some(k) = r.iter_mut().find(|k| k.path == path) {
                k.locked = method == "Lock";
            }
            return Ok(());
        }
        let c = self.conn()?;
        let obj = ObjectPath::try_from(path).map_err(|e| e.to_string())?;
        let r = c.call_method(Some(DEST), ROOT, Some(SERVICE), method, &(vec![obj],)).map_err(|e| e.to_string())?;
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = r.body().deserialize().map_err(|e| e.to_string())?;
        self.prompt(&prompt)
    }

    pub fn lock(&mut self, path: &str) -> Result<(), String> {
        self.lock_call(path, "Lock")
    }

    pub fn unlock(&mut self, path: &str) -> Result<(), String> {
        self.lock_call(path, "Unlock")
    }

    /// Mudar a senha do chaveiro (gnome-keyring): a atual e a nova vão pela sessão local.
    pub fn change_password(&mut self, path: &str, old: &str, new: &str) -> Result<(), String> {
        if let Backend::Fake(..) = self {
            return if old == "teste" { Ok(()) } else { Err("A senha atual não confere.".into()) };
        }
        let s = self.session()?;
        let c = self.conn()?;
        let obj = ObjectPath::try_from(path).map_err(|e| e.to_string())?;
        let secret = |v: &str| (s.clone(), Vec::<u8>::new(), v.as_bytes().to_vec(), "text/plain".to_string());
        c.call_method(Some(DEST), ROOT, Some("org.gnome.keyring.InternalUnsupportedGuiltRiddenInterface"), "ChangeWithMasterPassword", &(obj, secret(old), secret(new)))
            .map_err(|e| {
                let m = e.to_string();
                if m.contains("Denied") || m.contains("Invalid") { "A senha atual não confere.".to_string() } else { m }
            })?;
        Ok(())
    }
}

fn fake() -> Backend {
    let item = |n: u32, label: &str, kind: &str, account: &str, place: &str| Item { path: format!("/teste/{n}"), label: label.into(), kind: kind.into(), account: account.into(), place: place.into(), modified: 1_762_380_000 + n as u64 * 86_400 };
    let rings = vec![
        Keyring {
            path: "/teste/login".into(),
            label: "login".into(),
            locked: false,
            items: vec![
                item(1, "Casa 5G", "Senha de rede (Wi-Fi/VPN)", "", "Casa 5G"),
                item(2, "Servidor de arquivos", "Senha de rede", "ana", "smb://casa.local"),
                item(3, "Correio (IMAP)", "Senha de app", "ana@exemplo.org", "imap.exemplo.org"),
                item(4, "Navegador", "Senha de app", "", "chromium"),
            ],
        },
        Keyring { path: "/teste/trabalho".into(), label: "trabalho".into(), locked: true, items: Vec::new() },
    ];
    let secrets = (1..=4).map(|n| (format!("/teste/{n}"), format!("senha-de-teste-{n}"))).collect();
    Backend::Fake(rings, secrets)
}
