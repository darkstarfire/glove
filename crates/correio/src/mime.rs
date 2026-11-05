//! Mensagens de e-mail: ler (cabeçalhos, MIME, codificações) e montar
//! (texto em UTF-8 com quoted-printable e anexos em base64).

use base64::Engine;

/// Cabeçalhos "desdobrados" (linhas de continuação juntadas) e onde começa o corpo.
pub fn headers(raw: &[u8]) -> (Vec<(String, String)>, usize) {
    // O corpo começa depois da primeira linha vazia.
    let mut body_at = raw.len();
    let mut i = 0;
    while i < raw.len() {
        let end = raw[i..].iter().position(|c| *c == b'\n').map(|p| i + p + 1).unwrap_or(raw.len());
        let line = &raw[i..end];
        if line == b"\r\n" || line == b"\n" {
            body_at = end;
            break;
        }
        i = end;
    }
    let text = latin_or_utf8(&raw[..body_at]);
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let l = line.trim_end_matches('\r');
        if l.is_empty() {
            break;
        }
        if l.starts_with([' ', '\t']) {
            if let Some(last) = out.last_mut() {
                last.1.push(' ');
                last.1.push_str(l.trim());
            }
        } else if let Some((k, v)) = l.split_once(':') {
            out.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    (out, body_at)
}

pub fn header<'a>(h: &'a [(String, String)], k: &str) -> Option<&'a str> {
    h.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

fn latin_or_utf8(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|c| *c as char).collect(),
    }
}

