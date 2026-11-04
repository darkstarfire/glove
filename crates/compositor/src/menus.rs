//! Menus dos apps na barra global.
//!
//! Cada app do Glove se conecta a um socket local e manda seus menus em
//! texto simples (uma linha por item). Quando o usuário escolhe um item, o
//! compositor responde com `ACT <id>`. Protocolo (campos separados por TAB):
//!
//! ```text
//! app → glove:  HELLO <pid> | BEGIN | MENU <título> | ITEM <id> <rótulo> <atalho> <flags> | SEP
//!               | SUB <rótulo> <flags> … ENDSUB (submenu) | END
//! glove → app:  ACT <id>
//! ```
//! flags: `d` = desativado, `c` = marcado.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use smithay::reexports::calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction};

use crate::CalloopData;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub shortcut: String,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
    /// Itens do submenu (vazio = item comum).
    pub sub: Vec<Entry>,
    pub is_sub: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppMenu {
    pub title: String,
    pub items: Vec<Entry>,
}

pub struct MenuClient {
    pub key: u64,
    pub pid: u32,
    stream: UnixStream,
    buf: Vec<u8>,
    pending: Vec<AppMenu>,
    /// Submenus abertos com SUB e ainda sem ENDSUB.
    open_subs: Vec<Entry>,
    pub menus: Vec<AppMenu>,
    /// Pedido de captura de tela: (atraso em ms, abrir no Visualizador).
    pub shot: Option<(u64, bool)>,
    /// Pedido de fim de sessão ("encerrar", "reiniciar", "desligar"), vindo do glove-sair.
    pub end: Option<String>,
}

impl MenuClient {
    pub fn activate(&mut self, id: &str) {
        let _ = self.stream.write_all(format!("ACT\t{id}\n").as_bytes());
    }

    /// Lista que recebe o próximo item: o submenu aberto mais interno ou o menu.
    fn target(&mut self) -> Option<&mut Vec<Entry>> {
        match self.open_subs.last_mut() {
            Some(e) => Some(&mut e.sub),
            None => self.pending.last_mut().map(|m| &mut m.items),
        }
    }

    pub(crate) fn line(&mut self, line: &str) -> bool {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "HELLO" => self.pid = f.get(1).and_then(|p| p.parse().ok()).unwrap_or(0),
            "BEGIN" => {
                self.pending.clear();
                self.open_subs.clear();
            }
            "MENU" => {
                self.open_subs.clear();
                self.pending.push(AppMenu { title: f.get(1).unwrap_or(&"").to_string(), items: Vec::new() });
            }
            "ITEM" => {
                let flags = f.get(4).copied().unwrap_or("");
                let e = Entry {
                    id: f.get(1).unwrap_or(&"").to_string(),
                    label: f.get(2).unwrap_or(&"").to_string(),
                    shortcut: f.get(3).unwrap_or(&"").to_string(),
                    enabled: !flags.contains('d'),
                    checked: flags.contains('c'),
                    ..Default::default()
                };
                if let Some(t) = self.target() {
                    t.push(e);
                }
            }
            "SEP" => {
                if let Some(t) = self.target() {
                    t.push(Entry { separator: true, enabled: true, ..Default::default() });
                }
            }
            "SUB" => {
                let flags = f.get(2).copied().unwrap_or("");
                self.open_subs.push(Entry { label: f.get(1).unwrap_or(&"").to_string(), enabled: !flags.contains('d'), is_sub: true, ..Default::default() });
            }
            "ENDSUB" => {
                if let Some(e) = self.open_subs.pop() {
                    if let Some(t) = self.target() {
                        t.push(e);
                    }
                }
            }
            "SHOT" => {
                let delay = f.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                self.shot = Some((delay, f.get(2) == Some(&"1")));
            }
            "SAIR" => self.end = f.get(1).map(|k| k.to_string()),
            "END" => {
                self.menus = std::mem::take(&mut self.pending);
                return true;
            }
            _ => {}
        }
        false
    }
}

