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

fn has(prog: &str) -> bool {
    Command::new("sh").arg("-c").arg(format!("command -v {prog}")).output().is_ok_and(|o| o.status.success())
}

// ---------------------------------------------------------------------------
// Atualização de Software
// ---------------------------------------------------------------------------

/// (verificar, instalar) para o gerenciador de pacotes do sistema.
/// (Descoberto uma vez: cada consulta roda um shell, e o painel é desenhado a cada quadro.)
fn package_manager() -> Option<(&'static str, &'static str)> {
    static PM: std::sync::OnceLock<Option<(&'static str, &'static str)>> = std::sync::OnceLock::new();
    *PM.get_or_init(find_package_manager)
}

/// Atualizar o sistema inteiro sem perguntas (roda como administrador, pela janela de
/// senha): None no apt, que tem o comando próprio (com o andamento).
fn upgrade_all() -> Option<&'static str> {
    match package_manager()?.0 {
        "checkupdates" => Some("pacman -Syu --noconfirm"),
        c if c.starts_with("dnf") => Some("dnf -y upgrade --refresh"),
        c if c.starts_with("zypper") => Some("zypper -n update"),
        _ => None,
    }
}

fn find_package_manager() -> Option<(&'static str, &'static str)> {
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
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(read_os_name).clone()
}

fn read_os_name() -> String {
    std::fs::read_to_string("/etc/os-release").ok().and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string())).unwrap_or_else(|| "Linux".into())
}

/// O que o painel está fazendo como administrador.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// `apt update` (o Verificar Agora).
    Verificar,
    /// Só o Glove (no apt, o glove e o glove-os; nas outras distribuições, o pacote dele).
    Glove,
    /// O sistema inteiro (`apt full-upgrade`).
    Tudo,
    /// Acrescentar o repositório do Glove (fora do Glove OS).
    Repositorio,
}

/// Andamento do apt pela linha do `APT::Status-Fd`: (fração 0 a 1, o que está fazendo).
/// "dlstatus:2:40.5:Baixando x" (baixar, a primeira metade) e "pmstatus:x:30.0:Instalando x"
/// (instalar, a segunda).
pub fn apt_progress(line: &str) -> Option<(f32, String)> {
    let mut it = line.splitn(4, ':');
    let kind = it.next()?;
    let _ = it.next()?;
    let pct: f32 = it.next()?.trim().parse().ok()?;
    let text = it.next().unwrap_or("").trim().to_string();
    match kind {
        "dlstatus" => Some((pct / 200.0, text)),
        "pmstatus" => Some((0.5 + pct / 200.0, text)),
        _ => None,
    }
}

#[derive(Default)]
pub struct Atualizacao {
    result: Arc<Mutex<Option<Vec<String>>>>,
    list: Option<Vec<String>>,
    lendo: bool,
    /// O Glove: a versão nova publicada (buscada ao abrir o painel).
    nova: Arc<Mutex<Option<Result<crate::atualizar::Nova, String>>>>,
    glove: Option<Result<crate::atualizar::Nova, String>>,
    buscando: bool,
    /// O pedido de senha (a operação, o comando e a folha).
    pub prompt: Option<(Op, Vec<String>, crate::admin::Prompt)>,
    job: Option<(Op, std::sync::mpsc::Receiver<crate::admin::Step>)>,
    /// Barra de progresso: fração e o que está fazendo.
    progress: f32,
    fase: String,
    msg: Option<String>,
    /// O Glove mudou: oferece encerrar a sessão.
    atualizado: bool,
    /// A versão instalada e se o repositório do Glove está configurado: perguntados ao
    /// sistema ao abrir e depois de cada operação, não a cada quadro (rolar o painel
    /// rodava o dpkg dezenas de vezes por segundo e ficava lento).
    inst: Option<(String, bool)>,
}

