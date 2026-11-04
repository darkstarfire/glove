//! Monitor: uso de CPU, memória e lista de processos (lidos de /proc).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, tiny_skia::PathBuilder},
    run, AlertKind, App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};

const HISTORY: usize = 60;
const TOP_H: f32 = 120.0;

#[derive(Clone)]
struct Proc {
    pid: i32,
    ppid: i32,
    name: String,
    user: String,
    cpu: f32,
    mem_kb: u64,
}

/// Soma de "jiffies" da CPU: (ocupado, total).
fn cpu_times() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    let v: Vec<u64> = s.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    let total: u64 = v.iter().sum();
    let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
    (total - idle, total)
}

/// (total, disponível) em KB.
fn mem_info() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let get = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0);
    (get("MemTotal:"), get("MemAvailable:"))
}

fn users() -> HashMap<u32, String> {
    std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            Some((f.get(2)?.parse().ok()?, f.first()?.to_string()))
        })
        .collect()
}

fn human_kb(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    if mb >= 1024.0 { format!("{:.1} GB", mb / 1024.0).replace('.', ",") } else { format!("{mb:.1} MB").replace('.', ",") }
}

struct Monitor {
    procs: Vec<Proc>,
    /// Tempo de CPU de cada processo na leitura anterior.
    last_ticks: HashMap<i32, u64>,
    last_total: u64,
    last_cpu: (u64, u64),
    history: Vec<f32>,
    cpu_now: f32,
    mem: (u64, u64),
    selected: Option<i32>,
    table: TableState,
    /// Mostrar: 0 = todos os processos, 1 = os meus, 2 = em hierarquia.
    show: usize,
    /// Processos com os filhos recolhidos (na hierarquia).
    collapsed: HashSet<i32>,
    /// Usuário da sessão (para "Meus processos").
    me: String,
    users: HashMap<u32, String>,
    ncpu: f32,
    message: Option<String>,
    /// Pedido de forçar encerramento esperando confirmação: (pid, nome).
    confirm_kill: Option<(i32, String)>,
}

impl Monitor {
    fn new() -> Self {
        let ncpu = std::fs::read_to_string("/proc/stat").map(|s| s.lines().filter(|l| l.starts_with("cpu") && !l.starts_with("cpu ")).count()).unwrap_or(1).max(1) as f32;
        let mut m = Self {
            procs: Vec::new(),
            last_ticks: HashMap::new(),
            last_total: 0,
            last_cpu: cpu_times(),
            history: Vec::new(),
            cpu_now: 0.0,
            mem: mem_info(),
            selected: None,
            // Começa ordenado por % CPU, do maior para o menor.
            table: TableState::sorted_by(3, false),
            show: 0,
            collapsed: HashSet::new(),
            me: String::new(),
            users: users(),
            ncpu,
            message: None,
            confirm_kill: None,
        };
        let uid = unsafe { libc::getuid() };
        m.me = m.users.get(&uid).cloned().unwrap_or_else(|| uid.to_string());
        m.refresh();
        m
    }

