//! Instalador do Glove: do Glove ao vivo (a ISO) para o disco do computador.
//! Um assistente em páginas: Introdução, Idioma e Teclado, Fuso Horário, Disco,
//! Conta, Resumo e Instalação (o motor está em `motor.rs`).
//!
//! Só instala de verdade no Glove ao vivo. Em qualquer outro sistema abre apenas
//! com `--simular`, que percorre tudo e mostra os comandos sem executar nenhum.

mod discos;
mod motor;

use glove_config::tr;
use std::time::Duration;

use discos::Disco;
use glove_ui::{
    paint::draw::*,
    run, AlertKind, App, Rect, Ui, WindowOptions,
};
use motor::{Escolhas, Modo, Shared};

const SIDE_W: f32 = 210.0;
const BOTTOM_H: f32 = 52.0;
const PAGINAS: [&str; 7] = ["Introdução", "Idioma e Teclado", "Fuso Horário", "Disco", "Conta", "Resumo", "Instalação"];

const IDIOMAS: [(&str, &str); 8] = [
    ("pt_BR.UTF-8", "Português (Brasil)"),
    ("pt_PT.UTF-8", "Português (Portugal)"),
    ("en_US.UTF-8", "English (US)"),
    ("en_GB.UTF-8", "English (UK)"),
    ("es_ES.UTF-8", "Español"),
    ("fr_FR.UTF-8", "Français"),
    ("de_DE.UTF-8", "Deutsch"),
    ("it_IT.UTF-8", "Italiano"),
];

const TECLADOS: [(&str, &str, &str); 9] = [
    ("br", "", "Português (Brasil, ABNT2)"),
    ("us", "intl", "Inglês (EUA, internacional)"),
    ("us", "", "Inglês (EUA)"),
    ("gb", "", "Inglês (Reino Unido)"),
    ("pt", "", "Português (Portugal)"),
    ("es", "", "Espanhol"),
    ("fr", "", "Francês"),
    ("de", "", "Alemão"),
    ("it", "", "Italiano"),
];

fn fusos() -> Vec<String> {
    let t = std::fs::read_to_string("/usr/share/zoneinfo/zone1970.tab").unwrap_or_default();
    let mut v: Vec<String> = t.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split('\t').nth(2)).map(String::from).collect();
    v.sort();
    v.dedup();
    if v.is_empty() {
        v = vec!["America/Sao_Paulo".into(), "Europe/Lisbon".into(), "UTC".into()];
    }
    v
}

fn fuso_atual() -> String {
    std::fs::read_link("/etc/localtime").ok().and_then(|p| p.to_string_lossy().split("zoneinfo/").nth(1).map(String::from)).unwrap_or_else(|| "America/Sao_Paulo".into())
}

