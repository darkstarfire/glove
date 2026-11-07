//! A conversa com o servidor IRC numa thread: conecta (com TLS), se apresenta,
//! entra nos canais e traduz cada linha do protocolo em eventos para a janela.
//! O que a janela quer mandar chega por um canal e sai na mesma conexão.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Conta {
    pub servidor: String,
    pub porta: u16,
    pub tls: bool,
    pub apelido: String,
    pub nome: String,
    /// Canais para entrar ao conectar.
    pub canais: Vec<String>,
}

impl Default for Conta {
    fn default() -> Self {
        let user = std::env::var("USER").unwrap_or_else(|_| "glove".into());
        Self { servidor: "irc.libera.chat".into(), porta: 6697, tls: true, apelido: user.clone(), nome: user, canais: vec!["#glove".into()] }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    /// Conectado, com o apelido aceito.
    Pronto(String),
    /// Linha do servidor (boas-vindas, avisos) para a conversa do servidor.
    Servidor(String),
    /// Entrou num canal (nós).
    Entrou(String),
    Saiu(String),
    /// Mensagem: conversa (canal ou apelido), de quem, texto, se é /me.
    Msg { conversa: String, de: String, texto: String, acao: bool },
    /// Acontecimento numa conversa (alguém entrou, saiu, mudou de nome…).
    Sistema { conversa: String, texto: String },
    /// Quem está no canal.
    Nomes { canal: String, nomes: Vec<String> },
    Apelido(String),
    Caiu(String),
}

enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Stream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(b),
            Stream::Tls(s) => s.read(b),
        }
    }
    fn send(&mut self, line: &str) -> std::io::Result<()> {
        let data = format!("{line}\r\n");
        match self {
            Stream::Plain(s) => s.write_all(data.as_bytes()).and_then(|_| s.flush()),
            Stream::Tls(s) => s.write_all(data.as_bytes()).and_then(|_| s.flush()),
        }
    }
}

fn tls_config() -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    for c in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(c);
    }
    if roots.is_empty() {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().expect("TLS").with_root_certificates(roots).with_no_client_auth())
}

