//! Trabalho de rede numa linha de execução à parte: a janela manda pedidos
//! e lê o resultado em `Shared` (redesenhando de tempos em tempos).

use glove_config::tr;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::imap::{Folder, Header, Imap};
use crate::mime;
use crate::net::Security;
use crate::store::{self, Account, Row};

pub enum Cmd {
    SetAccounts(Vec<Account>),
    /// Abre a pasta (mostra a cópia local e depois atualiza pelo servidor).
    Open(usize, String),
    /// Busca de novo a lista de pastas e a pasta aberta.
    Refresh,
    Read(u32),
    Flag(u32, &'static str, bool),
    Delete(u32),
    /// (janela de escrever, conta, mensagem, resposta a (pasta, uid)).
    Send(u32, usize, Box<mime::Outgoing>, Option<(String, u32)>),
    /// Testa a conta (Entrar no IMAP) antes de salvar: (pedido, conta).
    Test(u32, Account),
}

pub struct Message {
    pub uid: u32,
    pub parsed: mime::Parsed,
}

#[derive(Default)]
pub struct Shared {
    pub folders: Vec<Vec<Folder>>,
    pub current: Option<(usize, String)>,
    pub rows: Vec<Row>,
    pub open: Option<Message>,
    pub busy: Option<String>,
    pub error: Option<String>,
    pub info: Option<String>,
    /// Janelas de escrever cuja mensagem saiu (ou o erro).
    pub sent: Vec<(u32, Result<(), String>)>,
    pub tested: Vec<(u32, Result<(), String>)>,
    /// Muda a cada atualização (a janela redesenha).
    pub generation: u64,
}

pub fn sec_imap(a: &Account) -> Security {
    a.imap_sec
}

fn is_gmail(a: &Account) -> bool {
    a.imap_host.ends_with("gmail.com")
}

pub fn row_from(h: &Header) -> Row {
    let (hs, _) = mime::headers(&h.head);
    let get = |k: &str| mime::header(&hs, k).map(mime::decode_words).unwrap_or_default();
    let ct = get("content-type").to_lowercase();
    Row {
        uid: h.uid,
        seen: h.seen,
        flagged: h.flagged,
        answered: h.answered,
        size: h.size,
        from: get("from"),
        to: get("to"),
        cc: get("cc"),
        subject: get("subject"),
        date: store::parse_date(&get("date")),
        message_id: get("message-id"),
        references: get("references"),
        attach: ct.starts_with("multipart/mixed"),
    }
}

struct Worker {
    accounts: Vec<Account>,
    conns: HashMap<usize, (Imap, Option<String>)>,
    shared: Arc<Mutex<Shared>>,
    current: Option<(usize, String)>,
}

impl Worker {
    fn set<F: FnOnce(&mut Shared)>(&self, f: F) {
        let mut s = self.shared.lock().unwrap();
        f(&mut s);
        s.generation += 1;
    }

    fn conn(&mut self, acct: usize) -> Result<&mut Imap, String> {
        if !self.conns.contains_key(&acct) {
            let a = self.accounts.get(acct).ok_or(tr!("conta não encontrada"))?.clone();
            let c = Imap::connect(&a.imap_host, a.imap_port, sec_imap(&a), &a.user, &a.password)?;
            self.conns.insert(acct, (c, None));
        }
        Ok(&mut self.conns.get_mut(&acct).unwrap().0)
    }

    /// Conexão com a pasta já aberta (SELECT só quando muda).
    fn selected(&mut self, acct: usize, folder: &str) -> Result<(u32, u32), String> {
        self.conn(acct)?;
        let (c, sel) = self.conns.get_mut(&acct).unwrap();
        let r = c.select(folder)?;
        *sel = Some(folder.to_string());
        Ok(r)
    }

    /// Faz `f` e, se a conexão caiu, entra de novo e tenta outra vez.
    fn retry<T>(&mut self, acct: usize, mut f: impl FnMut(&mut Self) -> Result<T, String>) -> Result<T, String> {
        match f(self) {
            Ok(v) => Ok(v),
            Err(e) => {
                let lost = e.contains("fechou") || e.contains("Broken pipe") || e.contains("reset") || e.contains("timed out") || e.contains("os error");
                if !lost {
                    return Err(e);
                }
                self.conns.remove(&acct);
                f(self)
            }
        }
    }