impl Atualizacao {
    fn buscar_glove(&mut self) {
        self.inst = None;
        self.buscando = true;
        let r = self.nova.clone();
        std::thread::spawn(move || *r.lock().unwrap() = Some(crate::atualizar::buscar()));
    }

    /// A lista do que o apt tem para atualizar (o que ele já sabe; sem senha).
    fn ler_lista(&mut self) {
        let Some((check, _)) = package_manager() else { return };
        self.lendo = true;
        let r = self.result.clone();
        std::thread::spawn(move || {
            let out = Command::new("sh").arg("-c").arg(check).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            *r.lock().unwrap() = Some(parse_updates(&out));
        });
    }

    /// O comando da operação. `versao`: a versão nova do Glove (fora do apt, o pacote é
    /// buscado por ela). None se não há o que rodar (versão inesperada).
    fn comando(op: Op, versao: &str) -> Option<Vec<String>> {
        use crate::atualizar as a;
        let apt = "DEBIAN_FRONTEND=noninteractive apt-get -o APT::Status-Fd=1 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold -y";
        let sh = |c: String| vec!["sh".to_string(), "-c".to_string(), c];
        Some(match op {
            Op::Verificar => sh("apt-get update".into()),
            Op::Glove if a::pacote() != a::Pacote::Apt => sh(a::script_atualizar(a::pacote(), versao, &a::repo_url())?),
            Op::Glove => sh(format!(
                "apt-get update -o Dir::Etc::sourcelist={} -o Dir::Etc::sourceparts=- -o APT::Get::List-Cleanup=0 && {apt} install --only-upgrade glove glove-os",
                crate::atualizar::SOURCES
            )),
            Op::Tudo => match upgrade_all() {
                Some(c) => sh(c.into()),
                None => sh(format!("{apt} full-upgrade")),
            },
            Op::Repositorio => crate::atualizar::comando_repositorio(),
        })
    }

    /// Roda como administrador: direto se o sudo não pede senha, senão pela folha.
    fn como_admin(&mut self, op: Op) {
        self.msg = None;
        let versao = self.glove.as_ref().and_then(|r| r.as_ref().ok()).map(|n| n.versao.clone()).unwrap_or_default();
        let Some(args) = Self::comando(op, &versao) else {
            self.msg = Some(tr!("Não deu certo: {}", tr!("A resposta do repositório veio num formato inesperado.")));
            return;
        };
        let what = match op {
            Op::Verificar => tr!("Procurar atualizações."),
            Op::Glove => tr!("Atualizar o Glove."),
            Op::Tudo => tr!("Instalar as atualizações do sistema."),
            Op::Repositorio => tr!("Adicionar o repositório do Glove."),
        };
        match crate::admin::sudo_state() {
            crate::admin::Sudo::Free => self.start(op, args, None),
            crate::admin::Sudo::No => self.msg = Some(tr!("Só uma conta de administrador pode atualizar o sistema.").into()),
            crate::admin::Sudo::Password => self.prompt = Some((op, args, crate::admin::Prompt::new(what.into()))),
        }
    }

    /// O comando informa o andamento (a barra)? Só o apt; os outros mostram o giratório.
    fn com_andamento(op: Op) -> bool {
        match op {
            Op::Verificar | Op::Repositorio => false,
            Op::Glove => crate::atualizar::pacote() == crate::atualizar::Pacote::Apt,
            Op::Tudo => upgrade_all().is_none(),
        }
    }

    fn start(&mut self, op: Op, args: Vec<String>, password: Option<String>) {
        self.progress = 0.0;
        self.fase = match op {
            Op::Verificar => tr!("Procurando atualizações…").into(),
            Op::Repositorio => tr!("Adicionando o repositório…").into(),
            Op::Glove if !Self::com_andamento(op) => tr!("Baixando e instalando o Glove…").into(),
            _ => tr!("Preparando…").into(),
        };
        self.job = Some((op, crate::admin::run_stream(args, password)));
    }