/// Bytes num conjunto de caracteres para texto.
pub fn decode_charset(b: &[u8], charset: &str) -> String {
    let cs = charset.trim().trim_matches('"').to_ascii_lowercase();
    match cs.as_str() {
        "iso-8859-1" | "latin1" | "iso8859-1" | "us-ascii" | "ascii" | "iso-8859-15" => {
            if cs == "us-ascii" || cs == "ascii" {
                if let Ok(s) = std::str::from_utf8(b) {
                    return s.to_string();
                }
            }
            b.iter().map(|c| if cs == "iso-8859-15" && *c == 0xA4 { '€' } else { *c as char }).collect()
        }
        "windows-1252" | "cp1252" => b.iter().map(|c| cp1252(*c)).collect(),
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

fn cp1252(c: u8) -> char {
    const HI: [char; 32] = ['€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ'];
    if (0x80..0xA0).contains(&c) { HI[(c - 0x80) as usize] } else { c as char }
}

pub fn qp_decode(s: &[u8], header: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == b'=' {
            if s.get(i + 1) == Some(&b'\r') && s.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if s.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            let hex = s.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
            out.push(c);
        } else if header && c == b'_' {
            out.push(b' ');
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

pub fn b64_decode(s: &[u8]) -> Vec<u8> {
    let clean: Vec<u8> = s.iter().copied().filter(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=')).collect();
    base64::engine::general_purpose::STANDARD.decode(&clean).or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(clean.iter().copied().filter(|c| *c != b'=').collect::<Vec<u8>>())).unwrap_or_default()
}

/// Palavras codificadas (RFC 2047) de um cabeçalho: =?utf-8?B?...?= e =?...?Q?...?=.
pub fn decode_words(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut last_was_word = false;
    while let Some(start) = rest.find("=?") {
        let before = &rest[..start];
        let after = &rest[start + 2..];
        let parsed = (|| {
            let (cs, r) = after.split_once('?')?;
            let (enc, r) = r.split_once('?')?;
            let end = r.find("?=")?;
            Some((cs, enc, &r[..end], &r[end + 2..]))
        })();
        match parsed {
            Some((cs, enc, text, tail)) => {
                // Espaço entre duas palavras codificadas some.
                if !(last_was_word && before.trim().is_empty()) {
                    out.push_str(before);
                }
                let bytes = if enc.eq_ignore_ascii_case("b") { b64_decode(text.as_bytes()) } else { qp_decode(text.as_bytes(), true) };
                let cs = cs.split('*').next().unwrap_or(cs);
                out.push_str(&decode_charset(&bytes, cs));
                rest = tail;
                last_was_word = true;
            }
            None => {
                out.push_str(&rest[..start + 2]);
                rest = after;
                last_was_word = false;
            }
        }
    }
    out.push_str(rest);
    out
}

/// "tipo/subtipo; a=b; c="d"" → ("tipo/subtipo", [(a,b),(c,d)]).
pub fn params(v: &str) -> (String, Vec<(String, String)>) {
    let mut parts = split_params(v).into_iter();
    let kind = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    let mut ps: Vec<(String, String)> = Vec::new();
    for p in parts {
        if let Some((k, val)) = p.split_once('=') {
            let k = k.trim().to_ascii_lowercase();
            let mut val = val.trim().trim_matches('"').to_string();
            // RFC 2231: nome*=utf-8''texto%20com%20espaços (e nome*0*=... em pedaços).
            if let Some(base) = k.strip_suffix('*') {
                let base = base.split('*').next().unwrap_or(base).to_string();
                let enc = val.splitn(3, '\'').collect::<Vec<_>>();
                let (cs, text) = if enc.len() == 3 { (enc[0].to_string(), enc[2].to_string()) } else { ("utf-8".into(), val.clone()) };
                let bytes = percent_decode(&text);
                val = decode_charset(&bytes, if cs.is_empty() { "utf-8" } else { &cs });
                match ps.iter_mut().find(|(n, _)| *n == base) {
                    Some(e) => e.1.push_str(&val),
                    None => ps.push((base, val)),
                }
                continue;
            }
            if let Some((base, _)) = k.split_once('*') {
                match ps.iter_mut().find(|(n, _)| n == base) {
                    Some(e) => e.1.push_str(&val),
                    None => ps.push((base.to_string(), val)),
                }
                continue;
            }
            ps.push((k, decode_words(&val)));
        }
    }
    (kind, ps)
}

fn split_params(v: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut q = false;
    for c in v.chars() {
        match c {
            '"' => {
                q = !q;
                out.last_mut().unwrap().push(c);
            }
            ';' if !q => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if let Some(v) = b.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[allow(dead_code)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Default)]
#[allow(dead_code)]
pub struct Parsed {
    pub from: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub date: String,
    pub message_id: String,
    pub references: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
}

/// Lê uma mensagem inteira.
pub fn parse(raw: &[u8]) -> Parsed {
    let (h, body_at) = headers(raw);
    let get = |k: &str| header(&h, k).map(decode_words).unwrap_or_default();
    let mut p = Parsed { from: get("from"), to: get("to"), cc: get("cc"), subject: get("subject"), date: get("date"), message_id: get("message-id"), references: get("references"), ..Default::default() };
    let mut plain = Vec::new();
    let mut html = Vec::new();
    walk(&h, &raw[body_at.min(raw.len())..], &mut plain, &mut html, &mut p.attachments, 0);
    p.text = if !plain.is_empty() { plain.join("\n\n") } else { html.iter().map(|x| html_to_text(x)).collect::<Vec<_>>().join("\n\n") };
    p.text = p.text.replace("\r\n", "\n").trim_end().to_string();
    p
}

fn walk(h: &[(String, String)], body: &[u8], plain: &mut Vec<String>, html: &mut Vec<String>, att: &mut Vec<Attachment>, depth: usize) {
    let (kind, ps) = params(header(h, "content-type").unwrap_or("text/plain; charset=us-ascii"));
    let param = |k: &str| ps.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let (disp, dps) = params(header(h, "content-disposition").unwrap_or(""));
    let name = dps.iter().find(|(n, _)| n == "filename").map(|(_, v)| v.clone()).or_else(|| param("name"));
    if kind.starts_with("multipart/") && depth < 12 {
        let Some(b) = param("boundary") else { return };
        for part in split_parts(body, &b) {
            let (ph, at) = headers(part);
            walk(&ph, &part[at.min(part.len())..], plain, html, att, depth + 1);
        }
        return;
    }
    if kind == "message/rfc822" && depth < 12 {
        let inner = parse(body);
        plain.push(format!("---------- Mensagem encaminhada ----------\nDe: {}\nAssunto: {}\nData: {}\n\n{}", inner.from, inner.subject, inner.date, inner.text));
        att.extend(inner.attachments);
        return;
    }
    let enc = header(h, "content-transfer-encoding").unwrap_or("").trim().to_ascii_lowercase();
    let data = match enc.as_str() {
        "base64" => b64_decode(body),
        "quoted-printable" => qp_decode(body, false),
        _ => body.to_vec(),
    };
    let is_text = kind.starts_with("text/plain") || kind.starts_with("text/html");
    if disp == "attachment" || (name.is_some() && !is_text) || (!is_text && !kind.is_empty() && !kind.starts_with("text/")) {
        att.push(Attachment { name: name.unwrap_or_else(|| "anexo".into()), mime: kind, data });
        return;
    }
    let cs = param("charset").unwrap_or_else(|| "utf-8".into());
    let text = decode_charset(&data, &cs);
    if kind.starts_with("text/html") {
        html.push(text);
    } else {
        plain.push(text);
    }
}

fn split_parts<'a>(body: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let delim = format!("--{boundary}");
    let d = delim.as_bytes();
    let mut starts: Vec<(usize, usize, bool)> = Vec::new();
    let mut i = 0;
    while i + d.len() <= body.len() {
        let at_line = i == 0 || body[i - 1] == b'\n';
        if at_line && &body[i..i + d.len()] == d {
            let end_mark = body.get(i + d.len()..i + d.len() + 2) == Some(b"--");
            let line_end = body[i..].iter().position(|c| *c == b'\n').map(|p| i + p + 1).unwrap_or(body.len());
            starts.push((i, line_end, end_mark));
            if end_mark {
                break;
            }
            i = line_end;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for w in starts.windows(2) {
        let (_, content_start, last) = w[0];
        if last {
            break;
        }
        let mut end = w[1].0;
        // Tira o CRLF que antecede o delimitador.
        if end > content_start && body[end - 1] == b'\n' {
            end -= 1;
        }
        if end > content_start && body[end - 1] == b'\r' {
            end -= 1;
        }
        out.push(&body[content_start..end.max(content_start)]);
    }
    out
}

/// HTML para texto simples: tira estilos/scripts e etiquetas, guarda as quebras.
pub fn html_to_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() / 2);
    let lower = s.to_ascii_lowercase();
    let mut i = 0;
    let b = s.as_bytes();
    while i < b.len() {
        if b[i] == b'<' {
            let end = s[i..].find('>').map(|e| i + e + 1).unwrap_or(b.len());
            let tag = lower[i + 1..end.saturating_sub(1).max(i + 1)].trim_start_matches('/').split([' ', '/', '\n', '\t']).next().unwrap_or("").to_string();
            if (tag == "style" || tag == "script" || tag == "head") && !lower[i + 1..].starts_with('/') {
                let close = format!("</{tag}");
                i = lower[end..].find(&close).map(|e| end + e).unwrap_or(b.len());
                continue;
            }
            if matches!(tag.as_str(), "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "table" | "blockquote") && !out.ends_with('\n') {
                out.push('\n');
            }
            if tag == "li" && !lower[i + 1..].starts_with('/') {
                out.push_str("• ");
            }
            i = end;
        } else {
            let next = s[i..].find('<').map(|e| i + e).unwrap_or(b.len());
            let chunk = &s[i..next];
            let mut collapsed = String::new();
            for (k, w) in chunk.split_whitespace().enumerate() {
                if k > 0 {
                    collapsed.push(' ');
                }
                collapsed.push_str(w);
            }
            if chunk.starts_with(char::is_whitespace) && !out.ends_with([' ', '\n']) && !collapsed.is_empty() {
                out.push(' ');
            }
            out.push_str(&collapsed);
            if chunk.ends_with(char::is_whitespace) && !collapsed.is_empty() {
                out.push(' ');
            }
            i = next;
        }
    }
    let out = entities(&out);
    // No máximo uma linha em branco seguida.
    let mut clean = String::new();
    let mut blank = 0;
    for l in out.lines() {
        let l = l.trim();
        if l.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        clean.push_str(l);
        clean.push('\n');
    }
    clean.trim().to_string()
}

fn entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find('&') {
        out.push_str(&rest[..a]);
        let tail = &rest[a..];
        let end = tail.find(';').filter(|e| *e < 10);
        let Some(e) = end else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..e];
        let ch = match name {
            "nbsp" => Some(' '),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "ccedil" => Some('ç'),
            "atilde" => Some('ã'),
            "otilde" => Some('õ'),
            "aacute" => Some('á'),
            "eacute" => Some('é'),
            "iacute" => Some('í'),
            "oacute" => Some('ó'),
            "uacute" => Some('ú'),
            "acirc" => Some('â'),
            "ecirc" => Some('ê'),
            "ocirc" => Some('ô'),
            "agrave" => Some('à'),
            "hellip" => Some('…'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            _ if name.starts_with("#x") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
            _ if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=e]),
        }
        rest = &tail[e + 1..];
    }
    out.push_str(rest);
    out
}

/// Só o endereço de "Nome <a@b>" (ou o próprio texto).
pub fn address(s: &str) -> String {
    match (s.rfind('<'), s.rfind('>')) {
        (Some(a), Some(b)) if b > a => s[a + 1..b].trim().to_string(),
        _ => s.trim().trim_matches('"').to_string(),
    }
}

/// Nome de "Nome <a@b>" (ou o endereço).
pub fn display_name(s: &str) -> String {
    match s.find('<') {
        Some(a) if a > 0 => s[..a].trim().trim_matches('"').to_string(),
        _ => address(s),
    }
}

/// Lista de destinatários separados por vírgula (respeitando aspas).
pub fn split_addresses(s: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut q = false;
    for c in s.chars() {
        match c {
            '"' => {
                q = !q;
                out.last_mut().unwrap().push(c);
            }
            ',' | ';' if !q => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out.into_iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect()
}

/// Cabeçalho com acentos vira palavras codificadas em UTF-8.
pub fn encode_header(s: &str) -> String {
    if s.is_ascii() {
        return s.to_string();
    }
    // Pedaços curtos para a linha não passar de 76 caracteres.
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if cur.len() + c.len_utf8() > 45 {
            out.push(format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(cur.as_bytes())));
            cur.clear();
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(cur.as_bytes())));
    }
    out.join("\r\n ")
}

/// Endereço com nome: o nome é codificado se tiver acentos.
pub fn encode_address(s: &str) -> String {
    let addr = address(s);
    let name = display_name(s);
    if name == addr || name.is_empty() {
        addr
    } else if name.is_ascii() {
        format!("\"{}\" <{addr}>", name.replace('"', ""))
    } else {
        format!("{} <{addr}>", encode_header(&name))
    }
}

pub fn qp_encode(s: &str) -> String {
    let mut out = String::new();
    for line in s.replace("\r\n", "\n").split('\n') {
        let mut cur = String::new();
        let bytes = line.as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            let last = i + 1 == bytes.len();
            let enc = if (*b == b' ' || *b == b'\t') && last {
                format!("={b:02X}")
            } else if (33..=126).contains(b) && *b != b'=' || *b == b' ' || *b == b'\t' {
                (*b as char).to_string()
            } else {
                format!("={b:02X}")
            };
            if cur.len() + enc.len() > 75 {
                out.push_str(&cur);
                out.push_str("=\r\n");
                cur.clear();
            }
            cur.push_str(&enc);
        }
        out.push_str(&cur);
        out.push_str("\r\n");
    }
    out
}

