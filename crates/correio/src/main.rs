//! Correio (como o Mail do Tiger): caixas à esquerda, mensagens em cima e a
//! mensagem aberta embaixo; Nova Mensagem, Responder e Encaminhar abrem
//! janelas de escrever. IMAP para ler e SMTP para enviar, com TLS.

mod imap;
mod mime;
mod net;
mod smtp;
mod store;
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glove_kit::{
    paint::{draw::*, icons, icons::Icon, Color},
    run, AlertKind, App, Choice, Column, FileBrowser, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, TextBuffer, Ui, WindowOptions, SEARCH_H,
};
use glove_kit::ui::Key;
use net::Security;
use store::{Account, Row};
use worker::{Cmd, Shared};

const TOOL_H: f32 = 58.0;
const SIDE_W: f32 = 180.0;
const FOOT_H: f32 = 22.0;
const ACCOUNTS_WIN: u32 = 1000;
/// Busca automática de mensagens novas.
const AUTO_CHECK: Duration = Duration::from_secs(300);

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn downloads() -> PathBuf {
    let out = std::process::Command::new("xdg-user-dir").arg("DOWNLOAD").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let p = PathBuf::from(out);
    if p.is_dir() && p != home { p } else { home.join("Downloads") }
}

fn size_text(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{} KB", n / 1024)
    } else {
        format!("{:.1} MB", n as f64 / 1048576.0).replace('.', glove_config::virgula())
    }
}

/// Quebra em linhas que caibam (palavras longas demais são cortadas).
fn wrap(ui: &Ui, text: &str, px: f32, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let try_line = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if ui.text_width(&try_line, px, false) <= max {
                line = try_line;
                continue;
            }
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            // A palavra sozinha não cabe: corta por caracteres.
            let mut cur = String::new();
            for c in word.chars() {
                cur.push(c);
                if ui.text_width(&cur, px, false) > max && cur.chars().count() > 1 {
                    let last = cur.pop().unwrap();
                    out.push(std::mem::take(&mut cur));
                    cur.push(last);
                }
            }
            line = cur;
        }
        out.push(line);
    }
    out
}

/// Segurança pela porta (o que os servidores usam de fato).
fn sec_for(host: &str, port: u16, smtp: bool) -> Security {
    let local = host == "localhost" || host == "127.0.0.1" || host == "::1";
    match (smtp, port) {
        (false, 993) | (true, 465) => Security::Tls,
        (true, 587) | (true, 25) if !local => Security::StartTls,
        _ if local => Security::Plain,
        (false, _) => Security::Tls,
        (true, _) => Security::StartTls,
    }
}

struct Compose {
    acct: usize,
    to: String,
    cc: String,
    subject: String,
    body: TextBuffer,
    attachments: Vec<(String, Vec<u8>)>,
    reply: Option<(String, u32)>,
    in_reply_to: String,
    references: String,
    sending: bool,
    error: Option<String>,
    fb: Option<FileBrowser>,
    confirm_close: bool,
    first: bool,
}

impl Compose {
    fn blank(acct: usize) -> Self {
        Compose {
            acct,
            to: String::new(),
            cc: String::new(),
            subject: String::new(),
            body: TextBuffer::new(""),
            attachments: Vec::new(),
            reply: None,
            in_reply_to: String::new(),
            references: String::new(),
            sending: false,
            error: None,
            fb: None,
            confirm_close: false,
            first: true,
        }
    }

    fn has_content(&self) -> bool {
        !self.to.trim().is_empty() || !self.subject.trim().is_empty() || !self.body.text().trim().is_empty() || !self.attachments.is_empty()
    }
}

struct AccountForm {
    /// Conta sendo editada (None = nova).
    index: Option<usize>,
    a: Account,
    imap_port: String,
    smtp_port: String,
    /// O usuário mexeu nos servidores (não preencher mais sozinho).
    touched: bool,
    testing: bool,
    error: Option<String>,
    test_id: u32,
    /// Põe o cursor no e-mail ao abrir (uma vez só).
    focus: bool,
}

impl AccountForm {
    fn new(index: Option<usize>, a: Account) -> Self {
        let touched = index.is_some();
        AccountForm { index, imap_port: a.imap_port.to_string(), smtp_port: a.smtp_port.to_string(), a, touched, testing: false, error: None, test_id: 0, focus: index.is_none() }
    }

    fn finished(&self) -> Account {
        let mut a = self.a.clone();
        a.imap_port = self.imap_port.trim().parse().unwrap_or(993);
        a.smtp_port = self.smtp_port.trim().parse().unwrap_or(587);
        a.imap_sec = sec_for(&a.imap_host, a.imap_port, false);
        a.smtp_sec = sec_for(&a.smtp_host, a.smtp_port, true);
        if a.user.trim().is_empty() {
            a.user = a.email.clone();
        }
        if a.name.trim().is_empty() {
            a.name = a.email.split('@').next().unwrap_or("").to_string();
        }
        a
    }
}

struct Correio {
    /// Imprimir (Ctrl+P) a mensagem aberta.
    print: Option<glove_kit::print::PrintSheet>,
    shared: Arc<Mutex<Shared>>,
    tx: Sender<Cmd>,
    accounts: Vec<Account>,
    selected: Option<u32>,
    table: TableState,
    search: String,
    composes: HashMap<u32, Compose>,
    next_win: u32,
    form: Option<AccountForm>,
    /// Janela de contas aberta.
    accounts_open: bool,
    preview_scroll: f32,
    preview_cache: Option<(u32, i32, Vec<String>)>,
    last_check: Instant,
    message: Option<String>,
    message_at: Instant,
    confirm_remove: bool,
}

impl Correio {
    fn open_compose(&mut self, ui: &mut Ui, c: Compose) {
        let id = self.next_win;
        self.next_win += 1;
        let title = if c.subject.is_empty() { "Nova Mensagem".to_string() } else { c.subject.clone() };
        self.composes.insert(id, c);
        ui.open_window(id, WindowOptions { title, app_id: "glove.Correio".into(), size: (620, 480), fixed: false, min_size: None });
    }