    pub fn prompt_sheet(&mut self, ui: &mut Ui) {
        if let Some((op, args, mut p)) = self.prompt.take() {
            match p.sheet(ui) {
                crate::admin::Answer::Waiting => self.prompt = Some((op, args, p)),
                crate::admin::Answer::Cancel => {}
                crate::admin::Answer::Password(pw) => self.start(op, args, Some(pw)),
            }
        }
    }

    /// O andamento do comando em curso; no fim, a mensagem e o que muda no painel.
    fn acompanhar(&mut self, ui: &mut Ui) {
        let Some((op, rx)) = self.job.take() else { return };
        let mut done = None;
        while let Ok(step) = rx.try_recv() {
            match step {
                crate::admin::Step::Line(l) => {
                    if let Some((f, t)) = apt_progress(&l) {
                        self.progress = self.progress.max(f.min(1.0));
                        if !t.is_empty() {
                            self.fase = t;
                        }
                    }
                }
                crate::admin::Step::Done(r) => done = Some(r),
            }
        }
        let Some(r) = done else {
            self.job = Some((op, rx));
            ui.wake_after(Duration::from_millis(150));
            return;
        };
        match (op, r) {
            (_, Err(e)) if e.contains("lock") || e.contains("Could not get lock") || e.contains("bloque") => {
                self.msg = Some(tr!("Outro programa está instalando agora (talvez a atualização automática do sistema). Tente de novo em alguns minutos.").into());
            }
            (_, Err(e)) => self.msg = Some(tr!("Não deu certo: {}", e)),
            (Op::Verificar, Ok(())) => self.msg = None,
            (Op::Repositorio, Ok(())) => self.msg = Some(tr!("Repositório do Glove adicionado.").into()),
            (Op::Glove | Op::Tudo, Ok(())) => {
                let glove_mudou = op == Op::Glove || self.list.as_ref().is_some_and(|l| l.iter().any(|p| p.starts_with("glove ") || p.starts_with("glove-os ")));
                self.atualizado |= glove_mudou;
                self.msg = Some(if glove_mudou {
                    tr!("Atualizado. Encerre a sessão e entre de novo para usar o Glove novo.").into()
                } else {
                    tr!("Sistema atualizado.").into()
                });
            }
        }
        // Depois de qualquer coisa, relê o que há para atualizar.
        self.glove = None;
        self.buscar_glove();
        self.ler_lista();
    }

