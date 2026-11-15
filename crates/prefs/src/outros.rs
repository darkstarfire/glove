//! Painéis de prioridade baixa: Atualização de Software, Disco de
//! Inicialização, Fala e Impressão. O que precisa de administrador abre no
//! Terminal (com sudo), onde a senha é pedida.

use glove_config::tr;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{Ticks, Ui};

fn sibling(name: &str) -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.is_file()).unwrap_or_else(|| name.into())
}

/// Abre o Terminal do Glove rodando `cmd`.
fn in_terminal(cmd: &str) {
    if let Ok(mut c) = Command::new(sibling("glove-term")).arg("-e").arg(cmd).spawn() {
        std::thread::spawn(move || c.wait());
    }
}

fn has(prog: &str) -> bool {
    Command::new("sh").arg("-c").arg(format!("command -v {prog}")).output().is_ok_and(|o| o.status.success())
}

// ---------------------------------------------------------------------------
// Atualização de Software
// ---------------------------------------------------------------------------

/// (verificar, instalar) para o gerenciador de pacotes do sistema.
fn package_manager() -> Option<(&'static str, &'static str)> {
    if has("apt") {
        Some(("apt list --upgradable 2>/dev/null", "sudo apt update && sudo apt full-upgrade"))
    } else if has("checkupdates") {
        Some(("checkupdates", "sudo pacman -Syu"))
    } else if has("dnf") {
        Some(("dnf -q check-update", "sudo dnf upgrade"))
    } else if has("zypper") {
        Some(("zypper -q list-updates", "sudo zypper update"))
    } else {
        None
    }
}

/// Pacotes com atualização: "nome/versão" do apt ou "nome versão → nova" dos outros.
pub fn parse_updates(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("Listing") && !l.starts_with("Listando") && !l.starts_with("WARNING"))
        .map(|l| {
            // apt: "pacote/origem 1.2-3 amd64 [upgradable from: 1.2-2]"
            match l.split_once('/') {
                Some((name, rest)) => format!("{name} {}", rest.split_whitespace().nth(1).unwrap_or("")),
                None => l.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
            }
        })
        .collect()
}

fn os_name() -> String {
    std::fs::read_to_string("/etc/os-release").ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string())).unwrap_or_else(|| "Linux".into())
}

#[derive(Default)]
pub struct Atualizacao {
    result: Arc<Mutex<Option<Vec<String>>>>,
    list: Option<Vec<String>>,
    busy: bool,
    /// O Glove: a versão nova publicada (buscada ao abrir o painel).
    nova: Arc<Mutex<Option<Result<crate::atualizar::Nova, String>>>>,
    glove: Option<Result<crate::atualizar::Nova, String>>,
    buscando: bool,
    /// O pedido de senha (o comando que vai rodar e a folha).
    pub prompt: Option<(Vec<String>, crate::admin::Prompt)>,
    job: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    msg: Option<String>,
    atualizado: bool,
}

impl Atualizacao {
    fn buscar_glove(&mut self) {
        self.buscando = true;
        let r = self.nova.clone();
        std::thread::spawn(move || *r.lock().unwrap() = Some(crate::atualizar::buscar()));
    }

    /// Roda como administrador: direto se o sudo não pede senha, senão pela folha.
    fn como_admin(&mut self, args: Vec<String>, what: String) {
        self.msg = None;
        match crate::admin::sudo_state() {
            crate::admin::Sudo::Free => self.job = Some(crate::admin::run(args, None)),
            crate::admin::Sudo::No => self.msg = Some(tr!("Só uma conta de administrador pode atualizar o sistema.").into()),
            crate::admin::Sudo::Password => self.prompt = Some((args, crate::admin::Prompt::new(what))),
        }
    }

    pub fn prompt_sheet(&mut self, ui: &mut Ui) {
        if let Some((args, mut p)) = self.prompt.take() {
            match p.sheet(ui) {
                crate::admin::Answer::Waiting => self.prompt = Some((args, p)),
                crate::admin::Answer::Cancel => {}
                crate::admin::Answer::Password(pw) => self.job = Some(crate::admin::run(args, Some(pw))),
            }
        }
    }

