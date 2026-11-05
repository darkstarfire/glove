//! vCard 3.0 (o que interessa: nome, empresa, telefones, e-mails, endereço,
//! aniversário e nota), lido e escrito à mão.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contact {
    /// Nome do arquivo (sem .vcf) e UID.
    pub uid: String,
    pub first: String,
    pub last: String,
    pub org: String,
    /// (tipo, valor): "celular", "trabalho", "casa"…
    pub phones: Vec<(String, String)>,
    pub emails: Vec<(String, String)>,
    pub address: String,
    pub birthday: String,
    pub note: String,
    pub group: String,
}

impl Contact {
    /// Nome para mostrar: "Nome Sobrenome", a empresa ou "Sem Nome".
    pub fn display(&self) -> String {
        let n = format!("{} {}", self.first, self.last).trim().to_string();
        if !n.is_empty() {
            n
        } else if !self.org.is_empty() {
            self.org.clone()
        } else {
            "Sem Nome".into()
        }
    }
}

fn unescape(v: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for c in v.chars() {
        if esc {
            out.push(match c {
                'n' | 'N' => '\n',
                c => c,
            });
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else {
            out.push(c);
        }
    }
    out
}

fn escape(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;")
}

/// Tipo do telefone/e-mail em português, a partir do TYPE do vCard.
fn kind_of(params: &str) -> String {
    let p = params.to_ascii_lowercase();
    if p.contains("cell") {
        "celular"
    } else if p.contains("work") {
        "trabalho"
    } else if p.contains("home") {
        "casa"
    } else {
        "outro"
    }
    .into()
}

fn type_of(kind: &str) -> &'static str {
    match kind {
        "celular" => "CELL",
        "trabalho" => "WORK",
        "casa" => "HOME",
        _ => "OTHER",
    }
}

/// Todos os cartões de um texto .vcf (pode ter vários).
pub fn parse(text: &str) -> Vec<Contact> {
    // Linhas dobradas (começam com espaço) continuam a anterior.
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end_matches('\r');
        if (l.starts_with(' ') || l.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&l[1..]);
        } else {
            lines.push(l.to_string());
        }
    }
    let mut out = Vec::new();
    let mut cur: Option<Contact> = None;
    for l in lines {
        let Some((key, value)) = l.split_once(':') else { continue };
        let (name, params) = key.split_once(';').unwrap_or((key, ""));
        let name = name.rsplit('.').next().unwrap_or(name).to_ascii_uppercase();
        match name.as_str() {
            "BEGIN" if value.eq_ignore_ascii_case("VCARD") => cur = Some(Contact::default()),
            "END" if value.eq_ignore_ascii_case("VCARD") => {
                if let Some(c) = cur.take() {
                    out.push(c);
                }
            }
            _ => {
                let Some(c) = cur.as_mut() else { continue };
                match name.as_str() {
                    "N" => {
                        let parts: Vec<&str> = value.split(';').collect();
                        c.last = unescape(parts.first().unwrap_or(&""));
                        c.first = unescape(parts.get(1).unwrap_or(&""));
                    }
                    "FN" if c.first.is_empty() && c.last.is_empty() => {
                        let v = unescape(value);
                        let mut it = v.splitn(2, ' ');
                        c.first = it.next().unwrap_or("").to_string();
                        c.last = it.next().unwrap_or("").to_string();
                    }
                    "ORG" => c.org = unescape(value.split(';').next().unwrap_or("")),
                    "TEL" => c.phones.push((kind_of(params), unescape(value))),
                    "EMAIL" => c.emails.push((kind_of(params), unescape(value))),
                    "ADR" => c.address = value.split(';').map(unescape).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
                    "BDAY" => c.birthday = value.to_string(),
                    "NOTE" => c.note = unescape(value),
                    "UID" => c.uid = value.to_string(),
                    "CATEGORIES" => c.group = unescape(value.split(',').next().unwrap_or("")),
                    _ => {}
                }
            }
        }
    }
    out
}

pub fn serialize(c: &Contact) -> String {
    let mut s = String::from("BEGIN:VCARD\r\nVERSION:3.0\r\n");
    s.push_str(&format!("N:{};{};;;\r\n", escape(&c.last), escape(&c.first)));
    s.push_str(&format!("FN:{}\r\n", escape(&c.display())));
    if !c.org.is_empty() {
        s.push_str(&format!("ORG:{}\r\n", escape(&c.org)));
    }
    for (k, v) in c.phones.iter().filter(|p| !p.1.trim().is_empty()) {
        s.push_str(&format!("TEL;TYPE={}:{}\r\n", type_of(k), escape(v)));
    }
    for (k, v) in c.emails.iter().filter(|p| !p.1.trim().is_empty()) {
        s.push_str(&format!("EMAIL;TYPE={}:{}\r\n", type_of(k), escape(v)));
    }
    if !c.address.is_empty() {
        s.push_str(&format!("ADR;TYPE=HOME:;;{};;;;\r\n", escape(&c.address)));
    }
    if !c.birthday.is_empty() {
        s.push_str(&format!("BDAY:{}\r\n", c.birthday));
    }
    if !c.note.is_empty() {
        s.push_str(&format!("NOTE:{}\r\n", escape(&c.note)));
    }
    if !c.group.is_empty() {
        s.push_str(&format!("CATEGORIES:{}\r\n", escape(&c.group)));
    }
    s.push_str(&format!("UID:{}\r\nEND:VCARD\r\n", c.uid));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let c = Contact {
            uid: "abc".into(),
            first: "Maria".into(),
            last: "da Silva".into(),
            org: "Padaria, Ltda".into(),
            phones: vec![("celular".into(), "+55 11 99999-0000".into())],
            emails: vec![("trabalho".into(), "maria@exemplo.com".into())],
            address: "Rua A, 10".into(),
            birthday: "1990-05-04".into(),
            note: "Gosta de pão\nde queijo".into(),
            group: "Família".into(),
        };
        let back = parse(&serialize(&c));
        assert_eq!(back, [c]);
    }

    #[test]
    fn le_cartoes_de_fora() {
        let t = "BEGIN:VCARD\nVERSION:2.1\nFN:João Souza\nTEL;CELL:123\nitem1.EMAIL;TYPE=INTERNET:j@x.com\nNOTE:linha um \n dobrada\nEND:VCARD\nBEGIN:VCARD\nORG:Empresa X\nEND:VCARD\n";
        let v = parse(t);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].first, "João");
        assert_eq!(v[0].last, "Souza");
        assert_eq!(v[0].phones, [("celular".to_string(), "123".to_string())]);
        assert_eq!(v[0].emails[0].1, "j@x.com");
        assert_eq!(v[0].note, "linha um dobrada");
        assert_eq!(v[1].display(), "Empresa X");
    }
}
