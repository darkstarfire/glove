//! Monitor: uso de CPU, memória e lista de processos (lidos de /proc).

use glove_config::tr;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, tiny_skia::PathBuilder},
    run, AlertKind, App, Column, Menu, MenuItem, Rect, TableRow, TableState, TableStyle, Ui, WindowOptions,
};

const HISTORY: usize = 60;
/// Barra de ferramentas e o painel de abas embaixo.
const TOOL_H: f32 = 50.0;
const BOTTOM_H: f32 = 178.0;

/// Detalhe da CPU desde a leitura anterior: (usuário, sistema, ocioso), de 0 a 1.
fn cpu_split(prev: &[u64], cur: &[u64]) -> (f32, f32, f32) {
    let d = |i: usize| cur.get(i).copied().unwrap_or(0).saturating_sub(prev.get(i).copied().unwrap_or(0)) as f32;
    let total: f32 = (0..cur.len()).map(d).sum::<f32>().max(1.0);
    let user = d(0) + d(1);
    let system = d(2) + d(5) + d(6);
    let idle = d(3) + d(4);
    (user / total, system / total, idle / total)
}

fn cpu_fields() -> Vec<u64> {
    let s = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    s.lines().next().unwrap_or("").split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect()
}

/// Memória em KB: (total, livre, em cache, disponível, swap total, swap livre).
fn mem_detail() -> (u64, u64, u64, u64, u64, u64) {
    let s = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let get = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0u64);
    (get("MemTotal:"), get("MemFree:"), get("Cached:") + get("Buffers:"), get("MemAvailable:"), get("SwapTotal:"), get("SwapFree:"))
}

/// Discos inteiros (sem partições) do /proc/diskstats: (bytes lidos, bytes gravados, leituras, gravações).
pub fn disk_counters(text: &str) -> (u64, u64, u64, u64) {
    let mut t = (0, 0, 0, 0);
    for l in text.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let name = f[2];
        let whole = (name.starts_with("sd") || name.starts_with("vd") || name.starts_with("hd")) && !name.ends_with(|c: char| c.is_ascii_digit())
            || (name.starts_with("nvme") || name.starts_with("mmcblk")) && !name.contains('p');
        if !whole || name.starts_with("mmcblk") && name.contains("boot") {
            continue;
        }
        let n = |i: usize| f[i].parse::<u64>().unwrap_or(0);
        t.0 += n(5) * 512;
        t.1 += n(9) * 512;
        t.2 += n(3);
        t.3 += n(7);
    }
    t
}

/// Bytes recebidos e enviados (todas as interfaces menos a "lo").
pub fn net_counters(text: &str) -> (u64, u64) {
    let mut t = (0, 0);
    for l in text.lines().skip(2) {
        let Some((name, rest)) = l.split_once(':') else { continue };
        if name.trim() == "lo" {
            continue;
        }
        let f: Vec<u64> = rest.split_whitespace().filter_map(|v| v.parse().ok()).collect();
        if f.len() >= 9 {
            t.0 += f[0];
            t.1 += f[8];
        }
    }
    t
}

/// (total, livre) em bytes do sistema de arquivos de `path`.
fn disk_usage(path: &str) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some((st.f_blocks as u64 * st.f_frsize as u64, st.f_bavail as u64 * st.f_frsize as u64))
}

fn human_bytes(b: f64) -> String {
    let (v, u) = if b >= 1e9 { (b / 1e9, "GB") } else if b >= 1e6 { (b / 1e6, "MB") } else if b >= 1e3 { (b / 1e3, "KB") } else { (b, "B") };
    format!("{v:.1} {u}").replace('.', glove_config::virgula())
}

