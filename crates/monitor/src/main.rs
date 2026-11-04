//! Monitor: uso de CPU, memória e lista de processos (lidos de /proc).

use std::collections::HashMap;
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, tiny_skia::PathBuilder, Color},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const HISTORY: usize = 60;
const ROW_H: f32 = 19.0;
const TOP_H: f32 = 120.0;

#[derive(Clone)]
struct Proc {
    pid: i32,
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
    scroll: f32,
    users: HashMap<u32, String>,
    ncpu: f32,
    message: Option<String>,
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
            scroll: 0.0,
            users: users(),
            ncpu,
            message: None,
        };
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
            procs.push(Proc { pid, name, user, cpu, mem_kb: rss_pages * 4 });
        }
        procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal).then(b.mem_kb.cmp(&a.mem_kb)));
        self.procs = procs;
        self.last_ticks = ticks;
        self.last_total = total;
    }

    fn kill(&mut self, sig: i32) {
        let Some(pid) = self.selected else { return };
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

    fn table(&mut self, ui: &mut Ui, area: Rect) {
        let header = 20.0;
        let cols = [("Processo", 0.0), ("PID", 0.42), ("Usuário", 0.54), ("% CPU", 0.72), ("Memória", 0.84)];
        ui.c.fill_rect(area.x, area.y, area.w, header, &vgrad(area.y, area.y + header, &[(0.0, rgba(250, 250, 252, 255)), (1.0, rgba(214, 216, 222, 255))]));
        ui.c.hline(area.x, area.y + header - 1.0, area.w, rgba(150, 150, 156, 255));
        for (i, (t, f)) in cols.iter().enumerate() {
            let x = area.x + area.w * f;
            if i > 0 {
                ui.c.vline(x, area.y + 3.0, header - 6.0, rgba(160, 160, 166, 255));
            }
            ui.fonts.draw(ui.c, t, x + 8.0, area.y + 14.0, 11.0, false, rgba(30, 30, 30, 255));
        }
        let content = self.procs.len() as f32 * ROW_H;
        let body = ui.scroll_area(Rect::new(area.x, area.y + header, area.w, area.h - header), &mut self.scroll, content, ROW_H);
        let first = (self.scroll / ROW_H).floor() as usize;
        let count = (body.h / ROW_H).ceil() as usize + 1;
        for (i, p) in self.procs.iter().enumerate().skip(first).take(count) {
            let y = body.y + i as f32 * ROW_H - self.scroll;
            let (top, bot) = (y.max(body.y), (y + ROW_H).min(body.y + body.h));
            if bot > top && ui.hit(Rect::new(body.x, top, body.w, bot - top)).pressed {
                self.selected = Some(p.pid);
            }
        }
        let procs = &self.procs;
        let sel = self.selected;
        let off = self.scroll;
        let accent = ui.theme.accent;
        ui.clipped(5, body, |c, fonts| {
            c.clear(Color::WHITE);
            for (i, p) in procs.iter().enumerate().skip(first).take(count) {
                let y = i as f32 * ROW_H - off;
                let on = sel == Some(p.pid);
                if on {
                    c.fill_rect(0.0, y, body.w, ROW_H, &vgrad(y, y + ROW_H, &[(0.0, lighten(accent, 0.15)), (1.0, accent)]));
                } else if i % 2 == 1 {
                    c.fill_rect(0.0, y, body.w, ROW_H, &solid(rgba(237, 243, 254, 255)));
                }
                let ink = if on { Color::WHITE } else { rgba(20, 20, 20, 255) };
                let cells = [p.name.clone(), p.pid.to_string(), p.user.clone(), format!("{:.1}", p.cpu).replace('.', ","), human_kb(p.mem_kb)];
                for (j, (_, f)) in cols.iter().enumerate() {
                    let x = body.w * f + 8.0;
                    let next = cols.get(j + 1).map(|c| body.w * c.1).unwrap_or(body.w);
                    let t = fonts.ellipsize(&cells[j], 12.0, false, next - x - 6.0);
                    fonts.draw(c, &t, x, y + 14.0, 12.0, false, ink);
                }
            }
        });
    }
}

impl App for Monitor {
    fn ui(&mut self, ui: &mut Ui) {
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+Q" | "Delete" => acts.push("term".into()),
                "Ctrl+W" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "term" => self.kill(libc::SIGTERM),
                "kill" => self.kill(libc::SIGKILL),
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
        let info = self.message.clone().unwrap_or_else(|| format!("{} processos", self.procs.len()));
        ui.fonts.draw(ui.c, &info, 14.0, fy + 21.0, 11.0, false, rgba(60, 60, 60, 255));
        let has = self.selected.is_some();
        if has && ui.gel_button(Rect::new(w - 170.0, fy + 6.0, 150.0, 22.0), "Encerrar processo", rgba(240, 240, 243, 255), 12.0, false) {
            self.kill(libc::SIGTERM);
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