    /// A seção do Glove: versão instalada, a nova e o que há de novo.
    fn glove(&mut self, ui: &mut Ui) {
        use crate::atualizar as a;
        if let Some(r) = self.nova.lock().unwrap().take() {
            self.glove = Some(r);
            self.buscando = false;
        }
        let (inst, configurado) = self.inst.get_or_insert_with(|| (a::instalada(), a::configurado())).clone();
        ui.label_ex("Glove OS", 13.0, true, glove_ui::paint::draw::rgba(20, 20, 24, 255));
        ui.small(&tr!("Versão instalada: {}", inst));
        let busy = self.job.is_some();
        if a::copia_local_sobrando() {
            ui.paragraph(tr!("Atenção: há uma cópia antiga do Glove instalada pelo código em /usr/local, e a sessão a usa no lugar do pacote. Para removê-la, na pasta do código: ./glove-setup-tool.sh desinstalar (remove só a cópia de /usr/local) e reinicie o computador."));
            ui.space(4.0);
        }
        match &self.glove {
            None => ui.small(tr!("Consultando o repositório do Glove…")),
            Some(Err(e)) => ui.small(e),
            Some(Ok(n)) if a::mais_nova(&n.versao, &inst) => {
                let t = tr!("Versão nova: {} ({})", n.versao, n.data);
                ui.label_ex(&t, 12.0, true, glove_ui::paint::draw::rgba(30, 90, 170, 255));
                ui.small(tr!("O que há de novo:"));
                let linhas: Vec<String> = n.notas.lines().map(str::to_string).filter(|l| !l.trim().is_empty()).collect();
                let mut sel = None;
                ui.list(&mut sel, &linhas, ui.available_width(), 110.0 + crate::grow() * 0.5);
                ui.space(4.0);
                if a::pacote() == a::Pacote::Nenhum {
                    // Sem pacote não há o que o gerenciador atualizar.
                    ui.paragraph(tr!("O Glove foi instalado pelo código neste computador. Para atualizar, na pasta do código: git pull e ./glove-setup-tool.sh instalar"));
                } else if !configurado {
                    ui.small(tr!("Este sistema ainda não recebe as atualizações do Glove pelo gerenciador de pacotes."));
                    ui.set_active(!busy);
                    if ui.button(tr!("Adicionar o Repositório do Glove")) {
                        self.como_admin(Op::Repositorio);
                    }
                    ui.set_active(true);
                } else {
                    ui.set_active(!busy && !self.atualizado);
                    if ui.default_button(tr!("Atualizar o Glove")) {
                        self.como_admin(Op::Glove);
                    }
                    ui.set_active(true);
                }
            }
            Some(Ok(_)) => ui.small(tr!("O Glove está em dia.")),
        }
        ui.space(10.0);
        ui.separator();
        ui.space(8.0);
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        // Ao abrir: o que já se sabe (o Glove pelo repositório dele; o sistema pelo apt).
        if self.glove.is_none() && !self.buscando {
            self.buscar_glove();
        }
        if self.list.is_none() && !self.lendo {
            self.ler_lista();
        }
        if let Some(l) = self.result.lock().unwrap().take() {
            self.list = Some(l);
            self.lendo = false;
        }
        if self.buscando || self.lendo {
            ui.wake_after(Duration::from_millis(300));
        }
        self.acompanhar(ui);
        self.glove(ui);
        let t = tr!("Sistema: {}", os_name());
        ui.label(&t);
        ui.space(6.0);
        let Some((_, install)) = package_manager() else {
            ui.small(tr!("Não achei o gerenciador de pacotes deste sistema (apt, pacman, dnf ou zypper)."));
            return;
        };
        let apt = package_manager().is_some_and(|p| p.0.starts_with("apt"));
        let busy = self.job.is_some();
        let mut verificar = false;
        ui.horizontal(|ui| {
            ui.set_active(!busy);
            verificar = ui.button(tr!("Verificar Agora"));
            ui.set_active(true);
        });
        if verificar {
            if apt {
                self.como_admin(Op::Verificar);
            } else {
                self.glove = None;
                self.buscar_glove();
                self.ler_lista();
            }
        }
        // Em andamento: a barra e o que está sendo feito.
        if busy {
            ui.space(6.0);
            if self.job.as_ref().is_some_and(|j| !Self::com_andamento(j.0)) {
                ui.horizontal(|ui| {
                    ui.spinner(16.0);
                    ui.small(&self.fase);
                });
            } else {
                ui.progress(Some(self.progress), ui.available_width().min(360.0));
                let t = format!("{:.0}%  {}", self.progress * 100.0, self.fase);
                ui.small(&t);
            }
        }
        if let Some(m) = &self.msg {
            ui.space(4.0);
            ui.paragraph(m);
        }
        if self.atualizado && ui.button(tr!("Encerrar Sessão…")) {
            let _ = Command::new(sibling("glove-sair")).arg("encerrar").spawn();
        }
        ui.space(8.0);
        match &self.list {
            None => ui.small(tr!("Procurando atualizações…")),
            Some(l) if l.is_empty() => ui.small(tr!("Nenhuma atualização do sistema. (Verificar Agora consulta os repositórios de novo.)")),
            Some(l) => {
                let t = tr!("{} atualizações disponíveis:", l.len());
                ui.label(&t);
                let mut sel = None;
                let l = l.clone();
                ui.list(&mut sel, &l, ui.available_width(), 140.0 + crate::grow() * 0.5);
                ui.space(6.0);
                // A senha vai na janela de senha do sistema (no apt, no dnf, no pacman…).
                let _ = (apt, install);
                ui.set_active(!busy);
                if ui.default_button(tr!("Instalar Tudo")) {
                    self.como_admin(Op::Tudo);
                }
                ui.set_active(true);
            }
        }
    }
}