    /// A seção do Glove: versão instalada, a nova, o que há de novo e Atualizar.
    fn glove(&mut self, ui: &mut Ui) {
        use crate::atualizar as a;
        if self.glove.is_none() && !self.buscando {
            self.buscar_glove();
        }
        if let Some(r) = self.nova.lock().unwrap().take() {
            self.glove = Some(r);
            self.buscando = false;
        }
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok(r) => {
                    self.job = None;
                    match r {
                        Ok(()) if !a::configurado() => self.msg = Some(tr!("Repositório do Glove adicionado.").into()),
                        Ok(()) => {
                            self.atualizado = true;
                            self.msg = Some(tr!("Glove atualizado. Encerre a sessão e entre de novo para usar a versão nova.").into());
                        }
                        Err(e) => self.msg = Some(tr!("Não deu certo: {}", e)),
                    }
                    self.glove = None;
                }
                Err(_) => ui.wake_after(Duration::from_millis(200)),
            }
        }
        if self.buscando {
            ui.wake_after(Duration::from_millis(300));
        }
        let inst = a::instalada();
        ui.label_ex("Glove OS", 13.0, true, glove_ui::paint::draw::rgba(20, 20, 24, 255));
        ui.small(&tr!("Versão instalada: {}", inst));
        let busy = self.job.is_some() || self.buscando;
        match &self.glove {
            None => ui.small(tr!("Consultando o repositório do Glove…")),
            Some(Err(e)) => ui.small(e),
            Some(Ok(n)) if a::mais_nova(&n.versao, &inst) => {
                let t = tr!("Versão nova: {} ({})", n.versao, n.data);
                ui.label_ex(&t, 12.0, true, glove_ui::paint::draw::rgba(30, 90, 170, 255));
                ui.small(tr!("O que há de novo:"));
                let linhas: Vec<String> = n.notas.lines().map(str::to_string).filter(|l| !l.trim().is_empty()).collect();
                let mut sel = None;
                ui.list(&mut sel, &linhas, ui.available_width(), 120.0 + crate::grow() * 0.5);
                ui.space(4.0);
                if !a::configurado() {
                    ui.small(tr!("Este sistema ainda não recebe as atualizações do Glove pelo apt."));
                    ui.set_active(!busy);
                    if ui.button(tr!("Adicionar o Repositório do Glove")) {
                        self.como_admin(a::comando_repositorio(), tr!("Adicionar o repositório do Glove ao apt.").into());
                    }
                    ui.set_active(true);
                } else {
                    ui.horizontal(|ui| {
                        ui.set_active(!busy && !self.atualizado);
                        if ui.default_button(tr!("Atualizar o Glove")) {
                            self.como_admin(a::comando_atualizar(), tr!("Atualizar o Glove.").into());
                        }
                        ui.set_active(true);
                        if self.job.is_some() {
                            ui.spinner(16.0);
                            ui.small(tr!("Atualizando…"));
                        }
                    });
                }
            }
            Some(Ok(_)) => ui.small(tr!("O Glove está em dia.")),
        }
        if let Some(m) = &self.msg {
            ui.paragraph(m);
        }
        if self.atualizado {
            if ui.button(tr!("Encerrar Sessão…")) {
                let _ = Command::new(sibling("glove-sair")).arg("encerrar").spawn();
            }
        }
        ui.space(10.0);
        ui.separator();
        ui.space(8.0);
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if let Some(l) = self.result.lock().unwrap().take() {
            self.list = Some(l);
            self.busy = false;
        }
        if self.busy {
            ui.wake_after(Duration::from_millis(300));
        }
        self.glove(ui);
        let t = tr!("Sistema: {}", os_name());
        ui.label(&t);
        ui.space(6.0);
        let Some((check, install)) = package_manager() else {
            ui.small(tr!("Não achei o gerenciador de pacotes deste sistema (apt, pacman, dnf ou zypper)."));
            return;
        };
        let mut go = false;
        ui.horizontal(|ui| {
            ui.set_active(!self.busy);
            go = ui.default_button(tr!("Verificar Agora"));
            ui.set_active(true);
            if self.busy {
                ui.spinner(16.0);
                ui.small(tr!("Verificando…"));
            }
        });
        if go {
            self.busy = true;
            let r = self.result.clone();
            std::thread::spawn(move || {
                let out = Command::new("sh").arg("-c").arg(check).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
                *r.lock().unwrap() = Some(parse_updates(&out));
            });
        }
        ui.space(8.0);
        match &self.list {
            None => ui.small(tr!("Clique em Verificar Agora para ver as atualizações disponíveis.")),
            Some(l) if l.is_empty() => ui.small(tr!("Nenhuma atualização encontrada. (A lista vem da última consulta do sistema.)")),
            Some(l) => {
                let t = tr!("{} atualizações disponíveis:", l.len());
                ui.label(&t);
                let mut sel = None;
                ui.list(&mut sel, l, ui.available_width(), 180.0 + crate::grow());
                ui.space(6.0);
                if ui.button(tr!("Instalar no Terminal…")) {
                    in_terminal(install);
                }
                ui.small(tr!("O Terminal pede a sua senha de administrador."));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Disco de Inicialização
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct BootEntry {
    pub num: String,
    pub name: String,
}

/// Saída do efibootmgr: (atual, entradas).
pub fn parse_efiboot(text: &str) -> (String, Vec<BootEntry>) {
    let current = text.lines().find_map(|l| l.strip_prefix("BootCurrent: ")).unwrap_or("").trim().to_string();
    let entries = text
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("Boot")?;
            let (num, after) = rest.split_at(rest.find(|c: char| !c.is_ascii_hexdigit())?);
            if num.len() != 4 {
                return None;
            }
            let name = after.trim_start_matches('*').trim().split('\t').next().unwrap_or("").trim().to_string();
            Some(BootEntry { num: num.to_string(), name })
        })
        // Só sistemas e discos (sem "Enter Setup", "UEFI Shell"…).
        .filter(|e| !e.name.contains("Setup") && !e.name.contains("Shell") && !e.name.is_empty())
        .collect();
    (current, entries)
}