    fn open_accounts(&mut self, ui: &mut Ui) {
        if self.form.is_none() {
            self.form = Some(match self.accounts.first() {
                Some(a) => AccountForm::new(Some(0), a.clone()),
                None => AccountForm::new(None, Account { imap_port: 993, smtp_port: 587, ..Default::default() }),
            });
        }
        if !self.accounts_open {
            self.accounts_open = true;
            ui.open_window(ACCOUNTS_WIN, WindowOptions { title: "Contas de Correio".into(), app_id: "glove.Correio".into(), size: (560, 400), fixed: true, min_size: None });
        }
    }

    fn current(&self) -> Option<(usize, String)> {
        self.shared.lock().unwrap().current.clone()
    }

    fn selected_row(&self) -> Option<Row> {
        let s = self.shared.lock().unwrap();
        self.selected.and_then(|u| s.rows.iter().find(|r| r.uid == u).cloned())
    }

    /// Responder / Responder a Todos / Encaminhar a partir da mensagem aberta.
    fn respond(&mut self, ui: &mut Ui, kind: &str) {
        let Some((acct, folder)) = self.current() else { return };
        let Some(row) = self.selected_row() else { return };
        let (text, atts) = {
            let s = self.shared.lock().unwrap();
            match &s.open {
                Some(m) if m.uid == row.uid => (m.parsed.text.clone(), m.parsed.attachments.iter().map(|a| (a.name.clone(), a.data.clone())).collect::<Vec<_>>()),
                _ => (String::new(), Vec::new()),
            }
        };
        let me = mime::address(&self.accounts[acct].email).to_lowercase();
        let mut c = Compose::blank(acct);
        let subj = row.subject.clone();
        let low = subj.to_lowercase();
        if kind == "forward" {
            c.subject = if low.starts_with("enc:") || low.starts_with("fwd:") || low.starts_with("fw:") { subj } else { format!("Enc: {subj}") };
            c.body = TextBuffer::new(format!("\n\n---------- Mensagem encaminhada ----------\nDe: {}\nData: {}\nAssunto: {}\nPara: {}\n\n{}", row.from, store::long_date(row.date), row.subject, row.to, text));
            c.attachments = atts;
        } else {
            c.subject = if low.starts_with("re:") { subj } else { format!("Re: {subj}") };
            c.to = row.from.clone();
            if kind == "reply-all" {
                let others: Vec<String> = mime::split_addresses(&row.to).into_iter().chain(mime::split_addresses(&row.cc)).filter(|a| mime::address(a).to_lowercase() != me && mime::address(a).to_lowercase() != mime::address(&row.from).to_lowercase()).collect();
                c.cc = others.join(", ");
            }
            c.body = TextBuffer::new(format!("\n\nEm {}, {} escreveu:\n{}", store::long_date(row.date), mime::display_name(&row.from), mime::quote(&text)));
            c.reply = Some((folder, row.uid));
            c.in_reply_to = row.message_id.clone();
            c.references = row.references.clone();
        }
        self.open_compose(ui, c);
    }