    fn refresh_folder(&mut self, acct: usize, folder: &str) -> Result<(), String> {
        let email = self.accounts[acct].email.clone();
        let (exists, validity) = self.retry(acct, |w| w.selected(acct, folder))?;
        let (old_validity, _) = store::load_rows(&email, folder);
        if old_validity != 0 && old_validity != validity {
            store::clear_folder(&email, folder);
        }
        let heads = self.retry(acct, |w| {
            w.selected(acct, folder)?;
            w.conns.get_mut(&acct).unwrap().0.headers(exists, 400)
        })?;
        let mut rows: Vec<Row> = heads.iter().map(row_from).collect();
        rows.sort_by(|a, b| b.date.cmp(&a.date).then(b.uid.cmp(&a.uid)));
        store::save_rows(&email, folder, validity, &rows);
        if self.current.as_ref() == Some(&(acct, folder.to_string())) {
            self.set(|s| s.rows = rows);
        }
        Ok(())
    }

    fn refresh(&mut self) {
        let Some((acct, folder)) = self.current.clone() else { return };
        if acct >= self.accounts.len() {
            return;
        }
        self.set(|s| {
            s.busy = Some(tr!("Recebendo mensagens…").into());
            s.error = None;
        });
        let r = self.retry(acct, |w| w.conn(acct)?.folders()).and_then(|f| {
            let mut f = f;
            f.sort_by_key(|x| (role_order(&x.role), x.name.to_lowercase()));
            self.set(|s| {
                if s.folders.len() <= acct {
                    s.folders.resize(acct + 1, Vec::new());
                }
                s.folders[acct] = f;
            });
            self.refresh_folder(acct, &folder)
        });
        self.set(|s| {
            s.busy = None;
            if let Err(e) = r {
                s.error = Some(e);
            }
        });
    }

    fn run(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::SetAccounts(a) => {
                self.conns.clear();
                self.accounts = a;
                let n = self.accounts.len();
                self.set(|s| s.folders.resize(n, Vec::new()));
            }
            Cmd::Open(acct, folder) => {
                let Some(email) = self.accounts.get(acct).map(|a| a.email.clone()) else { return };
                let (_, rows) = store::load_rows(&email, &folder);
                self.current = Some((acct, folder.clone()));
                self.set(|s| {
                    s.current = Some((acct, folder.clone()));
                    s.rows = rows;
                    s.open = None;
                });
                self.refresh();
            }
            Cmd::Refresh => self.refresh(),
            Cmd::Read(uid) => {
                let Some((acct, folder)) = self.current.clone() else { return };
                let email = self.accounts[acct].email.clone();
                let path = store::body_path(&email, &folder, uid);
                let raw = match std::fs::read(&path) {
                    Ok(b) => Ok(b),
                    Err(_) => {
                        self.set(|s| s.busy = Some(tr!("Abrindo a mensagem…").into()));
                        let r = self.retry(acct, |w| {
                            w.selected(acct, &folder)?;
                            w.conns.get_mut(&acct).unwrap().0.body(uid)
                        });
                        if let Ok(b) = &r {
                            let _ = std::fs::create_dir_all(path.parent().unwrap());
                            let _ = std::fs::write(&path, b);
                        }
                        r
                    }
                };
                match raw {
                    Ok(b) => {
                        let parsed = mime::parse(&b);
                        self.set(|s| {
                            s.busy = None;
                            s.open = Some(Message { uid, parsed });
                        });
                        let unseen = self.shared.lock().unwrap().rows.iter().any(|r| r.uid == uid && !r.seen);
                        if unseen {
                            self.flag(uid, "\\Seen", true);
                        }
                    }
                    Err(e) => self.set(|s| {
                        s.busy = None;
                        s.error = Some(e);
                    }),
                }
            }
            Cmd::Flag(uid, flag, on) => self.flag(uid, flag, on),
            Cmd::Delete(uid) => {
                let Some((acct, folder)) = self.current.clone() else { return };
                let trash = self.shared.lock().unwrap().folders.get(acct).and_then(|f| f.iter().find(|x| x.role == "\\trash").map(|x| x.raw.clone()));
                let r = self.retry(acct, |w| {
                    w.selected(acct, &folder)?;
                    let c = &mut w.conns.get_mut(&acct).unwrap().0;
                    match &trash {
                        Some(t) if *t != folder => c.move_to(uid, t),
                        _ => c.expunge(uid),
                    }
                });
                let email = self.accounts[acct].email.clone();
                match r {
                    Ok(()) => {
                        let _ = std::fs::remove_file(store::body_path(&email, &folder, uid));
                        self.set(|s| {
                            s.rows.retain(|r| r.uid != uid);
                            if s.open.as_ref().is_some_and(|m| m.uid == uid) {
                                s.open = None;
                            }
                            store::save_rows(&email, &folder, store::load_rows(&email, &folder).0, &s.rows);
                        });
                    }
                    Err(e) => self.set(|s| s.error = Some(tr!("Não deu para apagar: {}", e))),
                }
            }
            Cmd::Send(win, acct, out, reply) => {
                let Some(a) = self.accounts.get(acct).cloned() else { return };
                self.set(|s| s.busy = Some(tr!("Enviando…").into()));
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                let host = a.email.rsplit('@').next().unwrap_or("glove.local").to_string();
                let msg = mime::build(&out, now, &host);
                let rcpt: Vec<String> = out.to.iter().chain(&out.cc).chain(&out.bcc).map(|x| mime::address(x)).collect();
                let r = crate::smtp::send(&a.smtp_host, a.smtp_port, a.smtp_sec, &a.user, &a.password, &mime::address(&a.email), &rcpt, &msg);
                if r.is_ok() {
                    // Cópia em Enviados (o Gmail já guarda sozinho).
                    let sent = self.shared.lock().unwrap().folders.get(acct).and_then(|f| f.iter().find(|x| x.role == "\\sent").map(|x| x.raw.clone()));
                    if let (Some(folder), false) = (sent, is_gmail(&a)) {
                        let _ = self.retry(acct, |w| w.conn(acct)?.append(&folder, msg.as_bytes(), "\\Seen"));
                    }
                    if let Some((folder, uid)) = reply {
                        let _ = self.retry(acct, |w| {
                            w.selected(acct, &folder)?;
                            w.conns.get_mut(&acct).unwrap().0.set_flag(uid, "\\Answered", true)
                        });
                        self.set(|s| {
                            if let Some(r) = s.rows.iter_mut().find(|r| r.uid == uid) {
                                r.answered = true;
                            }
                        });
                    }
                }
                self.set(|s| {
                    s.busy = None;
                    if r.is_ok() {
                        s.info = Some(tr!("Mensagem enviada.").into());
                    }
                    s.sent.push((win, r));
                });
            }
            Cmd::Test(id, a) => {
                let r = Imap::connect(&a.imap_host, a.imap_port, a.imap_sec, &a.user, &a.password).map(|c| c.logout());
                self.set(|s| s.tested.push((id, r)));
            }
        }
    }