pub fn socket_path(wayland_socket: &std::ffi::OsStr) -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    Some(PathBuf::from(dir).join(format!("glove-menus-{}", wayland_socket.to_string_lossy())))
}

/// Abre o socket e registra no loop de eventos (sem acordar a CPU à toa).
pub fn listen(handle: &LoopHandle<'static, CalloopData>, path: &PathBuf) -> std::io::Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    listener.set_nonblocking(true)?;
    let h = handle.clone();
    handle
        .insert_source(Generic::new(listener, Interest::READ, Mode::Level), move |_, l, data| {
            while let Ok((stream, _)) = l.accept() {
                if stream.set_nonblocking(true).is_err() {
                    continue;
                }
                let Ok(reader) = stream.try_clone() else { continue };
                data.state.menu_seq += 1;
                let key = data.state.menu_seq;
                data.state.menu_clients.push(MenuClient { key, pid: 0, stream, buf: Vec::new(), pending: Vec::new(), open_subs: Vec::new(), menus: Vec::new(), shot: None, end: None });
                let h2 = h.clone();
                let _ = h.insert_source(Generic::new(reader, Interest::READ, Mode::Level), move |_, s, data| {
                    let mut tmp = [0u8; 4096];
                    let mut closed = false;
                    let mut bytes = Vec::new();
                    loop {
                        // Segurança: só lemos do socket que pertence a esta fonte.
                        match unsafe { s.get_mut() }.read(&mut tmp) {
                            Ok(0) => {
                                closed = true;
                                break;
                            }
                            Ok(n) => bytes.extend_from_slice(&tmp[..n]),
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(_) => {
                                closed = true;
                                break;
                            }
                        }
                    }
                    let st = &mut data.state;
                    let mut end = None;
                    if let Some(c) = st.menu_clients.iter_mut().find(|c| c.key == key) {
                        c.buf.extend_from_slice(&bytes);
                        let mut changed = false;
                        while let Some(pos) = c.buf.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = c.buf.drain(..=pos).collect();
                            let text = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                            changed |= c.line(&text);
                        }
                        end = c.end.take();
                        if changed {
                            st.dirty = true;
                        }
                        if let Some((delay, open)) = c.shot.take() {
                            // Espera o atraso pedido (e a janela do app sumir) e captura.
                            let _ = h2.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(delay.max(300))), move |_, _, data| {
                                data.state.request_screenshot();
                                data.state.open_after_shot = open;
                                smithay::reexports::calloop::timer::TimeoutAction::Drop
                            });
                        }
                    }
                    if let Some(kind) = end {
                        st.end_session(&kind);
                    }
                    if closed {
                        st.menu_clients.retain(|c| c.key != key);
                        st.dirty = true;
                        return Ok(PostAction::Remove);
                    }
                    Ok(PostAction::Continue)
                });
            }
            Ok(PostAction::Continue)
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_submenus() {
        let (a, _b) = UnixStream::pair().unwrap();
        let mut c = MenuClient { key: 0, pid: 0, stream: a, buf: Vec::new(), pending: Vec::new(), open_subs: Vec::new(), menus: Vec::new(), shot: None, end: None };
        let text = "BEGIN\nMENU\tArquivo\nITEM\tnovo\tNovo\tCtrl+N\t\nSUB\tRecentes\t\nITEM\tr1\ta.txt\t\t\nSUB\tMais\td\nENDSUB\nENDSUB\nSEP\nEND";
        let done = text.lines().fold(false, |d, l| c.line(l) || d);
        assert!(done);
        let items = &c.menus[0].items;
        assert_eq!(items.len(), 3);
        assert!(items[1].is_sub && items[1].label == "Recentes");
        assert_eq!(items[1].sub[0].id, "r1");
        assert!(items[1].sub[1].is_sub && !items[1].sub[1].enabled);
        assert!(items[2].separator);
    }
}