    fn toolbar(&mut self, ui: &mut Ui) -> Option<&'static str> {
        let w = ui.c.w;
        ui.c.brushed_metal(0.0, 0.0, w, TOOL_H, rgba(226, 226, 228, 255), rgba(190, 190, 194, 255));
        ui.c.hline(0.0, TOOL_H - 1.0, w, rgba(120, 120, 126, 255));
        let has = self.selected.is_some();
        let items: [(&str, &'static str, bool); 6] = [("Apagar", "delete", has), ("Responder", "reply", has), ("Resp. a Todos", "reply-all", has), ("Encaminhar", "forward", has), ("Nova Mensagem", "new", !self.accounts.is_empty()), ("Receber", "get", !self.accounts.is_empty())];
        let mut act = None;
        let mut x = 12.0;
        for (label, id, on) in items {
            let bw = 76.0;
            let r = Rect::new(x, 4.0, bw, TOOL_H - 8.0);
            let hit = ui.hit(r);
            let ink = if on { rgba(40, 40, 46, 255) } else { rgba(150, 150, 156, 255) };
            if on && hit.down {
                ui.c.fill(rrect(r.x + 8.0, r.y, r.w - 16.0, 32.0, 6.0), &solid(rgba(0, 0, 0, 40)));
            }
            let (cx, cy) = (x + bw / 2.0, 20.0);
            tool_glyph(ui, id, cx, cy, ink);
            let tw = ui.text_width(label, 10.0, false);
            ui.fonts.draw(ui.c, label, cx - tw / 2.0, TOOL_H - 9.0, 10.0, false, ink);
            if on && hit.clicked {
                act = Some(id);
            }
            x += bw + 4.0;
        }
        ui.search_field_in(Rect::new(w - 200.0, (TOOL_H - SEARCH_H) / 2.0, 186.0, SEARCH_H), &mut self.search, "Buscar");
        act
    }

    fn sidebar(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(231, 237, 246, 255)));
        ui.c.vline(r.x + r.w - 1.0, r.y, r.h, rgba(160, 160, 166, 255));
        let folders = self.shared.lock().unwrap().folders.clone();
        let current = self.current();
        let mut y = r.y + 8.0;
        for (ai, a) in self.accounts.iter().enumerate() {
            let head = a.name.to_uppercase();
            let head = ui.fonts.ellipsize(&head, 10.0, true, r.w - 20.0);
            ui.fonts.draw(ui.c, &head, r.x + 10.0, y + 12.0, 10.0, true, rgba(110, 120, 135, 255));
            y += 20.0;
            let list = folders.get(ai).cloned().unwrap_or_default();
            let list = if list.is_empty() { vec![imap::Folder { raw: "INBOX".into(), name: "INBOX".into(), role: "inbox".into() }] } else { list };
            for f in list {
                let row = Rect::new(r.x, y, r.w - 1.0, 20.0);
                let sel = current.as_ref() == Some(&(ai, f.raw.clone()));
                if sel {
                    ui.c.fill_rect(row.x, row.y, row.w, row.h, &vgrad(row.y, row.y + row.h, &[(0.0, rgba(110, 150, 215, 255)), (1.0, rgba(60, 105, 190, 255))]));
                }
                let icon = match f.role.as_str() {
                    "\\trash" => Icon::Trash,
                    "\\sent" | "\\drafts" => Icon::Document,
                    _ => Icon::Folder,
                };
                icons::draw_bare(ui.c, ui.fonts, icon, r.x + 14.0, y + 2.0, 16.0, None);
                let label = ui.fonts.ellipsize(&worker::folder_label(&f), 12.0, false, r.w - 50.0);
                ui.fonts.draw(ui.c, &label, r.x + 36.0, y + 15.0, 12.0, sel, if sel { Color::WHITE } else { rgba(30, 30, 34, 255) });
                if ui.hit(row).clicked && !sel {
                    self.selected = None;
                    self.table.scroll = 0.0;
                    let _ = self.tx.send(Cmd::Open(ai, f.raw.clone()));
                }
                y += 20.0;
            }
            y += 8.0;
        }
    }

    fn preview(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        let Some(row) = self.selected_row() else {
            let t = "Nenhuma mensagem selecionada";
            let tw = ui.text_width(t, 12.0, false);
            ui.fonts.draw(ui.c, t, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0, 12.0, false, rgba(150, 150, 156, 255));
            return;
        };
        let (open_ok, attachments) = {
            let s = self.shared.lock().unwrap();
            match &s.open {
                Some(m) if m.uid == row.uid => (true, m.parsed.attachments.iter().map(|a| (a.name.clone(), a.data.len())).collect::<Vec<_>>()),
                _ => (false, Vec::new()),
            }
        };
        // Cabeçalho da mensagem.
        let grey = rgba(110, 110, 116, 255);
        let ink = rgba(25, 25, 30, 255);
        let mut fields = vec![("De:", row.from.clone()), ("Data:", store::long_date(row.date)), ("Para:", row.to.clone())];
        if !row.cc.is_empty() {
            fields.push(("Cc:", row.cc.clone()));
        }
        fields.insert(1, ("Assunto:", row.subject.clone()));
        let mut y = r.y + 18.0;
        for (k, v) in &fields {
            let tw = ui.text_width(k, 11.0, true);
            ui.fonts.draw(ui.c, k, r.x + 70.0 - tw, y, 11.0, true, grey);
            let v = ui.fonts.ellipsize(v, 11.0, *k == "Assunto:", r.w - 90.0);
            ui.fonts.draw(ui.c, &v, r.x + 76.0, y, 11.0, *k == "Assunto:", ink);
            y += 16.0;
        }
        // Anexos: nome, tamanho e Salvar.
        for (i, (name, size)) in attachments.iter().enumerate() {
            let label = format!("{name} ({})", size_text(*size));
            let label = ui.fonts.ellipsize(&label, 11.0, false, r.w - 190.0);
            icons::draw_bare(ui.c, ui.fonts, Icon::Clip, r.x + 74.0, y - 11.0, 16.0, None);
            ui.fonts.draw(ui.c, &label, r.x + 94.0, y + 2.0, 11.0, false, ink);
            let mut save = false;
            ui.area(Rect::new(r.x + r.w - 90.0, y - 12.0, 80.0, 20.0), |ui| save = ui.small_button("Salvar"));
            if save {
                let att = self.shared.lock().unwrap().open.as_ref().and_then(|m| m.parsed.attachments.get(i).map(|a| (a.name.clone(), a.data.clone())));
                if let Some((name, data)) = att {
                    let dir = downloads();
                    let _ = std::fs::create_dir_all(&dir);
                    let safe: String = name.chars().map(|c| if c == '/' { '_' } else { c }).collect();
                    let mut p = dir.join(&safe);
                    let mut n = 2;
                    while p.exists() {
                        p = dir.join(format!("{n} {safe}"));
                        n += 1;
                    }
                    self.message_at = Instant::now();
                    self.message = Some(match std::fs::write(&p, data) {
                        Ok(()) => format!("“{}” salvo em {}.", p.file_name().unwrap().to_string_lossy(), dir.display()),
                        Err(e) => format!("Não deu para salvar: {e}"),
                    });
                }
            }
            y += 20.0;
        }
        ui.c.hline(r.x + 10.0, y - 6.0, r.w - 20.0, rgba(210, 210, 214, 255));
        let body = Rect::new(r.x, y, r.w, r.y + r.h - y);
        if !open_ok {
            ui.fonts.draw(ui.c, "Carregando…", body.x + 20.0, body.y + 24.0, 12.0, false, grey);
            return;
        }
        let max = body.w - 36.0;
        let key = (row.uid, max as i32);
        if self.preview_cache.as_ref().map(|c| (c.0, c.1)) != Some(key) {
            let text = self.shared.lock().unwrap().open.as_ref().map(|m| m.parsed.text.clone()).unwrap_or_default();
            self.preview_cache = Some((key.0, key.1, wrap(ui, &text, 12.0, max)));
            self.preview_scroll = 0.0;
        }
        let lines = &self.preview_cache.as_ref().unwrap().2;
        let lh = 17.0;
        let total = lines.len() as f32 * lh + 30.0;
        self.preview_scroll = (self.preview_scroll - ui.scroll_in(body)).clamp(0.0, (total - body.h).max(0.0));
        let first = (self.preview_scroll / lh) as usize;
        let count = (body.h / lh) as usize + 2;
        let off = self.preview_scroll;
        let lines: Vec<String> = lines.iter().skip(first).take(count).cloned().collect();
        ui.clipped(9, body, |c, fonts| {
            c.clear(Color::WHITE);
            for (k, l) in lines.iter().enumerate() {
                let yy = 18.0 + (first + k) as f32 * lh - off;
                let quoted = l.starts_with('>');
                fonts.draw(c, l, 18.0, yy, 12.0, false, if quoted { rgba(40, 90, 170, 255) } else { rgba(25, 25, 30, 255) });
            }
        });
    }

    fn main_window(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        // Ações (menus e teclas).
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        let typing = ui.has_focus();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+N" => acts.push("new".into()),
                "Ctrl+R" => acts.push("reply".into()),
                "Ctrl+Shift+R" => acts.push("reply-all".into()),
                "Ctrl+Shift+F" => acts.push("forward".into()),
                "Ctrl+Shift+N" => acts.push("get".into()),
                "Ctrl+Shift+U" => acts.push("unread".into()),
                "Ctrl+Shift+L" => acts.push("flag".into()),
                "Ctrl+comma" | "Ctrl+," => acts.push("accounts".into()),
                "Ctrl+W" => acts.push("close".into()),
                "Ctrl+P" => acts.push("print".into()),
                _ if !typing && matches!(k.key, Key::Delete | Key::Backspace) => acts.push("delete".into()),
                _ if !typing && k.key == Key::Up => acts.push("up".into()),
                _ if !typing && k.key == Key::Down => acts.push("down".into()),
                _ => {}
            }
        }
        if let Some(a) = self.toolbar(ui) {
            acts.push(a.into());
        }
        let body_h = h - TOOL_H - FOOT_H;
        self.sidebar(ui, Rect::new(0.0, TOOL_H, SIDE_W, body_h));
        // Lista de mensagens (com a busca).
        let q = self.search.trim().to_lowercase();
        let rows: Vec<Row> = {
            let s = self.shared.lock().unwrap();
            s.rows.iter().filter(|r| q.is_empty() || r.subject.to_lowercase().contains(&q) || r.from.to_lowercase().contains(&q) || r.to.to_lowercase().contains(&q)).cloned().collect()
        };
        let lx = SIDE_W;
        let lw = w - lx;
        let list_h = (body_h * 0.42).max(120.0);
        let cols = [Column::new("", 0.065), Column::new("De", 0.27), Column::new("Assunto", 0.465), Column::new("Data", 0.2)];
        let t = now();
        let trows: Vec<TableRow> = rows
            .iter()
            .map(|r| {
                let mark = if !r.seen { "●" } else if r.flagged { "⚑" } else if r.answered { "↩" } else { "" };
                TableRow { cells: vec![mark.into(), mime::display_name(&r.from), r.subject.clone(), store::short_date(r.date, t)], icon: r.attach.then_some(Icon::Clip), ..TableRow::default() }
            })
            .collect();
        let sel = self.selected.and_then(|u| rows.iter().position(|r| r.uid == u));
        let resp = ui.table(Rect::new(lx, TOOL_H, lw, list_h), &cols, &trows, sel, &mut self.table, &TableStyle::default());
        let mut pick = resp.pressed.or(resp.double);
        for a in &acts {
            let cur = sel.unwrap_or(0);
            match a.as_str() {
                "up" if !rows.is_empty() => pick = Some(cur.saturating_sub(1)),
                "down" if !rows.is_empty() => pick = Some(if sel.is_none() { 0 } else { (cur + 1).min(rows.len() - 1) }),
                _ => {}
            }
        }
        if let Some(i) = pick {
            if let Some(r) = rows.get(i) {
                if self.selected != Some(r.uid) {
                    self.selected = Some(r.uid);
                    self.preview_cache = None;
                    let _ = self.tx.send(Cmd::Read(r.uid));
                }
            }
        }
        // Divisória e a mensagem.
        let py = TOOL_H + list_h;
        ui.c.fill_rect(lx, py, lw, 6.0, &vgrad(py, py + 6.0, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(200, 200, 204, 255))]));
        ui.c.hline(lx, py + 6.0, lw, rgba(150, 150, 156, 255));
        self.preview(ui, Rect::new(lx, py + 7.0, lw, body_h - list_h - 7.0));
        // Rodapé: contagem ou o que está acontecendo.
        let fy = h - FOOT_H;
        ui.c.fill_rect(0.0, fy, w, FOOT_H, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(214, 214, 218, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        let (busy, error, info) = {
            let mut s = self.shared.lock().unwrap();
            (s.busy.clone(), s.error.clone(), s.info.take())
        };
        if let Some(i) = info {
            self.message_at = Instant::now();
            self.message = Some(i);
        }
        if self.message.is_some() && self.message_at.elapsed() > Duration::from_secs(8) {
            self.message = None;
        }
        let unread = rows.iter().filter(|r| !r.seen).count();
        let status = if let Some(b) = &busy {
            b.clone()
        } else if let Some(e) = &error {
            e.clone()
        } else if let Some(m) = &self.message {
            m.clone()
        } else if self.accounts.is_empty() {
            "Nenhuma conta. Use Correio ▸ Contas… para adicionar uma.".into()
        } else {
            format!("{} {}{}", rows.len(), if rows.len() == 1 { "mensagem" } else { "mensagens" }, if unread > 0 { format!(", {unread} não lidas") } else { String::new() })
        };
        let col = if busy.is_none() && error.is_some() { rgba(170, 40, 30, 255) } else { rgba(50, 50, 56, 255) };
        let status = ui.fonts.ellipsize(&status, 11.0, false, w - 40.0);
        let tw = ui.text_width(&status, 11.0, false);
        ui.fonts.draw(ui.c, &status, (w - tw) / 2.0, fy + 15.0, 11.0, false, col);
        if busy.is_some() {
            // Rodinha de "trabalhando".
            let ang = (Instant::now().elapsed().as_millis() as f32 + now() as f32 * 1000.0) / 120.0;
            for k in 0..8 {
                let a = (k as f32 + ang) * std::f32::consts::PI / 4.0;
                let alpha = 40 + 26 * k as u8;
                ui.c.fill(circle(SIDE_W / 2.0 - 70.0 + 9.0 + a.cos() * 5.0, fy + 11.0 + a.sin() * 5.0, 1.4), &solid(rgba(60, 60, 66, alpha)));
            }
            ui.wake_after(Duration::from_millis(120));
        }

        for a in acts {
            match a.as_str() {
                "new" if !self.accounts.is_empty() => {
                    let acct = self.current().map(|c| c.0).unwrap_or(0);
                    self.open_compose(ui, Compose::blank(acct));
                }
                "reply" | "reply-all" | "forward" => self.respond(ui, &a),
                "get" => {
                    self.message = None;
                    let _ = self.tx.send(Cmd::Refresh);
                    self.last_check = Instant::now();
                }
                "delete" => {
                    if let Some(uid) = self.selected {
                        // A próxima da lista fica selecionada.
                        let next = rows.iter().position(|r| r.uid == uid).and_then(|i| rows.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| rows.get(j)))).map(|r| r.uid);
                        let _ = self.tx.send(Cmd::Delete(uid));
                        self.selected = next;
                        if let Some(n) = next {
                            let _ = self.tx.send(Cmd::Read(n));
                        }
                    }
                }
                "unread" => {
                    if let Some(r) = self.selected_row() {
                        let _ = self.tx.send(Cmd::Flag(r.uid, "\\Seen", !r.seen));
                    }
                }
                "flag" => {
                    if let Some(r) = self.selected_row() {
                        let _ = self.tx.send(Cmd::Flag(r.uid, "\\Flagged", !r.flagged));
                    }
                }
                "accounts" => self.open_accounts(ui),
                "print" => {
                    if let Some(row) = self.selected_row() {
                        let text = {
                            let s = self.shared.lock().unwrap();
                            s.open.as_ref().filter(|m| m.uid == row.uid).map(|m| m.parsed.text.clone()).unwrap_or_default()
                        };
                        let doc = format!("De: {}\nPara: {}\nData: {}\nAssunto: {}\n\n{text}", row.from, row.to, store::long_date(row.date), row.subject);
                        self.print = Some(glove_kit::print::PrintSheet::new(&row.subject, glove_kit::print::PrintContent::Text(doc)));
                    }
                }
                "close" => ui.close = true,
                _ => {}
            }
        }
        if self.accounts.is_empty() && !self.accounts_open {
            self.open_accounts(ui);
        }
    }

    fn compose_window(&mut self, ui: &mut Ui, id: u32) {
        let (w, h) = (ui.c.w, ui.c.h);
        // Resultado do envio.
        let result = {
            let mut s = self.shared.lock().unwrap();
            let i = s.sent.iter().position(|(win, _)| *win == id);
            i.map(|i| s.sent.remove(i).1)
        };
        let accounts = self.accounts.clone();
        let Some(c) = self.composes.get_mut(&id) else {
            ui.close = true;
            return;
        };
        match result {
            Some(Ok(())) => {
                ui.close = true;
                return;
            }
            Some(Err(e)) => {
                c.sending = false;
                c.error = Some(e);
            }
            None => {}
        }
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        if c.fb.is_none() {
            for k in ui.keys() {
                match k.combo.as_str() {
                    "Ctrl+Shift+D" | "Ctrl+Return" | "Ctrl+Enter" => acts.push("send".into()),
                    "Ctrl+Shift+A" => acts.push("attach".into()),
                    "Ctrl+W" => acts.push("close".into()),
                    _ => {}
                }
            }
        }
        // Barra: Enviar e Anexar.
        ui.c.brushed_metal(0.0, 0.0, w, TOOL_H, rgba(226, 226, 228, 255), rgba(190, 190, 194, 255));
        ui.c.hline(0.0, TOOL_H - 1.0, w, rgba(120, 120, 126, 255));
        let mut x = 12.0;
        for (label, act) in [("Enviar", "send"), ("Anexar", "attach")] {
            let r = Rect::new(x, 4.0, 70.0, TOOL_H - 8.0);
            let on = !c.sending;
            let ink = if on { rgba(40, 40, 46, 255) } else { rgba(150, 150, 156, 255) };
            let hit = ui.hit(r);
            tool_glyph(ui, act, x + 35.0, 20.0, ink);
            let tw = ui.text_width(label, 10.0, false);
            ui.fonts.draw(ui.c, label, x + 35.0 - tw / 2.0, TOOL_H - 9.0, 10.0, false, ink);
            if on && hit.clicked {
                acts.push(act.into());
            }
            x += 76.0;
        }
        // Campos.
        let lx = 80.0;
        let fw = w - lx - 16.0;
        let mut y = TOOL_H + 12.0;
        let first = std::mem::take(&mut c.first);
        let label = |ui: &mut Ui, t: &str, y: f32| {
            let tw = ui.text_width(t, 11.0, false);
            ui.fonts.draw(ui.c, t, lx - 8.0 - tw, y + 15.0, 11.0, false, rgba(90, 90, 96, 255));
        };
        label(ui, "Para:", y);
        let focus_to = first && c.to.is_empty();
        ui.text_field_in(Rect::new(lx, y, fw, 22.0), &mut c.to, focus_to);
        y += 30.0;
        label(ui, "Cc:", y);
        ui.text_field_in(Rect::new(lx, y, fw, 22.0), &mut c.cc, false);
        y += 30.0;
        label(ui, "Assunto:", y);
        ui.text_field_in(Rect::new(lx, y, fw, 22.0), &mut c.subject, false);
        y += 30.0;
        if accounts.len() > 1 {
            label(ui, "De:", y);
            let from = accounts.get(c.acct).map(|a| format!("{} <{}>", a.name, a.email)).unwrap_or_default();
            let mut clicked = false;
            ui.area(Rect::new(lx, y, fw.min(360.0), 22.0), |ui| clicked = ui.small_button(&from));
            if clicked {
                c.acct = (c.acct + 1) % accounts.len();
            }
            y += 30.0;
        }
        // Anexos.
        let mut remove = None;
        for (i, (name, data)) in c.attachments.iter().enumerate() {
            let t = format!("{name} ({})", size_text(data.len()));
            icons::draw_bare(ui.c, ui.fonts, Icon::Clip, lx - 2.0, y, 16.0, None);
            ui.fonts.draw(ui.c, &t, lx + 18.0, y + 13.0, 11.0, false, rgba(40, 40, 46, 255));
            let tw = ui.text_width(&t, 11.0, false) + 18.0;
            let xr = Rect::new(lx + tw + 8.0, y + 2.0, 14.0, 14.0);
            ui.c.fill(circle(xr.x + 7.0, xr.y + 7.0, 6.0), &solid(rgba(150, 150, 156, 255)));
            ui.fonts.draw(ui.c, "×", xr.x + 3.5, xr.y + 11.0, 10.0, true, Color::WHITE);
            if ui.hit(xr).clicked {
                remove = Some(i);
            }
            y += 18.0;
        }
        if let Some(i) = remove {
            c.attachments.remove(i);
        }
        // Texto.
        let body = Rect::new(0.0, y + 4.0, w, h - y - 4.0 - if c.error.is_some() || c.sending { 20.0 } else { 0.0 });
        ui.c.hline(0.0, body.y - 1.0, w, rgba(170, 170, 176, 255));
        ui.text_area(&mut c.body, body);
        if first && !c.to.is_empty() {
            c.body.set_cursor(0, false);
        }
        if c.sending || c.error.is_some() {
            let (t, col) = if c.sending { ("Enviando…".to_string(), rgba(50, 50, 56, 255)) } else { (c.error.clone().unwrap_or_default(), rgba(170, 40, 30, 255)) };
            let t = ui.fonts.ellipsize(&t, 11.0, false, w - 20.0);
            ui.fonts.draw(ui.c, &t, 10.0, h - 6.0, 11.0, false, col);
            if c.sending {
                ui.wake_after(Duration::from_millis(200));
            }
        }

        for a in acts {
            match a.as_str() {
                "send" if !c.sending => {
                    let to = mime::split_addresses(&c.to);
                    if to.is_empty() || !to.iter().all(|a| mime::address(a).contains('@')) {
                        c.error = Some("Confira os destinatários em “Para:”.".into());
                        continue;
                    }
                    let Some(acc) = accounts.get(c.acct) else { continue };
                    let from = if acc.name.is_empty() { acc.email.clone() } else { format!("{} <{}>", acc.name, acc.email) };
                    let out = mime::Outgoing {
                        from,
                        to,
                        cc: mime::split_addresses(&c.cc),
                        bcc: Vec::new(),
                        subject: c.subject.clone(),
                        body: c.body.text().to_string(),
                        in_reply_to: c.in_reply_to.clone(),
                        references: c.references.clone(),
                        attachments: c.attachments.clone(),
                    };
                    c.sending = true;
                    c.error = None;
                    let _ = self.tx.send(Cmd::Send(id, c.acct, Box::new(out), c.reply.clone()));
                }
                "attach" if !c.sending => c.fb = Some(FileBrowser::open(None, &[])),
                "close" => {
                    if c.has_content() && !c.sending {
                        c.confirm_close = true;
                    } else {
                        ui.close = true;
                    }
                }
                _ => {}
            }
        }
        if c.confirm_close {
            match ui.alert(AlertKind::Caution, "Descartar esta mensagem?", "O que foi escrito será perdido.", &["Cancelar", "Descartar"]) {
                Some(1) => {
                    c.confirm_close = false;
                    ui.close = true;
                }
                Some(_) => c.confirm_close = false,
                None => {}
            }
        }
        if let Some(fb) = &mut c.fb {
            if let Some(ch) = ui.file_sheet(fb) {
                c.fb = None;
                if let Choice::Picked(p) = ch {
                    match std::fs::read(&p) {
                        Ok(d) if d.len() > 25 * 1024 * 1024 => c.error = Some("Arquivo grande demais para anexar (mais de 25 MB).".into()),
                        Ok(d) => c.attachments.push((p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), d)),
                        Err(e) => c.error = Some(format!("Não deu para ler o arquivo: {e}")),
                    }
                }
                ui.redraw = true;
            }
        }
    }

    fn accounts_window(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        // Resultado da verificação.
        let tested = {
            let mut s = self.shared.lock().unwrap();
            let id = self.form.as_ref().map(|f| f.test_id).unwrap_or(0);
            let i = s.tested.iter().position(|(t, _)| *t == id);
            i.map(|i| s.tested.remove(i).1)
        };
        if ui.keys().iter().any(|k| k.combo == "Ctrl+W") {
            ui.close = true;
        }
        // Lista de contas.
        let lw = 160.0;
        let mut names: Vec<String> = self.accounts.iter().map(|a| if a.name.is_empty() { a.email.clone() } else { a.name.clone() }).collect();
        names.push("Nova Conta…".into());
        let mut sel = self.form.as_ref().map(|f| f.index.unwrap_or(self.accounts.len()));
        let before = sel;
        ui.area(Rect::new(12.0, 12.0, lw, h - 64.0), |ui| {
            ui.list(&mut sel, &names, lw, h - 64.0);
        });
        if sel != before {
            if let Some(i) = sel {
                self.form = Some(match self.accounts.get(i) {
                    Some(a) => AccountForm::new(Some(i), a.clone()),
                    None => AccountForm::new(None, Account { imap_port: 993, smtp_port: 587, ..Default::default() }),
                });
            }
        }
        let Some(f) = self.form.as_mut() else { return };
        if let Some(r) = tested {
            f.testing = false;
            match r {
                Ok(()) => {
                    let a = f.finished();
                    match f.index {
                        Some(i) => self.accounts[i] = a,
                        None => self.accounts.push(a),
                    }
                    let idx = f.index.unwrap_or(self.accounts.len() - 1);
                    f.index = Some(idx);
                    f.error = None;
                    if let Err(e) = store::save_accounts(&self.accounts) {
                        f.error = Some(format!("Não deu para salvar: {e}"));
                    }
                    let _ = self.tx.send(Cmd::SetAccounts(self.accounts.clone()));
                    if self.shared.lock().unwrap().current.as_ref().is_none_or(|c| c.0 == idx) {
                        let _ = self.tx.send(Cmd::Open(idx, "INBOX".into()));
                    }
                    self.message_at = Instant::now();
                    self.message = Some("Conta salva.".into());
                }
                Err(e) => f.error = Some(e),
            }
        }
        // Formulário.
        let fx = lw + 150.0;
        let fw = w - fx - 16.0;
        let mut y = 14.0;
        let label = |ui: &mut Ui, t: &str, y: f32| {
            let tw = ui.text_width(t, 11.0, false);
            ui.fonts.draw(ui.c, t, fx - 8.0 - tw, y + 15.0, 11.0, false, rgba(60, 60, 66, 255));
        };
        let old_email = f.a.email.clone();
        label(ui, "Nome completo:", y);
        ui.text_field_in(Rect::new(fx, y, fw, 22.0), &mut f.a.name, false);
        y += 30.0;
        label(ui, "E-mail:", y);
        let focus_email = std::mem::take(&mut f.focus);
        ui.text_field_in(Rect::new(fx, y, fw, 22.0), &mut f.a.email, focus_email);
        y += 30.0;
        if f.a.email != old_email && !f.touched {
            let g = store::guess(&f.a.email);
            f.a.imap_host = g.imap_host;
            f.a.smtp_host = g.smtp_host;
            f.imap_port = g.imap_port.to_string();
            f.smtp_port = g.smtp_port.to_string();
            f.a.user = g.user;
        }
        label(ui, "Senha:", y);
        ui.password_field_in(Rect::new(fx, y, fw, 22.0), &mut f.a.password, false);
        y += 38.0;
        let snapshot = (f.a.imap_host.clone(), f.imap_port.clone(), f.a.smtp_host.clone(), f.smtp_port.clone(), f.a.user.clone());
        label(ui, "Entrada (IMAP):", y);
        ui.text_field_in(Rect::new(fx, y, fw - 70.0, 22.0), &mut f.a.imap_host, false);
        ui.text_field_in(Rect::new(fx + fw - 60.0, y, 60.0, 22.0), &mut f.imap_port, false);
        y += 30.0;
        label(ui, "Saída (SMTP):", y);
        ui.text_field_in(Rect::new(fx, y, fw - 70.0, 22.0), &mut f.a.smtp_host, false);
        ui.text_field_in(Rect::new(fx + fw - 60.0, y, 60.0, 22.0), &mut f.smtp_port, false);
        y += 30.0;
        label(ui, "Usuário:", y);
        ui.text_field_in(Rect::new(fx, y, fw, 22.0), &mut f.a.user, false);
        y += 30.0;
        if snapshot != (f.a.imap_host.clone(), f.imap_port.clone(), f.a.smtp_host.clone(), f.smtp_port.clone(), f.a.user.clone()) {
            f.touched = true;
        }
        for (k, line) in ["A senha fica guardada só para você,", "em ~/.config/glove/correio.toml."].iter().enumerate() {
            ui.fonts.draw(ui.c, line, fx, y + 10.0 + k as f32 * 14.0, 10.0, false, rgba(120, 120, 126, 255));
        }
        if let Some(e) = &f.error {
            let e = ui.fonts.ellipsize(e, 11.0, false, w - lw - 40.0);
            ui.fonts.draw(ui.c, &e, lw + 28.0, h - 50.0, 11.0, false, rgba(170, 40, 30, 255));
        }
        // Botões.
        let ready = !f.a.email.contains(' ') && f.a.email.contains('@') && !f.a.imap_host.trim().is_empty() && !f.a.smtp_host.trim().is_empty() && !f.testing;
        let mut save = false;
        let mut remove = false;
        let mut save_anyway = false;
        let has_error = f.error.is_some();
        let editing = f.index.is_some();
        ui.area(Rect::new(w - 120.0, h - 36.0, 108.0, 26.0), |ui| {
            ui.set_enabled(ready);
            save = ui.button(if f.testing { "Verificando…" } else { "Salvar" });
            ui.set_enabled(true);
        });
        if has_error && !f.testing {
            ui.area(Rect::new(w - 290.0, h - 36.0, 160.0, 26.0), |ui| save_anyway = ui.small_button("Salvar Mesmo Assim"));
        }
        if editing {
            ui.area(Rect::new(12.0, h - 36.0, 90.0, 26.0), |ui| remove = ui.small_button("Remover"));
        }
        if save && ready {
            f.testing = true;
            f.error = None;
            f.test_id = f.test_id.wrapping_add(1).max(1);
            let _ = self.tx.send(Cmd::Test(f.test_id, f.finished()));
        }
        if f.testing {
            ui.wake_after(Duration::from_millis(200));
        }
        if save_anyway {
            self.shared.lock().unwrap().tested.push((f.test_id, Ok(())));
            ui.redraw = true;
        }
        if remove {
            self.confirm_remove = true;
        }
        if self.confirm_remove {
            match ui.alert(AlertKind::Caution, "Remover esta conta?", "As mensagens continuam no servidor; só a cópia local e os ajustes saem deste computador.", &["Cancelar", "Remover"]) {
                Some(1) => {
                    self.confirm_remove = false;
                    if let Some(i) = self.form.as_ref().and_then(|f| f.index) {
                        let a = self.accounts.remove(i);
                        let _ = std::fs::remove_dir_all(store::cache_root().join(a.email.replace('/', "_")));
                        let _ = store::save_accounts(&self.accounts);
                        let _ = self.tx.send(Cmd::SetAccounts(self.accounts.clone()));
                        self.shared.lock().unwrap().current = None;
                        self.shared.lock().unwrap().rows.clear();
                        if !self.accounts.is_empty() {
                            let _ = self.tx.send(Cmd::Open(0, "INBOX".into()));
                        }
                        self.form = Some(match self.accounts.first() {
                            Some(a) => AccountForm::new(Some(0), a.clone()),
                            None => AccountForm::new(None, Account { imap_port: 993, smtp_port: 587, ..Default::default() }),
                        });
                    }
                }
                Some(_) => self.confirm_remove = false,
                None => {}
            }
        }
    }
}

