//! Menus dos apps na barra global.
//!
//! Cada app do Glove se conecta a um socket local e manda seus menus em
//! texto simples (uma linha por item). Quando o usuário escolhe um item, o
//! compositor responde com `ACT <id>`. Protocolo (campos separados por TAB):
//!
//! ```text
//! app → glove:  HELLO <pid> | BEGIN | MENU <título> | ITEM <id> <rótulo> <atalho> <flags> | SEP
//!               | SUB <rótulo> <flags> … ENDSUB (submenu) | END
//!               | CTX <x> <y> … itens … CTXEND (menu de contexto solto no ponto x, y da
//!                 janela do app, desenhado pelo Glove por cima de tudo)
//! glove → app:  ACT <id>
//! ```
//! flags: `d` = desativado, `c` = marcado. ITEM pode ter um 6º campo, RRGGBB: a bolinha de cor.

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
    /// Bolinha de cor antes do nome (RRGGBB no protocolo).
    pub swatch: Option<(u8, u8, u8)>,
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
    /// Captura pedida: (atraso, abrir no Visualizador, modo: 0 tela, 1 área, 2 janela).
    pub shot: Option<(u64, bool, u8)>,
    /// Gravação pedida pelo painel: (modo 0 tela, 1 área, 2 janela; áudio do sistema; microfone).
    pub gravar: Option<(usize, bool, bool)>,
    /// Arquivo que o app pediu para pôr na área de transferência.
    pub copy: Option<PathBuf>,
    /// Pedido de fim de sessão ("logout", "quit", "restart", "shutdown"), vindo do glove-logout.
    pub end: Option<String>,
    /// Pedido de testar um protetor de tela (botão Testar das Preferências).
    pub saver: Option<String>,
    /// O documento tem alterações não salvas (ponto no botão de fechar).
    pub edited: bool,
    /// Arquivo aberto (ícone no título).
    pub doc: Option<PathBuf>,
    /// O app pediu a moldura do Glove nas janelas sem barra de título dele (as do GTK
    /// guardam em volta uma margem invisível para redimensionar): a sombra e os cantos.
    pub moldura_glove: bool,
    /// Menu de contexto sendo recebido e o pronto para abrir: (x, y na janela, itens).
    ctx_pending: Option<(f64, f64, Vec<Entry>)>,
    pub ctx: Option<(f64, f64, Vec<Entry>)>,
}

impl MenuClient {
    pub fn activate(&mut self, id: &str) {
        let _ = self.stream.write_all(format!("ACT\t{id}\n").as_bytes());
    }

    /// Lista que recebe o próximo item: o submenu aberto mais interno ou o menu.
    fn target(&mut self) -> Option<&mut Vec<Entry>> {
        match self.open_subs.last_mut() {
            Some(e) => Some(&mut e.sub),
            None => match &mut self.ctx_pending {
                Some((_, _, items)) => Some(items),
                None => self.pending.last_mut().map(|m| &mut m.items),
            },
        }
    }

