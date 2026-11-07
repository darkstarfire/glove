//! Central de Apps: procurar, instalar, remover e atualizar programas pelo apt
//! (pacotes do sistema) e pelo Flatpak (Flathub), com a cara da época.
//!
//! O que precisa de administrador (apt) pede a senha numa folha da própria
//! janela e roda com `sudo -S`: a senha vai só para a entrada do sudo, nunca
//! para a tela nem para o disco, e some da memória do app logo depois.

mod fontes;

use glove_config::tr;
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

use fontes::{Origem, Pacote, Passo, Trabalho};
use glove_ui::{
    paint::{draw::*, icons::Icon},
    run, App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};

const BAR_H: f32 = 44.0;
const INFO_H: f32 = 92.0;
const FLATHUB: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

#[derive(Clone)]
enum Acao {
    Instalar(Pacote),
    Remover(Pacote),
    AtualizarTudo,
    Procurar,
    AtivarFlathub,
}

impl Acao {
    fn passos(&self) -> Vec<Passo> {
        use fontes::passo;
        match self {
            Acao::Instalar(p) if p.origem == Origem::Apt => vec![passo(&["apt-get", "install", "-y", &p.id], true)],
            Acao::Instalar(p) => {
                let remoto = if p.remoto.is_empty() { "flathub" } else { p.remoto.as_str() };
                let mut v = Vec::new();
                if remoto == "flathub" {
                    v.push(passo(&["flatpak", "remote-add", "--user", "--if-not-exists", "flathub", FLATHUB], false));
                }
                v.push(passo(&["flatpak", "install", "-y", "--noninteractive", "--user", remoto, &p.id], false));
                v
            }
            Acao::Remover(p) if p.origem == Origem::Apt => vec![passo(&["apt-get", "remove", "-y", &p.id], true)],
            Acao::Remover(p) => vec![passo(&["flatpak", "uninstall", "-y", "--noninteractive", &p.id], false)],
            Acao::AtualizarTudo => {
                let mut v = Vec::new();
                if fontes::has("apt-get") {
                    v.push(passo(&["apt-get", "upgrade", "-y"], true));
                }
                if fontes::has("flatpak") {
                    v.push(passo(&["flatpak", "update", "-y", "--noninteractive"], false));
                }
                v
            }
            Acao::Procurar => {
                if fontes::has("apt-get") {
                    vec![passo(&["apt-get", "update"], true)]
                } else {
                    Vec::new()
                }
            }
            Acao::AtivarFlathub => vec![passo(&["flatpak", "remote-add", "--user", "--if-not-exists", "flathub", FLATHUB], false)],
        }
    }

    fn texto(&self) -> String {
        match self {
            Acao::Instalar(p) => tr!("Instalando {}…", p.nome),
            Acao::Remover(p) => tr!("Removendo {}…", p.nome),
            Acao::AtualizarTudo => tr!("Atualizando tudo…").into(),
            Acao::Procurar => tr!("Procurando atualizações…").into(),
            Acao::AtivarFlathub => tr!("Ativando o Flathub…").into(),
        }
    }
}

struct Senha {
    acao: Acao,
    texto: String,
    erro: bool,
}

struct Central {
    aba: usize,
    busca: String,
    lista: Vec<Pacote>,
    sel: Option<usize>,
    table: TableState,
    carregando: Option<Receiver<Vec<Pacote>>>,
    job: Option<(Arc<Trabalho>, Acao)>,
    senha: Option<Senha>,
    mensagem: Option<String>,
    flathub: bool,
}

impl Central {
    fn new() -> Self {
        let mut c = Self { aba: 1, busca: String::new(), lista: Vec::new(), sel: None, table: TableState::default(), carregando: None, job: None, senha: None, mensagem: None, flathub: fontes::has("flatpak") && fontes::flatpak_remotes() };
        c.carregar();
        c
    }