    fn refresh(&mut self) {
        let (busy, total) = cpu_times();
        let (b0, t0) = self.last_cpu;
        let dt = total.saturating_sub(t0).max(1);
        self.cpu_now = busy.saturating_sub(b0) as f32 / dt as f32;
        self.last_cpu = (busy, total);
        self.history.push(self.cpu_now);
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
        self.mem = mem_info();

        let mut procs = Vec::new();
        let mut ticks = HashMap::new();
        let dtotal = total.saturating_sub(self.last_total).max(1) as f32;
        for e in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
            let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else { continue };
            let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
            // O nome fica entre parênteses e pode conter espaços.
            let (Some(a), Some(b)) = (stat.find('('), stat.rfind(')')) else { continue };
            let name = stat[a + 1..b].to_string();
            let rest: Vec<&str> = stat[b + 2..].split_whitespace().collect();
            let ppid = rest.get(1).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let t = rest.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) + rest.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let rss_pages: u64 = rest.get(21).and_then(|v| v.parse().ok()).unwrap_or(0);
            let prev = self.last_ticks.get(&pid).copied().unwrap_or(t);
            let cpu = if self.last_total == 0 { 0.0 } else { (t.saturating_sub(prev)) as f32 / dtotal * self.ncpu * 100.0 };
            ticks.insert(pid, t);
            // Dono da pasta do processo = usuário (sem ler outro arquivo).
            let uid = {
                use std::os::unix::fs::MetadataExt;
                e.metadata().map(|m| m.uid()).unwrap_or(0)
            };
            let user = self.users.get(&uid).cloned().unwrap_or_else(|| uid.to_string());
            procs.push(Proc { pid, ppid, name, user, cpu, mem_kb: rss_pages * 4 });
        }
        self.procs = procs;
        self.last_ticks = ticks;
        self.last_total = total;
    }

    fn kill(&mut self, sig: i32) {
        let Some(pid) = self.selected else { return };
        self.kill_pid(pid, sig);
    }

    fn kill_pid(&mut self, pid: i32, sig: i32) {
        if unsafe { libc::kill(pid, sig) } != 0 {
            self.message = Some(format!("Não foi possível encerrar o processo {pid}: {}", std::io::Error::last_os_error()));
        } else {
            self.message = None;
        }
    }

    fn header(&self, ui: &mut Ui) {
        let w = ui.c.w;
        // Gráfico da CPU
        let g = Rect::new(14.0, 12.0, (w * 0.6).min(w - 260.0).max(200.0), TOP_H - 24.0);
        ui.c.fill(rrect(g.x, g.y, g.w, g.h, 4.0), &solid(rgba(12, 22, 16, 255)));
        for i in 1..4 {
            ui.c.hline(g.x, g.y + g.h * i as f32 / 4.0, g.w, rgba(40, 80, 50, 255));
        }
        if self.history.len() > 1 {
            let n = HISTORY as f32 - 1.0;
            let pt = |i: usize, v: f32| (g.x + g.w - (self.history.len() - 1 - i) as f32 / n * g.w, g.y + g.h - v.clamp(0.0, 1.0) * (g.h - 4.0) - 2.0);
            let mut area = PathBuilder::new();
            let mut line = PathBuilder::new();
            let (x0, _) = pt(0, 0.0);
            area.move_to(x0, g.y + g.h);
            for (i, v) in self.history.iter().enumerate() {
                let (x, y) = pt(i, *v);
                area.line_to(x, y);
                if i == 0 { line.move_to(x, y) } else { line.line_to(x, y) }
            }
            area.line_to(g.x + g.w, g.y + g.h);
            area.close();
            ui.c.fill(area.finish(), &vgrad(g.y, g.y + g.h, &[(0.0, rgba(80, 255, 110, 120)), (1.0, rgba(80, 255, 110, 10))]));
            ui.c.stroke(line.finish(), &solid(rgba(110, 255, 130, 255)), 1.5);
        }
        let t = format!("CPU {:.0}%", self.cpu_now * 100.0);
        ui.fonts.draw(ui.c, &t, g.x + 8.0, g.y + 16.0, 12.0, true, rgba(150, 255, 160, 255));

        // Memória
        let (total, avail) = self.mem;
        let used = total.saturating_sub(avail);
        let mx = g.x + g.w + 20.0;
        let mw = w - mx - 14.0;
        ui.fonts.draw(ui.c, "Memória", mx, 28.0, 13.0, true, rgba(20, 20, 20, 255));
        let bar = Rect::new(mx, 40.0, mw, 18.0);
        ui.c.fill(rrect(bar.x, bar.y, bar.w, bar.h, 9.0), &vgrad(bar.y, bar.y + bar.h, &[(0.0, rgba(160, 160, 166, 255)), (1.0, rgba(225, 225, 230, 255))]));
        let frac = if total > 0 { used as f32 / total as f32 } else { 0.0 };
        if frac > 0.02 {
            gel_rect(ui.c, bar.x, bar.y, (bar.w * frac).max(bar.h), bar.h, 9.0, if frac > 0.85 { rgba(230, 70, 60, 255) } else { ui.theme.accent });
        }
        let l1 = format!("Em uso: {}", human_kb(used));
        let l2 = format!("Livre: {}  ·  Total: {}", human_kb(avail), human_kb(total));
        ui.fonts.draw(ui.c, &l1, mx, 76.0, 12.0, false, rgba(30, 30, 30, 255));
        ui.fonts.draw(ui.c, &l2, mx, 94.0, 11.0, false, rgba(90, 90, 90, 255));
    }

    /// Compara dois processos pela coluna da ordenação.
    fn compare(&self, a: &Proc, b: &Proc) -> Ordering {
        let (col, asc) = self.table.sort.unwrap_or((3, false));
        let o = match col {
            0 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            1 => a.pid.cmp(&b.pid),
            2 => a.user.cmp(&b.user),
            3 => a.cpu.partial_cmp(&b.cpu).unwrap_or(Ordering::Equal),
            _ => a.mem_kb.cmp(&b.mem_kb),
        };
        let o = o.then(a.pid.cmp(&b.pid));
        if asc { o } else { o.reverse() }
    }

    /// Linhas visíveis, já ordenadas: (pid, linha da tabela).
    fn rows(&self) -> Vec<(i32, TableRow)> {
        let cells = |p: &Proc| vec![p.name.clone(), p.pid.to_string(), p.user.clone(), format!("{:.1}", p.cpu).replace('.', ","), human_kb(p.mem_kb)];
        let mut out = Vec::new();
        if self.show == 2 {
            // Hierarquia: cada processo embaixo do pai, irmãos na ordem escolhida.
            let pids: HashSet<i32> = self.procs.iter().map(|p| p.pid).collect();
            let mut kids: HashMap<i32, Vec<&Proc>> = HashMap::new();
            for p in &self.procs {
                let parent = if pids.contains(&p.ppid) && p.ppid != p.pid { p.ppid } else { 0 };
                kids.entry(parent).or_default().push(p);
            }
            for v in kids.values_mut() {
                v.sort_by(|a, b| self.compare(a, b));
            }
            let mut stack: Vec<(&Proc, u16)> = kids.get(&0).map(|v| v.iter().rev().map(|p| (*p, 0)).collect()).unwrap_or_default();
            while let Some((p, depth)) = stack.pop() {
                let children = kids.get(&p.pid);
                let open = !self.collapsed.contains(&p.pid);
                out.push((p.pid, TableRow { cells: cells(p), icon: None, depth, children: children.map(|_| open) }));
                if let (Some(ch), true) = (children, open) {
                    stack.extend(ch.iter().rev().map(|c| (*c, depth + 1)));
                }
            }
        } else {
            let mut v: Vec<&Proc> = self.procs.iter().filter(|p| self.show == 0 || p.user == self.me).collect();
            v.sort_by(|a, b| self.compare(a, b));
            out.extend(v.into_iter().map(|p| (p.pid, TableRow { cells: cells(p), ..TableRow::default() })));
        }
        out
    }

    fn table(&mut self, ui: &mut Ui, area: Rect) {
        let cols = [
            Column::new("Processo", 0.42),
            Column::new("PID", 0.1).right(),
            Column::new("Usuário", 0.18),
            Column::new("% CPU", 0.12).right(),
            Column::new("Memória", 0.18).right(),
        ];
        let list = self.rows();
        // Setas mudam a seleção; ← e → recolhem e abrem na hierarquia.
        let cur = self.selected.and_then(|s| list.iter().position(|(pid, _)| *pid == s));
        let keys = if self.confirm_kill.is_some() { Vec::new() } else { ui.keys().to_vec() };
        for k in keys {
            let n = list.len();
            match k.combo.as_str() {
                "Up" | "Down" if n > 0 => {
                    let i = match (cur, k.combo == "Up") {
                        (Some(i), true) => i.saturating_sub(1),
                        (Some(i), false) => (i + 1).min(n - 1),
                        (None, _) => 0,
                    };
                    self.selected = Some(list[i].0);
                    self.table.reveal(i);
                }
                "Left" | "Right" if self.show == 2 => {
                    if let Some(i) = cur {
                        if list[i].1.children.is_some() {
                            if k.combo == "Left" { self.collapsed.insert(list[i].0); } else { self.collapsed.remove(&list[i].0); }
                        }
                    }
                }
                _ => {}
            }
        }
        let sel = self.selected.and_then(|s| list.iter().position(|(pid, _)| *pid == s));
        let rows: Vec<TableRow> = list.iter().map(|(_, r)| r.clone()).collect();
        let r = ui.table(area, &cols, &rows, sel, &mut self.table, &TableStyle::default());
        if let Some(i) = r.pressed {
            self.selected = Some(list[i].0);
        }
        if let Some(i) = r.toggled {
            let pid = list[i].0;
            if !self.collapsed.remove(&pid) {
                self.collapsed.insert(pid);
            }
            ui.redraw = true;
        }
    }
}