    pub(crate) fn line(&mut self, line: &str) -> bool {
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "HELLO" => self.pid = f.get(1).and_then(|p| p.parse().ok()).unwrap_or(0),
            "MOLDURA" => self.moldura_glove = f.get(1) == Some(&"glove"),
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
                    swatch: f.get(5).and_then(|c| {
                        let v = u32::from_str_radix(c, 16).ok().filter(|_| c.len() == 6)?;
                        Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
                    }),
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
                self.shot = Some((delay, f.get(2) == Some(&"1"), f.get(3).and_then(|v| v.parse().ok()).unwrap_or(0)));
            }
            "COPIAR" => self.copy = f.get(1).filter(|p| !p.is_empty()).map(PathBuf::from),
            "SAIR" => self.end = f.get(1).map(|k| k.to_string()),
            "GRAVAR" => {
                self.gravar = Some((
                    f.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
                    f.get(2) == Some(&"1"),
                    f.get(3) == Some(&"1"),
                ))
            }
            "PROTETOR" => self.saver = f.get(1).map(|k| k.to_string()),
            "CTX" => {
                self.open_subs.clear();
                let n = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                self.ctx_pending = Some((n(1), n(2), Vec::new()));
            }
            "CTXEND" => {
                self.open_subs.clear();
                self.ctx = self.ctx_pending.take();
            }
            "EDITADO" => self.edited = f.get(1) == Some(&"1"),
            "DOC" => self.doc = f.get(1).filter(|p| !p.is_empty()).map(PathBuf::from),
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
                data.state.menu_clients.push(MenuClient { key, pid: 0, stream, buf: Vec::new(), pending: Vec::new(), open_subs: Vec::new(), menus: Vec::new(), shot: None, gravar: None, copy: None, end: None, saver: None, edited: false, doc: None, moldura_glove: false, ctx_pending: None, ctx: None });
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
                    let mut saver = None;
                    let mut copy = None;
                    let mut ctx = None;
                    if let Some(c) = st.menu_clients.iter_mut().find(|c| c.key == key) {
                        c.buf.extend_from_slice(&bytes);
                        let mut changed = false;
                        while let Some(pos) = c.buf.iter().position(|b| *b == b'\n') {
                            let line: Vec<u8> = c.buf.drain(..=pos).collect();
                            let text = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                            changed |= c.line(&text);
                        }
                        end = c.end.take();
                        saver = c.saver.take();
                        if changed {
                            st.dirty = true;
                        }
                        copy = c.copy.take();
                        ctx = c.ctx.take().map(|m| (c.pid, m));
                        if let Some((modo, sistema, microfone)) = c.gravar.take() {
                            // Espera a janela do painel sumir (ela não pode entrar no vídeo) e começa.
                            let _ = h2.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(400)), move |_, _, data| {
                                data.state.pedido_gravacao(modo, sistema, microfone);
                                smithay::reexports::calloop::timer::TimeoutAction::Drop
                            });
                        }
                        if let Some((delay, open, mode)) = c.shot.take() {
                            // Espera o atraso pedido (e a janela do app sumir) e captura (ou começa a escolher a área).
                            let _ = h2.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_millis(delay.max(300))), move |_, _, data| {
                                if mode > 0 {
                                    data.state.start_region_shot(open, mode == 2);
                                    return smithay::reexports::calloop::timer::TimeoutAction::Drop;
                                }
                                data.state.request_screenshot();
                                data.state.open_after_shot = open;
                                smithay::reexports::calloop::timer::TimeoutAction::Drop
                            });
                        }
                    }
                    if let Some(kind) = end {
                        st.end_session(&kind);
                    }
                    if let Some(kind) = saver {
                        st.start_saver(Some(kind), true);
                    }
                    if let Some(p) = copy.filter(|p: &PathBuf| p.is_file()) {
                        st.copy_file_to_clipboard(&p);
                    }
                    if let Some((pid, (x, y, items))) = ctx {
                        st.show_client_context_menu(pid, x, y, items);
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
        let mut c = MenuClient { key: 0, pid: 0, stream: a, buf: Vec::new(), pending: Vec::new(), open_subs: Vec::new(), menus: Vec::new(), shot: None, gravar: None, copy: None, end: None, saver: None, edited: false, doc: None, moldura_glove: false, ctx_pending: None, ctx: None };
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

    #[test]
    fn le_menu_de_contexto() {
        let (a, _b) = UnixStream::pair().unwrap();
        let mut c = MenuClient { key: 0, pid: 0, stream: a, buf: Vec::new(), pending: Vec::new(), open_subs: Vec::new(), menus: Vec::new(), shot: None, gravar: None, copy: None, end: None, saver: None, edited: false, doc: None, moldura_glove: false, ctx_pending: None, ctx: None };
        for l in "CTX\t12.5\t40\nITEM\topen\tAbrir\t\t\nSUB\tRótulo\t\nITEM\tlabel:0\tNenhum\t\tc\nENDSUB\nCTXEND".lines() {
            c.line(l);
        }
        let (x, y, items) = c.ctx.take().unwrap();
        assert_eq!((x, y, items.len()), (12.5, 40.0, 2));
        assert!(items[1].is_sub && items[1].sub[0].checked);
        // Os menus da barra não mudam.
        assert!(c.menus.is_empty());
    }
}