    /// Recarrega a lista da aba atual numa thread.
    fn carregar(&mut self) {
        let (tx, rx) = channel();
        let (aba, q) = (self.aba, self.busca.trim().to_string());
        std::thread::spawn(move || {
            let v = match aba {
                0 if q.is_empty() => Vec::new(),
                0 => {
                    let mut v = Vec::new();
                    if fontes::has("flatpak") {
                        let inst: HashSet<String> = fontes::flatpak_installed().into_iter().map(|p| p.id).collect();
                        v.extend(fontes::flatpak_search(&q, &inst));
                    }
                    if fontes::has("apt-cache") {
                        v.extend(fontes::apt_search(&q, &fontes::apt_installed()));
                    }
                    v
                }
                1 => {
                    let mut v = fontes::apt_apps();
                    if fontes::has("flatpak") {
                        v.extend(fontes::flatpak_installed());
                    }
                    v.sort_by_key(|p| p.nome.to_lowercase());
                    v
                }
                _ => fontes::updates(),
            };
            let _ = tx.send(v);
        });
        self.carregando = Some(rx);
        self.sel = None;
    }

    fn pedir(&mut self, a: Acao) {
        if self.job.is_some() {
            return;
        }
        if a.passos().iter().any(|p| p.admin) {
            self.senha = Some(Senha { acao: a, texto: String::new(), erro: false });
        } else {
            self.job = Some((fontes::rodar(a.passos(), None), a));
        }
    }

