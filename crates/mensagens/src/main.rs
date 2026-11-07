//! Mensagens: bate-papo no IRC (Libera.Chat por padrão, com TLS), em balões como
//! os da época: os seus à direita, em azul; os dos outros à esquerda.
//!
//! Na lateral, o servidor, os canais e as conversas particulares (com a contagem
//! do que não foi lido). Comandos: /join #canal, /part, /msg apelido texto,
//! /me ação, /nick novo. Mensagem particular ou com o seu apelido, com a janela
//! no fundo, vira aviso na central.

mod irc;

use glove_config::tr;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, SystemTime};

use glove_ui::{
    paint::{draw::*, icons::Icon},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};
use irc::{Conta, Ev};

const SIDE_W: f32 = 200.0;
const INPUT_H: f32 = 40.0;
const PX: f32 = 12.5;

#[derive(Clone, PartialEq)]
enum Tipo {
    Meu,
    Outro,
    Sistema,
}

#[derive(Clone)]
struct Linha {
    de: String,
    texto: String,
    tipo: Tipo,
    hora: String,
}

struct Conversa {
    nome: String,
    linhas: Vec<Linha>,
    nao_lidas: u32,
    membros: Vec<String>,
    /// Rolagem a partir do fim (0 = nas mais novas).
    rolagem: f32,
}

impl Conversa {
    fn new(nome: &str) -> Self {
        Self { nome: nome.to_string(), linhas: Vec::new(), nao_lidas: 0, membros: Vec::new(), rolagem: 0.0 }
    }
}

