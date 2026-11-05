//! Envio por SMTP: TLS direto (465) ou STARTTLS (587), AUTH PLAIN/LOGIN.

use base64::Engine;

use crate::net::{self, Conn, Security};

/// Lê uma resposta (com as linhas "250-" de continuação): (código, texto).
fn reply(c: &mut Conn) -> Result<(u16, String), String> {
    let mut text = String::new();
    loop {
        let l = net::read_line(c).map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&l).trim_end().to_string();
        let code: u16 = s.get(..3).and_then(|x| x.parse().ok()).ok_or_else(|| format!("Resposta estranha do servidor: {s}"))?;
        text.push_str(s.get(4..).unwrap_or(""));
        text.push('\n');
        if s.as_bytes().get(3) != Some(&b'-') {
            return Ok((code, text));
        }
    }
}

fn expect(c: &mut Conn, ok: &[u16]) -> Result<String, String> {
    let (code, text) = reply(c)?;
    if ok.contains(&code) { Ok(text) } else { Err(format!("{code} {}", text.trim())) }
}

fn say(c: &mut Conn, line: &str, ok: &[u16]) -> Result<String, String> {
    net::send(c, format!("{line}\r\n").as_bytes()).map_err(|e| e.to_string())?;
    expect(c, ok)
}

/// Pontos no começo da linha ganham outro ponto (o fim é ".").
pub fn dot_stuff(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len() + 16);
    for line in msg.split_inclusive('\n') {
        if line.starts_with('.') {
            out.push('.');
        }
        out.push_str(line);
    }
    if !out.ends_with("\r\n") {
        out.push_str("\r\n");
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn send(host: &str, port: u16, sec: Security, user: &str, pass: &str, from: &str, rcpt: &[String], msg: &str) -> Result<(), String> {
    let mut c = net::connect(host, port, sec).map_err(|e| format!("Não deu para conectar a {host}: {e}"))?;
    expect(&mut c, &[220])?;
    let me = "glove.local";
    let mut caps = say(&mut c, &format!("EHLO {me}"), &[250])?;
    if sec == Security::StartTls {
        say(&mut c, "STARTTLS", &[220])?;
        c = net::upgrade(host, c).map_err(|e| format!("Falha no TLS: {e}"))?;
        caps = say(&mut c, &format!("EHLO {me}"), &[250])?;
    }
    if !user.is_empty() {
        let up = caps.to_uppercase();
        let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
        let r = if up.contains("PLAIN") || !up.contains("LOGIN") {
            say(&mut c, &format!("AUTH PLAIN {}", b64(&format!("\0{user}\0{pass}"))), &[235])
        } else {
            say(&mut c, "AUTH LOGIN", &[334]).and_then(|_| say(&mut c, &b64(user), &[334])).and_then(|_| say(&mut c, &b64(pass), &[235]))
        };
        r.map_err(|e| if e.starts_with("535") || e.starts_with("534") { "O servidor de envio recusou o usuário ou a senha.".to_string() } else { e })?;
    }
    say(&mut c, &format!("MAIL FROM:<{from}>"), &[250])?;
    for r in rcpt {
        say(&mut c, &format!("RCPT TO:<{r}>"), &[250, 251]).map_err(|e| format!("Destinatário recusado ({r}): {e}"))?;
    }
    say(&mut c, "DATA", &[354])?;
    net::send(&mut c, dot_stuff(msg).as_bytes()).map_err(|e| e.to_string())?;
    say(&mut c, ".", &[250])?;
    let _ = say(&mut c, "QUIT", &[221]);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn pontos() {
        assert_eq!(super::dot_stuff("a\r\n.b\r\n..c"), "a\r\n..b\r\n...c\r\n");
    }
}