#[derive(Clone)]
struct Proc {
    pid: i32,
    ppid: i32,
    name: String,
    user: String,
    cpu: f32,
    mem_kb: u64,
    threads: u64,
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
    if mb >= 1024.0 { format!("{:.1} GB", mb / 1024.0).replace('.', glove_config::virgula()) } else { format!("{mb:.1} MB").replace('.', glove_config::virgula()) }
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
    /// Filtro da barra (nome ou PID).
    filter: String,
    /// Aba do painel de baixo.
    tab: usize,
    cpu_prev: Vec<u64>,
    cpu_split: (f32, f32, f32),
    /// Leituras anteriores e quando foram: disco e rede.
    disk_prev: (u64, u64, u64, u64),
    net_prev: (u64, u64),
    stamp: std::time::Instant,
    /// Taxas por segundo: (lido, gravado, leituras, gravações) e (recebido, enviado).
    disk_rate: (f32, f32, f32, f32),
    net_rate: (f32, f32),
    disk_hist: Vec<(f32, f32)>,
    net_hist: Vec<(f32, f32)>,
    disk_total: (u64, u64),
    net_total: (u64, u64),
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
            filter: String::new(),
            tab: 0,
            cpu_prev: cpu_fields(),
            cpu_split: (0.0, 0.0, 1.0),
            disk_prev: disk_counters(&std::fs::read_to_string("/proc/diskstats").unwrap_or_default()),
            net_prev: net_counters(&std::fs::read_to_string("/proc/net/dev").unwrap_or_default()),
            stamp: std::time::Instant::now(),
            disk_rate: (0.0, 0.0, 0.0, 0.0),
            net_rate: (0.0, 0.0),
            disk_hist: Vec::new(),
            net_hist: Vec::new(),
            disk_total: (0, 0),
            net_total: (0, 0),
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
        let cur = cpu_fields();
        self.cpu_split = cpu_split(&self.cpu_prev, &cur);
        self.cpu_prev = cur;
        // Disco e rede: diferença desde a última leitura, por segundo.
        let dt = self.stamp.elapsed().as_secs_f32().max(0.1);
        self.stamp = std::time::Instant::now();
        let d = disk_counters(&std::fs::read_to_string("/proc/diskstats").unwrap_or_default());
        let p = self.disk_prev;
        let rate = |a: u64, b: u64| a.saturating_sub(b) as f32 / dt;
        self.disk_rate = (rate(d.0, p.0), rate(d.1, p.1), rate(d.2, p.2), rate(d.3, p.3));
        self.disk_prev = d;
        self.disk_total = (d.0, d.1);
        let n = net_counters(&std::fs::read_to_string("/proc/net/dev").unwrap_or_default());
        self.net_rate = (rate(n.0, self.net_prev.0), rate(n.1, self.net_prev.1));
        self.net_prev = n;
        self.net_total = n;
        self.disk_hist.push((self.disk_rate.0, self.disk_rate.1));
        self.net_hist.push(self.net_rate);
        for h in [&mut self.disk_hist, &mut self.net_hist] {
            if h.len() > HISTORY {
                h.remove(0);
            }
        }

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
            let threads: u64 = rest.get(17).and_then(|v| v.parse().ok()).unwrap_or(1);
            let prev = self.last_ticks.get(&pid).copied().unwrap_or(t);
            let cpu = if self.last_total == 0 { 0.0 } else { (t.saturating_sub(prev)) as f32 / dtotal * self.ncpu * 100.0 };
            ticks.insert(pid, t);
            // Dono da pasta do processo = usuário (sem ler outro arquivo).
            let uid = {
                use std::os::unix::fs::MetadataExt;
                e.metadata().map(|m| m.uid()).unwrap_or(0)
            };
            let user = self.users.get(&uid).cloned().unwrap_or_else(|| uid.to_string());
            procs.push(Proc { pid, ppid, name, user, cpu, mem_kb: rss_pages * 4, threads });
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
            self.message = Some(tr!("Não foi possível encerrar o processo {}: {}", pid, std::io::Error::last_os_error()));
        } else {
            self.message = None;
        }
    }

    /// Gráfico verde da era: área e linha de uma ou duas séries (0 a 1).
    fn graph(ui: &mut Ui, g: Rect, series: &[(&[f32], (u8, u8, u8))]) {
        ui.c.fill(rrect(g.x, g.y, g.w, g.h, 4.0), &solid(rgba(12, 22, 16, 255)));
        for i in 1..4 {
            ui.c.hline(g.x, g.y + g.h * i as f32 / 4.0, g.w, rgba(40, 80, 50, 255));
        }
        for (data, (r, gg, b)) in series {
            if data.len() < 2 {
                continue;
            }
            let n = HISTORY as f32 - 1.0;
            let pt = |i: usize, v: f32| (g.x + g.w - (data.len() - 1 - i) as f32 / n * g.w, g.y + g.h - v.clamp(0.0, 1.0) * (g.h - 4.0) - 2.0);
            let mut area = PathBuilder::new();
            let mut line = PathBuilder::new();
            let (x0, _) = pt(0, 0.0);
            area.move_to(x0, g.y + g.h);
            for (i, v) in data.iter().enumerate() {
                let (x, y) = pt(i, *v);
                area.line_to(x, y);
                if i == 0 { line.move_to(x, y) } else { line.line_to(x, y) }
            }
            area.line_to(g.x + g.w, g.y + g.h);
            area.close();
            ui.c.fill(area.finish(), &vgrad(g.y, g.y + g.h, &[(0.0, rgba(*r, *gg, *b, 110)), (1.0, rgba(*r, *gg, *b, 10))]));
            ui.c.stroke(line.finish(), &solid(rgba(*r, *gg, *b, 255)), 1.5);
        }
    }

    /// Pizza (Memória e Uso do Disco): fatias (fração, cor).
    fn pie(ui: &mut Ui, cx: f32, cy: f32, r: f32, slices: &[(f32, glove_ui::paint::Color)]) {
        let mut a0 = -std::f32::consts::FRAC_PI_2;
        ui.c.fill(circle(cx + 1.0, cy + 2.0, r), &solid(rgba(0, 0, 0, 50)));
        for (f, c) in slices {
            let a1 = a0 + f.clamp(0.0, 1.0) * std::f32::consts::TAU;
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy);
            let steps = ((a1 - a0) / 0.1).ceil().max(1.0) as usize;
            for i in 0..=steps {
                let a = a0 + (a1 - a0) * i as f32 / steps as f32;
                pb.line_to(cx + r * a.cos(), cy + r * a.sin());
            }
            pb.close();
            ui.c.fill(pb.finish(), &solid(*c));
            a0 = a1;
        }
        ui.c.fill(circle(cx - r * 0.25, cy - r * 0.3, r * 0.6), &radial(cx - r * 0.25, cy - r * 0.3, r * 0.6, &[(0.0, rgba(255, 255, 255, 90)), (1.0, rgba(255, 255, 255, 0))]));
        ui.c.stroke(circle(cx, cy, r), &solid(rgba(0, 0, 0, 90)), 1.0);
    }

    /// Lista "rótulo: valor" com o quadradinho da cor (legenda).
    fn legend(ui: &mut Ui, x: f32, y: f32, rows: &[(&str, String, Option<glove_ui::paint::Color>)]) {
        for (i, (k, v, c)) in rows.iter().enumerate() {
            let yy = y + i as f32 * 18.0;
            let mut tx = x;
            if let Some(c) = c {
                ui.c.fill(rrect(x, yy - 9.0, 10.0, 10.0, 2.0), &solid(*c));
                tx += 16.0;
            }
            let t = format!("{k}:");
            ui.fonts.draw(ui.c, &t, tx, yy, 11.0, false, rgba(60, 60, 66, 255));
            let kw = ui.text_width(&t, 11.0, false);
            ui.fonts.draw(ui.c, v, tx + kw + 6.0, yy, 11.0, true, rgba(20, 20, 24, 255));
        }
    }

    /// Painel de abas embaixo: CPU, Memória, Disco, Uso do Disco e Rede.
    fn panels(&mut self, ui: &mut Ui, area: Rect) {
        let mut tab = self.tab;
        let shown = tab;
        ui.area(area, |ui| {
            ui.tab_box(&mut tab, &["CPU", tr!("Memória"), tr!("Atividade do Disco"), tr!("Uso do Disco"), tr!("Rede")], area.h - 8.0, |ui| {
                let (x0, y0) = ui.cursor();
                let w = ui.available_width();
                let gh = area.h - 62.0;
                let g = Rect::new(x0 + w - (w * 0.45).max(200.0), y0, (w * 0.45).max(200.0), gh);
                let green = rgba(70, 200, 80, 255);
                let red = rgba(220, 70, 60, 255);
                let blue = rgba(70, 130, 230, 255);
                match shown {
                    0 => {
                        let (u, sy, id) = self.cpu_split;
                        let threads: u64 = self.procs.iter().map(|p| p.threads).sum();
                        Self::legend(ui, x0, y0 + 12.0, &[
                            (tr!("Usuário"), format!("{:.1} %", u * 100.0).replace('.', glove_config::virgula()), Some(green)),
                            (tr!("Sistema"), format!("{:.1} %", sy * 100.0).replace('.', glove_config::virgula()), Some(red)),
                            (tr!("Ocioso"), format!("{:.1} %", id * 100.0).replace('.', glove_config::virgula()), Some(rgba(30, 30, 34, 255))),
                            (tr!("Processos"), self.procs.len().to_string(), None),
                            ("Threads", threads.to_string(), None),
                        ]);
                        Self::graph(ui, g, &[(&self.history, (110, 255, 130))]);
                        let t = format!("CPU {:.0}%", self.cpu_now * 100.0);
                        ui.fonts.draw(ui.c, &t, g.x + 8.0, g.y + 16.0, 12.0, true, rgba(150, 255, 160, 255));
                    }
                    1 => {
                        let (total, free, cache, avail, swt, swf) = mem_detail();
                        let used = total.saturating_sub(avail);
                        let cached = cache.min(total.saturating_sub(used).saturating_sub(free));
                        let t = total.max(1) as f32;
                        Self::legend(ui, x0, y0 + 12.0, &[
                            (tr!("Em uso"), human_kb(used), Some(red)),
                            (tr!("Em cache"), human_kb(cached), Some(blue)),
                            (tr!("Livre"), human_kb(free), Some(green)),
                            ("Total", human_kb(total), None),
                            ("Swap em uso", tr!("{} de {}", human_kb(swt.saturating_sub(swf)), human_kb(swt)), None),
                        ]);
                        let r = (gh / 2.0 - 4.0).min(60.0);
                        Self::pie(ui, g.x + g.w / 2.0, g.y + gh / 2.0, r, &[(used as f32 / t, red), (cached as f32 / t, blue), (free as f32 / t, green)]);
                    }
                    2 => {
                        let (rb, wb, ro, wo) = self.disk_rate;
                        Self::legend(ui, x0, y0 + 12.0, &[
                            (tr!("Leitura"), format!("{}/s", human_bytes(rb as f64)), Some(green)),
                            (tr!("Gravação"), format!("{}/s", human_bytes(wb as f64)), Some(red)),
                            (tr!("Leituras"), format!("{:.0} por segundo", ro), None),
                            (tr!("Gravações"), format!("{:.0} por segundo", wo), None),
                            (tr!("Lido / gravado"), format!("{} / {}", human_bytes(self.disk_total.0 as f64), human_bytes(self.disk_total.1 as f64)), None),
                        ]);
                        let peak = self.disk_hist.iter().map(|(a, b)| a.max(*b)).fold(1.0f32, f32::max);
                        let r: Vec<f32> = self.disk_hist.iter().map(|(a, _)| a / peak).collect();
                        let wr: Vec<f32> = self.disk_hist.iter().map(|(_, b)| b / peak).collect();
                        Self::graph(ui, g, &[(&r, (110, 255, 130)), (&wr, (255, 110, 100))]);
                    }
                    3 => {
                        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
                        let (total, free) = disk_usage(&home).or_else(|| disk_usage("/")).unwrap_or((1, 0));
                        let used = total.saturating_sub(free);
                        Self::legend(ui, x0, y0 + 12.0, &[
                            (tr!("Em uso"), human_bytes(used as f64), Some(blue)),
                            (tr!("Livre"), human_bytes(free as f64), Some(green)),
                            ("Total", human_bytes(total as f64), None),
                        ]);
                        let r = (gh / 2.0 - 4.0).min(60.0);
                        let t = total.max(1) as f32;
                        Self::pie(ui, g.x + g.w / 2.0, g.y + gh / 2.0, r, &[(used as f32 / t, blue), (free as f32 / t, green)]);
                    }
                    _ => {
                        let (rx, tx) = self.net_rate;
                        Self::legend(ui, x0, y0 + 12.0, &[
                            (tr!("Recebendo"), format!("{}/s", human_bytes(rx as f64)), Some(green)),
                            (tr!("Enviando"), format!("{}/s", human_bytes(tx as f64)), Some(red)),
                            (tr!("Recebido"), human_bytes(self.net_total.0 as f64), None),
                            (tr!("Enviado"), human_bytes(self.net_total.1 as f64), None),
                        ]);
                        let peak = self.net_hist.iter().map(|(a, b)| a.max(*b)).fold(1.0f32, f32::max);
                        let r: Vec<f32> = self.net_hist.iter().map(|(a, _)| a / peak).collect();
                        let t: Vec<f32> = self.net_hist.iter().map(|(_, b)| b / peak).collect();
                        Self::graph(ui, g, &[(&r, (110, 255, 130)), (&t, (255, 110, 100))]);
                    }
                }
                ui.alloc(w, gh);
            });
        });
        self.tab = tab;
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
        let cells = |p: &Proc| vec![p.name.clone(), p.pid.to_string(), p.user.clone(), format!("{:.1}", p.cpu).replace('.', glove_config::virgula()), human_kb(p.mem_kb)];
        let mut out = Vec::new();
        if self.show == 2 && self.filter.trim().is_empty() {
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
                out.push((p.pid, TableRow { cells: cells(p), icon: None, depth, children: children.map(|_| open), tint: None }));
                if let (Some(ch), true) = (children, open) {
                    stack.extend(ch.iter().rev().map(|c| (*c, depth + 1)));
                }
            }
        } else {
            let f = self.filter.trim().to_lowercase();
            let mut v: Vec<&Proc> = self.procs.iter().filter(|p| (self.show == 0 || p.user == self.me) && (f.is_empty() || p.name.to_lowercase().contains(&f) || p.pid.to_string() == f)).collect();
            v.sort_by(|a, b| self.compare(a, b));
            out.extend(v.into_iter().map(|p| (p.pid, TableRow { cells: cells(p), ..TableRow::default() })));
        }
        out
    }

    fn table(&mut self, ui: &mut Ui, area: Rect) {
        let cols = [
            Column::new(tr!("Processo"), 0.42),
            Column::new("PID", 0.1).right(),
            Column::new(tr!("Usuário"), 0.18),
            Column::new("% CPU", 0.12).right(),
            Column::new(tr!("Memória"), 0.18).right(),
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
                "startup" | "services" => {
                    // As Preferências mostram o que abre sozinho e quanto gasta.
                    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-prefs"))).filter(|p| p.is_file());
                    let arg = if a == "startup" { "inicio" } else { "servicos" };
                    let _ = std::process::Command::new(exe.unwrap_or_else(|| "glove-prefs".into())).arg(arg).spawn();
                }
                _ => {}
            }
        }
        let (w, h) = (ui.c.w, ui.c.h);
        // Barra de ferramentas: Encerrar Processo, Mostrar e Filtro (como no 10.4).
        ui.c.fill_rect(0.0, 0.0, w, TOOL_H, &vgrad(0.0, TOOL_H, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(204, 204, 208, 255))]));
        ui.c.hline(0.0, TOOL_H - 1.0, w, rgba(140, 140, 146, 255));
        let has = self.selected.is_some();
        let lab = |ui: &mut Ui, t: &str, cx: f32| {
            let tw = ui.text_width(t, 10.0, false);
            ui.fonts.draw(ui.c, t, cx - tw / 2.0, TOOL_H - 6.0, 10.0, false, rgba(40, 40, 44, 255));
        };
        ui.area(Rect::new(14.0, 6.0, 120.0, 26.0), |ui| {
            ui.set_active(has && !asking);
            if ui.small_button(tr!("Encerrar")) {
                self.kill(libc::SIGTERM);
            }
            ui.set_active(!asking);
        });
        lab(ui, tr!("Encerrar Processo"), 14.0 + 40.0);
        let shows = [tr!("Todos os processos").to_string(), tr!("Meus processos").to_string(), tr!("Em hierarquia").to_string()];
        let mx = (w / 2.0 - 85.0).max(150.0);
        ui.area(Rect::new(mx, 6.0, 170.0, 22.0), |ui| {
            if ui.popup(&mut self.show, &shows, 170.0) {
                self.table.scroll = 0.0;
            }
        });
        lab(ui, tr!("Mostrar"), mx + 85.0);
        let fw = 150.0;
        ui.search_field_in(Rect::new(w - fw - 14.0, 8.0, fw, glove_ui::SEARCH_H), &mut self.filter, tr!("Nome ou PID"));
        lab(ui, tr!("Filtro"), w - fw / 2.0 - 14.0);

        let area = Rect::new(0.0, TOOL_H, w, h - TOOL_H - BOTTOM_H - 18.0);
        self.table(ui, area);
        // Linha de status entre a tabela e o painel.
        let sy = area.y + area.h;
        ui.c.fill_rect(0.0, sy, w, 18.0, &solid(rgba(232, 232, 236, 255)));
        let info = self.message.clone().unwrap_or_else(|| tr!("{} processos", self.procs.len()));
        ui.fonts.draw(ui.c, &info, 12.0, sy + 13.0, 10.0, false, rgba(70, 70, 76, 255));
        self.panels(ui, Rect::new(10.0, sy + 20.0, w - 20.0, BOTTOM_H - 4.0));

        if let Some((pid, name)) = self.confirm_kill.clone() {
            let title = tr!("Forçar o encerramento de “{}”?", name);
            let text = tr!("O processo é interrompido na hora, sem chance de salvar. Alterações não salvas serão perdidas.");
            match ui.alert(AlertKind::Caution, &title, text, &[tr!("Cancelar"), tr!("Forçar encerramento")]) {
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
                tr!("Processo"),
                vec![
                    MenuItem::new("term", tr!("Encerrar processo"), "Ctrl+Q").enabled(has),
                    MenuItem::new("kill", tr!("Forçar encerramento"), "").enabled(has),
                    MenuItem::Separator,
                    MenuItem::new("startup", tr!("Itens de Início…"), ""),
                    MenuItem::new("services", tr!("Serviços…"), ""),
                    MenuItem::Separator,
                    MenuItem::new("close", tr!("Fechar janela"), "Ctrl+W"),
                ],
            ),
        ]
    }
}

