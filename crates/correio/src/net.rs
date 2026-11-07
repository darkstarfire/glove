//! Conexão TCP com ou sem TLS (rustls), com STARTTLS para o SMTP.

use glove_config::tr;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Security {
    /// TLS desde o começo (IMAP 993, SMTP 465).
    #[default]
    Tls,
    /// Começa aberto e passa a TLS (SMTP 587).
    StartTls,
    /// Sem criptografia (só para servidores na própria máquina).
    Plain,
}

pub enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Read for Stream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(b),
            Stream::Tls(s) => s.read(b),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(b),
            Stream::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

fn tls_config() -> Arc<ClientConfig> {
    static CFG: std::sync::OnceLock<Arc<ClientConfig>> = std::sync::OnceLock::new();
    CFG.get_or_init(|| {
        let mut roots = RootCertStore::empty();
        // Os certificados do sistema; sem eles, a lista embutida.
        for c in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(c);
        }
        if roots.is_empty() {
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().expect("TLS").with_root_certificates(roots).with_no_client_auth();
        Arc::new(cfg)
    })
    .clone()
}

pub type Conn = BufReader<Stream>;

fn tcp(host: &str, port: u16) -> std::io::Result<TcpStream> {
    use std::net::ToSocketAddrs;
    let mut last = std::io::Error::other(tr!("endereço não encontrado"));
    for addr in (host, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(15)) {
            Ok(s) => {
                s.set_read_timeout(Some(Duration::from_secs(60)))?;
                s.set_write_timeout(Some(Duration::from_secs(60)))?;
                return Ok(s);
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn wrap_tls(host: &str, tcp: TcpStream) -> std::io::Result<Stream> {
    let name = ServerName::try_from(host.to_string()).map_err(|_| std::io::Error::other(tr!("nome de servidor inválido")))?;
    let conn = ClientConnection::new(tls_config(), name).map_err(std::io::Error::other)?;
    let mut s = StreamOwned::new(conn, tcp);
    // Faz o aperto de mão agora, para o erro de certificado aparecer aqui.
    while s.conn.is_handshaking() {
        s.conn.complete_io(&mut s.sock)?;
    }
    Ok(Stream::Tls(Box::new(s)))
}

/// Abre a conexão (com TLS quando `Security::Tls`).
pub fn connect(host: &str, port: u16, sec: Security) -> std::io::Result<Conn> {
    let t = tcp(host, port)?;
    let s = if sec == Security::Tls { wrap_tls(host, t)? } else { Stream::Plain(t) };
    Ok(BufReader::new(s))
}

/// Passa uma conexão aberta a TLS (depois do STARTTLS aceito).
pub fn upgrade(host: &str, c: Conn) -> std::io::Result<Conn> {
    match c.into_inner() {
        Stream::Plain(t) => Ok(BufReader::new(wrap_tls(host, t)?)),
        s => Ok(BufReader::new(s)),
    }
}

/// Uma linha (com o CRLF).
pub fn read_line(c: &mut Conn) -> std::io::Result<Vec<u8>> {
    let mut v = Vec::new();
    let n = c.read_until(b'\n', &mut v)?;
    if n == 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, tr!("o servidor fechou a conexão")));
    }
    Ok(v)
}

pub fn send(c: &mut Conn, data: &[u8]) -> std::io::Result<()> {
    let s = c.get_mut();
    s.write_all(data)?;
    s.flush()
}
