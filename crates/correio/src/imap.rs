//! Cliente IMAP mínimo: entrar, listar pastas, ler cabeçalhos e mensagens,
//! marcar como lida, mover para o Lixo e guardar cópias (APPEND).

use std::io::Read;

use crate::net::{self, Conn, Security};

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Atom(String),
    Str(Vec<u8>),
    List(Vec<Val>),
    Nil,
}

impl Val {
    pub fn text(&self) -> String {
        match self {
            Val::Atom(a) => a.clone(),
            Val::Str(b) => String::from_utf8_lossy(b).into_owned(),
            _ => String::new(),
        }
    }
    pub fn bytes(&self) -> &[u8] {
        match self {
            Val::Str(b) => b,
            Val::Atom(a) => a.as_bytes(),
            _ => &[],
        }
    }
    pub fn list(&self) -> &[Val] {
        match self {
            Val::List(v) => v,
            _ => &[],
        }
    }
}

/// Lê os valores de uma resposta (com os literais {n} já embutidos).
pub fn parse(b: &[u8]) -> Vec<Val> {
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        if let Some(v) = value(b, &mut i) {
            out.push(v);
        }
    }
    out
}

fn value(b: &[u8], i: &mut usize) -> Option<Val> {
    while *i < b.len() && matches!(b[*i], b' ' | b'\r' | b'\n') {
        *i += 1;
    }
    if *i >= b.len() {
        return None;
    }
    match b[*i] {
        b'(' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                while *i < b.len() && matches!(b[*i], b' ' | b'\r' | b'\n') {
                    *i += 1;
                }
                if *i >= b.len() {
                    break;
                }
                if b[*i] == b')' {
                    *i += 1;
                    break;
                }
                match value(b, i) {
                    Some(x) => v.push(x),
                    None => break,
                }
            }
            Some(Val::List(v))
        }
        b')' => {
            *i += 1;
            None
        }
        b'"' => {
            *i += 1;
            let mut s = Vec::new();
            while *i < b.len() && b[*i] != b'"' {
                if b[*i] == b'\\' && *i + 1 < b.len() {
                    *i += 1;
                }
                s.push(b[*i]);
                *i += 1;
            }
            *i += 1;
            Some(Val::Str(s))
        }
        b'{' => {
            let close = b[*i..].iter().position(|c| *c == b'}').map(|p| *i + p)?;
            let n: usize = std::str::from_utf8(&b[*i + 1..close]).ok()?.trim_end_matches('+').parse().ok()?;
            let mut start = close + 1;
            if b.get(start) == Some(&b'\r') {
                start += 1;
            }
            if b.get(start) == Some(&b'\n') {
                start += 1;
            }
            let end = (start + n).min(b.len());
            *i = end;
            Some(Val::Str(b[start..end].to_vec()))
        }
        _ => {
            // Átomo; o que estiver entre [ ] faz parte dele (BODY[HEADER.FIELDS (A B)]).
            let mut s = String::new();
            let mut depth = 0;
            while *i < b.len() {
                let c = b[*i];
                if depth == 0 && matches!(c, b' ' | b'(' | b')' | b'\r' | b'\n') {
                    break;
                }
                if c == b'[' {
                    depth += 1;
                } else if c == b']' {
                    depth -= 1;
                }
                s.push(c as char);
                *i += 1;
            }
            if s.eq_ignore_ascii_case("NIL") {
                Some(Val::Nil)
            } else {
                Some(Val::Atom(s))
            }
        }
    }
}

/// Nome de pasta no UTF-7 modificado do IMAP para texto.
pub fn decode_mutf7(s: &str) -> String {
    use base64::Engine;
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find('&') {
        out.push_str(&rest[..a]);
        let tail = &rest[a + 1..];
        let Some(e) = tail.find('-') else {
            out.push_str(&rest[a..]);
            return out;
        };
        if e == 0 {
            out.push('&');
        } else {
            let b64 = tail[..e].replace(',', "/");
            let bytes = base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64.trim_end_matches('=')).unwrap_or_default();
            let units: Vec<u16> = bytes.chunks(2).filter(|c| c.len() == 2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            out.push_str(&String::from_utf16_lossy(&units));
        }
        rest = &tail[e + 1..];
    }
    out.push_str(rest);
    out
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Clone, Debug)]
pub struct Folder {
    /// Nome no servidor (para os comandos).
    pub raw: String,
    /// Nome para mostrar.
    pub name: String,
    /// \Sent, \Trash, \Drafts, \Junk… (ou "inbox").
    pub role: String,
}

