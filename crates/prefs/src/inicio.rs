//! Painel Itens de Início: apps que abrem sozinhos ao entrar (autostart do XDG).

use glove_config::tr;
use crate::servicos::{self, Service};
use glove_config::inicio::{self, Item};
use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use glove_ui::{
    paint::{draw::*, Color},
    Rect, Ui,
};

#[derive(Default)]
pub struct Inicio {
    items: Vec<Item>,
    /// Memória dos itens que estão rodando agora (mesma ordem de `items`).
    mem: Vec<Option<u64>>,
    sel: Option<usize>,
    /// Folha "+" aberta: (apps instalados, escolhido).
    picker: Option<(Vec<(String, String)>, Option<usize>)>,
    message: Option<String>,
    /// 0: Aplicativos (autostart); 1: Serviços.
    pub view: usize,
    services: Vec<Service>,
    svc_sel: Option<usize>,
    /// Ligando ou desligando um serviço: (qual, resposta).
    job: Option<(usize, Receiver<Result<(), String>>)>,
    used_h: f32,
}

const ROW_H: f32 = 22.0;
const HEAD_H: f32 = 20.0;

impl Inicio {
    pub fn refresh(&mut self) {
        self.items = inicio::items();
        let procs = process_memory();
        self.mem = self.items.iter().map(|i| item_memory(&i.exec, &procs)).collect();
        if self.sel.is_some_and(|s| s >= self.items.len()) {
            self.sel = None;
        }
        self.services = servicos::installed();
        if self.svc_sel.is_some_and(|s| s >= self.services.len()) {
            self.svc_sel = None;
        }
    }

    pub fn sheet_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Altura que o conteúdo pediu no último quadro (a caixa de abas acompanha).
    pub fn used_height(&self) -> f32 {
        self.used_h
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let top = ui.cursor().1;
        ui.tabs(&mut self.view, &[tr!("Aplicativos"), tr!("Serviços")]);
        ui.space(8.0);
        if self.view == 0 {
            self.apps_ui(ui);
        } else {
            self.services_ui(ui);
        }
        if let Some(m) = &self.message {
            let m = m.clone();
            small_wrapped(ui, &m);
        }
        let used = ui.cursor().1 - top;
        if (used - self.used_h).abs() > 0.5 {
            self.used_h = used;
            ui.redraw = true;
        }
    }