/// Desenhos dos botões da barra.
fn tool_glyph(ui: &mut Ui, id: &str, cx: f32, cy: f32, ink: Color) {
    let c = &mut *ui.c;
    let arrow = |c: &mut Canvas, x: f32, y: f32, dir: f32, double: bool| {
        // Seta curva (responder) ou reta (encaminhar).
        let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
        pb.move_to(x + 8.0 * dir, y + 8.0);
        pb.quad_to(x + 8.0 * dir, y - 2.0, x - 2.0 * dir, y - 2.0);
        c.stroke(pb.finish(), &solid(ink), 2.6);
        let mut tip = glove_kit::paint::tiny_skia::PathBuilder::new();
        tip.move_to(x - 9.0 * dir, y - 2.0);
        tip.line_to(x - 1.0 * dir, y - 8.0);
        tip.line_to(x - 1.0 * dir, y + 4.0);
        tip.close();
        c.fill(tip.finish(), &solid(ink));
        if double {
            let mut t2 = glove_kit::paint::tiny_skia::PathBuilder::new();
            t2.move_to(x - 14.0 * dir, y - 2.0);
            t2.line_to(x - 6.0 * dir, y - 8.0);
            t2.line_to(x - 6.0 * dir, y + 4.0);
            t2.close();
            c.fill(t2.finish(), &solid(ink));
        }
    };
    let envelope = |c: &mut Canvas, x: f32, y: f32| {
        c.fill(rrect(x - 13.0, y - 8.0, 26.0, 17.0, 2.0), &vgrad(y - 8.0, y + 9.0, &[(0.0, rgba(255, 255, 255, 255)), (1.0, rgba(220, 220, 226, 255))]));
        c.stroke(rrect(x - 13.0, y - 8.0, 26.0, 17.0, 2.0), &solid(ink), 1.2);
        let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
        pb.move_to(x - 13.0, y - 8.0);
        pb.line_to(x, y + 2.0);
        pb.line_to(x + 13.0, y - 8.0);
        c.stroke(pb.finish(), &solid(ink), 1.2);
    };
    match id {
        "delete" => icons::draw_bare(c, ui.fonts, Icon::Trash, cx - 13.0, cy - 14.0, 26.0, None),
        "reply" => arrow(c, cx + 2.0, cy + 2.0, 1.0, false),
        "reply-all" => arrow(c, cx + 5.0, cy + 2.0, 1.0, true),
        "forward" => arrow(c, cx - 2.0, cy + 2.0, -1.0, false),
        "new" => icons::draw_bare(c, ui.fonts, Icon::TextEditor, cx - 13.0, cy - 14.0, 26.0, None),
        "get" | "send" => {
            envelope(c, cx, cy);
            if id == "send" {
                // Avião de papel por cima.
                let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
                pb.move_to(cx + 3.0, cy - 12.0);
                pb.line_to(cx + 16.0, cy - 15.0);
                pb.line_to(cx + 10.0, cy - 3.0);
                pb.close();
                c.fill(pb.finish(), &solid(rgba(60, 120, 210, 255)));
            } else {
                c.fill(circle(cx + 10.0, cy + 7.0, 6.0), &solid(rgba(60, 120, 210, 255)));
                c.fill_rect(cx + 9.0, cy + 3.0, 2.0, 5.0, &solid(Color::WHITE));
                let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
                pb.move_to(cx + 7.0, cy + 7.0);
                pb.line_to(cx + 13.0, cy + 7.0);
                pb.line_to(cx + 10.0, cy + 10.5);
                pb.close();
                c.fill(pb.finish(), &solid(Color::WHITE));
            }
        }
        "attach" => {
            // Clipe.
            let mut pb = glove_kit::paint::tiny_skia::PathBuilder::new();
            pb.move_to(cx - 3.0, cy + 10.0);
            pb.line_to(cx - 3.0, cy - 8.0);
            pb.quad_to(cx - 3.0, cy - 13.0, cx + 1.0, cy - 13.0);
            pb.quad_to(cx + 5.0, cy - 13.0, cx + 5.0, cy - 8.0);
            pb.line_to(cx + 5.0, cy + 8.0);
            pb.quad_to(cx + 5.0, cy + 12.0, cx + 1.0, cy + 12.0);
            pb.quad_to(cx - 1.0, cy + 12.0, cx + 1.0, cy + 8.0);
            pb.line_to(cx + 1.0, cy - 6.0);
            c.stroke(pb.finish(), &solid(ink), 1.8);
        }
        _ => {}
    }
}