impl App for Monitor {
    fn ui(&mut self, ui: &mut Ui) {
        let asking = self.confirm_kill.is_some();
        ui.set_enabled(!asking);
        let mut acts: Vec<String> = if asking { Vec::new() } else { ui.menu_actions().to_vec() };
        for k in ui.keys().iter().filter(|_| !asking) {
            match k.combo.as_str() {
                "Ctrl+Q" | "Delete" => acts.push("term".into()),
                "Ctrl+W" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "term" => self.kill(libc::SIGTERM),
                "kill" => {
                    if let Some(pid) = self.selected {
                        let name = self.procs.iter().find(|p| p.pid == pid).map(|p| p.name.clone()).unwrap_or_else(|| pid.to_string());
                        self.confirm_kill = Some((pid, name));
                    }
                }
                "close" => ui.close = true,
                _ => {}
            }
        }
        let (w, h) = (ui.c.w, ui.c.h);
        self.header(ui);
        let area = Rect::new(0.0, TOP_H, w, h - TOP_H - 34.0);
        self.table(ui, area);

        // Rodapé: total de processos e botão de encerrar.
        let fy = h - 34.0;
        ui.c.fill_rect(0.0, fy, w, 34.0, &vgrad(fy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(216, 216, 220, 255))]));
        ui.c.hline(0.0, fy, w, rgba(150, 150, 156, 255));
        let shows = ["Todos os processos".to_string(), "Meus processos".to_string(), "Em hierarquia".to_string()];
        ui.area(Rect::new(14.0, fy + 6.0, 170.0, 22.0), |ui| {
            if ui.popup(&mut self.show, &shows, 170.0) {
                self.table.scroll = 0.0;
            }
        });
        let info = self.message.clone().unwrap_or_else(|| format!("{} processos", self.procs.len()));
        ui.fonts.draw(ui.c, &info, 198.0, fy + 21.0, 11.0, false, rgba(60, 60, 60, 255));
        let has = self.selected.is_some();
        if has && ui.gel_button(Rect::new(w - 170.0, fy + 6.0, 150.0, 22.0), "Encerrar processo", rgba(240, 240, 243, 255), 12.0, false) {
            self.kill(libc::SIGTERM);
        }

        if let Some((pid, name)) = self.confirm_kill.clone() {
            let title = format!("Forçar o encerramento de “{name}”?");
            let text = "O processo é interrompido na hora, sem chance de salvar. Alterações não salvas serão perdidas.";
            match ui.alert(AlertKind::Caution, &title, text, &["Cancelar", "Forçar encerramento"]) {
                Some(1) => {
                    self.confirm_kill = None;
                    self.kill_pid(pid, libc::SIGKILL);
                    ui.redraw = true;
                }
                Some(_) => {
                    self.confirm_kill = None;
                    ui.redraw = true;
                }
                None => {}
            }
        }
    }

    fn tick(&self) -> Option<Duration> {
        Some(Duration::from_secs(2))
    }

    fn on_tick(&mut self) {
        self.refresh();
    }

    fn menus(&self) -> Vec<Menu> {
        let has = self.selected.is_some();
        vec![
            Menu::new(
                "Processo",
                vec![
                    MenuItem::new("term", "Encerrar processo", "Ctrl+Q").enabled(has),
                    MenuItem::new("kill", "Forçar encerramento", "").enabled(has),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar janela", "Ctrl+W"),
                ],
            ),
        ]
    }
}

fn main() {
    let opts = WindowOptions { title: "Monitor".into(), app_id: "glove.Monitor".into(), size: (640, 480), fixed: false };
    if let Err(e) = run(opts, Monitor::new()) {
        eprintln!("glove-monitor: {e}");
    }
}