fn agora() -> String {
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

fn conta_path() -> std::path::PathBuf {
    glove_config::dir().join("mensagens.toml")
}

fn notify(summary: &str, body: &str) {
    use std::collections::HashMap;
    let Ok(c) = zbus::blocking::Connection::session() else { return };
    let hints: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
    let _ = c.call_method(Some("org.freedesktop.Notifications"), "/org/freedesktop/Notifications", Some("org.freedesktop.Notifications"), "Notify", &(tr!("Mensagens"), 0u32, "glove-mensagens", summary, body, vec!["default", "Abrir"], hints, -1i32));
}

/// Folha da conta: os campos em texto.
struct FolhaConta {
    servidor: String,
    porta: String,
    tls: bool,
    apelido: String,
    nome: String,
    canais: String,
}

struct Mensagens {
    conta: Conta,
    conv: Vec<Conversa>,
    atual: usize,
    texto: String,
    link: Option<(Sender<String>, Receiver<Ev>)>,
    apelido: String,
    estado: String,
    folha: Option<FolhaConta>,
    /// "+": entrar num canal ou conversar com alguém.
    nova: Option<String>,
    ativa: bool,
}

impl Mensagens {
    fn new() -> Self {
        let conta: Option<Conta> = std::fs::read_to_string(conta_path()).ok().and_then(|t| toml::from_str(&t).ok());
        let mut m = Self { conta: conta.clone().unwrap_or_default(), conv: vec![Conversa::new("")], atual: 0, texto: String::new(), link: None, apelido: String::new(), estado: tr!("Desconectado").into(), folha: None, nova: None, ativa: true };
        m.conv[0].nome = m.conta.servidor.clone();
        match conta {
            Some(_) => m.conectar(),
            None => m.abrir_folha(),
        }
        m
    }

    fn abrir_folha(&mut self) {
        let c = &self.conta;
        self.folha = Some(FolhaConta { servidor: c.servidor.clone(), porta: c.porta.to_string(), tls: c.tls, apelido: c.apelido.clone(), nome: c.nome.clone(), canais: c.canais.join(", ") });
    }

    fn conectar(&mut self) {
        if let Some((tx, _)) = self.link.take() {
            let _ = tx.send("\u{0}SAIR".into());
        }
        self.conv.truncate(1);
        self.conv[0].nome = self.conta.servidor.clone();
        self.atual = 0;
        self.estado = tr!("Conectando…").into();
        self.link = Some(irc::start(self.conta.clone()));
    }

    fn conversa(&mut self, nome: &str) -> usize {
        if nome.is_empty() || nome == "*" {
            return 0;
        }
        match self.conv.iter().position(|c| c.nome.eq_ignore_ascii_case(nome)) {
            Some(i) => i,
            None => {
                self.conv.push(Conversa::new(nome));
                self.conv.len() - 1
            }
        }
    }

    fn push(&mut self, i: usize, l: Linha) {
        let c = &mut self.conv[i];
        c.linhas.push(l);
        if c.linhas.len() > 2000 {
            c.linhas.drain(..500);
        }
        if i != self.atual || !self.ativa {
            c.nao_lidas += 1;
        }
    }

    fn eventos(&mut self) -> bool {
        let mut evs = Vec::new();
        if let Some((_, rx)) = &self.link {
            while let Ok(e) = rx.try_recv() {
                evs.push(e);
            }
        }
        let got = !evs.is_empty();
        for e in evs {
            let sis = |t: String| Linha { de: String::new(), texto: t, tipo: Tipo::Sistema, hora: agora() };
            match e {
                Ev::Pronto(n) => {
                    self.apelido = n.clone();
                    self.estado = tr!("Conectado como {}", n);
                }
                Ev::Apelido(n) => {
                    self.apelido = n.clone();
                    self.estado = tr!("Conectado como {}", n);
                }
                Ev::Servidor(t) => self.push(0, sis(t)),
                Ev::Entrou(c) => {
                    let i = self.conversa(&c);
                    self.atual = i;
                    self.push(i, sis(tr!("Você entrou em {}.", c)));
                }
                Ev::Saiu(c) => {
                    if let Some(i) = self.conv.iter().position(|x| x.nome.eq_ignore_ascii_case(&c)).filter(|i| *i > 0) {
                        self.conv.remove(i);
                        self.atual = self.atual.min(self.conv.len() - 1);
                    }
                }
                Ev::Msg { conversa, de, texto, acao } => {
                    let i = self.conversa(&conversa);
                    let particular = !conversa.starts_with('#');
                    let mencao = !self.apelido.is_empty() && texto.to_lowercase().contains(&self.apelido.to_lowercase());
                    if (particular || mencao) && !self.ativa {
                        notify(&if particular { de.clone() } else { format!("{de} — {conversa}") }, &texto);
                    }
                    let texto = if acao { format!("* {de} {texto}") } else { texto };
                    self.push(i, Linha { de, texto, tipo: Tipo::Outro, hora: agora() });
                }
                Ev::Sistema { conversa, texto } => {
                    if conversa == "*" {
                        // Saídas e trocas de nome: nos canais onde a pessoa estava.
                        let nick = texto.trim_start_matches("← ").split(' ').next().unwrap_or("").to_string();
                        let alvo: Vec<usize> = (1..self.conv.len()).filter(|i| self.conv[*i].membros.iter().any(|m| m.eq_ignore_ascii_case(&nick))).collect();
                        for i in alvo {
                            self.push(i, sis(texto.clone()));
                        }
                    } else {
                        let i = self.conversa(&conversa);
                        self.push(i, sis(texto));
                    }
                }
                Ev::Nomes { canal, nomes } => {
                    let i = self.conversa(&canal);
                    self.conv[i].membros = nomes;
                }
                Ev::Caiu(e) => {
                    self.estado = tr!("Desconectado: {}", e);
                    self.link = None;
                    self.push(0, sis(tr!("Desconectado: {}", e)));
                }
            }
        }
        got
    }

    fn mandar(&mut self, linha: &str) {
        let Some((tx, _)) = &self.link else { return };
        let tx = tx.clone();
        let alvo = self.conv[self.atual].nome.clone();
        let eu = self.apelido.clone();
        let meu = |t: String| Linha { de: eu.clone(), texto: t, tipo: Tipo::Meu, hora: agora() };
        if let Some(cmd) = linha.strip_prefix('/') {
            let (c, resto) = cmd.split_once(' ').unwrap_or((cmd, ""));
            match c.to_lowercase().as_str() {
                "join" | "j" => {
                    let _ = tx.send(format!("JOIN {}", resto.trim()));
                }
                "part" | "leave" => {
                    let ch = if resto.trim().is_empty() { alvo } else { resto.trim().to_string() };
                    let _ = tx.send(format!("PART {ch}"));
                }
                "msg" | "query" => {
                    let (quem, t) = resto.split_once(' ').unwrap_or((resto, ""));
                    let i = self.conversa(quem.trim());
                    self.atual = i;
                    if !t.is_empty() {
                        let _ = tx.send(format!("PRIVMSG {} :{t}", quem.trim()));
                        self.push(i, meu(t.to_string()));
                    }
                }
                "me" if self.atual > 0 => {
                    let _ = tx.send(format!("PRIVMSG {alvo} :\u{1}ACTION {resto}\u{1}"));
                    let i = self.atual;
                    self.push(i, meu(format!("* {eu} {resto}")));
                }
                "nick" => {
                    let _ = tx.send(format!("NICK {}", resto.trim()));
                }
                _ => {
                    let _ = tx.send(cmd.to_string());
                }
            }
            return;
        }
        if self.atual == 0 {
            let _ = tx.send(linha.to_string());
            return;
        }
        let _ = tx.send(format!("PRIVMSG {alvo} :{linha}"));
        let i = self.atual;
        self.push(i, meu(linha.to_string()));
    }

    fn lateral(&mut self, ui: &mut Ui) {
        let h = ui.c.h;
        ui.c.fill_rect(0.0, 0.0, SIDE_W, h, &solid(rgba(226, 231, 238, 255)));
        ui.c.vline(SIDE_W - 1.0, 0.0, h, rgba(150, 156, 166, 255));
        // Estado: bolinha verde (conectado) ou cinza, e o apelido.
        let on = self.link.is_some() && !self.apelido.is_empty();
        gel_circle(ui.c, 16.0, 18.0, 5.5, if on { rgba(60, 190, 70, 255) } else { rgba(160, 160, 166, 255) });
        let st = ui.fonts.ellipsize(&self.estado, 11.0, false, SIDE_W - 36.0);
        ui.fonts.draw(ui.c, &st, 28.0, 22.0, 11.0, false, rgba(60, 60, 66, 255));
        let mut y = 38.0;
        let mut pick = None;
        for (i, c) in self.conv.iter().enumerate() {
            let r = Rect::new(0.0, y, SIDE_W - 1.0, 26.0);
            let on = i == self.atual;
            if on {
                ui.c.fill_rect(r.x, r.y, r.w, r.h, &vgrad(r.y, r.y + r.h, &[(0.0, rgba(110, 160, 230, 255)), (1.0, rgba(60, 115, 200, 255))]));
            }
            let icon = if i == 0 { Icon::Server } else if c.nome.starts_with('#') { Icon::Messages } else { Icon::from_name("contacts") };
            glove_ui::paint::icons::draw(ui.c, ui.fonts, icon, 10.0, y + 4.0, 18.0, None);
            let col = if on { rgba(255, 255, 255, 255) } else { rgba(30, 30, 34, 255) };
            let nome = ui.fonts.ellipsize(&c.nome, 12.0, c.nao_lidas > 0 || on, SIDE_W - 76.0);
            ui.fonts.draw(ui.c, &nome, 34.0, y + 17.5, 12.0, c.nao_lidas > 0 || on, col);
            if c.nao_lidas > 0 {
                let t = c.nao_lidas.min(99).to_string();
                let tw = ui.text_width(&t, 10.0, true);
                let bx = SIDE_W - 14.0 - tw - 10.0;
                ui.c.fill(rrect(bx, y + 6.0, tw + 10.0, 14.0, 7.0), &solid(if on { rgba(255, 255, 255, 230) } else { rgba(90, 110, 150, 255) }));
                ui.fonts.draw(ui.c, &t, bx + 5.0, y + 16.5, 10.0, true, if on { rgba(60, 115, 200, 255) } else { rgba(255, 255, 255, 255) });
            }
            if ui.hit(r).clicked {
                pick = Some(i);
            }
            y += 26.0;
        }
        if let Some(i) = pick {
            self.atual = i;
            self.conv[i].nao_lidas = 0;
        }
        let mut act = None;
        ui.area(Rect::new(10.0, h - 34.0, SIDE_W - 20.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                if ui.button("+") {
                    act = Some("nova");
                }
                if ui.button(tr!("Conta…")) {
                    act = Some("conta");
                }
            });
        });
        match act {
            Some("nova") => self.nova = Some(String::new()),
            Some("conta") => self.abrir_folha(),
            _ => {}
        }
    }

    fn conversa_ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let area = Rect::new(SIDE_W, 0.0, w - SIDE_W, h - INPUT_H);
        ui.c.fill_rect(area.x, area.y, area.w, area.h, &solid(rgba(255, 255, 255, 255)));
        let maxw = (area.w * 0.68).min(460.0);
        let fonts = ui.fonts;
        // Mede de baixo para cima e desenha só o que aparece.
        let c = &mut self.conv[self.atual];
        let wheel = ui.scroll_in(area);
        if wheel != 0.0 {
            c.rolagem = (c.rolagem - wheel * 3.0).max(0.0);
        }
        let mut y = area.y + area.h - 10.0 + c.rolagem;
        let old = ui.c.clip();
        let s = ui.c.scale;
        ui.c.set_clip(Some([(area.x * s) as i32, (area.y * s) as i32, ((area.x + area.w) * s) as i32, ((area.y + area.h) * s) as i32]));
        let mut topo = y;
        for (k, l) in c.linhas.iter().enumerate().rev() {
            if y < area.y - 400.0 {
                break;
            }
            match l.tipo {
                Tipo::Sistema => {
                    let lines = glove_ui::paint::chrome::wrap_lines(fonts, &l.texto, 11.0, false, area.w - 60.0);
                    let hh = lines.len() as f32 * 14.0 + 8.0;
                    y -= hh;
                    for (j, t) in lines.iter().enumerate() {
                        let tw = fonts.measure(t, 11.0, false);
                        fonts.draw(ui.c, t, area.x + (area.w - tw) / 2.0, y + 14.0 + j as f32 * 14.0, 11.0, false, rgba(130, 130, 136, 255));
                    }
                }
                _ => {
                    let lines = glove_ui::paint::chrome::wrap_lines(fonts, &l.texto, PX, false, maxw - 24.0);
                    let tw = lines.iter().map(|t| fonts.measure(t, PX, false)).fold(0.0f32, f32::max);
                    let bw = tw + 24.0;
                    let bh = lines.len() as f32 * 16.0 + 12.0;
                    // Nome de quem fala, quando muda.
                    let show_name = l.tipo == Tipo::Outro && k.checked_sub(1).and_then(|p| c.linhas.get(p)).is_none_or(|p| p.de != l.de || p.tipo != Tipo::Outro);
                    let gap = if show_name { 16.0 } else { 4.0 };
                    y -= bh + gap;
                    let mine = l.tipo == Tipo::Meu;
                    let bx = if mine { area.x + area.w - bw - 14.0 } else { area.x + 14.0 };
                    if show_name {
                        fonts.draw(ui.c, &format!("{}  {}", l.de, l.hora), bx + 8.0, y + 11.0, 10.0, false, rgba(120, 120, 126, 255));
                    }
                    let by = y + gap - 2.0;
                    let r = (bh / 2.0).min(12.0);
                    if mine {
                        ui.c.fill(rrect(bx, by + 1.0, bw, bh, r), &solid(rgba(0, 0, 0, 30)));
                        ui.c.fill(rrect(bx, by, bw, bh, r), &vgrad(by, by + bh, &[(0.0, rgba(120, 180, 255, 255)), (0.5, rgba(60, 130, 235, 255)), (1.0, rgba(40, 105, 215, 255))]));
                        ui.c.fill(rrect(bx + 2.0, by + 1.0, bw - 4.0, bh * 0.45, r), &solid(rgba(255, 255, 255, 50)));
                    } else {
                        ui.c.fill(rrect(bx, by + 1.0, bw, bh, r), &solid(rgba(0, 0, 0, 25)));
                        ui.c.fill(rrect(bx, by, bw, bh, r), &vgrad(by, by + bh, &[(0.0, rgba(248, 248, 250, 255)), (1.0, rgba(222, 224, 230, 255))]));
                        ui.c.stroke(rrect(bx + 0.5, by + 0.5, bw - 1.0, bh - 1.0, r), &solid(rgba(0, 0, 0, 40)), 1.0);
                    }
                    let ink = if mine { rgba(255, 255, 255, 255) } else { rgba(25, 25, 28, 255) };
                    for (j, t) in lines.iter().enumerate() {
                        fonts.draw(ui.c, t, bx + 12.0, by + 18.0 + j as f32 * 16.0, PX, false, ink);
                    }
                }
            }
            topo = y;
        }
        // Não rola para além da primeira mensagem.
        if topo > area.y + 10.0 && c.rolagem > 0.0 {
            c.rolagem = (c.rolagem - (topo - area.y - 10.0)).max(0.0);
        }
        ui.c.set_clip(old);
        if c.linhas.is_empty() {
            let t = if self.atual == 0 { tr!("Mensagens do servidor aparecem aqui.") } else { tr!("Nenhuma mensagem ainda.") };
            let tw = fonts.measure(t, 12.0, false);
            fonts.draw(ui.c, t, area.x + (area.w - tw) / 2.0, area.h / 2.0, 12.0, false, rgba(140, 140, 146, 255));
        }
        // Campo de texto embaixo.
        let y0 = h - INPUT_H;
        ui.c.fill_rect(SIDE_W, y0, w - SIDE_W, INPUT_H, &vgrad(y0, h, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(212, 212, 216, 255))]));
        ui.c.hline(SIDE_W, y0, w - SIDE_W, rgba(150, 150, 156, 255));
        let tw = w - SIDE_W - 24.0;
        let foco = self.folha.is_none() && self.nova.is_none();
        let (_, enter) = ui.text_field_in(Rect::new(SIDE_W + 12.0, y0 + 9.0, tw, 22.0), &mut self.texto, foco);
        if enter && !self.texto.trim().is_empty() {
            let t = std::mem::take(&mut self.texto);
            self.mandar(t.trim());
            self.conv[self.atual].rolagem = 0.0;
        }
    }

    fn folha_conta(&mut self, ui: &mut Ui) {
        let Some(f) = &mut self.folha else { return };
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 60)));
        let (pw, ph) = (440.0, 280.0);
        let x = (w - pw) / 2.0;
        ui.c.fill(rrect4(x, 0.0, pw, ph, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(236, 236, 238, 255)));
        ui.c.stroke(rrect4(x + 0.5, -0.5, pw - 1.0, ph, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(0, 0, 0, 90)), 1.0);
        glove_ui::paint::icons::draw(ui.c, ui.fonts, Icon::Messages, x + 16.0, 16.0, 48.0, None);
        ui.fonts.draw(ui.c, tr!("Conta de bate-papo (IRC)"), x + 76.0, 34.0, 13.0, true, rgba(25, 25, 28, 255));
        ui.fonts.draw(ui.c, tr!("O Libera.Chat é aberto e gratuito; escolha um apelido."), x + 76.0, 52.0, 11.0, false, rgba(90, 90, 96, 255));
        let mut ok = false;
        let mut cancel = false;
        let rows: [(&str, usize); 5] = [(tr!("Servidor:"), 0), (tr!("Porta:"), 1), (tr!("Apelido:"), 2), (tr!("Nome:"), 3), (tr!("Canais:"), 4)];
        for (k, (label, i)) in rows.iter().enumerate() {
            let y = 76.0 + k as f32 * 30.0;
            let lw = ui.text_width(label, 12.0, false);
            ui.fonts.draw(ui.c, label, x + 120.0 - lw, y + 16.0, 12.0, false, rgba(25, 25, 28, 255));
            let v = match i {
                0 => &mut f.servidor,
                1 => &mut f.porta,
                2 => &mut f.apelido,
                3 => &mut f.nome,
                _ => &mut f.canais,
            };
            let (_, e) = ui.text_field_in(Rect::new(x + 128.0, y, if *i == 1 { 80.0 } else { pw - 150.0 }, 22.0), v, false);
            ok |= e;
            if *i == 1 {
                ui.area(Rect::new(x + 222.0, y + 2.0, 160.0, 22.0), |ui| {
                    ui.checkbox(&mut f.tls, tr!("Conexão segura (TLS)"));
                });
            }
        }
        ui.area(Rect::new(x + pw - 210.0, ph - 40.0, 200.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ok |= ui.default_button(tr!("Conectar"));
            });
        });
        if cancel {
            self.folha = None;
        } else if ok && !f.servidor.trim().is_empty() && !f.apelido.trim().is_empty() {
            self.conta = Conta {
                servidor: f.servidor.trim().into(),
                porta: f.porta.trim().parse().unwrap_or(if f.tls { 6697 } else { 6667 }),
                tls: f.tls,
                apelido: f.apelido.trim().replace(' ', "_"),
                nome: f.nome.trim().into(),
                canais: f.canais.split([',', ' ']).map(str::trim).filter(|c| !c.is_empty()).map(|c| if c.starts_with('#') { c.to_string() } else { format!("#{c}") }).collect(),
            };
            if let Ok(t) = toml::to_string(&self.conta) {
                let _ = std::fs::create_dir_all(glove_config::dir());
                let _ = std::fs::write(conta_path(), t);
            }
            self.folha = None;
            self.conectar();
        }
    }

    fn folha_nova(&mut self, ui: &mut Ui) {
        let Some(t) = &mut self.nova else { return };
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 60)));
        let (pw, ph) = (380.0, 130.0);
        let x = (w - pw) / 2.0;
        ui.c.fill(rrect4(x, 0.0, pw, ph, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(236, 236, 238, 255)));
        ui.fonts.draw(ui.c, tr!("Canal (#nome) ou apelido de alguém:"), x + 20.0, 30.0, 12.0, true, rgba(25, 25, 28, 255));
        let (_, mut ok) = ui.text_field_in(Rect::new(x + 20.0, 44.0, pw - 40.0, 22.0), t, true);
        let mut cancel = false;
        ui.area(Rect::new(x + pw - 200.0, ph - 40.0, 190.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ok |= ui.default_button("OK");
            });
        });
        if cancel {
            self.nova = None;
        } else if ok {
            let alvo = t.trim().to_string();
            self.nova = None;
            if alvo.starts_with('#') {
                self.mandar(&format!("/join {alvo}"));
            } else if !alvo.is_empty() {
                self.atual = self.conversa(&alvo);
            }
        }
    }
}