    fn apps_ui(&mut self, ui: &mut Ui) {
        ui.label(tr!("Estes itens abrem sozinhos quando você entra no Glove:"));
        ui.space(6.0);
        let w = ui.available_width();
        // A lista cresce com os itens (a janela acompanha), até 16 linhas.
        let list_h = HEAD_H + 2.0 + self.items.len().clamp(6, 16) as f32 * ROW_H;
        let r = ui.alloc(w, list_h);
        let mem_x = r.x + r.w - 76.0;
        table_frame(ui, r, &[(tr!("Abrir"), r.x + 8.0), (tr!("Item"), r.x + 56.0), (tr!("Comando"), r.x + r.w * 0.45), (tr!("Memória"), mem_x)]);
        let ink = rgba(20, 20, 20, 255);
        if self.items.is_empty() {
            ui.fonts.draw(ui.c, tr!("Nenhum item. Use + para escolher um aplicativo."), r.x + 12.0, r.y + HEAD_H + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        }
        let mut toggle = None;
        for (k, it) in self.items.iter().enumerate().take(((list_h - HEAD_H - 2.0) / ROW_H) as usize) {
            let (rr, sel) = row(ui, r, k, self.sel == Some(k));
            let y = rr.y;
            let t = if sel { Color::WHITE } else { ink };
            let dim = if sel { Color::WHITE } else { rgba(90, 90, 96, 255) };
            checkbox(ui, Rect::new(rr.x + 14.0, y + 4.0, 14.0, 14.0), it.enabled);
            let name = if it.name.is_empty() { it.id.clone() } else { it.name.clone() };
            let name = ui.fonts.ellipsize(&name, 12.0, false, r.w * 0.45 - 60.0);
            ui.fonts.draw(ui.c, &name, rr.x + 55.0, y + 15.0, 12.0, false, t);
            let cmd = ui.fonts.ellipsize(&it.exec, 11.0, false, mem_x - r.x - r.w * 0.45 - 10.0);
            ui.fonts.draw(ui.c, &cmd, r.x + r.w * 0.45, y + 15.0, 11.0, false, dim);
            let mem = self.mem.get(k).copied().flatten().map(servicos::human).unwrap_or_else(|| "—".into());
            ui.fonts.draw(ui.c, &mem, mem_x, y + 15.0, 11.0, false, dim);
            // Um clique só por linha: na coluna "Abrir" liga/desliga; no resto, escolhe.
            if ui.hit(rr).clicked {
                if ui.pointer().0 < rr.x + 44.0 {
                    toggle = Some(k);
                } else {
                    self.sel = Some(k);
                }
            }
        }
        if let Some(k) = toggle {
            let it = self.items[k].clone();
            if let Err(e) = inicio::set_enabled(&it.id, !it.enabled) {
                self.message = Some(tr!("Não foi possível mudar “{}”: {}", it.name, e));
            }
            self.refresh();
        }
        ui.space(4.0);
        let (mut plus, mut minus) = (false, false);
        ui.horizontal(|ui| {
            ui.spacing(4.0);
            plus = ui.small_button("+");
            ui.set_active(self.sel.is_some());
            minus = ui.small_button("−");
            ui.set_active(true);
        });
        if plus {
            self.picker = Some((inicio::installed_apps(), None));
        }
        if minus {
            if let Some(it) = self.sel.and_then(|k| self.items.get(k)).cloned() {
                if let Err(e) = inicio::remove(&it) {
                    self.message = Some(tr!("Não foi possível tirar “{}”: {}", it.name, e));
                }
                self.sel = None;
                self.refresh();
            }
        }
        ui.space(4.0);
        small_wrapped(ui, tr!("Itens do sistema não são apagados: tirar só faz com que não abram. Vale na próxima entrada."));
        small_wrapped(ui, tr!("Memória: quanto o item gasta agora (— quando não está aberto)."));
    }

    fn services_ui(&mut self, ui: &mut Ui) {
        // Resposta do pkexec/systemctl.
        if let Some((k, rx)) = &self.job {
            match rx.try_recv() {
                Ok(r) => {
                    let name = self.services.get(*k).map(|s| servicos::known()[s.known].name).unwrap_or("");
                    self.message = r.err().map(|e| tr!("Não foi possível mudar “{}”: {}", name, e));
                    self.job = None;
                    self.refresh();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui.wake_after(std::time::Duration::from_millis(150)),
                Err(_) => self.job = None,
            }
        }
        ui.label(tr!("Serviços que quase sempre podem ser desligados:"));
        ui.space(6.0);
        let w = ui.available_width();
        let list_h = HEAD_H + 2.0 + self.services.len().clamp(6, 20) as f32 * ROW_H;
        let r = ui.alloc(w, list_h);
        let mem_x = r.x + r.w - 76.0;
        let state_x = mem_x - 90.0;
        table_frame(ui, r, &[(tr!("Ligado"), r.x + 8.0), (tr!("Serviço"), r.x + 56.0), (tr!("Agora"), state_x), (tr!("Memória"), mem_x)]);
        let ink = rgba(20, 20, 20, 255);
        if self.services.is_empty() {
            ui.fonts.draw(ui.c, tr!("Nenhum dos serviços conhecidos está instalado."), r.x + 12.0, r.y + HEAD_H + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        }
        let busy = self.job.is_some();
        let mut toggle = None;
        for (k, s) in self.services.iter().enumerate().take(((list_h - HEAD_H - 2.0) / ROW_H) as usize) {
            let (rr, sel) = row(ui, r, k, self.svc_sel == Some(k));
            let y = rr.y;
            let t = if sel { Color::WHITE } else { ink };
            let dim = if sel { Color::WHITE } else { rgba(90, 90, 96, 255) };
            let kn = &servicos::known()[s.known];
            checkbox(ui, Rect::new(rr.x + 14.0, y + 4.0, 14.0, 14.0), s.on);
            let name = ui.fonts.ellipsize(kn.name, 12.0, false, state_x - rr.x - 64.0);
            ui.fonts.draw(ui.c, &name, rr.x + 55.0, y + 15.0, 12.0, false, t);
            let state = if busy && self.job.as_ref().is_some_and(|j| j.0 == k) {
                tr!("mudando…")
            } else if s.running {
                tr!("rodando")
            } else {
                tr!("parado")
            };
            ui.fonts.draw(ui.c, state, state_x, y + 15.0, 11.0, false, dim);
            let mem = s.memory.filter(|_| s.running).map(servicos::human).unwrap_or_else(|| "—".into());
            ui.fonts.draw(ui.c, &mem, mem_x, y + 15.0, 11.0, false, dim);
            if ui.hit(rr).clicked {
                self.svc_sel = Some(k);
                if ui.pointer().0 < rr.x + 44.0 && !busy {
                    toggle = Some(k);
                }
            }
        }
        if let Some(k) = toggle {
            let s = self.services[k].clone();
            self.message = None;
            self.job = Some((k, servicos::set(&s, !s.on)));
            ui.wake_after(std::time::Duration::from_millis(150));
        }
        ui.space(6.0);
        let desc_top = ui.cursor().1;
        match self.svc_sel.and_then(|k| self.services.get(k)) {
            Some(s) => {
                let kn = &servicos::known()[s.known];
                small_wrapped(ui, kn.desc);
                let units = s.units.join(", ");
                small_wrapped(ui, &tr!("Unidades: {}", units));
            }
            None => {
                small_wrapped(ui, tr!("Escolha um serviço para ver o que ele faz."))
            }
        }
        // Espaço fixo para a descrição: a caixa não pula ao escolher outro serviço.
        let used = ui.cursor().1 - desc_top;
        ui.space((52.0 - used).max(0.0));
        ui.space(4.0);
        small_wrapped(ui, tr!("Desligar para agora e nas próximas vezes que o computador ligar. Os do sistema pedem a senha de administrador."));
    }

    /// Folha com os apps instalados para escolher um.
    pub fn picker_sheet(&mut self, ui: &mut Ui) {
        let Some((apps, mut sel)) = self.picker.take() else { return };
        let (sw, sh) = (420.0, 330.0);
        let x = ((ui.c.w - sw) / 2.0).round();
        let y = 78.0;
        ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
        ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
        ui.fonts.draw(ui.c, tr!("Escolha um aplicativo para abrir ao entrar:"), x + 20.0, y + 26.0, 13.0, true, rgba(20, 20, 20, 255));
        let rows: Vec<String> = apps.iter().map(|a| a.0.clone()).collect();
        let (mut add, mut cancel) = (false, false);
        ui.area(Rect::new(x + 20.0, y + 40.0, sw - 40.0, sh - 50.0), |ui| {
            ui.list(&mut sel, &rows, sw - 40.0, 230.0);
            ui.space(8.0);
            ui.horizontal(|ui| {
                ui.space(sw - 40.0 - 200.0);
                cancel = ui.button(tr!("Cancelar"));
                ui.set_active(sel.is_some());
                add |= ui.default_button(tr!("Adicionar"));
                ui.set_active(true);
            });
        });
        let esc = ui.keys().iter().any(|k| k.key == glove_ui::ui::Key::Escape);
        if cancel || esc {
            return;
        }
        if add {
            if let Some((name, exec)) = sel.and_then(|i| apps.get(i)) {
                if let Err(e) = inicio::add(name, exec) {
                    self.message = Some(tr!("Não foi possível pôr “{}”: {}", name, e));
                }
                self.refresh();
                self.sel = self.items.iter().position(|i| i.name == *name);
            }
            return;
        }
        self.picker = Some((apps, sel));
    }
}

/// Texto pequeno quebrado na largura disponível.
fn small_wrapped(ui: &mut Ui, t: &str) {
    let max = ui.available_width();
    let mut line = String::new();
    for word in t.split_whitespace() {
        let test = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if ui.fonts.measure(&test, 11.0, false) > max && !line.is_empty() {
            ui.small(&line);
            line = word.to_string();
        } else {
            line = test;
        }
    }
    if !line.is_empty() {
        ui.small(&line);
    }
}

/// Caixa com cabeçalho de colunas (título, x).
fn table_frame(ui: &mut Ui, r: Rect, cols: &[(&str, f32)]) {
    ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
    ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
    ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, HEAD_H, &vgrad(r.y, r.y + HEAD_H, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(222, 222, 226, 255))]));
    ui.c.hline(r.x + 1.0, r.y + HEAD_H, r.w - 2.0, rgba(170, 170, 176, 255));
    for (t, x) in cols {
        ui.fonts.draw(ui.c, t, *x, r.y + 14.0, 11.0, false, rgba(20, 20, 20, 255));
    }
}

