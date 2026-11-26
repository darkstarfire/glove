//! The clipboard, kept by Glove. Whatever is copied (in a Wayland or an X11 app) goes on
//! to the apps of the other kind at once, and its light types (text, HTML, the list of
//! files, a PNG) are read into a history. The app that copied stays the owner while it is
//! open (pasting keeps every format it offers, even its own); when it closes, Glove takes
//! over with what it kept, so pasting still works. Anything copied before can be copied
//! again (the menu in the bar, or Super+Shift+V where one is typing, which also pastes).
//!
//! Nothing is written to disk. A copy that a password manager marks as secret
//! (`x-kde-passwordManagerHint`) is kept only as the current one, out of the history.

use std::{
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
    time::{Duration, Instant},
};

use glove_config::tr;
use smithay::{
    reexports::calloop::channel::{channel, Event as ChannelEvent, Sender},
    wayland::selection::{
        data_device::{current_data_device_selection_userdata, request_data_device_client_selection, set_data_device_selection},
        SelectionTarget,
    },
};

use crate::{
    shell::{MenuAction, MenuNode},
    state::Glove,
};
use glove_paint::chrome::MenuItem;

/// Who offers the selection that Glove set.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipOwner {
    /// An entry of the history (by its id).
    History(u64),
    /// An X11 app owns it (Glove passes the requests on) while it is being read.
    X11,
}

pub struct ClipEntry {
    pub id: u64,
    /// The data in each type (text, picture, list of files…), as the app gave it.
    pub data: Vec<(String, Arc<Vec<u8>>)>,
    /// What the menu shows.
    pub label: String,
    /// Marked as secret: not shown in the history.
    pub secret: bool,
}

#[derive(Default)]
pub struct Clipboard {
    /// Newest first.
    pub history: Vec<ClipEntry>,
    /// The entry that is the clipboard now.
    pub current: Option<u64>,
    next_id: u64,
    /// Each new copy counts one; a reading of an older copy that arrives late is dropped.
    generation: u64,
    tx: Option<Sender<ClipRead>>,
    /// The Wayland app that owns the clipboard now (it copied); Glove takes over when it
    /// goes away. Or an X11 app owns it.
    owner_client: Option<smithay::reexports::wayland_server::backend::ClientId>,
    x11_owned: bool,
    /// The entry with what the owner copied (read), not yet offered by Glove.
    pending: Option<u64>,
    /// The owner went away before the reading ended: take over when it ends.
    owner_gone: bool,
    gone_watch: bool,
}

/// Wayland apps that went away (from `ClientData::disconnected`, outside the state).
pub static GONE: std::sync::Mutex<Option<Sender<smithay::reexports::wayland_server::backend::ClientId>>> = std::sync::Mutex::new(None);

pub struct ClipRead {
    generation: u64,
    data: Vec<(String, Vec<u8>)>,
}

/// How many entries the history keeps, and how much in all.
const MAX_ENTRIES: usize = 25;
const MAX_TOTAL: usize = 128 << 20;
/// The most read from one copy (each type, all types) and how long it may take.
const MAX_TYPE: usize = 32 << 20;
const MAX_COPY: usize = 64 << 20;
const READ_TIME: Duration = Duration::from_secs(3);
/// Types not worth reading (X11 bookkeeping).
const SKIP: [&str; 6] = ["TARGETS", "MULTIPLE", "TIMESTAMP", "SAVE_TARGETS", "DELETE", "INSERT_SELECTION"];
const SECRET_HINT: &str = "x-kde-passwordManagerHint";

/// A pipe: (read end, write end), closed on exec.
fn pipe() -> Option<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return None;
    }
    Some(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Reads all the pipes at the same time (an app may write them in any order) until each
/// ends, up to the limits. Only the types that ended whole are kept.
fn read_all(pipes: Vec<(String, OwnedFd)>) -> Vec<(String, Vec<u8>)> {
    let deadline = Instant::now() + READ_TIME;
    let mut open: Vec<(String, OwnedFd, Vec<u8>)> = pipes.into_iter().map(|(m, f)| (m, f, Vec::new())).collect();
    let mut done = Vec::new();
    let mut total = 0usize;
    let mut buf = vec![0u8; 64 << 10];
    while !open.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let mut pfds: Vec<libc::pollfd> = open.iter().map(|(_, f, _)| libc::pollfd { fd: f.as_raw_fd(), events: libc::POLLIN, revents: 0 }).collect();
        let n = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, left.as_millis().min(1000) as i32) };
        if n < 0 {
            break;
        }
        let mut finished = Vec::new();
        for (i, p) in pfds.iter().enumerate() {
            if p.revents == 0 {
                continue;
            }
            let r = unsafe { libc::read(p.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if r > 0 {
                let r = r as usize;
                total += r;
                open[i].2.extend_from_slice(&buf[..r]);
                if open[i].2.len() > MAX_TYPE || total > MAX_COPY {
                    // Too big: this type is left out.
                    open[i].2 = Vec::new();
                    finished.push((i, false));
                }
            } else if r == 0 {
                finished.push((i, true));
            } else if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                finished.push((i, false));
            }
        }
        finished.sort_by(|a, b| b.0.cmp(&a.0));
        for (i, whole) in finished {
            let (m, _, data) = open.remove(i);
            if whole {
                done.push((m, data));
            }
        }
    }
    done
}