/// Nome de usuário a partir do nome: minúsculas, sem acentos nem espaços.
fn usuario_de(nome: &str) -> String {
    let primeiro = nome.split_whitespace().next().unwrap_or("");
    primeiro
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn usuario_valido(u: &str) -> bool {
    !u.is_empty() && u.len() <= 32 && u.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && u.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') && !["root", "glove", "daemon", "bin", "sys", "sudo"].contains(&u)
}

fn computador_valido(h: &str) -> bool {
    !h.is_empty() && h.len() <= 63 && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !h.starts_with('-')
}

struct Instalador {
    simular: bool,
    pagina: usize,
    idioma: usize,
    teclado: usize,
    fusos: Vec<String>,
    fuso: Option<usize>,
    busca_fuso: String,
    discos: Vec<Disco>,
    disco: usize,
    modo: usize,
    nome: String,
    usuario: String,
    usuario_tocado: bool,
    senha: String,
    senha2: String,
    computador: String,
    computador_tocado: bool,
    confirmar: bool,
    trabalho: Option<Shared>,
    /// Senha de administrador para o sudo (fora do Glove ao vivo).
    senha_admin: Option<String>,
    pedir_admin: bool,
    texto_admin: String,
    uefi: bool,
}

impl Instalador {
    fn new(simular: bool) -> Self {
        let fusos = fusos();
        let atual = fuso_atual();
        let fuso = fusos.iter().position(|f| *f == atual);
        let lang = std::env::var("LANG").unwrap_or_default();
        let idioma = IDIOMAS.iter().position(|(l, _)| lang.starts_with(&l[..5])).unwrap_or(0);
        let teclado = match idioma {
            0 => 0,
            1 => 4,
            2 => 2,
            3 => 3,
            4 => 5,
            5 => 6,
            6 => 7,
            _ => 8,
        };
        let discos: Vec<Disco> = discos::listar().into_iter().filter(|d| !d.ao_vivo).collect();
        Self {
            simular,
            pagina: 0,
            idioma,
            teclado,
            fusos,
            fuso,
            busca_fuso: String::new(),
            discos,
            disco: 0,
            modo: 0,
            nome: String::new(),
            usuario: String::new(),
            usuario_tocado: false,
            senha: String::new(),
            senha2: String::new(),
            computador: String::new(),
            computador_tocado: false,
            confirmar: false,
            trabalho: None,
            senha_admin: None,
            pedir_admin: false,
            texto_admin: String::new(),
            uefi: std::path::Path::new("/sys/firmware/efi").exists(),
        }
    }

    fn modo(&self) -> Modo {
        let d = self.discos.get(self.disco);
        if self.modo == 1 && d.is_some_and(|d| self.livre_ok(d)) { Modo::Livre } else { Modo::Inteiro }
    }

    /// Dá para instalar no espaço livre deste disco? (UEFI, GPT, com partição EFI e 20 GB livres)
    fn livre_ok(&self, d: &Disco) -> bool {
        self.uefi && d.tabela == "gpt" && d.esp().is_some() && d.livre_bytes() >= 20_000_000_000
    }

    fn escolhas(&self) -> Option<Escolhas> {
        let d = self.discos.get(self.disco)?.clone();
        let (l, v, _) = TECLADOS[self.teclado];
        Some(Escolhas {
            idioma: IDIOMAS[self.idioma].0.into(),
            layout: l.into(),
            variante: v.into(),
            fuso: self.fuso.and_then(|i| self.fusos.get(i)).cloned().unwrap_or_else(|| "UTC".into()),
            disco: d,
            modo: self.modo(),
            nome: self.nome.trim().into(),
            usuario: self.usuario.trim().into(),
            senha: self.senha.clone(),
            computador: self.computador.trim().into(),
            uefi: self.uefi,
        })
    }

    /// A página atual está completa (pode Continuar)?
    fn pronta(&self) -> bool {
        match self.pagina {
            2 => self.fuso.is_some(),
            3 => !self.discos.is_empty(),
            4 => !self.nome.trim().is_empty() && usuario_valido(self.usuario.trim()) && self.senha.len() >= 4 && self.senha == self.senha2 && computador_valido(self.computador.trim()),
            5 => self.confirmar,
            _ => true,
        }
    }

    fn comecar(&mut self) {
        let Some(e) = self.escolhas() else { return };
        if !self.simular && self.senha_admin.is_none() && !motor::sudo_sem_senha() {
            self.pedir_admin = true;
            return;
        }
        let plano = motor::plano(&e);
        self.trabalho = Some(motor::instalar(plano, self.senha_admin.clone(), self.simular));
        self.pagina = 6;
    }

    fn lateral(&self, ui: &mut Ui) {
        let h = ui.c.h;
        ui.c.fill_rect(0.0, 0.0, SIDE_W, h, &vgrad(0.0, h, &[(0.0, rgba(228, 234, 242, 255)), (1.0, rgba(206, 214, 226, 255))]));
        ui.c.vline(SIDE_W - 1.0, 0.0, h, rgba(150, 156, 166, 255));
        glove_ui::paint::icons::logo(ui.c, (SIDE_W - 72.0) / 2.0, 18.0, 72.0, ui.theme.accent);
        for (i, p) in PAGINAS.iter().enumerate() {
            let y = 120.0 + i as f32 * 28.0;
            let (on, feita) = (i == self.pagina, i < self.pagina);
            let cor = if on { rgba(40, 110, 220, 255) } else if feita { rgba(120, 128, 140, 255) } else { rgba(190, 196, 206, 255) };
            ui.c.fill(circle(26.0, y - 4.0, 5.0), &solid(cor));
            ui.fonts.draw(ui.c, tr(p), 40.0, y, 12.5, on, if on { rgba(20, 20, 24, 255) } else { rgba(80, 84, 92, 255) });
        }
        if self.simular {
            ui.c.fill(rrect(12.0, h - 40.0, SIDE_W - 24.0, 24.0, 12.0), &solid(rgba(240, 160, 40, 255)));
            let t = tr!("SIMULAÇÃO");
            let tw = ui.text_width(t, 11.0, true);
            ui.fonts.draw(ui.c, t, (SIDE_W - tw) / 2.0, h - 24.0, 11.0, true, rgba(255, 255, 255, 255));
        }
    }

    fn titulo(ui: &mut Ui, t: &str, sub: &str) {
        ui.fonts.draw(ui.c, t, SIDE_W + 28.0, 44.0, 18.0, true, rgba(20, 20, 24, 255));
        ui.fonts.draw(ui.c, sub, SIDE_W + 28.0, 66.0, 12.0, false, rgba(90, 90, 96, 255));
    }

    fn conteudo(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let area = Rect::new(SIDE_W + 28.0, 86.0, w - SIDE_W - 56.0, h - 86.0 - BOTTOM_H - 10.0);
        match self.pagina {
            0 => {
                Self::titulo(ui, tr!("Bem-vindo ao instalador do Glove"), tr!("O Glove vai ser copiado para o disco deste computador."));
                ui.area(area, |ui| {
                    ui.paragraph(tr!("Em poucos passos você escolhe o idioma, o teclado, o fuso horário, o disco e cria a sua conta. A instalação leva alguns minutos."));
                    ui.space(10.0);
                    ui.paragraph(tr!("Faça uma cópia dos seus arquivos antes de continuar: se você escolher apagar um disco, tudo o que está nele será perdido."));
                    ui.space(10.0);
                    let tipo = if self.uefi { tr!("Este computador inicia por UEFI.") } else { tr!("Este computador inicia pelo modo BIOS (legado).") };
                    ui.small(tipo);
                    if !self.simular && !motor::ao_vivo() {
                        ui.space(8.0);
                        ui.small(tr!("Atenção: isto não é o Glove ao vivo (a imagem do sistema não foi encontrada)."));
                    }
                });
            }
            1 => {
                Self::titulo(ui, tr!("Idioma e Teclado"), tr!("O idioma do sistema e o tipo do seu teclado."));
                ui.area(area, |ui| {
                    let nomes: Vec<String> = IDIOMAS.iter().map(|(_, n)| n.to_string()).collect();
                    ui.form_row(tr!("Idioma:"), 90.0, |ui| {
                        ui.popup(&mut self.idioma, &nomes, 260.0);
                    });
                    ui.space(8.0);
                    let tecl: Vec<String> = TECLADOS.iter().map(|(_, _, n)| tr(n).to_string()).collect();
                    ui.form_row(tr!("Teclado:"), 90.0, |ui| {
                        ui.popup(&mut self.teclado, &tecl, 260.0);
                    });
                    ui.space(14.0);
                    ui.small(tr!("Teste o teclado aqui:"));
                    let mut t = String::new();
                    ui.text_field(&mut t, 360.0);
                });
            }
            2 => {
                Self::titulo(ui, tr!("Fuso Horário"), tr!("Escolha a cidade mais perto de você."));
                ui.area(Rect::new(area.x, area.y, area.w, 26.0), |ui| {
                    ui.search_field(&mut self.busca_fuso, tr!("Procurar cidade"), 260.0);
                });
                let q = self.busca_fuso.to_lowercase().replace(' ', "_");
                let idx: Vec<usize> = (0..self.fusos.len()).filter(|&i| q.is_empty() || self.fusos[i].to_lowercase().contains(&q)).collect();
                let rows: Vec<String> = idx.iter().map(|&i| self.fusos[i].replace('_', " ")).collect();
                let mut sel = self.fuso.and_then(|f| idx.iter().position(|&i| i == f));
                ui.area(Rect::new(area.x, area.y + 34.0, area.w, area.h - 34.0), |ui| {
                    if ui.list(&mut sel, &rows, area.w, area.h - 40.0) {}
                });
                if let Some(s) = sel {
                    self.fuso = idx.get(s).copied();
                }
            }
            3 => {
                Self::titulo(ui, tr!("Disco"), tr!("Onde instalar o Glove."));
                if self.discos.is_empty() {
                    ui.area(area, |ui| ui.paragraph(tr!("Nenhum disco encontrado (é preciso um disco de pelo menos 8 GB, sem contar o pendrive do Glove ao vivo).")));
                    return;
                }
                let nomes: Vec<String> = self.discos.iter().map(|d| format!("{} — {} ({})", d.nome, discos::tamanho(d.tamanho), d.path)).collect();
                let d = self.discos[self.disco.min(self.discos.len() - 1)].clone();
                let livre = self.livre_ok(&d);
                ui.area(area, |ui| {
                    ui.form_row(tr!("Disco:"), 60.0, |ui| {
                        ui.popup(&mut self.disco, &nomes, area.w - 80.0);
                    });
                    ui.space(10.0);
                    // As partições que já estão no disco.
                    if d.particoes.is_empty() {
                        ui.small(tr!("O disco está vazio."));
                    } else {
                        ui.small(tr!("Hoje o disco tem:"));
                        for p in d.particoes.iter().take(6) {
                            let nome = if p.rotulo.is_empty() { String::new() } else { format!("“{}” ", p.rotulo) };
                            ui.small(&format!("  • {} {}{} {}", p.path, nome, p.fs, discos::tamanho(p.tamanho)));
                        }
                    }
                    ui.space(10.0);
                    let livre_txt = if livre { tr!("Instalar ao lado, no espaço livre ({})", discos::tamanho(d.livre_bytes())) } else { tr!("Instalar ao lado do sistema atual").to_string() };
                    let ops = [tr!("Apagar o disco inteiro e instalar o Glove").to_string(), livre_txt];
                    let ops_ref: Vec<&str> = ops.iter().map(String::as_str).collect();
                    ui.radio_group(&mut self.modo, &ops_ref);
                    if self.modo == 1 && !livre {
                        self.modo = 0;
                    }
                    ui.space(6.0);
                    if !livre {
                        ui.small(tr!("Para instalar ao lado é preciso iniciar por UEFI, ter uma partição EFI e 20 GB livres."));
                        ui.small(tr!("Libere espaço antes no outro sistema (ou com o Discos)."));
                    }
                    if self.modo == 0 {
                        ui.space(6.0);
                        ui.label_ex(&tr!("Tudo o que está em {} será apagado.", d.nome), 12.5, true, rgba(190, 30, 30, 255));
                    }
                });
            }
            4 => {
                Self::titulo(ui, tr!("Conta"), tr!("A sua conta de administrador no Glove."));
                let antes = (self.nome.clone(), self.usuario.clone());
                ui.area(area, |ui| {
                    let lw = 150.0;
                    ui.form_row(tr!("Nome completo:"), lw, |ui| {
                        ui.text_field(&mut self.nome, 260.0);
                    });
                    ui.form_row(tr!("Nome de usuário:"), lw, |ui| {
                        ui.text_field(&mut self.usuario, 180.0);
                    });
                    ui.form_row(tr!("Senha:"), lw, |ui| {
                        let r = ui.alloc(200.0, 22.0);
                        ui.password_field_in(r, &mut self.senha, false);
                    });
                    ui.form_row(tr!("Confirme a senha:"), lw, |ui| {
                        let r = ui.alloc(200.0, 22.0);
                        ui.password_field_in(r, &mut self.senha2, false);
                    });
                    ui.form_row(tr!("Nome do computador:"), lw, |ui| {
                        ui.text_field(&mut self.computador, 200.0);
                    });
                    ui.space(10.0);
                    let u = self.usuario.trim();
                    if !u.is_empty() && !usuario_valido(u) {
                        ui.small(tr!("O nome de usuário usa só letras minúsculas, números, - e _, começando por letra."));
                    } else if !self.senha.is_empty() && self.senha.len() < 4 {
                        ui.small(tr!("A senha precisa de pelo menos 4 caracteres."));
                    } else if !self.senha2.is_empty() && self.senha != self.senha2 {
                        ui.small(tr!("As senhas não são iguais."));
                    } else if !self.computador.trim().is_empty() && !computador_valido(self.computador.trim()) {
                        ui.small(tr!("O nome do computador usa só letras sem acento, números e -."));
                    }
                });
                // Usuário e computador sugeridos a partir do nome (até a pessoa mexer).
                if self.usuario != antes.1 {
                    self.usuario_tocado = true;
                }
                if self.nome != antes.0 {
                    if !self.usuario_tocado {
                        self.usuario = usuario_de(&self.nome);
                    }
                    if !self.computador_tocado {
                        self.computador = format!("{}-glove", usuario_de(&self.nome));
                    }
                }
            }
            5 => {
                Self::titulo(ui, tr!("Resumo"), tr!("Confira antes de instalar."));
                let Some(e) = self.escolhas() else { return };
                ui.area(area, |ui| {
                    let lw = 150.0;
                    let linha = |ui: &mut Ui, k: &str, v: &str| {
                        ui.form_row(k, lw, |ui| ui.label(v));
                    };
                    linha(ui, tr!("Idioma:"), IDIOMAS[self.idioma].1);
                    linha(ui, tr!("Teclado:"), tr(TECLADOS[self.teclado].2));
                    linha(ui, tr!("Fuso horário:"), &e.fuso.replace('_', " "));
                    linha(ui, tr!("Disco:"), &format!("{} ({})", e.disco.nome, e.disco.path));
                    linha(ui, tr!("Como:"), if e.modo == Modo::Inteiro { tr!("apagar o disco inteiro") } else { tr!("no espaço livre, ao lado do outro sistema") });
                    linha(ui, tr!("Conta:"), &format!("{} ({})", e.nome, e.usuario));
                    linha(ui, tr!("Computador:"), &e.computador);
                    ui.space(12.0);
                    if e.modo == Modo::Inteiro {
                        ui.label_ex(&tr!("Tudo o que está em {} será apagado.", e.disco.nome), 12.5, true, rgba(190, 30, 30, 255));
                    }
                    ui.checkbox(&mut self.confirmar, tr!("Entendi. Pode instalar."));
                });
            }
            _ => self.instalando(ui, area),
        }
    }

    fn instalando(&mut self, ui: &mut Ui, area: Rect) {
        let Some(t) = self.trabalho.clone() else { return };
        let (texto, fracao, passo, fim, reg) = {
            let a = t.lock().unwrap();
            (a.texto.clone(), a.fracao, a.passo, a.fim.clone(), a.registro.iter().rev().take(14).rev().cloned().collect::<Vec<_>>())
        };
        let total = motor::plano(&self.escolhas().unwrap()).iter().map(|p| p.peso).collect::<Vec<_>>();
        let feito: f32 = total.iter().take(passo).sum::<f32>() + total.get(passo).copied().unwrap_or(0.0) * fracao;
        match &fim {
            None => {
                Self::titulo(ui, tr!("Instalando…"), &texto);
                ui.wake_after(Duration::from_millis(200));
            }
            Some(Ok(())) => Self::titulo(ui, tr!("Pronto!"), if self.simular { tr!("Simulação concluída: nada foi alterado.") } else { tr!("O Glove foi instalado. Tire o pendrive e reinicie.") }),
            Some(Err(_)) => Self::titulo(ui, tr!("A instalação não terminou"), tr!("Nada mais foi alterado depois do erro. Veja o registro abaixo.")),
        }
        ui.area(Rect::new(area.x, area.y, area.w, 24.0), |ui| ui.progress(Some(if fim.is_some() { 1.0 } else { feito.min(1.0) }), area.w));
        let ry = area.y + 34.0;
        ui.c.fill(rrect(area.x, ry, area.w, area.h - 34.0, 4.0), &solid(rgba(32, 34, 40, 255)));
        let mono = ui.doc_fonts;
        for (k, l) in reg.iter().enumerate() {
            let t = ui.fonts.ellipsize(l, 10.5, false, area.w - 16.0);
            mono.draw(ui.c, &t, area.x + 8.0, ry + 16.0 + k as f32 * 14.0, 10.5, false, if l.starts_with("ERRO") { rgba(255, 120, 110, 255) } else if l.starts_with("==") { rgba(140, 200, 255, 255) } else { rgba(200, 204, 210, 255) });
        }
    }

    fn rodape(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let y = h - BOTTOM_H;
        ui.c.hline(SIDE_W, y, w - SIDE_W, rgba(170, 170, 176, 255));
        let fim = self.trabalho.as_ref().and_then(|t| t.lock().unwrap().fim.clone());
        let pronta = self.pronta();
        let mut act = None;
        // Continuar sempre no mesmo lugar (à direita); Voltar à esquerda dele.
        ui.area(Rect::new(w - 130.0, y + 13.0, 116.0, 28.0), |ui| {
            if self.pagina == 6 {
                if let Some(r) = &fim {
                    if r.is_ok() && !self.simular {
                        if ui.default_button(tr!("Reiniciar")) {
                            act = Some("reiniciar");
                        }
                    } else if ui.default_button(tr!("Fechar")) {
                        act = Some("fechar");
                    }
                }
                return;
            }
            ui.set_enabled(pronta && !self.pedir_admin);
            let rotulo = if self.pagina == 5 { tr!("Instalar") } else { tr!("Continuar") };
            if ui.default_button(rotulo) {
                act = Some("seguir");
            }
            ui.set_enabled(true);
        });
        if self.pagina > 0 && self.pagina < 6 {
            ui.area(Rect::new(w - 250.0, y + 13.0, 110.0, 28.0), |ui| {
                if ui.button(tr!("Voltar|Instalador")) {
                    act = Some("voltar");
                }
            });
        }
        match act {
            Some("voltar") => self.pagina -= 1,
            Some("seguir") if self.pagina == 5 => self.comecar(),
            Some("seguir") => self.pagina += 1,
            Some("reiniciar") => {
                let _ = std::process::Command::new("systemctl").arg("reboot").spawn();
            }
            Some("fechar") => ui.close = true,
            _ => {}
        }
    }

    fn folha_admin(&mut self, ui: &mut Ui) {
        if !self.pedir_admin {
            return;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        ui.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 60)));
        let (pw, ph) = (380.0, 150.0);
        let x = (w - pw) / 2.0;
        ui.c.fill(rrect4(x, 0.0, pw, ph, [0.0, 0.0, 8.0, 8.0]), &solid(rgba(236, 236, 238, 255)));
        ui.fonts.draw(ui.c, tr!("Senha de administrador"), x + 20.0, 30.0, 13.0, true, rgba(25, 25, 28, 255));
        ui.fonts.draw(ui.c, tr!("Para mexer nos discos, digite a senha deste sistema."), x + 20.0, 48.0, 11.0, false, rgba(90, 90, 96, 255));
        let ok = ui.password_field_in(Rect::new(x + 20.0, 62.0, pw - 40.0, 22.0), &mut self.texto_admin, true);
        let (mut ok2, mut cancel) = (false, false);
        ui.area(Rect::new(x + pw - 200.0, ph - 40.0, 190.0, 26.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button(tr!("Cancelar"));
                ok2 = ui.default_button("OK");
            });
        });
        if cancel {
            self.pedir_admin = false;
            self.texto_admin.clear();
        } else if (ok || ok2) && !self.texto_admin.is_empty() {
            self.senha_admin = Some(std::mem::take(&mut self.texto_admin));
            self.pedir_admin = false;
            self.comecar();
        }
    }
}