#[derive(Clone, Debug, Default)]
pub struct Header {
    pub uid: u32,
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    pub size: u32,
    pub head: Vec<u8>,
}

pub struct Imap {
    c: Conn,
    tag: u32,
    pub caps: String,
}

impl Imap {
    pub fn connect(host: &str, port: u16, sec: Security, user: &str, pass: &str) -> Result<Self, String> {
        let c = net::connect(host, port, sec).map_err(|e| format!("Não deu para conectar a {host}: {e}"))?;
        let mut s = Imap { c, tag: 0, caps: String::new() };
        let greet = net::read_line(&mut s.c).map_err(|e| e.to_string())?;
        if !greet.starts_with(b"* OK") && !greet.starts_with(b"* PREAUTH") {
            return Err(format!("Resposta estranha do servidor: {}", String::from_utf8_lossy(&greet).trim()));
        }
        let r = s.cmd(&format!("LOGIN {} {}", quote(user), quote(pass))).map_err(|e| if e.contains("NO") || e.contains("AUTHENTICATIONFAILED") { "Usuário ou senha recusados pelo servidor.".to_string() } else { e })?;
        let _ = r;
        if let Ok(r) = s.cmd("CAPABILITY") {
            s.caps = r.iter().map(|l| String::from_utf8_lossy(l).to_uppercase()).collect::<Vec<_>>().join(" ");
        }
        Ok(s)
    }

    /// Lê uma resposta lógica (linha + literais).
    fn read_resp(&mut self) -> std::io::Result<Vec<u8>> {
        let mut out = net::read_line(&mut self.c)?;
        loop {
            // Termina em {n}\r\n: vem um literal de n bytes e a linha continua.
            let t = out.strip_suffix(b"\r\n").or_else(|| out.strip_suffix(b"\n")).unwrap_or(&out);
            let Some(open) = t.iter().rposition(|c| *c == b'{') else { break };
            if !t.ends_with(b"}") {
                break;
            }
            let Some(n) = std::str::from_utf8(&t[open + 1..t.len() - 1]).ok().and_then(|x| x.parse::<usize>().ok()) else { break };
            let mut lit = vec![0u8; n];
            self.c.read_exact(&mut lit)?;
            out.extend_from_slice(&lit);
            let more = net::read_line(&mut self.c)?;
            out.extend_from_slice(&more);
        }
        Ok(out)
    }

    /// Envia um comando e devolve as respostas "*" até o OK.
    pub fn cmd(&mut self, c: &str) -> Result<Vec<Vec<u8>>, String> {
        self.tag += 1;
        let tag = format!("g{}", self.tag);
        net::send(&mut self.c, format!("{tag} {c}\r\n").as_bytes()).map_err(|e| e.to_string())?;
        self.collect(&tag)
    }

    fn collect(&mut self, tag: &str) -> Result<Vec<Vec<u8>>, String> {
        let mut v = Vec::new();
        loop {
            let r = self.read_resp().map_err(|e| e.to_string())?;
            if r.starts_with(tag.as_bytes()) && r.get(tag.len()) == Some(&b' ') {
                let rest = String::from_utf8_lossy(&r[tag.len() + 1..]).trim().to_string();
                return if rest.starts_with("OK") { Ok(v) } else { Err(rest) };
            }
            v.push(r);
        }
    }