/// The text in the copy, if there is text.
fn text_of(data: &[(String, Vec<u8>)]) -> Option<String> {
    ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT"].iter().find_map(|t| data.iter().find(|(m, _)| m == t)).map(|(_, d)| String::from_utf8_lossy(d).into_owned())
}

/// What the menu shows for a copy: the start of the text, the size of a picture, the
/// name of the file (or how many).
fn label_of(data: &[(String, Vec<u8>)]) -> String {
    if let Some((_, uris)) = data.iter().find(|(m, _)| m == "text/uri-list") {
        let files: Vec<&str> = std::str::from_utf8(uris).unwrap_or("").lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
        if files.len() == 1 {
            let name = files[0].rsplit('/').next().unwrap_or(files[0]);
            return crate::desk::decode_uri(name);
        }
        if files.len() > 1 {
            return tr!("{} arquivos", files.len()).to_string();
        }
    }
    if let Some(t) = text_of(data) {
        let one: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if !one.is_empty() {
            let mut s: String = one.chars().take(60).collect();
            if one.chars().count() > 60 {
                s.push('…');
            }
            return s;
        }
    }
    if let Some((_, png)) = data.iter().find(|(m, _)| m == "image/png") {
        // The size is in the header (IHDR): width and height after the signature.
        if png.len() > 24 {
            let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
            let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
            return tr!("Imagem ({}×{})", w, h).to_string();
        }
    }
    if data.iter().any(|(m, _)| m.starts_with("image/")) {
        return tr!("Imagem").to_string();
    }
    tr!("Conteúdo copiado").to_string()
}

impl Glove {
    fn clip_sender(&mut self) -> Sender<ClipRead> {
        if let Some(tx) = &self.clip.tx {
            return tx.clone();
        }
        let (tx, rx) = channel::<ClipRead>();
        let _ = self.loop_handle.insert_source(rx, |ev, _, data| {
            if let ChannelEvent::Msg(r) = ev {
                data.state.clip_arrived(r);
            }
        });
        self.clip.tx = Some(tx.clone());
        tx
    }

    /// The types read for the history (and to take over when the app goes): the text
    /// ones, HTML, the list of files and a picture. The heavy or app-own types are left to
    /// the app while it is open.
    fn clip_types(mimes: &[String]) -> Vec<String> {
        const LIGHT: [&str; 10] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING", "TEXT", "text/html", "text/uri-list", "x-special/gnome-copied-files", "image/png", SECRET_HINT];
        let mut v: Vec<String> = mimes.iter().filter(|m| LIGHT.contains(&m.as_str())).cloned().collect();
        // A picture in another format only: that one.
        if v.is_empty() {
            v.extend(mimes.iter().filter(|m| m.starts_with("image/")).take(1).cloned());
        }
        v.retain(|m| !SKIP.contains(&m.as_str()));
        v
    }

    /// Watches for apps going away (the owner of the clipboard among them).
    fn clip_watch_gone(&mut self) {
        if self.clip.gone_watch {
            return;
        }
        self.clip.gone_watch = true;
        let (tx, rx) = channel::<smithay::reexports::wayland_server::backend::ClientId>();
        if let Ok(mut g) = GONE.lock() {
            *g = Some(tx);
        }
        let _ = self.loop_handle.insert_source(rx, |ev, _, data| {
            if let ChannelEvent::Msg(id) = ev {
                data.state.clip_client_gone(id);
            }
        });
    }