fn connect(c: &Conta) -> std::io::Result<Stream> {
    use std::net::ToSocketAddrs;
    let mut last = std::io::Error::other("endereço não encontrado");
    for addr in (c.servidor.as_str(), c.porta).to_socket_addrs()? {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(15)) {
            Ok(t) => {
                t.set_read_timeout(Some(Duration::from_millis(200)))?;
                if !c.tls {
                    return Ok(Stream::Plain(t));
                }
                let name = ServerName::try_from(c.servidor.clone()).map_err(|_| std::io::Error::other("nome de servidor inválido"))?;
                let conn = ClientConnection::new(tls_config(), name).map_err(std::io::Error::other)?;
                let mut s = StreamOwned::new(conn, t);
                s.sock.set_read_timeout(Some(Duration::from_secs(15)))?;
                while s.conn.is_handshaking() {
                    s.conn.complete_io(&mut s.sock)?;
                }
                s.sock.set_read_timeout(Some(Duration::from_millis(200)))?;
                return Ok(Stream::Tls(Box::new(s)));
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Uma linha do IRC: (prefixo, comando, parâmetros com o último "trailing").
pub fn parse(line: &str) -> (String, String, Vec<String>) {
    let mut rest = line.trim_end_matches(['\r', '\n']);
    let mut prefix = String::new();
    if let Some(r) = rest.strip_prefix(':') {
        let (p, r2) = r.split_once(' ').unwrap_or((r, ""));
        prefix = p.to_string();
        rest = r2;
    }
    let (head, trailing) = match rest.split_once(" :") {
        Some((h, t)) => (h, Some(t)),
        None => (rest, None),
    };
    let mut parts = head.split_whitespace();
    let cmd = parts.next().unwrap_or("").to_uppercase();
    let mut params: Vec<String> = parts.map(String::from).collect();
    if let Some(t) = trailing {
        params.push(t.to_string());
    } else if let Some(t) = rest.strip_prefix(':') {
        params.push(t.to_string());
    }
    (prefix, cmd, params)
}

pub fn nick_of(prefix: &str) -> &str {
    prefix.split('!').next().unwrap_or(prefix)
}

/// Liga a conversa com o servidor. Devolve por onde mandar linhas e de onde vêm os eventos.
pub fn start(conta: Conta) -> (Sender<String>, Receiver<Ev>) {
    let (tx_out, rx_out) = channel::<String>();
    let (tx_ev, rx_ev) = channel::<Ev>();
    std::thread::spawn(move || {
        let mut s = match connect(&conta) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx_ev.send(Ev::Caiu(e.to_string()));
                return;
            }
        };
        let mut nick = conta.apelido.clone();
        let _ = s.send(&format!("NICK {nick}"));
        let _ = s.send(&format!("USER {} 0 * :{}", conta.apelido, if conta.nome.is_empty() { &conta.apelido } else { &conta.nome }));
        let mut buf: Vec<u8> = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut nomes: std::collections::HashMap<String, Vec<String>> = Default::default();
        loop {
            // O que a janela mandou.
            while let Ok(l) = rx_out.try_recv() {
                if l == "\u{0}SAIR" {
                    let _ = s.send("QUIT :Glove");
                    return;
                }
                if s.send(&l).is_err() {
                    let _ = tx_ev.send(Ev::Caiu("conexão perdida".into()));
                    return;
                }
            }
            match s.read(&mut tmp) {
                Ok(0) => {
                    let _ = tx_ev.send(Ev::Caiu("o servidor fechou a conexão".into()));
                    return;
                }
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => continue,
                Err(e) => {
                    let _ = tx_ev.send(Ev::Caiu(e.to_string()));
                    return;
                }
            }
            while let Some(i) = buf.iter().position(|b| *b == b'\n') {
                let raw: Vec<u8> = buf.drain(..=i).collect();
                let line = String::from_utf8_lossy(&raw).into_owned();
                let (prefix, cmd, p) = parse(&line);
                let from = nick_of(&prefix).to_string();
                let last = p.last().cloned().unwrap_or_default();
                let ev = match cmd.as_str() {
                    "PING" => {
                        let _ = s.send(&format!("PONG :{last}"));
                        None
                    }
                    "001" => {
                        nick = p.first().cloned().unwrap_or(nick);
                        for c in &conta.canais {
                            let _ = s.send(&format!("JOIN {c}"));
                        }
                        Some(Ev::Pronto(nick.clone()))
                    }
                    "433" => {
                        nick.push('_');
                        let _ = s.send(&format!("NICK {nick}"));
                        None
                    }
                    "PRIVMSG" | "NOTICE" if p.len() >= 2 => {
                        let target = &p[0];
                        let to_me = target.eq_ignore_ascii_case(&nick);
                        let (acao, texto) = match last.strip_prefix("\u{1}ACTION ").and_then(|t| t.strip_suffix('\u{1}')) {
                            Some(t) => (true, t.to_string()),
                            None => (false, last.clone()),
                        };
                        if texto.starts_with('\u{1}') {
                            // Outros pedidos CTCP: a versão responde; o resto, ignora.
                            if texto.starts_with("\u{1}VERSION") {
                                let _ = s.send(&format!("NOTICE {from} :\u{1}VERSION Glove Mensagens\u{1}"));
                            }
                            None
                        } else if cmd == "NOTICE" && (from.is_empty() || from.contains('.') || target == "*") {
                            Some(Ev::Servidor(texto))
                        } else {
                            Some(Ev::Msg { conversa: if to_me { from.clone() } else { target.clone() }, de: from.clone(), texto, acao })
                        }
                    }
                    "JOIN" => {
                        let chan = p.first().cloned().unwrap_or_default();
                        if from.eq_ignore_ascii_case(&nick) { Some(Ev::Entrou(chan)) } else { Some(Ev::Sistema { conversa: chan, texto: format!("→ {from}") }) }
                    }
                    "PART" => {
                        let chan = p.first().cloned().unwrap_or_default();
                        if from.eq_ignore_ascii_case(&nick) { Some(Ev::Saiu(chan)) } else { Some(Ev::Sistema { conversa: chan, texto: format!("← {from}") }) }
                    }
                    "QUIT" => Some(Ev::Sistema { conversa: "*".into(), texto: format!("← {from}") }),
                    "NICK" => {
                        if from.eq_ignore_ascii_case(&nick) {
                            nick = last.clone();
                            Some(Ev::Apelido(last))
                        } else {
                            Some(Ev::Sistema { conversa: "*".into(), texto: format!("{from} → {last}") })
                        }
                    }
                    "KICK" if p.len() >= 2 => Some(Ev::Sistema { conversa: p[0].clone(), texto: format!("✕ {} ({})", p[1], last) }),
                    "332" if p.len() >= 3 => Some(Ev::Sistema { conversa: p[1].clone(), texto: last }),
                    "353" if p.len() >= 3 => {
                        let chan = p[p.len() - 2].clone();
                        nomes.entry(chan).or_default().extend(last.split_whitespace().map(|n| n.trim_start_matches(['@', '+', '%', '~', '&']).to_string()));
                        None
                    }
                    "366" if p.len() >= 2 => nomes.remove(&p[1]).map(|n| Ev::Nomes { canal: p[1].clone(), nomes: n }),
                    "ERROR" => Some(Ev::Caiu(last)),
                    c if c.len() == 3 && c.chars().all(|x| x.is_ascii_digit()) => (!last.is_empty()).then(|| Ev::Servidor(last)),
                    _ => None,
                };
                if let Some(e) = ev {
                    if tx_ev.send(e).is_err() {
                        return;
                    }
                }
            }
        }
    });
    (tx_out, rx_ev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linhas() {
        let (p, c, a) = parse(":ana!a@host PRIVMSG #glove :oi, tudo bem?\r\n");
        assert_eq!((nick_of(&p), c.as_str()), ("ana", "PRIVMSG"));
        assert_eq!(a, vec!["#glove".to_string(), "oi, tudo bem?".to_string()]);
        let (_, c, a) = parse("PING :servidor.net");
        assert_eq!((c.as_str(), a), ("PING", vec!["servidor.net".to_string()]));
        let (_, c, a) = parse(":srv 353 eu = #glove :@ana +bia caio");
        assert_eq!(c, "353");
        assert_eq!(a.last().unwrap(), "@ana +bia caio");
    }
}
