//! Cliente do protocolo do greetd: mensagens JSON precedidas do tamanho (u32 na
//! ordem do processador), pelo socket em $GREETD_SOCK. Sem dependências: as
//! mensagens são objetos simples de texto.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug, PartialEq)]
pub enum Reply {
    Success,
    /// `auth`: senha errada (auth_error); senão, outro erro.
    Error { auth: bool, description: String },
    /// Pergunta do PAM: tipo ("visible", "secret", "info" ou "error") e texto.
    Auth { kind: String, message: String },
}

/// Texto JSON entre aspas, com escapes.
pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Valor de texto de uma chave num objeto JSON simples (sem objetos aninhados).
pub fn field(json: &str, key: &str) -> Option<String> {
    let pat = format!("{}:", quote(key));
    let start = json.find(&pat)? + pat.len();
    let rest = json[start..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = rest[1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?).unwrap_or('?'));
                }
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

pub fn parse_reply(json: &str) -> Option<Reply> {
    Some(match field(json, "type")?.as_str() {
        "success" => Reply::Success,
        "error" => Reply::Error { auth: field(json, "error_type").as_deref() == Some("auth_error"), description: field(json, "description").unwrap_or_default() },
        "auth_message" => Reply::Auth { kind: field(json, "auth_message_type").unwrap_or_default(), message: field(json, "auth_message").unwrap_or_default() },
        _ => return None,
    })
}

pub struct Greetd {
    stream: UnixStream,
}

impl Greetd {
    pub fn connect() -> std::io::Result<Self> {
        let path = std::env::var_os("GREETD_SOCK").ok_or_else(|| std::io::Error::other("GREETD_SOCK não definido (rode pelo greetd)"))?;
        Ok(Self { stream: UnixStream::connect(path)? })
    }

    fn send(&mut self, json: &str) -> std::io::Result<Reply> {
        let len = json.len() as u32;
        self.stream.write_all(&len.to_ne_bytes())?;
        self.stream.write_all(json.as_bytes())?;
        let mut lb = [0u8; 4];
        self.stream.read_exact(&mut lb)?;
        let n = u32::from_ne_bytes(lb) as usize;
        if n > 1 << 20 {
            return Err(std::io::Error::other("resposta grande demais"));
        }
        let mut buf = vec![0u8; n];
        self.stream.read_exact(&mut buf)?;
        let text = String::from_utf8_lossy(&buf);
        parse_reply(&text).ok_or_else(|| std::io::Error::other(format!("resposta inesperada: {text}")))
    }

    pub fn create_session(&mut self, user: &str) -> std::io::Result<Reply> {
        self.send(&format!("{{\"type\":\"create_session\",\"username\":{}}}", quote(user)))
    }

    /// Resposta à pergunta do PAM (None = só confirmar uma mensagem).
    pub fn respond(&mut self, response: Option<&str>) -> std::io::Result<Reply> {
        match response {
            Some(r) => self.send(&format!("{{\"type\":\"post_auth_message_response\",\"response\":{}}}", quote(r))),
            None => self.send("{\"type\":\"post_auth_message_response\"}"),
        }
    }

    pub fn start_session(&mut self, cmd: &[String], env: &[String]) -> std::io::Result<Reply> {
        let list = |v: &[String]| v.iter().map(|s| quote(s)).collect::<Vec<_>>().join(",");
        self.send(&format!("{{\"type\":\"start_session\",\"cmd\":[{}],\"env\":[{}]}}", list(cmd), list(env)))
    }

    pub fn cancel(&mut self) -> std::io::Result<Reply> {
        self.send("{\"type\":\"cancel_session\"}")
    }
}

/// Entra: cria a sessão, responde ao PAM com a senha e inicia `cmd`.
/// Erro: texto para mostrar (a sessão é cancelada).
pub fn log_in(user: &str, password: &str, cmd: &[String], env: &[String]) -> Result<(), String> {
    let io = |e: std::io::Error| format!("Erro ao falar com o greetd: {e}");
    let mut g = Greetd::connect().map_err(io)?;
    let mut reply = g.create_session(user).map_err(io)?;
    // Algumas perguntas podem vir em sequência (ex.: aviso + senha).
    for _ in 0..8 {
        match reply {
            Reply::Auth { kind, .. } => {
                reply = match kind.as_str() {
                    "secret" => g.respond(Some(password)),
                    "visible" => g.respond(Some(user)),
                    _ => g.respond(None),
                }
                .map_err(io)?;
            }
            Reply::Success => {
                return match g.start_session(cmd, env).map_err(io)? {
                    Reply::Success => Ok(()),
                    Reply::Error { description, .. } => {
                        let _ = g.cancel();
                        Err(format!("Não foi possível iniciar a sessão: {description}"))
                    }
                    other => {
                        let _ = g.cancel();
                        Err(format!("Resposta inesperada: {other:?}"))
                    }
                };
            }
            Reply::Error { auth, description } => {
                let _ = g.cancel();
                return Err(if auth { "Senha incorreta.".into() } else { description });
            }
        }
    }
    let _ = g.cancel();
    Err("O login pediu mais informações do que o esperado.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json() {
        assert_eq!(quote("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        let r = r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Senha: "}"#;
        assert_eq!(parse_reply(r), Some(Reply::Auth { kind: "secret".into(), message: "Senha: ".into() }));
        assert_eq!(parse_reply(r#"{"type":"success"}"#), Some(Reply::Success));
        let e = r#"{"type":"error","error_type":"auth_error","description":"falhou \"x\""}"#;
        assert_eq!(parse_reply(e), Some(Reply::Error { auth: true, description: "falhou \"x\"".into() }));
    }

    /// Conversa completa com um greetd de mentira (outro lado de um socketpair).
    #[test]
    fn conversa() {
        let (a, mut b) = UnixStream::pair().unwrap();
        fn read(b: &mut UnixStream) -> String {
            let mut lb = [0u8; 4];
            b.read_exact(&mut lb).unwrap();
            let mut buf = vec![0u8; u32::from_ne_bytes(lb) as usize];
            b.read_exact(&mut buf).unwrap();
            String::from_utf8(buf).unwrap()
        }
        fn write(b: &mut UnixStream, s: &str) {
            b.write_all(&(s.len() as u32).to_ne_bytes()).unwrap();
            b.write_all(s.as_bytes()).unwrap();
        }
        let server = std::thread::spawn(move || {
            let mut got = vec![read(&mut b)];
            write(&mut b, r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password:"}"#);
            got.push(read(&mut b));
            write(&mut b, r#"{"type":"success"}"#);
            got
        });
        let mut g = Greetd { stream: a };
        assert!(matches!(g.create_session("ana").unwrap(), Reply::Auth { .. }));
        assert_eq!(g.respond(Some("s3nh@")).unwrap(), Reply::Success);
        let got = server.join().unwrap();
        assert_eq!(got[0], r#"{"type":"create_session","username":"ana"}"#);
        assert_eq!(got[1], r#"{"type":"post_auth_message_response","response":"s3nh@"}"#);
    }
}