    /// A Wayland app copied: it goes on to the X11 apps at once, and the light types are
    /// read (after this call: the new selection is only set when the handler returns).
    pub fn clip_from_wayland(&mut self, mimes: Vec<String>) {
        self.clip_watch_gone();
        self.clip.generation += 1;
        let generation = self.clip.generation;
        self.clip.owner_client = self.seat.get_keyboard().and_then(|k| k.current_focus()).and_then(|f| {
            use smithay::{reexports::wayland_server::Resource, wayland::seat::WaylandFocus};
            f.wl_surface().and_then(|s| s.client()).map(|c| c.id())
        });
        self.clip.x11_owned = false;
        self.clip.pending = None;
        self.clip.owner_gone = false;
        if let Some(xwm) = self.xwm.as_mut() {
            let _ = xwm.new_selection(SelectionTarget::Clipboard, Some(mimes.clone()));
        }
        let types = Self::clip_types(&mimes);
        self.loop_handle.insert_idle(move |data| {
            let st = &mut data.state;
            if st.clip.generation != generation {
                return;
            }
            let seat = st.seat.clone();
            let mut pipes = Vec::new();
            for m in types {
                let Some((r, w)) = pipe() else { continue };
                if request_data_device_client_selection(&seat, m.clone(), w).is_ok() {
                    pipes.push((m, r));
                }
            }
            st.clip_read(generation, pipes);
        });
    }

    /// A Wayland app went away: if it owned the clipboard, Glove takes over with what it
    /// kept.
    pub fn clip_client_gone(&mut self, id: smithay::reexports::wayland_server::backend::ClientId) {
        if self.clip.owner_client.as_ref() != Some(&id) {
            return;
        }
        self.clip.owner_client = None;
        self.clip_owner_gone();
    }

    /// The X11 app that owned the clipboard let it go (it closed).
    pub fn clip_x11_cleared(&mut self) {
        if self.clip.x11_owned {
            self.clip.x11_owned = false;
            self.clip_owner_gone();
        }
    }

    fn clip_owner_gone(&mut self) {
        match self.clip.pending.take() {
            Some(id) => {
                if let Some(e) = self.clip.history.iter().find(|e| e.id == id) {
                    let mimes: Vec<String> = e.data.iter().map(|(m, _)| m.clone()).collect();
                    self.clip_offer(id, mimes);
                }
            }
            None => self.clip.owner_gone = true,
        }
    }

    /// An X11 app copied: the Wayland apps can paste at once (Glove passes the requests
    /// on), and it is read to be kept.
    pub fn clip_from_x11(&mut self, mimes: Vec<String>) {
        self.clip.generation += 1;
        let generation = self.clip.generation;
        self.clip.owner_client = None;
        self.clip.x11_owned = true;
        self.clip.pending = None;
        self.clip.owner_gone = false;
        let dh = self.display_handle.clone();
        set_data_device_selection(&dh, &self.seat.clone(), mimes.clone(), ClipOwner::X11);
        let handle = self.loop_handle.clone();
        let mut pipes = Vec::new();
        if let Some(xwm) = self.xwm.as_mut() {
            for m in Self::clip_types(&mimes) {
                let Some((r, w)) = pipe() else { continue };
                if xwm.send_selection(SelectionTarget::Clipboard, m.clone(), w, handle.clone()).is_ok() {
                    pipes.push((m, r));
                }
            }
        }
        self.clip_read(generation, pipes);
    }

    fn clip_read(&mut self, generation: u64, pipes: Vec<(String, OwnedFd)>) {
        if pipes.is_empty() {
            return;
        }
        let tx = self.clip_sender();
        std::thread::spawn(move || {
            let data = read_all(pipes);
            let _ = tx.send(ClipRead { generation, data });
        });
    }

    /// The copy was read: it goes into the history; the app stays the owner (Glove takes
    /// over when it goes away, or already went).
    fn clip_arrived(&mut self, r: ClipRead) {
        if r.generation != self.clip.generation || r.data.is_empty() {
            return;
        }
        let id = self.clip_add(r.data);
        self.clip.current = Some(id);
        self.clip.pending = Some(id);
        if std::mem::take(&mut self.clip.owner_gone) {
            self.clip_owner_gone();
        }
    }

    /// Keeps a copy made by Glove itself (files of the desk, a screenshot) as the newest
    /// entry and makes it the clipboard.
    pub fn clip_keep(&mut self, data: Vec<(String, Vec<u8>)>) {
        let mimes: Vec<String> = data.iter().map(|(m, _)| m.clone()).collect();
        let id = self.clip_add(data);
        self.clip.generation += 1;
        self.clip.owner_client = None;
        self.clip.x11_owned = false;
        self.clip.pending = None;
        self.clip_offer(id, mimes);
    }