    fn flag(&mut self, uid: u32, flag: &'static str, on: bool) {
        let Some((acct, folder)) = self.current.clone() else { return };
        let email = self.accounts[acct].email.clone();
        // Muda na tela já; o servidor vem depois.
        self.set(|s| {
            if let Some(r) = s.rows.iter_mut().find(|r| r.uid == uid) {
                match flag {
                    "\\Seen" => r.seen = on,
                    "\\Flagged" => r.flagged = on,
                    _ => {}
                }
            }
            store::save_rows(&email, &folder, store::load_rows(&email, &folder).0, &s.rows);
        });
        let r = self.retry(acct, |w| {
            w.selected(acct, &folder)?;
            w.conns.get_mut(&acct).unwrap().0.set_flag(uid, flag, on)
        });
        if let Err(e) = r {
            self.set(|s| s.error = Some(e));
        }
    }
}

/// Ordem das pastas: Entrada, Rascunhos, Enviados, Lixo Eletrônico, Lixo, as outras.
pub fn role_order(role: &str) -> u8 {
    match role {
        "inbox" => 0,
        "\\drafts" => 1,
        "\\sent" => 2,
        "\\junk" => 3,
        "\\trash" => 4,
        "\\archive" | "\\all" | "\\flagged" => 6,
        _ => 5,
    }
}

/// Nome em português das pastas especiais.
pub fn folder_label(f: &Folder) -> String {
    match f.role.as_str() {
        "inbox" => tr!("Entrada").into(),
        "\\sent" => tr!("Enviados").into(),
        "\\drafts" => tr!("Rascunhos").into(),
        "\\trash" => tr!("Lixo").into(),
        "\\junk" => tr!("Lixo Eletrônico").into(),
        _ => f.name.clone(),
    }
}

pub fn start(shared: Arc<Mutex<Shared>>, accounts: Vec<Account>) -> Sender<Cmd> {
    let (tx, rx): (Sender<Cmd>, Receiver<Cmd>) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut w = Worker { accounts: Vec::new(), conns: HashMap::new(), shared, current: None };
        w.run(Cmd::SetAccounts(accounts));
        for cmd in rx {
            w.run(cmd);
        }
        for (_, (c, _)) in w.conns.drain() {
            c.logout();
        }
    });
    tx
}