#[derive(Default)]
pub struct Inicializacao {
    loaded: bool,
    current: String,
    entries: Vec<BootEntry>,
    sel: Option<usize>,
}

impl Inicializacao {
    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.loaded {
            self.loaded = true;
            if let Ok(o) = Command::new("efibootmgr").output() {
                let (c, e) = parse_efiboot(&String::from_utf8_lossy(&o.stdout));
                self.current = c;
                self.entries = e;
                self.sel = self.entries.iter().position(|e| e.num == self.current);
            }
        }
        if self.entries.is_empty() {
            ui.small(tr!("Não foi possível ler as opções de inicialização (precisa de UEFI e do efibootmgr)."));
            return;
        }
        ui.label(tr!("Escolha o sistema para iniciar o computador:"));
        ui.space(6.0);
        let rows: Vec<String> = self.entries.iter().map(|e| if e.num == self.current { tr!("{} (em uso agora)", e.name) } else { e.name.clone() }).collect();
        ui.list(&mut self.sel, &rows, ui.available_width(), 150.0 + crate::grow());
        ui.space(8.0);
        let chosen = self.sel.and_then(|i| self.entries.get(i)).cloned();
        let (mut once, mut always) = (false, false);
        ui.horizontal(|ui| {
            ui.set_active(chosen.is_some());
            always = ui.button(tr!("Usar Sempre…"));
            once = ui.default_button(tr!("Reiniciar Nele…"));
            ui.set_active(true);
        });
        if let (true, Some(e)) = (once, chosen.clone()) {
            in_terminal(&format!("sudo efibootmgr -n {} && echo 'O computador vai reiniciar em 5 s (Ctrl+C cancela).' && sleep 5 && systemctl reboot", e.num));
        }
        if let (true, Some(e)) = (always, chosen) {
            let rest: Vec<String> = self.entries.iter().filter(|x| x.num != e.num).map(|x| x.num.clone()).collect();
            let order = std::iter::once(e.num.clone()).chain(rest).collect::<Vec<_>>().join(",");
            in_terminal(&format!("sudo efibootmgr -o {order}"));
        }
        ui.space(4.0);
        ui.small(tr!("As mudanças abrem no Terminal, que pede a senha de administrador."));
    }
}