    /// The copy as the newest entry of the history (the same again: the older goes).
    fn clip_add(&mut self, data: Vec<(String, Vec<u8>)>) -> u64 {
        let secret = data.iter().any(|(m, d)| m == SECRET_HINT && d.starts_with(b"secret"));
        let label = label_of(&data);
        // The same again (the same text, or the same types and data): the older one goes.
        let text = text_of(&data);
        self.clip.history.retain(|e| {
            let same_text = text.is_some() && text_of(&e.data.iter().map(|(m, d)| (m.clone(), d.to_vec())).collect::<Vec<_>>()) == text;
            let same_all = e.data.len() == data.len() && e.data.iter().zip(&data).all(|(a, b)| a.0 == b.0 && *a.1 == b.1);
            !(same_text || same_all) && !e.secret
        });
        self.clip.next_id += 1;
        let id = self.clip.next_id;
        let data: Vec<(String, Arc<Vec<u8>>)> = data.into_iter().map(|(m, d)| (m, Arc::new(d))).collect();
        self.clip.history.insert(0, ClipEntry { id, data, label, secret });
        // Limits: the number of entries and the memory.
        self.clip.history.truncate(MAX_ENTRIES);
        let mut total = 0usize;
        self.clip.history.retain(|e| {
            total += e.data.iter().map(|(_, d)| d.len()).sum::<usize>();
            total <= MAX_TOTAL || e.id == id
        });
        id
    }

    fn clip_offer(&mut self, id: u64, mimes: Vec<String>) {
        self.clip.current = Some(id);
        let dh = self.display_handle.clone();
        set_data_device_selection(&dh, &self.seat.clone(), mimes.clone(), ClipOwner::History(id));
        if let Some(xwm) = self.xwm.as_mut() {
            let _ = xwm.new_selection(SelectionTarget::Clipboard, Some(mimes));
        }
    }

    /// Makes an entry of the history the clipboard again (it goes to the top).
    pub fn clip_use(&mut self, id: u64) {
        let Some(i) = self.clip.history.iter().position(|e| e.id == id) else { return };
        let e = self.clip.history.remove(i);
        let mimes: Vec<String> = e.data.iter().map(|(m, _)| m.clone()).collect();
        self.clip.history.insert(0, e);
        self.clip.generation += 1;
        self.clip.owner_client = None;
        self.clip.x11_owned = false;
        self.clip.pending = None;
        self.clip_offer(id, mimes);
    }

    pub fn clip_delete(&mut self, id: u64) {
        self.clip.history.retain(|e| e.id != id);
    }

    /// Clears the history (what is the clipboard now stays, so pasting still works).
    pub fn clip_clear(&mut self) {
        let cur = self.clip.current;
        self.clip.history.retain(|e| Some(e.id) == cur);
        for e in &mut self.clip.history {
            e.secret = true;
        }
    }