fn b64_lines(data: &[u8]) -> String {
    let s = base64::engine::general_purpose::STANDARD.encode(data);
    let mut out = String::with_capacity(s.len() + s.len() / 76 * 2 + 2);
    for chunk in s.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push_str("\r\n");
    }
    out
}

/// Data no formato do e-mail (RFC 5322), no fuso local.
pub fn rfc_date(t: i64) -> String {
    let (tm, off) = unsafe {
        let tt = t as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&tt, &mut tm);
        (tm, tm.tm_gmtoff)
    };
    const D: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let sign = if off < 0 { '-' } else { '+' };
    let o = off.abs() / 60;
    format!("{}, {:02} {} {} {:02}:{:02}:{:02} {}{:02}{:02}", D[tm.tm_wday as usize % 7], tm.tm_mday, M[tm.tm_mon as usize % 12], tm.tm_year + 1900, tm.tm_hour, tm.tm_min, tm.tm_sec, sign, o / 60, o % 60)
}

pub struct Outgoing {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub in_reply_to: String,
    pub references: String,
    pub attachments: Vec<(String, Vec<u8>)>,
}

fn mime_for(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "txt" => "text/plain",
        "zip" => "application/zip",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "mp3" => "audio/mpeg",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
}

/// Monta a mensagem pronta para o SMTP (CRLF, sem Bcc no cabeçalho).
pub fn build(m: &Outgoing, now: i64, host: &str) -> String {
    let id = format!("<{}.{}@{}>", now, std::process::id(), if host.is_empty() { "glove.local" } else { host });
    let mut h = String::new();
    h.push_str(&format!("Date: {}\r\n", rfc_date(now)));
    h.push_str(&format!("From: {}\r\n", encode_address(&m.from)));
    h.push_str(&format!("To: {}\r\n", m.to.iter().map(|a| encode_address(a)).collect::<Vec<_>>().join(", ")));
    if !m.cc.is_empty() {
        h.push_str(&format!("Cc: {}\r\n", m.cc.iter().map(|a| encode_address(a)).collect::<Vec<_>>().join(", ")));
    }
    h.push_str(&format!("Subject: {}\r\n", encode_header(&m.subject)));
    h.push_str(&format!("Message-ID: {id}\r\n"));
    if !m.in_reply_to.is_empty() {
        h.push_str(&format!("In-Reply-To: {}\r\n", m.in_reply_to));
        let refs = format!("{} {}", m.references, m.in_reply_to);
        h.push_str(&format!("References: {}\r\n", refs.trim()));
    }
    h.push_str("MIME-Version: 1.0\r\nUser-Agent: Glove Correio\r\n");
    let text_part = format!("Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n{}", qp_encode(&m.body));
    if m.attachments.is_empty() {
        return format!("{h}{text_part}");
    }
    let boundary = format!("=_glove_{now:x}_{}", std::process::id());
    let mut s = format!("{h}Content-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\nEsta é uma mensagem em várias partes (MIME).\r\n--{boundary}\r\n{text_part}");
    for (name, data) in &m.attachments {
        let enc_name = if name.is_ascii() { format!("\"{}\"", name.replace('"', "")) } else { format!("\"{}\"", encode_header(name).replace("\r\n ", " ")) };
        let star = if name.is_ascii() { String::new() } else { format!("; filename*=UTF-8''{}", name.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>()) };
        s.push_str(&format!(
            "--{boundary}\r\nContent-Type: {}; name={enc_name}\r\nContent-Disposition: attachment; filename={enc_name}{star}\r\nContent-Transfer-Encoding: base64\r\n\r\n{}",
            mime_for(name),
            b64_lines(data)
        ));
    }
    s.push_str(&format!("--{boundary}--\r\n"));
    s
}

