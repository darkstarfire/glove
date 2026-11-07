//! Terminal do Glove: o núcleo de terminal do Alacritty (sequências de escape,
//! grade, histórico) desenhado com o toolkit do Glove.

use glove_config::tr;
mod blocks;

use std::cell::RefCell;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::rc::Rc;

use alacritty_terminal::{
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{cell::Flags, Config, Term, TermMode},
    vte::ansi::{Color as AColor, CursorShape, NamedColor, Processor, Rgb},
};
use glove_kit::{
    paint::{draw::*, Color, Fonts},
    run,
    ui::Key,
    App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const PAD: f32 = 4.0;
const DEFAULT_PX: f32 = 12.0;

#[derive(Clone)]
struct Listener(Rc<RefCell<Vec<Event>>>);

impl EventListener for Listener {
    fn send_event(&self, e: Event) {
        self.0.borrow_mut().push(e);
    }
}

struct Size {
    cols: usize,
    lines: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Cores 16 a 255 (cubo 6×6×6 e cinzas); as 16 primeiras vêm do tema.
fn indexed(pal: &[(u8, u8, u8); 16], i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => pal[i as usize],
        16..=231 => {
            let i = i - 16;
            let c = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (c(i / 36), c((i / 6) % 6), c(i % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    }
}

struct Terminal {
    /// Tema de cores (Preferências ▸ Terminal), trocado na hora.
    theme: glove_config::TermTheme,
    term: Term<Listener>,
    parser: Processor,
    events: Rc<RefCell<Vec<Event>>>,
    pty: OwnedFd,
    child: std::process::Child,
    exited: bool,
    mono: Fonts,
    px: f32,
    cols: usize,
    lines: usize,
    title: String,
    last_title: String,
    selecting: bool,
    /// Célula do último evento de mouse enviado ao programa (botão esquerdo pressionado).
    mouse_down: Option<(usize, usize)>,
    /// Texto que o programa pediu para copiar (OSC 52).
    pending_copy: Option<String>,
}

/// Abre um pseudoterminal e inicia o shell nele.
fn spawn_shell(cols: u16, rows: u16) -> std::io::Result<(OwnedFd, std::process::Child)> {
    use std::os::unix::process::CommandExt;
    let (mut master, mut slave) = (0, 0);
    let ws = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
    if unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), &ws) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    unsafe {
        let fl = libc::fcntl(master.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, fl | libc::O_NONBLOCK);
        libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
    let mut cmd = std::process::Command::new(&shell);
    // "glove-term -e COMANDO": roda o comando e depois fica no shell (para ver o resultado).
    let args: Vec<String> = std::env::args().collect();
    if let Some(c) = args.iter().position(|a| a == "-e").and_then(|i| args.get(i + 1)) {
        cmd.arg("-c").arg(format!("{c}; printf '\\n[Terminou. Pode fechar esta janela.]\\n'; exec \"{shell}\""));
    }
    cmd.env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env_remove("GLOVE_MENU_SOCKET")
        .current_dir(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
        .stdin(slave.try_clone()?)
        .stdout(slave.try_clone()?)
        .stderr(slave);
    unsafe {
        cmd.pre_exec(|| {
            // Nova sessão com o terminal como controlador (Ctrl+C etc. funcionam).
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    Ok((master, child))
}

impl Terminal {
    fn new() -> std::io::Result<Self> {
        let (cols, lines) = (80, 24);
        let (pty, child) = spawn_shell(cols as u16, lines as u16)?;
        let events = Rc::new(RefCell::new(Vec::new()));
        let config = Config { scrolling_history: 5000, ..Default::default() };
        let term = Term::new(config, &Size { cols, lines }, Listener(events.clone()));
        let cfg = glove_config::Config::load();
        let mono = Fonts::load_family(&cfg.terminal_font).ok_or_else(|| std::io::Error::other(tr!("sem fonte monoespaçada")))?;
        let px = if cfg.terminal_font_size >= 6.0 { cfg.terminal_font_size } else { DEFAULT_PX };
        Ok(Self {
            theme: glove_config::term_theme(&cfg.terminal_theme),
            term,
            parser: Processor::new(),
            events,
            pty,
            child,
            exited: false,
            mono,
            px,
            cols,
            lines,
            title: tr!("Terminal").into(),
            last_title: String::new(),
            selecting: false,
            mouse_down: None,
            pending_copy: None,
        })
    }

    fn cell(&self) -> (f32, f32) {
        let (w, _) = self.mono.mono_metrics(self.px);
        (w, (self.px * 1.3).round())
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.exited {
            return;
        }
        let mut f = unsafe { std::fs::File::from_raw_fd(self.pty.as_raw_fd()) };
        let _ = f.write_all(bytes);
        std::mem::forget(f); // o descritor continua sendo do OwnedFd
    }

    /// Ajusta a grade ao tamanho da janela (e avisa o shell).
    fn resize(&mut self, w: f32, h: f32) {
        let (cw, ch) = self.cell();
        let cols = (((w - 2.0 * PAD - 15.0) / cw).floor() as usize).max(10);
        let lines = (((h - 2.0 * PAD) / ch).floor() as usize).max(3);
        if cols != self.cols || lines != self.lines {
            self.cols = cols;
            self.lines = lines;
            self.term.resize(Size { cols, lines });
            let ws = libc::winsize { ws_row: lines as u16, ws_col: cols as u16, ws_xpixel: 0, ws_ypixel: 0 };
            unsafe { libc::ioctl(self.pty.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
        }
    }

    fn handle_events(&mut self) {
        let evs: Vec<Event> = self.events.borrow_mut().drain(..).collect();
        for e in evs {
            match e {
                Event::PtyWrite(s) => self.write(s.as_bytes()),
                Event::Title(t) => self.title = t,
                Event::ResetTitle => self.title = tr!("Terminal").into(),
                Event::ClipboardStore(_, text) => self.pending_copy = Some(text),
                Event::ColorRequest(i, f) => {
                    let (r, g, b) = if i == 256 + 0 { self.theme.fg } else if i < 256 { indexed(&self.theme.palette, i as u8) } else { self.theme.bg };
                    let reply = f(Rgb { r, g, b });
                    self.write(reply.as_bytes());
                }
                _ => {}
            }
        }
    }

    fn color(&self, c: AColor, fg: bool, bold: bool) -> (u8, u8, u8) {
        let colors = self.term.colors();
        match c {
            AColor::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
            AColor::Indexed(i) => colors[i as usize].map(|c| (c.r, c.g, c.b)).unwrap_or_else(|| indexed(&self.theme.palette, i)),
            AColor::Named(n) => {
                let idx = n as usize;
                if let Some(c) = colors[idx] {
                    return (c.r, c.g, c.b);
                }
                match n {
                    NamedColor::Foreground | NamedColor::BrightForeground => self.theme.fg,
                    NamedColor::Background => self.theme.bg,
                    NamedColor::Cursor => self.theme.cursor,
                    NamedColor::DimForeground => {
                        let (f, b) = (self.theme.fg, self.theme.bg);
                        (((f.0 as u16 + b.0 as u16) / 2) as u8, ((f.1 as u16 + b.1 as u16) / 2) as u8, ((f.2 as u16 + b.2 as u16) / 2) as u8)
                    }
                    _ if idx < 8 && bold && fg => self.theme.palette[idx + 8],
                    _ if idx < 16 => self.theme.palette[idx],
                    _ => {
                        if fg {
                            self.theme.fg
                        } else {
                            self.theme.bg
                        }
                    }
                }
            }
        }
    }

    /// Pixel (relativo à área do texto) → ponto na grade.
    fn point_at(&self, x: f32, y: f32) -> (Point, Side) {
        let (cw, ch) = self.cell();
        let col = ((x - PAD) / cw).max(0.0);
        let row = ((y - PAD) / ch).floor().clamp(0.0, (self.lines - 1) as f32) as i32;
        let side = if col.fract() < 0.5 { Side::Left } else { Side::Right };
        let col = (col as usize).min(self.cols - 1);
        let offset = self.term.grid().display_offset() as i32;
        (Point::new(Line(row - offset), Column(col)), side)
    }

    /// Teclas especiais → bytes para o shell (padrão xterm, com modificadores).
    fn key_bytes(&self, key: Key, combo: &str) -> Option<Vec<u8>> {
        let parts: Vec<&str> = combo.split('+').collect();
        let last = *parts.last().unwrap_or(&"");
        let (shift, alt, ctrl) = (parts.contains(&"Shift"), parts.contains(&"Alt"), parts.contains(&"Ctrl"));
        // Código de modificador do xterm: 1 + Shift(1) + Alt(2) + Ctrl(4).
        let m = 1 + shift as u8 + 2 * alt as u8 + 4 * ctrl as u8;
        let app_cursor = self.term.mode().contains(TermMode::APP_CURSOR);
        let csi_letter = |c: char| -> Vec<u8> {
            if m > 1 {
                format!("\x1b[1;{m}{c}").into_bytes()
            } else if app_cursor {
                format!("\x1bO{c}").into_bytes()
            } else {
                format!("\x1b[{c}").into_bytes()
            }
        };
        let tilde = |n: u8| -> Vec<u8> { if m > 1 { format!("\x1b[{n};{m}~").into_bytes() } else { format!("\x1b[{n}~").into_bytes() } };
        let esc_prefix = |mut v: Vec<u8>| -> Vec<u8> {
            if alt {
                v.insert(0, 0x1b);
            }
            v
        };
        Some(match key {
            Key::Enter => esc_prefix(vec![b'\r']),
            Key::Backspace => esc_prefix(vec![if ctrl { 0x08 } else { 0x7f }]),
            Key::Tab if shift => b"\x1b[Z".to_vec(),
            Key::Tab => vec![b'\t'],
            Key::Escape => vec![0x1b],
            Key::Up => csi_letter('A'),
            Key::Down => csi_letter('B'),
            Key::Right => csi_letter('C'),
            Key::Left => csi_letter('D'),
            Key::Home => csi_letter('H'),
            Key::End => csi_letter('F'),
            Key::Delete => tilde(3),
            Key::PageUp => tilde(5),
            Key::PageDown => tilde(6),
            Key::Other => {
                // Ctrl+letra → caractere de controle (Ctrl+C = 0x03 …)
                if ctrl && !shift && last.len() == 1 {
                    let c = last.as_bytes()[0].to_ascii_uppercase();
                    if c.is_ascii_uppercase() || b"[\\]^_@".contains(&c) {
                        return Some(esc_prefix(vec![c & 0x1f]));
                    }
                }
                if ctrl && last == "space" {
                    return Some(vec![0]);
                }
                let f14 = |c: char| -> Vec<u8> { if m > 1 { format!("\x1b[1;{m}{c}").into_bytes() } else { format!("\x1bO{c}").into_bytes() } };
                match last {
                    "F1" => f14('P'),
                    "F2" => f14('Q'),
                    "F3" => f14('R'),
                    "F4" => f14('S'),
                    "F5" => tilde(15),
                    "F6" => tilde(17),
                    "F7" => tilde(18),
                    "F8" => tilde(19),
                    "F9" => tilde(20),
                    "F10" => tilde(21),
                    "F11" => tilde(23),
                    "F12" => tilde(24),
                    "Insert" => tilde(2),
                    _ => return None,
                }
            }
        })
    }

    /// Envia um evento de mouse ao programa (modo SGR ou o antigo X10).
    fn mouse_report(&mut self, button: u8, col: usize, row: usize, press: bool) {
        let mode = *self.term.mode();
        if mode.contains(TermMode::SGR_MOUSE) {
            let s = format!("\x1b[<{button};{};{}{}", col + 1, row + 1, if press { 'M' } else { 'm' });
            self.write(s.as_bytes());
        } else {
            let b = if press { button } else { 3 };
            let (x, y) = ((col + 33).min(255) as u8, (row + 33).min(255) as u8);
            self.write(&[0x1b, b'[', b'M', 32 + b, x, y]);
        }
    }

    /// Célula (coluna, linha visível) sob o ponteiro.
    fn cell_at(&self, x: f32, y: f32) -> (usize, usize) {
        let (cw, ch) = self.cell();
        let col = (((x - PAD) / cw).max(0.0) as usize).min(self.cols - 1);
        let row = (((y - PAD) / ch).max(0.0) as usize).min(self.lines - 1);
        (col, row)
    }

    fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.term.mode().contains(TermMode::BRACKETED_PASTE) {
            self.write(b"\x1b[200~");
            self.write(text.as_bytes());
            self.write(b"\x1b[201~");
        } else {
            self.write(text.as_bytes());
        }
        self.term.scroll_display(Scroll::Bottom);
    }

    fn draw(&self, c: &mut Canvas, fonts_ui: &Fonts, area: Rect, accent: Color) {
        let (cw, ch) = self.cell();
        let (_, ascent) = self.mono.mono_metrics(self.px);
        let (tb, tf, tc) = (self.theme.bg, self.theme.fg, self.theme.cursor);
        c.fill_rect(area.x, area.y, area.w, area.h, &solid(rgba(tb.0, tb.1, tb.2, 255)));
        let content = self.term.renderable_content();
        let offset = content.display_offset as i32;
        let sel = content.selection;
        let cursor = content.cursor.point;
        let shape = content.cursor.shape;
        let show_cursor = content.mode.contains(TermMode::SHOW_CURSOR) && offset == 0 && shape != CursorShape::Hidden;
        let mut cursor_rect: Option<(f32, f32, f32, (u8, u8, u8))> = None;
        let mut buf = [0u8; 4];
        // Primeiro todos os fundos, depois as letras: o fundo da célula seguinte não
        // corta o pedaço de letra que passa da célula (itálico, ícones, blocos).
        let mut glyphs: Vec<(f32, f32, f32, (u8, u8, u8), char, Flags, Option<(u8, u8, u8)>)> = Vec::new();
        for cell in content.display_iter {
            let row = cell.point.line.0 + offset;
            if row < 0 || row as usize >= self.lines {
                continue;
            }
            let x = area.x + PAD + cell.point.column.0 as f32 * cw;
            let y = area.y + PAD + row as f32 * ch;
            let flags = cell.flags;
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let bold = flags.contains(Flags::BOLD);
            let mut fg = self.color(cell.fg, true, bold);
            let mut bg = self.color(cell.bg, false, false);
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let selected = sel.as_ref().is_some_and(|s| s.contains(cell.point));
            let is_cursor = show_cursor && cell.point == cursor;
            let width = if flags.contains(Flags::WIDE_CHAR) { 2.0 * cw } else { cw };
            if selected {
                bg = { let l = lighten(accent, 0.6); ((l.red() * 255.0) as u8, (l.green() * 255.0) as u8, (l.blue() * 255.0) as u8) };
            }
            if is_cursor {
                if shape == CursorShape::Block {
                    std::mem::swap(&mut fg, &mut bg);
                    if bg == tb {
                        bg = tc;
                        fg = tb;
                    }
                    let _ = tf;
                } else {
                    // Barra, sublinhado ou bloco vazado: desenhado por cima depois.
                    cursor_rect = Some((x, y, width, fg));
                }
            }
            if bg != tb {
                c.fill_cell(x, y, width, ch, rgba(bg.0, bg.1, bg.2, 255));
            }
            let uc = cell.underline_color().map(|u| self.color(u, true, false));
            glyphs.push((x, y, width, fg, cell.c, flags, uc));
        }
        for (x, y, width, fg, chr, flags, uc) in glyphs {
            let bold = flags.contains(Flags::BOLD);
            if chr != ' ' && chr != '\t' && !flags.contains(Flags::HIDDEN) {
                let alpha = if flags.contains(Flags::DIM) { 150 } else { 255 };
                let col = rgba(fg.0, fg.1, fg.2, alpha);
                if !crate::blocks::draw(c, chr, x, y, width, ch, col) {
                    let s: &str = chr.encode_utf8(&mut buf);
                    self.mono.draw(c, s, x, y + ((ch - self.px * 1.17) / 2.0).max(0.0) + ascent, self.px, bold, col);
                }
            }
            if flags.intersects(Flags::ALL_UNDERLINES) {
                let uc = uc.unwrap_or(fg);
                let ink = solid(rgba(uc.0, uc.1, uc.2, 255));
                let uy = y + ch - 2.0;
                if flags.contains(Flags::UNDERCURL) {
                    // Sublinhado ondulado (erros do LSP etc.).
                    let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
                    pb.move_to(x, uy);
                    let mut xx = x;
                    let step = (cw / 2.0).max(2.0);
                    let mut up = true;
                    while xx < x + width {
                        pb.quad_to(xx + step / 2.0, if up { uy - 2.0 } else { uy + 2.0 }, xx + step, uy);
                        xx += step;
                        up = !up;
                    }
                    c.stroke(pb.finish(), &ink, 1.0);
                } else if flags.contains(Flags::DOUBLE_UNDERLINE) {
                    c.fill_rect(x, uy - 2.0, width, 1.0, &ink);
                    c.fill_rect(x, uy, width, 1.0, &ink);
                } else if flags.contains(Flags::DOTTED_UNDERLINE) || flags.contains(Flags::DASHED_UNDERLINE) {
                    let dash = if flags.contains(Flags::DOTTED_UNDERLINE) { 1.0 } else { 3.0 };
                    let mut xx = x;
                    while xx < x + width {
                        c.fill_rect(xx, uy, dash, 1.0, &ink);
                        xx += dash * 2.0;
                    }
                } else {
                    c.fill_rect(x, uy, width, 1.0, &ink);
                }
            }
            if flags.contains(Flags::STRIKEOUT) {
                c.fill_rect(x, y + ch / 2.0, width, 1.0, &solid(rgba(fg.0, fg.1, fg.2, 255)));
            }
        }
        if let Some((x, y, width, col)) = cursor_rect {
            let ink = solid(rgba(col.0, col.1, col.2, 255));
            match shape {
                CursorShape::Beam => c.fill_rect(x, y, 2.0, ch, &ink),
                CursorShape::Underline => c.fill_rect(x, y + ch - 2.0, width, 2.0, &ink),
                _ => c.stroke(rrect(x + 0.5, y + 0.5, width - 1.0, ch - 1.0, 0.0), &ink, 1.0),
            }
        }
        if self.exited {
            let msg = tr!("[Processo concluído]");
            let tw = fonts_ui.measure(msg, 12.0, true);
            c.fill(rrect(area.x + area.w - tw - 30.0, area.y + area.h - 30.0, tw + 20.0, 22.0, 11.0), &solid(rgba(0, 0, 0, 150)));
            fonts_ui.draw(c, msg, area.x + area.w - tw - 20.0, area.y + area.h - 14.0, 12.0, true, Color::WHITE);
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Encerra o shell junto com a janela.
        unsafe { libc::kill(self.child.id() as i32, libc::SIGHUP) };
        let _ = self.child.try_wait();
    }
}

impl App for Terminal {
    /// Preferências mudaram: o tema de cores vale na hora, nesta janela também.
    fn config_changed(&mut self) {
        self.theme = glove_config::term_theme(&glove_config::Config::load().terminal_theme);
    }

    fn watch_fd(&self) -> Option<RawFd> {
        Some(self.pty.as_raw_fd())
    }

    fn fd_ready(&mut self) -> bool {
        let mut buf = [0u8; 65536];
        loop {
            let n = unsafe { libc::read(self.pty.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
            if n > 0 {
                self.parser.advance(&mut self.term, &buf[..n as usize]);
                continue;
            }
            let err = std::io::Error::last_os_error();
            if n < 0 && err.kind() == std::io::ErrorKind::WouldBlock {
                break;
            }
            if n < 0 && err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            // Fim (shell saiu) ou erro.
            self.exited = true;
            let _ = self.child.try_wait();
            self.handle_events();
            return false;
        }
        self.handle_events();
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        self.resize(w, h);
        let area = Rect::new(0.0, 0.0, w - 15.0, h);

        // Colar que chegou
        if let Some(t) = ui.take_paste() {
            self.paste(&t);
        }
        // Arquivos ou pastas soltos aqui: o caminho completo, entre aspas se preciso
        // (como no Terminal do Mac), seguido de um espaço.
        ui.drop_as_copy();
        if let Some((paths, _)) = ui.take_drop() {
            let text: Vec<String> = paths.iter().map(|p| shell_quote(&p.to_string_lossy())).collect();
            self.paste(&format!("{} ", text.join(" ")));
        }
        // Teclado: atalhos do Terminal primeiro, o resto vai para o shell.
        if ui.menu_actions().iter().any(|a| a == "prefs") {
            // Preferências do Terminal: o painel Terminal das Preferências do Glove.
            let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-prefs"))).filter(|p| p.is_file());
            let _ = std::process::Command::new(exe.unwrap_or_else(|| "glove-prefs".into())).arg("terminal").spawn();
        }
        let keys: Vec<_> = ui.keys().to_vec();
        for k in &keys {
            match k.combo.as_str() {
                "Ctrl+Shift+C" => {
                    if let Some(t) = self.term.selection_to_string() {
                        ui.copy(&t);
                    }
                    continue;
                }
                "Ctrl+Shift+V" => {
                    ui.request_paste();
                    continue;
                }
                "Ctrl+Shift+N" => {
                    let _ = std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "glove-term".into())).spawn();
                    continue;
                }
                "Ctrl+Shift+W" => {
                    ui.close = true;
                    continue;
                }
                "Ctrl+Shift+equal" | "Ctrl+Shift+plus" | "Ctrl+equal" => {
                    self.px = (self.px + 1.0).min(32.0);
                    continue;
                }
                "Ctrl+minus" | "Ctrl+Shift+minus" => {
                    self.px = (self.px - 1.0).max(8.0);
                    continue;
                }
                "Ctrl+Shift+K" => {
                    // Limpa o histórico de rolagem.
                    self.term.grid_mut().clear_history();
                    continue;
                }
                "Shift+Page_Up" | "Shift+Prior" => {
                    self.term.scroll_display(Scroll::PageUp);
                    continue;
                }
                "Shift+Page_Down" | "Shift+Next" => {
                    self.term.scroll_display(Scroll::PageDown);
                    continue;
                }
                _ => {}
            }
            if let Some(bytes) = self.key_bytes(k.key, &k.combo) {
                self.term.scroll_display(Scroll::Bottom);
                self.write(&bytes);
            }
        }
        // Enter, Tab, Backspace etc. já foram enviados como teclas acima.
        let typed: String = ui.typed().chars().filter(|c| !c.is_control()).collect();
        if !typed.is_empty() {
            self.term.scroll_display(Scroll::Bottom);
            if ui.mods().alt {
                self.write(b"\x1b");
            }
            self.write(typed.as_bytes());
        }

        if let Some(t) = self.pending_copy.take() {
            ui.copy(&t);
        }

        let resp = ui.hit(area);
        let (px, py) = ui.pointer();
        let mode = *self.term.mode();
        let wheel = ui.scroll_in(area);
        let to_program = mode.intersects(TermMode::MOUSE_MODE) && !ui.mods().shift;
        if to_program {
            // O programa (vim, htop…) recebe cliques, arrasto e roda.
            let (col, row) = self.cell_at(px, py);
            if resp.pressed {
                self.mouse_report(0, col, row, true);
                self.mouse_down = Some((col, row));
            } else if resp.down {
                if ui.moved() && mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) && self.mouse_down != Some((col, row)) {
                    self.mouse_report(32, col, row, true);
                    self.mouse_down = Some((col, row));
                }
            } else if self.mouse_down.take().is_some() {
                self.mouse_report(0, col, row, false);
            }
            if wheel != 0.0 {
                let n = (wheel / 15.0 * 3.0).round() as i32;
                for _ in 0..n.unsigned_abs().max(1) {
                    self.mouse_report(if wheel < 0.0 { 64 } else { 65 }, col, row, true);
                }
            }
        } else {
            // Selecionar arrastando; clique duplo seleciona a palavra.
            if resp.pressed {
                let (pt, side) = self.point_at(px, py);
                let ty = if resp.double { SelectionType::Semantic } else { SelectionType::Simple };
                self.term.selection = Some(Selection::new(ty, pt, side));
                self.selecting = true;
            } else if resp.down && self.selecting {
                let (pt, side) = self.point_at(px, py);
                if let Some(s) = self.term.selection.as_mut() {
                    s.update(pt, side);
                }
            } else if !resp.down {
                if self.selecting && self.term.selection.as_ref().and_then(|s| s.to_range(&self.term)).is_none() {
                    self.term.selection = None;
                }
                self.selecting = false;
            }
            if wheel != 0.0 {
                let lines = (wheel / 15.0 * 3.0).round() as i32;
                if mode.contains(TermMode::ALT_SCREEN) {
                    // Tela cheia sem mouse (ex.: less): a roda vira setas.
                    let seq: &[u8] = if lines > 0 { b"\x1b[B" } else { b"\x1b[A" };
                    for _ in 0..lines.unsigned_abs() {
                        self.write(seq);
                    }
                } else {
                    self.term.scroll_display(Scroll::Delta(-lines));
                }
            }
        }

        // Título
        let title = format!("{} — {}×{}", self.title, self.cols, self.lines);
        if title != self.last_title {
            ui.set_title(&title);
            self.last_title = title;
        }

        let accent = ui.theme.accent;
        let fonts = ui.fonts;
        self.draw(ui.c, fonts, area, accent);

        // Barra de rolagem (indica a posição no histórico)
        let sb = Rect::new(w - 15.0, 0.0, 15.0, h - 15.0);
        ui.c.fill_rect(sb.x, sb.y, sb.w, sb.h, &linear(sb.x, 0.0, sb.x + sb.w, 0.0, &[(0.0, rgba(210, 210, 214, 255)), (0.5, rgba(240, 240, 242, 255)), (1.0, rgba(220, 220, 224, 255))]));
        ui.c.vline(sb.x, sb.y, sb.h, rgba(150, 150, 156, 255));
        let hist = self.term.grid().history_size();
        if hist > 0 {
            let total = (hist + self.lines) as f32;
            let th = (sb.h * self.lines as f32 / total).max(28.0);
            let off = self.term.grid().display_offset() as f32;
            let ty = sb.y + (sb.h - th) * (1.0 - off / hist as f32);
            gel_rect(ui.c, sb.x + 2.0, ty + 1.0, sb.w - 3.0, th - 2.0, (sb.w - 3.0) / 2.0, accent);
        }
    }

    fn menus(&self) -> Vec<Menu> {
        let sel = self.term.selection.is_some();
        vec![
            Menu::app(vec![MenuItem::new("prefs", tr!("Preferências…"), "")]),
            Menu::new("Shell", vec![MenuItem::new("new", tr!("Nova janela"), "Ctrl+Shift+N"), MenuItem::Separator, MenuItem::new("close", tr!("Fechar"), "Ctrl+Shift+W")]),
            Menu::new(
                tr!("Editar"),
                vec![
                    MenuItem::new("copy", tr!("Copiar"), "Ctrl+Shift+C").enabled(sel),
                    MenuItem::new("paste", tr!("Colar"), "Ctrl+Shift+V"),
                    MenuItem::Separator,
                    MenuItem::new("clear", tr!("Limpar histórico"), "Ctrl+Shift+K"),
                ],
            ),
            Menu::new(tr!("Fonte"), vec![MenuItem::new("bigger", tr!("Aumentar"), "Ctrl+equal"), MenuItem::new("smaller", tr!("Diminuir"), "Ctrl+minus")]),
        ]
    }
}

pub fn main() {
    let term = match Terminal::new() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("glove-term: não consegui abrir o terminal: {e}");
            std::process::exit(1);
        }
    };
    let (cw, ch) = term.cell();
    let size = ((80.0 * cw + 2.0 * PAD + 15.0).ceil() as u32, (24.0 * ch + 2.0 * PAD).ceil() as u32);
    let opts = WindowOptions { title: tr!("Terminal").into(), app_id: "glove.Terminal".into(), size, fixed: false, min_size: None };
    if let Err(e) = run(opts, term) {
        eprintln!("glove-term: {e}");
    }
}

/// Caminho pronto para o shell: sem aspas se só tiver caracteres seguros; senão
/// entre aspas simples (com ' escapado como '\'').
fn shell_quote(s: &str) -> String {
    let safe = !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "/._-+,:@%=".contains(c));
    if safe {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod drop_tests {
    #[test]
    fn caminhos_para_o_shell() {
        assert_eq!(super::shell_quote("/home/ana/notas.txt"), "/home/ana/notas.txt");
        assert_eq!(super::shell_quote("/home/ana/Área de trabalho"), "'/home/ana/Área de trabalho'");
        assert_eq!(super::shell_quote("/tmp/it's"), "'/tmp/it'\\''s'");
    }
}