#[cfg(test)]
mod apt_tests {
    #[test]
    fn andamento_do_apt() {
        assert_eq!(super::apt_progress("dlstatus:3:40:Baixando firefox"), Some((0.2, "Baixando firefox".into())));
        let (f, t) = super::apt_progress("pmstatus:glove:50.0:Instalando glove").unwrap();
        assert!((f - 0.75).abs() < 1e-6 && t == "Instalando glove");
        assert_eq!(super::apt_progress("Lendo listas de pacotes..."), None);
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
    /// O pedido de senha aberto: o comando, se reinicia depois, e a janela de senha.
    pub prompt: Option<(Vec<String>, bool, crate::admin::Prompt)>,
    /// O efibootmgr rodando como administrador (reinicia depois?).
    job: Option<(bool, std::sync::mpsc::Receiver<Result<(), String>>)>,
    msg: Option<String>,
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
            let what = tr!("Reiniciar o computador em “{}”.", e.name);
            self.como_admin(vec!["efibootmgr".into(), "-n".into(), e.num.clone()], true, what);
        }
        if let (true, Some(e)) = (always, chosen) {
            let rest: Vec<String> = self.entries.iter().filter(|x| x.num != e.num).map(|x| x.num.clone()).collect();
            let order = std::iter::once(e.num.clone()).chain(rest).collect::<Vec<_>>().join(",");
            let what = tr!("Iniciar sempre em “{}”.", e.name);
            self.como_admin(vec!["efibootmgr".into(), "-o".into(), order], false, what);
        }
        // O efibootmgr terminou: a mensagem e, no "Reiniciar Nele", reinicia.
        if let Some((reboot, rx)) = &self.job {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    if *reboot {
                        self.msg = Some(tr!("Reiniciando…").into());
                        let _ = Command::new("systemctl").arg("reboot").spawn();
                    } else {
                        self.msg = Some(tr!("Pronto: o computador passa a iniciar nesse sistema.").into());
                        self.loaded = false;
                    }
                    self.job = None;
                }
                Ok(Err(e)) => {
                    self.msg = Some(tr!("Não deu certo: {}", e));
                    self.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(Duration::from_millis(200)),
                Err(_) => self.job = None,
            }
        }
        ui.space(4.0);
        match &self.msg {
            Some(m) => ui.small(m),
            None => ui.small(tr!("As mudanças pedem a senha de administrador.")),
        }
    }

    /// Roda o efibootmgr como administrador: direto se o sudo não pede senha, senão pela
    /// janela de senha do sistema.
    fn como_admin(&mut self, args: Vec<String>, reboot: bool, what: String) {
        self.msg = None;
        match crate::admin::sudo_state() {
            crate::admin::Sudo::Free => self.job = Some((reboot, crate::admin::run(args, None))),
            crate::admin::Sudo::No => self.msg = Some(tr!("Só uma conta de administrador pode mudar o disco de inicialização.").into()),
            crate::admin::Sudo::Password => self.prompt = Some((args, reboot, crate::admin::Prompt::new(what))),
        }
    }

    /// A janela de senha aberta (ou a folha, sem o glove-senha).
    pub fn prompt_sheet(&mut self, ui: &mut Ui) {
        if let Some((args, reboot, mut p)) = self.prompt.take() {
            match p.sheet(ui) {
                crate::admin::Answer::Waiting => self.prompt = Some((args, reboot, p)),
                crate::admin::Answer::Cancel => {}
                crate::admin::Answer::Password(pw) => self.job = Some((reboot, crate::admin::run(args, Some(pw)))),
            }
        }
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