/// Linhas citadas para Responder ("> ").
pub fn quote(text: &str) -> String {
    text.lines().map(|l| if l.starts_with('>') || l.trim().is_empty() { format!(">{}", l.trim_end()) } else { format!("> {l}") }).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palavras_codificadas() {
        assert_eq!(decode_words("=?UTF-8?B?T2zDoSBtdW5kbw==?="), "Olá mundo");
        assert_eq!(decode_words("=?iso-8859-1?Q?Reuni=E3o_de_hoje?= amanh\u{e3}"), "Reunião de hoje amanhã");
        assert_eq!(decode_words("=?utf-8?q?a?= =?utf-8?q?b?="), "ab");
        assert_eq!(decode_words("Sem codificação"), "Sem codificação");
    }

    #[test]
    fn multipart_com_anexo() {
        let raw = b"From: =?utf-8?Q?Jos=C3=A9?= <jose@ex.com>\r\nSubject: Teste\r\nContent-Type: multipart/mixed; boundary=\"XX\"\r\n\r\npreambulo\r\n--XX\r\nContent-Type: multipart/alternative; boundary=YY\r\n\r\n--YY\r\nContent-Type: text/plain; charset=iso-8859-1\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nOl=E1, tudo bem?=\r\n Sim.\r\n--YY\r\nContent-Type: text/html\r\n\r\n<p>Ol&aacute;</p>\r\n--YY--\r\n--XX\r\nContent-Type: application/pdf; name=\"doc.pdf\"\r\nContent-Disposition: attachment; filename*=utf-8''rela%C3%A7%C3%A3o.pdf\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVsbG8=\r\n--XX--\r\n";
        let p = parse(raw);
        assert_eq!(p.from, "José <jose@ex.com>");
        assert_eq!(p.text, "Olá, tudo bem? Sim.");
        assert_eq!(p.attachments.len(), 1);
        assert_eq!(p.attachments[0].name, "relação.pdf");
        assert_eq!(p.attachments[0].data, b"Hello");
    }

    #[test]
    fn so_html() {
        let raw = b"Subject: x\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><head><style>p{color:red}</style></head><body><p>Linha   um</p><p>Linha&nbsp;dois &amp; tr&#234;s</p><ul><li>item</li></ul></body></html>";
        assert_eq!(parse(raw).text, "Linha um\nLinha dois & três\n• item");
    }

    #[test]
    fn montar_e_ler_de_volta() {
        let m = Outgoing {
            from: "Fábio <f@ex.com>".into(),
            to: vec!["a@ex.com".into()],
            cc: vec![],
            bcc: vec!["oculto@ex.com".into()],
            subject: "Relatório de ação".into(),
            body: "Olá!\nLinha com = sinal e acentos: ção.".into(),
            in_reply_to: String::new(),
            references: String::new(),
            attachments: vec![("nota.txt".into(), b"conteudo".to_vec())],
        };
        let s = build(&m, 1_700_000_000, "ex.com");
        assert!(!s.contains("oculto@ex.com"));
        assert!(s.lines().all(|l| l.len() <= 998));
        let p = parse(s.as_bytes());
        assert_eq!(p.subject, "Relatório de ação");
        assert_eq!(p.from, "Fábio <f@ex.com>");
        assert_eq!(p.text, "Olá!\nLinha com = sinal e acentos: ção.");
        assert_eq!(p.attachments[0].name, "nota.txt");
        assert_eq!(p.attachments[0].data, b"conteudo");
    }

    #[test]
    fn enderecos() {
        assert_eq!(split_addresses("\"Silva, Ana\" <ana@x.com>, b@y.com"), ["\"Silva, Ana\" <ana@x.com>", "b@y.com"]);
        assert_eq!(address("Ana <ana@x.com>"), "ana@x.com");
        assert_eq!(display_name("\"Ana\" <ana@x.com>"), "Ana");
    }
}