// ---------------------------------------------------------------------------
// Fala
// ---------------------------------------------------------------------------

/// Vozes do speech-dispatcher em português (e a padrão de cada língua).
fn voices() -> Vec<(String, String)> {
    let out = Command::new("spd-say").arg("-L").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut v: Vec<(String, String)> = out
        .lines()
        .skip(1)
        .filter_map(|l| {
            // Colunas alinhadas: nome (pode ter espaços), língua, variante.
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() < 3 {
                return None;
            }
            let lang = parts[parts.len() - 2];
            let name = parts[..parts.len() - 2].join(" ");
            lang.starts_with("pt").then(|| (name, lang.to_string()))
        })
        .collect();
    v.dedup();
    v
}

#[derive(Default)]
pub struct Fala {
    voices: Option<Vec<(String, String)>>,
    sel: Option<usize>,
    rate: f32,
}

impl Fala {
    pub fn ui(&mut self, ui: &mut Ui) {
        if !has("spd-say") {
            ui.small(tr!("A fala precisa do speech-dispatcher (spd-say) instalado."));
            return;
        }
        let voices = self.voices.get_or_insert_with(voices).clone();
        ui.label(tr!("Voz:"));
        let rows: Vec<String> = voices.iter().map(|v| v.0.clone()).collect();
        if self.sel.is_none() && !rows.is_empty() {
            self.sel = Some(0);
        }
        ui.list(&mut self.sel, &rows, ui.available_width(), 150.0 + crate::grow());
        ui.space(8.0);
        ui.form_row(tr!("Velocidade"), 100.0, |ui| {
            let t = Ticks { count: 5, labels: &[(0.0, tr!("Devagar")), (1.0, tr!("Depressa"))], snap: false };
            ui.slider_ticks(&mut self.rate, -60.0, 60.0, 260.0, &t, true);
        });
        ui.space(8.0);
        if ui.default_button(tr!("Ouvir")) {
            let mut c = Command::new("spd-say");
            c.arg("-r").arg((self.rate.round() as i32).to_string());
            if let Some((name, lang)) = self.sel.and_then(|i| voices.get(i)) {
                c.arg("-l").arg(lang);
                if let Some(variant) = name.split('+').nth(1) {
                    c.arg("-y").arg(variant);
                }
            }
            c.arg("Olá! Esta é a voz do Glove. Que bom ter você por aqui.");
            if let Ok(mut ch) = c.spawn() {
                std::thread::spawn(move || ch.wait());
            }
        }
        ui.space(4.0);
        ui.small(tr!("As vozes vêm do speech-dispatcher; apps que leem texto em voz alta usam a mesma."));
    }
}

// ---------------------------------------------------------------------------
// Impressão
// ---------------------------------------------------------------------------

/// Impressoras do `lpstat -p` e a padrão do `lpstat -d`.
pub fn parse_printers(p: &str, d: &str) -> (Vec<(String, bool)>, Option<String>) {
    let list = p
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("printer ").or_else(|| l.strip_prefix("impressora "))?;
            let name = rest.split_whitespace().next()?.to_string();
            Some((name, rest.contains("idle") || rest.contains("ociosa")))
        })
        .collect();
    let def = d.lines().find_map(|l| l.split_once(": ").map(|(_, n)| n.trim().to_string())).filter(|n| !n.is_empty());
    (list, def)
}