    fn abrir(p: &Pacote) {
        match p.origem {
            Origem::Flatpak => {
                let _ = std::process::Command::new("flatpak").args(["run", &p.id]).spawn();
            }
            Origem::Apt => {
                // O primeiro .desktop do pacote, pelo Exec dele.
                let files = std::process::Command::new("dpkg").args(["-L", &p.id]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
                let exec = files.lines().filter(|l| l.starts_with("/usr/share/applications/") && l.ends_with(".desktop")).find_map(|f| std::fs::read_to_string(f).ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("Exec=").map(String::from))));
                if let Some(e) = exec {
                    let clean: String = e.split_whitespace().filter(|w| !w.starts_with('%')).collect::<Vec<_>>().join(" ");
                    let _ = std::process::Command::new("sh").arg("-c").arg(clean).spawn();
                }
            }
        }
    }

    fn barra(&mut self, ui: &mut Ui) {
        let w = ui.c.w;
        ui.c.fill_rect(0.0, 0.0, w, BAR_H, &vgrad(0.0, BAR_H, &[(0.0, rgba(232, 232, 236, 255)), (1.0, rgba(204, 204, 210, 255))]));
        ui.c.hline(0.0, BAR_H - 1.0, w, rgba(130, 130, 136, 255));
        let mut nova = None;
        let mut enter = false;
        ui.area(Rect::new(12.0, 10.0, 360.0, 26.0), |ui| {
            if let Some(i) = ui.segmented_text(&[tr!("Buscar"), tr!("Instalados"), tr!("Atualizações")], Some(self.aba)) {
                nova = Some(i);
            }
        });
        if self.aba == 0 {
            ui.area(Rect::new(w - 252.0, 11.0, 240.0, 24.0), |ui| {
                enter = ui.search_field(&mut self.busca, tr!("Nome do app"), 240.0);
            });
        }
        if let Some(i) = nova.filter(|i| *i != self.aba) {
            self.aba = i;
            self.lista.clear();
            self.carregar();
        }
        if enter {
            self.carregar();
        }
    }

    fn info(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let y = h - INFO_H;
        ui.c.fill_rect(0.0, y, w, INFO_H, &vgrad(y, h, &[(0.0, rgba(240, 240, 242, 255)), (1.0, rgba(222, 222, 226, 255))]));
        ui.c.hline(0.0, y, w, rgba(150, 150, 156, 255));
        let ink = rgba(25, 25, 28, 255);
        let gray = rgba(100, 100, 106, 255);
        // Trabalho em andamento: a última linha das ferramentas.
        if let Some((t, a)) = &self.job {
            ui.fonts.draw(ui.c, &a.texto(), 16.0, y + 26.0, 13.0, true, ink);
            let l = ui.fonts.ellipsize(&t.ultima(), 11.0, false, w - 40.0);
            ui.fonts.draw(ui.c, &l, 16.0, y + 46.0, 11.0, false, gray);
            ui.area(Rect::new(16.0, y + 58.0, w - 32.0, 20.0), |ui| ui.progress(None, w - 32.0));
            return;
        }
        let sel = self.sel.and_then(|i| self.lista.get(i)).cloned();
        let mut acao = None;
        match &sel {
            Some(p) => {
                let n = ui.fonts.ellipsize(&p.nome, 14.0, true, w - 300.0);
                ui.fonts.draw(ui.c, &n, 16.0, y + 26.0, 14.0, true, ink);
                let origem = match p.origem {
                    Origem::Apt => tr!("Pacote do sistema (apt)").to_string(),
                    Origem::Flatpak => tr!("Flatpak ({})", if p.remoto.is_empty() { "flathub" } else { &p.remoto }),
                };
                let sub = if p.versao.is_empty() { format!("{} — {}", p.id, origem) } else { format!("{} {} — {}", p.id, p.versao, origem) };
                let sub = ui.fonts.ellipsize(&sub, 11.0, false, w - 300.0);
                ui.fonts.draw(ui.c, &sub, 16.0, y + 44.0, 11.0, false, gray);
                let r = ui.fonts.ellipsize(&p.resumo, 12.0, false, w - 300.0);
                ui.fonts.draw(ui.c, &r, 16.0, y + 66.0, 12.0, false, ink);
                let p = p.clone();
                let aba = self.aba;
                ui.area(Rect::new(w - 270.0, y + 28.0, 256.0, 26.0), |ui| {
                    ui.horizontal(|ui| {
                        if aba == 2 {
                            return;
                        }
                        if p.instalado {
                            if ui.button(tr!("Abrir")) {
                                Self::abrir(&p);
                            }
                            if ui.button(tr!("Remover")) {
                                acao = Some(Acao::Remover(p.clone()));
                            }
                        } else if ui.default_button(tr!("Instalar")) {
                            acao = Some(Acao::Instalar(p.clone()));
                        }
                    });
                });
            }
            None => {
                let t = match self.aba {
                    0 if !fontes::has("apt-cache") && !fontes::has("flatpak") => tr!("Nem o apt nem o Flatpak estão instalados neste sistema."),
                    0 => tr!("Digite o nome de um app e pressione Enter."),
                    1 => tr!("Escolha um app para abrir ou remover."),
                    _ if self.lista.is_empty() && self.carregando.is_none() => tr!("Tudo em dia."),
                    _ => tr!("Atualizações disponíveis."),
                };
                ui.fonts.draw(ui.c, t, 16.0, y + 30.0, 12.5, false, gray);
            }
        }
        // Atualizações: procurar e atualizar tudo; Flatpak sem remoto: ativar o Flathub.
        ui.area(Rect::new(16.0, y + 50.0, w - 300.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                if self.aba == 2 {
                    if ui.button(tr!("Procurar Atualizações")) {
                        acao = Some(Acao::Procurar);
                    }
                    if !self.lista.is_empty() && ui.default_button(tr!("Atualizar Tudo")) {
                        acao = Some(Acao::AtualizarTudo);
                    }
                } else if fontes::has("flatpak") && !self.flathub && sel.is_none() && ui.button(tr!("Ativar o Flathub")) {
                    acao = Some(Acao::AtivarFlathub);
                }
            });
        });
        if let Some(a) = acao {
            self.pedir(a);
        }
    }

    /// Folha da senha de administrador.
    fn folha_senha(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.senha else { return };
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 60)));
        let (pw, ph) = (400.0, 170.0);
        let (x, y) = ((w - pw) / 2.0, 0.0);
        ui.c.fill(rrect4(x, y, pw, ph, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(236, 236, 238, 255)));
        ui.c.stroke(rrect4(x + 0.5, y - 0.5, pw - 1.0, ph, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(0, 0, 0, 90)), 1.0);
        glove_ui::paint::icons::draw(ui.c, ui.fonts, Icon::from_name("appcenter"), x + 18.0, y + 18.0, 48.0, None);
        let ink = rgba(25, 25, 28, 255);
        ui.fonts.draw(ui.c, tr!("Digite a sua senha"), x + 80.0, y + 32.0, 13.0, true, ink);
        let t = s.acao.texto();
        let t = ui.fonts.ellipsize(&t, 11.5, false, pw - 100.0);
        ui.fonts.draw(ui.c, &t, x + 80.0, y + 50.0, 11.5, false, rgba(80, 80, 86, 255));
        let enter = ui.password_field_in(Rect::new(x + 80.0, y + 66.0, pw - 100.0, 22.0), &mut s.texto, true);
        if s.erro {
            ui.fonts.draw(ui.c, tr!("Senha incorreta. Tente de novo."), x + 80.0, y + 106.0, 11.0, false, rgba(180, 30, 30, 255));
        }
        let (mut ok, mut cancel) = (enter, false);
        ui.area(Rect::new(x + pw - 200.0, y + ph - 40.0, 190.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ok |= ui.default_button("OK");
            });
        });
        if ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape) {
            cancel = true;
        }
        if cancel {
            self.senha = None;
        } else if ok && !s.texto.is_empty() {
            if fontes::senha_ok(&s.texto) {
                let s = self.senha.take().unwrap();
                self.job = Some((fontes::rodar(s.acao.passos(), Some(s.texto)), s.acao));
            } else {
                s.erro = true;
                s.texto.clear();
            }
        }
    }
}