impl App for Correio {
    fn ui(&mut self, ui: &mut Ui) {
        let id = ui.window_id();
        // Algo mudou na rede: redesenha de vez em quando enquanto há trabalho.
        if self.shared.lock().unwrap().busy.is_some() {
            ui.wake_after(Duration::from_millis(250));
        }
        match id {
            0 => {
                self.main_window(ui);
                // Imprimir a mensagem aberta (folha por cima da janela).
                if let Some(p) = &mut self.print {
                    if ui.print_sheet(p) {
                        self.print = None;
                        ui.redraw = true;
                    }
                }
            }
            ACCOUNTS_WIN => self.accounts_window(ui),
            _ => self.compose_window(ui, id),
        }
    }

    fn tick(&self) -> Option<Duration> {
        Some(Duration::from_secs(2))
    }

    fn on_tick(&mut self) {
        // De tempos em tempos, procura mensagens novas.
        if !self.accounts.is_empty() && self.last_check.elapsed() >= AUTO_CHECK && self.shared.lock().unwrap().busy.is_none() {
            self.last_check = Instant::now();
            let _ = self.tx.send(Cmd::Refresh);
        }
    }

    fn close_window(&mut self, id: u32) -> bool {
        match self.composes.get_mut(&id) {
            Some(c) if c.has_content() && !c.sending => {
                c.confirm_close = true;
                false
            }
            _ => true,
        }
    }

