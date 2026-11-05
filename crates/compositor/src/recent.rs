//! Itens Recentes do menu principal: apps abertos (guardados pelo Glove) e
//! documentos (lidos do `recently-used.xbel`, o mesmo arquivo dos apps GTK).

use std::path::PathBuf;

/// Quantos itens de cada tipo o menu mostra.
/// Quantos guardar; o menu mostra quantos as Preferências pedirem (até isso).
pub const MAX: usize = 50;

#[derive(Default)]
pub struct Recent {
    /// app_id, do mais recente para o mais antigo.
    pub apps: Vec<String>,
    /// Momento do último "Limpar Menu" (ISO 8601, UTC): documentos mais antigos somem.
    cleared: String,
}

fn state_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("glove/recentes"))
}

fn xbel_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("recently-used.xbel"))
}

impl Recent {
    pub fn load() -> Self {
        let mut r = Self::default();
        let text = state_file().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("app", id)) => r.apps.push(id.to_string()),
                Some(("limpo", t)) => r.cleared = t.to_string(),
                _ => {}
            }
        }
        r
    }

    fn save(&self) {
        let Some(p) = state_file() else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let mut out = String::new();
        if !self.cleared.is_empty() {
            out.push_str(&format!("limpo {}\n", self.cleared));
        }
        for a in &self.apps {
            out.push_str(&format!("app {a}\n"));
        }
        let _ = std::fs::write(p, out);
    }

    /// Põe o app no topo da lista. Devolve verdadeiro se a lista mudou.
    pub fn add_app(&mut self, app_id: &str) -> bool {
        if app_id.is_empty() || self.apps.first().is_some_and(|a| a == app_id) {
            return false;
        }
        self.apps.retain(|a| a != app_id);
        self.apps.insert(0, app_id.to_string());
        self.apps.truncate(MAX);
        self.save();
        true
    }

    /// "Limpar Menu": esquece os apps e esconde os documentos usados até agora.
    pub fn clear(&mut self) {
        self.apps.clear();
        self.cleared = iso_now();
        self.save();
    }

    /// Documentos recentes que ainda existem, do mais recente para o mais antigo.
    pub fn documents(&self) -> Vec<PathBuf> {
        let text = xbel_file().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let mut docs: Vec<(String, PathBuf)> = parse_xbel(&text)
            .into_iter()
            .filter(|(t, p)| *t > self.cleared && p.is_file())
            .collect();
        docs.sort_by(|a, b| b.0.cmp(&a.0));
        docs.into_iter().map(|(_, p)| p).take(MAX).collect()
    }
}

/// Valor de um atributo dentro de uma tag (`name="valor"`).
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// (data mais recente, caminho) de cada `<bookmark>` com `file://`.
fn parse_xbel(text: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for chunk in text.split("<bookmark ").skip(1) {
        let tag = format!(" {}", chunk.split('>').next().unwrap_or(""));
        let Some(path) = attr(&tag, "href").and_then(|h| h.strip_prefix("file://")) else { continue };
        let when = ["visited", "modified", "added"].iter().filter_map(|k| attr(&tag, k)).max().unwrap_or("").to_string();
        out.push((when, PathBuf::from(unescape(&percent_decode(path)))));
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

/// Agora em ISO 8601 (UTC), comparável com as datas do xbel.
fn iso_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Dias desde 1970 → data civil (algoritmo de Howard Hinnant).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_xbel() {
        let x = r#"<xbel><bookmark href="file:///home/a/Meu%20texto.txt" added="2026-01-01T10:00:00Z" modified="2026-01-02T10:00:00Z" visited="2026-01-03T10:00:00.5Z">
        </bookmark><bookmark href="https://site" modified="2026-01-05T00:00:00Z"></bookmark>
        <bookmark href="file:///tmp/a&amp;b.png" added="2025-12-01T00:00:00Z"></bookmark></xbel>"#;
        let v = parse_xbel(x);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], ("2026-01-03T10:00:00.5Z".to_string(), PathBuf::from("/home/a/Meu texto.txt")));
        assert_eq!(v[1].1, PathBuf::from("/tmp/a&b.png"));
    }

    #[test]
    fn data_iso() {
        let t = iso_now();
        assert_eq!(t.len(), 20);
        assert!(t.as_str() > "2025-01-01");
    }
}