    pub fn folders(&mut self) -> Result<Vec<Folder>, String> {
        let r = self.cmd("LIST \"\" \"*\"")?;
        let mut out = Vec::new();
        for line in r {
            if !line.starts_with(b"* LIST") {
                continue;
            }
            let vals = parse(&line[6..]);
            let flags: Vec<String> = vals.first().map(|v| v.list().iter().map(|f| f.text().to_lowercase()).collect()).unwrap_or_default();
            if flags.iter().any(|f| f == "\\noselect" || f == "\\nonexistent") {
                continue;
            }
            let raw = vals.get(2).map(Val::text).unwrap_or_default();
            if raw.is_empty() {
                continue;
            }
            let sep = vals.get(1).map(Val::text).unwrap_or_default();
            let mut role = flags.iter().find(|f| matches!(f.as_str(), "\\sent" | "\\trash" | "\\drafts" | "\\junk" | "\\archive" | "\\all" | "\\flagged")).cloned().unwrap_or_default();
            if raw.eq_ignore_ascii_case("INBOX") {
                role = "inbox".into();
            }
            let decoded = decode_mutf7(&raw);
            let name = if sep.is_empty() { decoded.clone() } else { decoded.rsplit(sep.as_str()).next().unwrap_or(&decoded).to_string() };
            out.push(Folder { raw, name, role });
        }
        // Pastas sem marca de uso, mas com nomes conhecidos.
        for f in &mut out {
            if f.role.is_empty() {
                let n = f.name.to_lowercase();
                f.role = match n.as_str() {
                    "sent" | "sent items" | "sent messages" | "enviados" | "itens enviados" => "\\sent",
                    "trash" | "deleted" | "deleted items" | "deleted messages" | "lixo" | "lixeira" | "itens excluídos" => "\\trash",
                    "drafts" | "rascunhos" => "\\drafts",
                    "junk" | "spam" | "lixo eletrônico" => "\\junk",
                    _ => "",
                }
                .into();
            }
        }
        Ok(out)
    }

    /// Abre a pasta; devolve (quantas mensagens, UIDVALIDITY).
    pub fn select(&mut self, folder: &str) -> Result<(u32, u32), String> {
        let r = self.cmd(&format!("SELECT {}", quote(folder)))?;
        let mut exists = 0;
        let mut validity = 0;
        for l in r {
            let s = String::from_utf8_lossy(&l);
            let w: Vec<&str> = s.split_whitespace().collect();
            if w.len() >= 3 && w[2].eq_ignore_ascii_case("EXISTS") {
                exists = w[1].parse().unwrap_or(0);
            }
            if let Some(p) = s.find("UIDVALIDITY ") {
                validity = s[p + 12..].split(|c: char| !c.is_ascii_digit()).next().and_then(|x| x.parse().ok()).unwrap_or(0);
            }
        }
        Ok((exists, validity))
    }

    /// Cabeçalhos das últimas `max` mensagens da pasta aberta.
    pub fn headers(&mut self, exists: u32, max: u32) -> Result<Vec<Header>, String> {
        if exists == 0 {
            return Ok(Vec::new());
        }
        let first = exists.saturating_sub(max - 1).max(1);
        let r = self.cmd(&format!("FETCH {first}:{exists} (UID FLAGS RFC822.SIZE BODY.PEEK[HEADER.FIELDS (FROM TO CC SUBJECT DATE MESSAGE-ID REFERENCES CONTENT-TYPE)])"))?;
        Ok(r.iter().filter_map(|l| fetch_item(l)).collect())
    }

    /// A mensagem inteira (sem marcar como lida).
    pub fn body(&mut self, uid: u32) -> Result<Vec<u8>, String> {
        let r = self.cmd(&format!("UID FETCH {uid} (BODY.PEEK[])"))?;
        for l in &r {
            if let Some(v) = fetch_values(l) {
                for w in v.chunks(2) {
                    if w.len() == 2 && w[0].text().to_uppercase().starts_with("BODY[") {
                        return Ok(w[1].bytes().to_vec());
                    }
                }
            }
        }
        Err("A mensagem não veio do servidor (talvez tenha sido apagada).".into())
    }

    pub fn set_flag(&mut self, uid: u32, flag: &str, on: bool) -> Result<(), String> {
        self.cmd(&format!("UID STORE {uid} {}FLAGS.SILENT ({flag})", if on { "+" } else { "-" })).map(|_| ())
    }

    /// Move para outra pasta (MOVE quando dá; senão COPY + apagar).
    pub fn move_to(&mut self, uid: u32, dest: &str) -> Result<(), String> {
        if self.caps.contains("MOVE") {
            return self.cmd(&format!("UID MOVE {uid} {}", quote(dest))).map(|_| ());
        }
        self.cmd(&format!("UID COPY {uid} {}", quote(dest)))?;
        self.expunge(uid)
    }