    fn window_closed(&mut self, id: u32) {
        if id == ACCOUNTS_WIN {
            self.accounts_open = false;
        }
        self.composes.remove(&id);
    }

    fn menus(&self) -> Vec<Menu> {
        let has = self.selected.is_some();
        vec![
            Menu::new(
                "Arquivo",
                vec![
                    MenuItem::new("new", "Nova Mensagem", "Ctrl+N"),
                    MenuItem::Separator,
                    MenuItem::new("attach", "Anexar Arquivo…", "Ctrl+Shift+A"),
                    MenuItem::new("send", "Enviar", "Ctrl+Shift+D"),
                    MenuItem::Separator,
                    MenuItem::new("accounts", "Contas…", "Ctrl+comma"),
                    MenuItem::Separator,
                    MenuItem::new("print", "Imprimir…", "Ctrl+P"),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar Janela", "Ctrl+W"),
                ],
            ),
            Menu::new(
                "Mensagem",
                vec![
                    MenuItem::new("get", "Receber Mensagens", "Ctrl+Shift+N"),
                    MenuItem::Separator,
                    MenuItem::new("reply", "Responder", "Ctrl+R").enabled(has),
                    MenuItem::new("reply-all", "Responder a Todos", "Ctrl+Shift+R").enabled(has),
                    MenuItem::new("forward", "Encaminhar", "Ctrl+Shift+F").enabled(has),
                    MenuItem::Separator,
                    MenuItem::new("unread", "Marcar como Lida/Não Lida", "Ctrl+Shift+U").enabled(has),
                    MenuItem::new("flag", "Marcar com Bandeira", "Ctrl+Shift+L").enabled(has),
                    MenuItem::Separator,
                    MenuItem::new("delete", "Apagar", "").enabled(has),
                ],
            ),
        ]
    }
}

pub fn main() {
    let accounts = store::load_accounts();
    let shared: Arc<Mutex<Shared>> = Arc::default();
    let tx = worker::start(shared.clone(), accounts.clone());
    if !accounts.is_empty() {
        let _ = tx.send(Cmd::Open(0, "INBOX".into()));
    }
    let app = Correio {
        print: None,
        shared,
        tx,
        accounts,
        selected: None,
        table: TableState::default(),
        search: String::new(),
        composes: HashMap::new(),
        next_win: 1,
        form: None,
        accounts_open: false,
        preview_scroll: 0.0,
        preview_cache: None,
        last_check: Instant::now(),
        message: None,
        message_at: Instant::now(),
        confirm_remove: false,
    };
    let opts = WindowOptions { title: "Correio".into(), app_id: "glove.Correio".into(), size: (900, 600), fixed: false, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-correio: {e}");
    }
}