impl App for Instalador {
    fn ui(&mut self, ui: &mut Ui) {
        self.lateral(ui);
        ui.set_enabled(!self.pedir_admin);
        self.conteudo(ui);
        self.rodape(ui);
        ui.set_enabled(true);
        self.folha_admin(ui);
    }

    fn close_requested(&mut self) -> bool {
        // Instalando: não fecha no meio (o disco ficaria pela metade).
        self.trabalho.as_ref().is_none_or(|t| t.lock().unwrap().fim.is_some())
    }
}

/// Fora do Glove ao vivo (e sem --simular): só explica e sai.
struct SoAoVivo;

impl App for SoAoVivo {
    fn ui(&mut self, ui: &mut Ui) {
        if ui.alert_window(AlertKind::Caution, tr!("O instalador funciona só no Glove ao vivo."), tr!("Inicie o computador pelo pendrive ou DVD do Glove para instalar. Para ver o instalador sem mexer em nada, use glove-instalador --simular."), &["OK"]).is_some() {
            ui.close = true;
        }
    }
}

pub fn main() {
    let simular = std::env::args().any(|a| a == "--simular");
    if !simular && !motor::ao_vivo() {
        let opts = WindowOptions { title: tr!("Instalar o Glove").into(), app_id: "glove.Instalador".into(), size: (460, 160), fixed: true, min_size: None };
        let _ = run(opts, SoAoVivo);
        return;
    }
    let opts = WindowOptions { title: tr!("Instalar o Glove").into(), app_id: "glove.Instalador".into(), size: (760, 520), fixed: true, min_size: None };
    if let Err(e) = run(opts, Instalador::new(simular)) {
        eprintln!("glove-instalador: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes() {
        assert_eq!(usuario_de("João da Silva"), "joao");
        assert!(usuario_valido("ana_2"));
        assert!(!usuario_valido("Ana"));
        assert!(!usuario_valido("root"));
        assert!(computador_valido("pc-da-ana"));
        assert!(!computador_valido("pc da ana"));
    }
}