    /// Writes the data of an entry in the type asked for (in another thread: a big
    /// picture into a pipe takes a while).
    pub fn clip_send(&self, id: u64, mime: &str, fd: OwnedFd) {
        let Some(d) = self.clip.history.iter().find(|e| e.id == id).and_then(|e| e.data.iter().find(|(m, _)| m == mime).map(|(_, d)| d.clone())) else { return };
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = std::fs::File::from(fd).write_all(&d);
        });
    }

    /// An X11 app wants what the Wayland apps (or Glove) offer.
    pub fn clip_send_to_x11(&mut self, mime: String, fd: OwnedFd) {
        let owner = current_data_device_selection_userdata(&self.seat).map(|o| o.clone());
        match owner {
            Some(ClipOwner::History(id)) => self.clip_send(id, &mime, fd),
            Some(ClipOwner::X11) => {}
            None => {
                let _ = request_data_device_client_selection(&self.seat, mime, fd);
            }
        }
    }

    /// A Wayland app wants what Glove offers.
    pub fn clip_send_to_wayland(&mut self, owner: &ClipOwner, mime: String, fd: OwnedFd) {
        match owner {
            ClipOwner::History(id) => self.clip_send(*id, &mime, fd),
            ClipOwner::X11 => {
                let handle = self.loop_handle.clone();
                if let Some(xwm) = self.xwm.as_mut() {
                    let _ = xwm.send_selection(SelectionTarget::Clipboard, mime, fd, handle);
                }
            }
        }
    }

    /// The history as a menu. `paste`: choosing also pastes (the window where one is
    /// typing); otherwise it only becomes the clipboard.
    pub fn clipboard_menu(&self, paste: bool) -> Vec<MenuNode> {
        let shown: Vec<&ClipEntry> = self.clip.history.iter().filter(|e| !e.secret).take(20).collect();
        let mut v = Vec::new();
        if shown.is_empty() {
            v.push(MenuNode::new(MenuItem::item(tr!("Nada copiado ainda")).disabled(), None));
        }
        for e in &shown {
            let action = if paste { MenuAction::ClipPaste(e.id) } else { MenuAction::ClipUse(e.id) };
            v.push(MenuNode::new(MenuItem::check(e.label.clone(), self.clip.current == Some(e.id)), Some(action)));
        }
        if !paste && !shown.is_empty() {
            v.push(MenuNode::sep());
            let mut del = MenuNode::new(MenuItem::sub(tr!("Apagar")), None);
            del.sub = shown.iter().map(|e| MenuNode::new(MenuItem::item(e.label.clone()), Some(MenuAction::ClipDelete(e.id)))).collect();
            v.push(del);
            v.push(MenuNode::new(MenuItem::item(tr!("Limpar Histórico")), Some(MenuAction::ClipClear)));
        }
        v
    }

    /// Super+Shift+V: the history next to where one is typing (the mouse, if it is on the
    /// window in focus; otherwise the middle of that window). Choosing pastes.
    pub fn show_clipboard_popup(&mut self) {
        let nodes = self.clipboard_menu(true);
        let pos = match self.focused_window().and_then(|w| self.space.element_geometry(&w)) {
            Some(g) if g.to_f64().contains(self.pointer_pos) => self.pointer_pos,
            Some(g) => ((g.loc.x + g.size.w / 3) as f64, (g.loc.y + g.size.h / 3) as f64).into(),
            None => self.pointer_pos,
        };
        self.show_context_menu(nodes, pos);
    }

    /// Chooses an entry and pastes it in the window in focus (Ctrl+V; in a terminal,
    /// Ctrl+Shift+V).
    pub fn clip_paste(&mut self, id: u64) {
        self.clip_use(id);
        let terminal = self.focused_window().map(|w| crate::shell::window_info(&w).1.to_lowercase()).is_some_and(|a| a.contains("term") || a.contains("konsole") || a.contains("kitty") || a.contains("alacritty") || a.contains("ptyxis"));
        // After the menu closes and the window has the keyboard again.
        let _ = self.loop_handle.insert_source(smithay::reexports::calloop::timer::Timer::from_duration(Duration::from_millis(60)), move |_, _, data| {
            data.state.inject_paste(terminal);
            smithay::reexports::calloop::timer::TimeoutAction::Drop
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_no_menu() {
        let t = |s: &str| vec![("text/plain;charset=utf-8".to_string(), s.as_bytes().to_vec())];
        assert_eq!(label_of(&t("  olá\n  mundo  ")), "olá mundo");
        let long = "a".repeat(80);
        assert_eq!(label_of(&t(&long)).chars().count(), 61);
        let files = vec![("text/uri-list".to_string(), b"file:///home/x/foto%20nova.png\r\n".to_vec())];
        assert_eq!(label_of(&files), "foto nova.png");
        let mut png = vec![0u8; 30];
        png[16..20].copy_from_slice(&800u32.to_be_bytes());
        png[20..24].copy_from_slice(&600u32.to_be_bytes());
        assert_eq!(label_of(&[("image/png".to_string(), png)]), "Imagem (800×600)");
    }

    #[test]
    fn le_os_canos_juntos() {
        let (r1, w1) = pipe().unwrap();
        let (r2, w2) = pipe().unwrap();
        std::thread::spawn(move || {
            use std::io::Write;
            let mut a = std::fs::File::from(w1);
            let mut b = std::fs::File::from(w2);
            // Big enough to fill a pipe: written in turns, read together.
            let chunk = vec![7u8; 100_000];
            b.write_all(&chunk).unwrap();
            drop(b);
            a.write_all(b"texto").unwrap();
        });
        let mut got = read_all(vec![("a".into(), r1), ("b".into(), r2)]);
        got.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(got[0], ("a".to_string(), b"texto".to_vec()));
        assert_eq!(got[1].1.len(), 100_000);
    }
}