    /// Apaga de vez (já no Lixo).
    pub fn expunge(&mut self, uid: u32) -> Result<(), String> {
        self.set_flag(uid, "\\Deleted", true)?;
        if self.caps.contains("UIDPLUS") {
            self.cmd(&format!("UID EXPUNGE {uid}")).map(|_| ())
        } else {
            self.cmd("EXPUNGE").map(|_| ())
        }
    }

    /// Guarda uma cópia na pasta (Enviados, Rascunhos).
    pub fn append(&mut self, folder: &str, msg: &[u8], flags: &str) -> Result<(), String> {
        self.tag += 1;
        let tag = format!("g{}", self.tag);
        net::send(&mut self.c, format!("{tag} APPEND {} ({flags}) {{{}}}\r\n", quote(folder), msg.len()).as_bytes()).map_err(|e| e.to_string())?;
        let go = self.read_resp().map_err(|e| e.to_string())?;
        if !go.starts_with(b"+") {
            return Err(String::from_utf8_lossy(&go).trim().to_string());
        }
        net::send(&mut self.c, msg).map_err(|e| e.to_string())?;
        net::send(&mut self.c, b"\r\n").map_err(|e| e.to_string())?;
        self.collect(&tag).map(|_| ())
    }

    pub fn logout(mut self) {
        let _ = self.cmd("LOGOUT");
    }
}

/// Valores de "* n FETCH (...)".
fn fetch_values(l: &[u8]) -> Option<Vec<Val>> {
    let s = l.get(..l.len().min(40))?;
    let txt = String::from_utf8_lossy(s).to_uppercase();
    let p = txt.find(" FETCH ")?;
    let vals = parse(&l[p + 7..]);
    vals.into_iter().next().map(|v| v.list().to_vec())
}

fn fetch_item(l: &[u8]) -> Option<Header> {
    let v = fetch_values(l)?;
    let mut h = Header::default();
    for w in v.chunks(2) {
        if w.len() < 2 {
            break;
        }
        let k = w[0].text().to_uppercase();
        match k.as_str() {
            "UID" => h.uid = w[1].text().parse().unwrap_or(0),
            "RFC822.SIZE" => h.size = w[1].text().parse().unwrap_or(0),
            "FLAGS" => {
                let f: Vec<String> = w[1].list().iter().map(|x| x.text().to_lowercase()).collect();
                h.seen = f.iter().any(|x| x == "\\seen");
                h.flagged = f.iter().any(|x| x == "\\flagged");
                h.answered = f.iter().any(|x| x == "\\answered");
            }
            _ if k.starts_with("BODY[") => h.head = w[1].bytes().to_vec(),
            _ => {}
        }
    }
    (h.uid > 0).then_some(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_fetch_com_literal() {
        let l = b"* 3 FETCH (UID 45 FLAGS (\\Seen \\Answered) RFC822.SIZE 900 BODY[HEADER.FIELDS (FROM SUBJECT)] {26}\r\nFrom: a@b\r\nSubject: Oi\r\n\r\n)\r\n";
        let h = fetch_item(l).unwrap();
        assert_eq!(h.uid, 45);
        assert!(h.seen && h.answered && !h.flagged);
        assert_eq!(h.size, 900);
        assert_eq!(h.head, b"From: a@b\r\nSubject: Oi\r\n\r\n");
    }

    #[test]
    fn utf7_modificado() {
        assert_eq!(decode_mutf7("Itens Exclu&AO0-dos"), "Itens Excluídos");
        assert_eq!(decode_mutf7("A&-B"), "A&B");
        assert_eq!(decode_mutf7("[Gmail]/Lixeira"), "[Gmail]/Lixeira");
    }

    #[test]
    fn lista_com_aspas() {
        let v = parse(b"(\\HasNoChildren \\Sent) \"/\" \"[Gmail]/E-mails enviados\"\r\n");
        assert_eq!(v[0].list().len(), 2);
        assert_eq!(v[1].text(), "/");
        assert_eq!(v[2].text(), "[Gmail]/E-mails enviados");
    }
}