impl App for Mensagens {
    fn tick(&self) -> Option<Duration> {
        self.link.as_ref().map(|_| Duration::from_millis(300))
    }

    fn on_tick(&mut self) {}

    fn ui(&mut self, ui: &mut Ui) {
        self.ativa = ui.window_active();
        self.eventos();
        if self.ativa {
            let i = self.atual;
            self.conv[i].nao_lidas = 0;
        }
        for a in ui.menu_actions().to_vec() {
            match a.as_str() {
                "conta" => self.abrir_folha(),
                "reconectar" => self.conectar(),
                "nova" => self.nova = Some(String::new()),
                "sair-canal" if self.atual > 0 => {
                    let n = self.conv[self.atual].nome.clone();
                    if n.starts_with('#') {
                        self.mandar(&format!("/part {n}"));
                    } else {
                        self.conv.remove(self.atual);
                        self.atual -= 1;
                    }
                }
                _ => {}
            }
        }
        let modal = self.folha.is_some() || self.nova.is_some();
        ui.set_enabled(!modal);
        self.lateral(ui);
        self.conversa_ui(ui);
        ui.set_enabled(true);
        self.folha_conta(ui);
        self.folha_nova(ui);
        let title = if self.atual == 0 { tr!("Mensagens").to_string() } else { format!("{} — {}", self.conv[self.atual].nome, tr!("Mensagens")) };
        ui.set_title(&title);
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            tr!("Conversa"),
            vec![
                MenuItem::new("nova", tr!("Nova Conversa…"), "Ctrl+N"),
                MenuItem::new("sair-canal", tr!("Fechar Conversa"), "Ctrl+W").enabled(self.atual > 0),
                MenuItem::Separator,
                MenuItem::new("conta", tr!("Conta…"), ""),
                MenuItem::new("reconectar", tr!("Reconectar"), "Ctrl+R"),
            ],
        )]
    }

    fn close_requested(&mut self) -> bool {
        if let Some((tx, _)) = &self.link {
            let _ = tx.send("\u{0}SAIR".into());
        }
        true
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Mensagens").into(), app_id: "glove.Mensagens".into(), size: (760, 520), fixed: false, min_size: Some((560, 380)) };
    if let Err(e) = run(opts, Mensagens::new()) {
        eprintln!("glove-mensagens: {e}");
    }
}