impl App for Central {
    fn ui(&mut self, ui: &mut Ui) {
        // Lista chegando.
        if let Some(rx) = &self.carregando {
            match rx.try_recv() {
                Ok(v) => {
                    self.lista = v;
                    self.carregando = None;
                    self.table.scroll = 0.0;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(Duration::from_millis(150)),
                Err(_) => self.carregando = None,
            }
        }
        // Trabalho terminou: recarrega a lista e avisa o resultado.
        if let Some((t, a)) = &self.job {
            let fim = t.fim.lock().unwrap().take();
            match fim {
                Some(r) => {
                    if let Err(e) = r {
                        self.mensagem = Some(e);
                    }
                    if matches!(a, Acao::AtivarFlathub | Acao::Instalar(_)) {
                        self.flathub = fontes::flatpak_remotes();
                    }
                    self.job = None;
                    self.carregar();
                }
                None => ui.wake_after(Duration::from_millis(200)),
            }
        }
        if ui.menu_actions().iter().any(|a| a == "refresh") && self.job.is_none() {
            if self.aba == 2 {
                self.pedir(Acao::Procurar);
            } else {
                self.carregar();
            }
        }
        let modal = self.senha.is_some() || self.mensagem.is_some();
        ui.set_enabled(!modal);
        self.barra(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        let area = Rect::new(0.0, BAR_H, w, h - BAR_H - INFO_H);
        let rows: Vec<TableRow> = self
            .lista
            .iter()
            .map(|p| TableRow {
                cells: vec![p.nome.clone(), match p.origem { Origem::Apt => "apt".into(), Origem::Flatpak => "Flatpak".into() }, p.versao.clone(), if p.instalado && self.aba == 0 { tr!("Instalado").into() } else { p.resumo.clone() }],
                icon: Some(match p.origem { Origem::Flatpak => Icon::from_name(&p.id), Origem::Apt => Icon::from_name(&p.id) }),
                depth: 0,
                children: None,
                tint: None,
            })
            .collect();
        let cols = [Column::new(tr!("Nome"), 0.3), Column::new(tr!("Origem"), 0.1), Column::new(tr!("Versão"), 0.15), Column::new(tr!("Descrição"), 0.45)];
        let r = ui.table(area, &cols, &rows, self.sel, &mut self.table, &TableStyle { row_h: 22.0, ..TableStyle::default() });
        if let Some(i) = r.pressed {
            self.sel = Some(i);
        }
        if self.carregando.is_some() {
            let t = tr!("Procurando…");
            let tw = ui.text_width(t, 12.0, false);
            ui.c.fill(rrect((w - tw) / 2.0 - 34.0, area.y + area.h / 2.0 - 18.0, tw + 52.0, 32.0, 8.0), &solid(rgba(0, 0, 0, 140)));
            ui.fonts.draw(ui.c, t, (w - tw) / 2.0 + 8.0, area.y + area.h / 2.0 + 2.0, 12.0, false, rgba(255, 255, 255, 255));
            glove_ui::paint::draw::spinner(ui.c, (w - tw) / 2.0 - 12.0, area.y + area.h / 2.0 - 2.0, 7.0, 0.0, rgba(255, 255, 255, 255));
        }
        self.info(ui);
        ui.set_enabled(true);
        self.folha_senha(ui);
        if let Some(m) = self.mensagem.clone() {
            if ui.alert(glove_ui::AlertKind::Stop, tr!("A operação não deu certo."), &m, &["OK"]).is_some() {
                self.mensagem = None;
            }
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(tr!("Loja"), vec![MenuItem::new("refresh", tr!("Procurar Atualizações"), "Ctrl+R")])]
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Central de Apps").into(), app_id: "glove.Central".into(), size: (820, 560), fixed: false, min_size: Some((640, 420)) };
    if let Err(e) = run(opts, Central::new()) {
        eprintln!("glove-central: {e}");
    }
}