#[derive(Default)]
pub struct Impressao {
    loaded: bool,
    printers: Vec<(String, bool)>,
    default: Option<String>,
    sel: Option<usize>,
    message: Option<String>,
}

impl Impressao {
    fn load(&mut self) {
        let run = |a: &str| Command::new("lpstat").arg(a).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let (p, d) = parse_printers(&run("-p"), &run("-d"));
        self.printers = p;
        self.default = d;
        self.loaded = true;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if !has("lpstat") {
            ui.small(tr!("A impressão precisa do CUPS instalado."));
            return;
        }
        if !self.loaded {
            self.load();
        }
        ui.label(tr!("Impressoras:"));
        let rows: Vec<String> = self
            .printers
            .iter()
            .map(|(n, idle)| format!("{n}{}{}", if Some(n) == self.default.as_ref() { tr!(" (padrão)") } else { "" }, if *idle { "" } else { tr!(" — ocupada") }))
            .collect();
        ui.list(&mut self.sel, &rows, ui.available_width(), 150.0 + crate::grow());
        if rows.is_empty() {
            ui.small(tr!("Nenhuma impressora configurada."));
        }
        ui.space(8.0);
        let chosen = self.sel.and_then(|i| self.printers.get(i)).map(|p| p.0.clone());
        let (mut add, mut def, mut queue) = (false, false, false);
        ui.horizontal(|ui| {
            add = ui.button(tr!("Adicionar…"));
            ui.set_active(chosen.is_some());
            def = ui.button(tr!("Tornar Padrão"));
            queue = ui.button(tr!("Ver Fila…"));
            ui.set_active(true);
        });
        let open = |url: &str| {
            if let Ok(mut c) = Command::new("xdg-open").arg(url).spawn() {
                std::thread::spawn(move || c.wait());
            }
        };
        if add {
            open("http://localhost:631/admin");
        }
        if let (true, Some(p)) = (queue, chosen.clone()) {
            open(&format!("http://localhost:631/printers/{p}"));
        }
        if let (true, Some(p)) = (def, chosen) {
            // lpoptions -d sem root vale só para este usuário (~/.cups/lpoptions).
            self.message = Some(match Command::new("lpoptions").args(["-d", &p]).status() {
                Ok(s) if s.success() => tr!("“{}” é a impressora padrão.", p),
                _ => tr!("Não foi possível mudar a impressora padrão.").into(),
            });
            self.load();
        }
        ui.space(4.0);
        ui.small(tr!("Adicionar e a fila abrem a página do CUPS no navegador."));
        if let Some(m) = &self.message {
            let m = m.clone();
            ui.small(&m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atualizacoes() {
        let u = parse_updates("Listing... Done\nfirefox/resolute-updates 140.0-1 amd64 [upgradable from: 139.0-1]\nlibc6/resolute 2.42-2 amd64 [upgradable from: 2.42-1]\n");
        assert_eq!(u, ["firefox 140.0-1", "libc6 2.42-2"]);
        assert_eq!(parse_updates("linux 6.9.1 -> 6.9.2\n"), ["linux 6.9.1"]);
    }

    #[test]
    fn efiboot() {
        let (c, e) = parse_efiboot("BootCurrent: 0003\nTimeout: 2 seconds\nBootOrder: 0003,0001\nBoot0000* Enter Setup\tFvVol\nBoot0001* Internal eMMC\tPciRoot\nBoot0003* Ubuntu\tHD(1)\n");
        assert_eq!(c, "0003");
        assert_eq!(e, [BootEntry { num: "0001".into(), name: "Internal eMMC".into() }, BootEntry { num: "0003".into(), name: "Ubuntu".into() }]);
    }

    #[test]
    fn impressoras() {
        let (l, d) = parse_printers("printer HP_Laser is idle.  enabled since x\nprinter Epson now printing Epson-1.\n", "system default destination: HP_Laser\n");
        assert_eq!(l, [("HP_Laser".to_string(), true), ("Epson".to_string(), false)]);
        assert_eq!(d.as_deref(), Some("HP_Laser"));
    }
}