/// Fundo da linha `k` (listrada ou escolhida); devolve o retângulo dela.
fn row(ui: &mut Ui, r: Rect, k: usize, sel: bool) -> (Rect, bool) {
    let rr = Rect::new(r.x + 1.0, r.y + HEAD_H + 1.0 + k as f32 * ROW_H, r.w - 2.0, ROW_H);
    if sel {
        ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(ui.theme.accent));
    } else if k % 2 == 1 {
        ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(rgba(237, 243, 254, 255)));
    }
    (rr, sel)
}

fn checkbox(ui: &mut Ui, cb: Rect, on: bool) {
    ui.c.fill(rrect(cb.x, cb.y, cb.w, cb.h, 3.0), &solid(if on { ui.theme.accent } else { Color::WHITE }));
    ui.c.stroke(rrect(cb.x + 0.5, cb.y + 0.5, cb.w - 1.0, cb.h - 1.0, 3.0), &solid(rgba(90, 90, 96, 255)), 1.0);
    if on {
        ui.fonts.draw(ui.c, "✓", cb.x + 2.0, cb.y + 12.0, 12.0, true, Color::WHITE);
    }
}

/// Memória (RSS) dos processos desta conta, somada pelo nome do programa
/// (argv[0] e, para scripts, argv[1]).
fn process_memory() -> HashMap<String, u64> {
    let uid = unsafe { libc::getuid() };
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
    let mut out: HashMap<String, u64> = HashMap::new();
    for e in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let name = e.file_name();
        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let dir = e.path();
        use std::os::unix::fs::MetadataExt;
        if std::fs::metadata(&dir).map(|m| m.uid()).ok() != Some(uid) {
            continue;
        }
        let Some(rss) = std::fs::read_to_string(dir.join("statm")).ok().and_then(|t| t.split_whitespace().nth(1)?.parse::<u64>().ok()) else { continue };
        let Ok(cmd) = std::fs::read(dir.join("cmdline")) else { continue };
        let mut args = cmd.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned());
        let mut names = Vec::new();
        if let Some(a0) = args.next() {
            names.push(base(&a0));
            if let Some(a1) = args.next().filter(|a| !a.starts_with('-')) {
                names.push(base(&a1));
            }
        }
        names.dedup();
        for n in names {
            *out.entry(n).or_default() += rss * page;
        }
    }
    out
}

fn base(p: &str) -> String {
    p.rsplit('/').next().unwrap_or(p).to_string()
}

/// Memória do item, pelo programa do comando (sem "env", "sh -c"…).
fn item_memory(exec: &str, procs: &HashMap<String, u64>) -> Option<u64> {
    let prog = exec.split_whitespace().find(|w| !w.contains('=') && *w != "env")?;
    let b = base(prog);
    if matches!(b.as_str(), "sh" | "bash" | "dash") {
        return None;
    }
    procs.get(&b).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memoria_do_item() {
        let mut p = HashMap::new();
        p.insert("anydesk".to_string(), 50u64 << 20);
        p.insert("flameshot".to_string(), 30u64 << 20);
        assert_eq!(item_memory("/usr/bin/anydesk --tray", &p), Some(50 << 20));
        assert_eq!(item_memory("env QT_QPA_PLATFORM=wayland flameshot", &p), Some(30 << 20));
        assert_eq!(item_memory("sh -c 'flameshot'", &p), None);
        assert_eq!(item_memory("remmina -i", &p), None);
    }
}