pub fn main() {
    let opts = WindowOptions { title: tr!("Monitor").into(), app_id: "glove.Monitor".into(), size: (720, 600), fixed: false, min_size: None };
    if let Err(e) = run(opts, Monitor::new()) {
        eprintln!("glove-monitor: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contadores() {
        let d = "   8       0 sda 100 0 2000 0 50 0 4000 0 0 0 0\n   8       1 sda1 90 0 1800 0 40 0 3000 0 0 0 0\n 179       0 mmcblk0 10 0 8 0 1 0 16 0 0 0 0\n 179       1 mmcblk0p1 5 0 4 0 1 0 8 0 0 0 0\n";
        assert_eq!(disk_counters(d), ((2000 + 8) * 512, (4000 + 16) * 512, 110, 51));
        let n = "Inter-|   Receive\n face |bytes\n    lo: 500 1 0 0 0 0 0 0 500 1 0 0 0 0 0 0\n  wlan0: 1000 10 0 0 0 0 0 0 300 5 0 0 0 0 0 0\n";
        assert_eq!(net_counters(n), (1000, 300));
        let (u, s, i) = cpu_split(&[0, 0, 0, 0, 0, 0, 0], &[30, 10, 20, 40, 0, 0, 0]);
        assert!((u - 0.4).abs() < 1e-6 && (s - 0.2).abs() < 1e-6 && (i - 0.4).abs() < 1e-6);
    }
}
