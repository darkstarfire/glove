//! Menus dos apps na barra global.
//!
//! Cada app do Glove se conecta a um socket local e manda seus menus em
//! texto simples (uma linha por item). Quando o usuário escolhe um item, o
//! compositor responde com `ACT <id>`. Protocolo (campos separados por TAB):
//!
//! ```text
//! app → glove:  HELLO <pid> | BEGIN | MENU <título> | ITEM <id> <rótulo> <atalho> <flags> | SEP | END
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
    pub menus: Vec<AppMenu>,
    /// Pedido de captura de tela: (atraso em ms, abrir no Visualizador).
    pub shot: Option<(u64, bool)>,
}

impl MenuClient {
    pub fn activate(&mut self, id: &str) {
        let _ = self.stream.write_all(format!("ACT\t{id}\n").as_bytes());
    }

    fn line(&mut self, line: &str) -> bool {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "HELLO" => self.pid = f.get(1).and_then(|p| p.parse().ok()).unwrap_or(0),
            "BEGIN" => self.pending.clear(),
            "MENU" => self.pending.push(AppMenu { title: f.get(1).unwrap_or(&"").to_string(), items: Vec::new() }),
            "ITEM" => {
                if let Some(m) = self.pending.last_mut() {
                    let flags = f.get(4).copied().unwrap_or("");
                    m.items.push(Entry {
                        id: f.get(1).unwrap_or(&"").to_string(),
                        label: f.get(2).unwrap_or(&"").to_string(),
                        shortcut: f.get(3).unwrap_or(&"").to_string(),
                        enabled: !flags.contains('d'),
                        checked: flags.contains('c'),
                        separator: false,
                    });
                }
            }
            "SEP" => {
                if let Some(m) = self.pending.last_mut() {
                    m.items.push(Entry { separator: true, enabled: true, ..Default::default() });
                }
            }
            "SHOT" => {
                let delay = f.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                self.shot = Some((delay, f.get(2) == Some(&"1")));
            }
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
                data.state.menu_clients.push(MenuClient { key, pid: 0, stream, buf: Vec::new(), pending: Vec::new(), menus: Vec::new(), shot: None });
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
                    if let Some(c) = st.menu_clients.iter_mut().find(|c| c.key == key) {
                        c.buf.extend_from_slice(&bytes);
                        let mut changed = false;
                        while let Some(pos) = c.buf.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = c.buf.drain(..=pos).collect();
                            let text = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                            changed |= c.line(&text);
                        }
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
