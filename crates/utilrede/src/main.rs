//! Utilitário de Rede (como o Network Utility do Tiger): abas Informações, Ping,
//! Lookup, Traceroute, Whois, Finger e Portas. Usa as ferramentas do sistema
//! (ping, tracepath, host) e fala direto os protocolos simples (whois na porta
//! 43, finger na 79, varredura de portas por conexão TCP), sem precisar de root.

use glove_config::tr;
use std::io::{BufRead, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_ui::{
    paint::{draw::*, Color, Fonts},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const TABS: [&str; 7] = ["Informações", "Ping", "Lookup", "Traceroute", "Whois", "Finger", "Portas"];
const LOOKUP_TYPES: [(&str, &str); 6] = [("", "Endereço (padrão)"), ("A", "IPv4 (A)"), ("AAAA", "IPv6 (AAAA)"), ("MX", "Correio (MX)"), ("NS", "Servidores (NS)"), ("TXT", "Texto (TXT)")];

/// Um trabalho em andamento: a saída cresce numa thread.
#[derive(Clone, Default)]
struct Job {
    out: Arc<Mutex<String>>,
    running: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
}

impl Job {
    fn push(&self, s: &str) {
        let mut o = self.out.lock().unwrap();
        o.push_str(s);
        // Saída sem fim (ping contínuo): guarda só o fim.
        if o.len() > 200_000 {
            let cut = o.len() - 150_000;
            let cut = (cut..o.len()).find(|&i| o.is_char_boundary(i)).unwrap_or(cut);
            o.drain(..cut);
        }
    }

    /// Roda um programa e vai juntando a saída.
    fn command(prog: &str, args: Vec<String>) -> Self {
        let job = Job::default();
        job.running.store(true, Ordering::Relaxed);
        let j = job.clone();
        let prog = prog.to_string();
        std::thread::spawn(move || {
            let child = std::process::Command::new(&prog).args(&args).env("LC_ALL", "C").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn();
            match child {
                Ok(mut c) => {
                    j.pid.store(c.id(), Ordering::Relaxed);
                    let err = c.stderr.take();
                    if let Some(out) = c.stdout.take() {
                        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                            j.push(&format!("{line}\n"));
                        }
                    }
                    if let Some(mut e) = err {
                        let mut s = String::new();
                        let _ = e.read_to_string(&mut s);
                        j.push(&s);
                    }
                    let _ = c.wait();
                }
                Err(e) => j.push(&tr!("Não foi possível usar o {}: {}\n", prog, e)),
            }
            j.running.store(false, Ordering::Relaxed);
        });
        job
    }

    /// Roda uma função numa thread (os protocolos que o app fala sozinho).
    fn thread(f: impl FnOnce(&Job) + Send + 'static) -> Self {
        let job = Job::default();
        job.running.store(true, Ordering::Relaxed);
        let j = job.clone();
        std::thread::spawn(move || {
            f(&j);
            j.running.store(false, Ordering::Relaxed);
        });
        job
    }

    fn stop(&self) {
        let pid = self.pid.load(Ordering::Relaxed);
        if pid != 0 {
            unsafe {
                libc::kill(pid as i32, libc::SIGINT);
            }
        }
        self.running.store(false, Ordering::Relaxed);
    }
}

fn first(cmds: &[&str]) -> Option<String> {
    let path = std::env::var("PATH").unwrap_or_default();
    cmds.iter().find(|c| path.split(':').any(|d| std::path::Path::new(d).join(c).is_file())).map(|c| c.to_string())
}

/// Whois: pergunta à IANA quem cuida do domínio e segue a indicação ("refer:").
fn whois(j: &Job, query: &str) {
    let ask = |server: &str| -> Result<String, String> {
        let addr = (server, 43).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or(tr!("servidor não encontrado"))?;
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| e.to_string())?;
        s.set_read_timeout(Some(Duration::from_secs(10))).ok();
        s.write_all(format!("{query}\r\n").as_bytes()).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        Ok(String::from_utf8_lossy(&out).into_owned())
    };
    j.push(tr!("Consultando whois.iana.org…\n"));
    match ask("whois.iana.org") {
        Ok(t) => {
            let refer = t.lines().find_map(|l| l.strip_prefix("refer:").map(|r| r.trim().to_string()));
            match refer {
                Some(r) => {
                    j.push(&tr!("Consultando {}…\n\n", r));
                    match ask(&r) {
                        Ok(t2) => j.push(&t2),
                        Err(e) => j.push(&tr!("Erro: {}\n", e)),
                    }
                }
                None => j.push(&format!("\n{t}")),
            }
        }
        Err(e) => j.push(&tr!("Erro: {}\n", e)),
    }
}

/// Finger: "usuario@máquina" na porta 79.
fn finger(j: &Job, query: &str) {
    let (user, host) = query.rsplit_once('@').unwrap_or(("", query));
    let addr = match (host, 79).to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => return j.push(tr!("Máquina não encontrada.\n")),
    };
    match TcpStream::connect_timeout(&addr, Duration::from_secs(6)) {
        Ok(mut s) => {
            s.set_read_timeout(Some(Duration::from_secs(8))).ok();
            let _ = s.write_all(format!("{user}\r\n").as_bytes());
            let mut out = Vec::new();
            let _ = s.read_to_end(&mut out);
            j.push(&String::from_utf8_lossy(&out));
        }
        Err(e) => j.push(&tr!("A máquina não respondeu na porta 79 (finger): {}\n", e)),
    }
}

/// Nome do serviço da porta (/etc/services).
fn service(port: u16) -> String {
    static S: std::sync::OnceLock<std::collections::HashMap<u16, String>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let t = std::fs::read_to_string("/etc/services").unwrap_or_default();
        t.lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let mut f = l.split_whitespace();
                let name = f.next()?;
                let (p, proto) = f.next()?.split_once('/')?;
                if proto != "tcp" {
                    return None;
                }
                Some((p.parse().ok()?, name.to_string()))
            })
            .collect()
    })
    .get(&port)
    .cloned()
    .unwrap_or_default()
}

/// Varredura: tenta conectar em cada porta (várias ao mesmo tempo).
fn scan(j: &Job, host: &str, from: u16, to: u16) {
    let Some(ip) = (host, 0).to_socket_addrs().ok().and_then(|mut a| a.next()).map(|a| a.ip()) else {
        return j.push(tr!("Máquina não encontrada.\n"));
    };
    j.push(&tr!("Procurando portas abertas em {} ({}), de {} a {}…\n\n", host, ip, from, to));
    let ports: Vec<u16> = (from..=to).collect();
    let found = Arc::new(Mutex::new(Vec::new()));
    for chunk in ports.chunks(64) {
        if !j.running.load(Ordering::Relaxed) {
            break;
        }
        let handles: Vec<_> = chunk
            .iter()
            .map(|&p| {
                let found = found.clone();
                std::thread::spawn(move || {
                    if TcpStream::connect_timeout(&(ip, p).into(), Duration::from_millis(400)).is_ok() {
                        found.lock().unwrap().push(p);
                    }
                })
            })
            .collect();
        for h in handles {
            let _ = h.join();
        }
        let mut f = found.lock().unwrap();
        f.sort_unstable();
        for p in f.drain(..) {
            j.push(&format!("Porta {p:<6} aberta   {}\n", service(p)));
        }
    }
    j.push(tr!("\nVarredura terminada.\n"));
}

/// Interfaces de rede: (nome, linhas de informação).
fn interfaces() -> Vec<(String, Vec<(String, String)>)> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/net") else { return v };
    let brief = std::process::Command::new("ip").args(["-brief", "address"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    // As de verdade (com aparelho: Wi-Fi, cabo) primeiro; as virtuais (Docker, pontes) e a "lo" por último.
    names.sort_by_key(|n| (n == "lo", !std::path::Path::new("/sys/class/net").join(n).join("device").exists()));
    for n in names {
        let base = std::path::Path::new("/sys/class/net").join(&n);
        let read = |f: &str| std::fs::read_to_string(base.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
        let ips: Vec<String> = brief.lines().find(|l| l.split_whitespace().next() == Some(n.as_str())).map(|l| l.split_whitespace().skip(2).map(str::to_string).collect()).unwrap_or_default();
        let speed = read("speed").parse::<i64>().ok().filter(|s| *s > 0).map(|s| format!("{s} Mb/s")).unwrap_or_else(|| "—".into());
        let kind = if base.join("wireless").exists() { "Wi-Fi" } else if n == "lo" { "Interna (loopback)" } else { "Cabo (Ethernet)" };
        let state = match read("operstate").as_str() {
            "up" => tr!("Conectada").to_string(),
            "down" => tr!("Desconectada").to_string(),
            o => o.to_string(),
        };
        let num = |f: &str| read(&format!("statistics/{f}"));
        v.push((
            n.clone(),
            vec![
                (tr!("Tipo").into(), kind.into()),
                (tr!("Estado").into(), state),
                (tr!("Endereço físico").into(), read("address")),
                ("Endereços IP".into(), if ips.is_empty() { "—".into() } else { ips.join("  ") }),
                (tr!("Velocidade").into(), speed),
                (tr!("Pacotes enviados").into(), num("tx_packets")),
                (tr!("Pacotes recebidos").into(), num("rx_packets")),
                (tr!("Erros (envio/recebimento)").into(), format!("{} / {}", num("tx_errors"), num("rx_errors"))),
            ],
        ));
    }
    v
}

struct Util {
    tab: usize,
    host: String,
    count: String,
    lookup_type: usize,
    from: String,
    to: String,
    job: Option<Job>,
    ifaces: Vec<(String, Vec<(String, String)>)>,
    iface: usize,
    mono: Option<Fonts>,
    scroll: f32,
    follow: bool,
    /// Tamanho da saída no último quadro (cresceu: redesenha a janela toda).
    last_len: usize,
}

impl Util {
    fn start(&mut self) {
        if let Some(j) = &self.job {
            j.stop();
        }
        let host = self.host.trim().to_string();
        if host.is_empty() && self.tab != 0 {
            return;
        }
        self.scroll = 0.0;
        self.follow = true;
        self.job = Some(match self.tab {
            1 => {
                let n = self.count.trim().parse::<u32>().unwrap_or(5).clamp(1, 1000);
                Job::command("ping", vec!["-c".into(), n.to_string(), host])
            }
            2 => {
                let t = LOOKUP_TYPES[self.lookup_type].0;
                match first(&["host", "dig"]) {
                    Some(p) if p == "host" => Job::command("host", if t.is_empty() { vec![host] } else { vec!["-t".into(), t.into(), host] }),
                    Some(_) => Job::command("dig", vec!["+noall".into(), "+answer".into(), host, if t.is_empty() { "A".into() } else { t.into() }]),
                    None => Job::command("getent", vec!["ahosts".into(), host]),
                }
            }
            3 => match first(&["traceroute", "tracepath"]) {
                Some(p) if p == "traceroute" => Job::command("traceroute", vec![host]),
                Some(_) => Job::command("tracepath", vec![host]),
                None => Job::thread(|j| j.push(tr!("Nem traceroute nem tracepath estão instalados.\n"))),
            },
            4 => Job::thread(move |j| whois(j, &host)),
            5 => Job::thread(move |j| finger(j, &host)),
            6 => {
                let a = self.from.trim().parse::<u16>().unwrap_or(1).max(1);
                let b = self.to.trim().parse::<u16>().unwrap_or(1024).max(a);
                Job::thread(move |j| scan(j, &host, a, b))
            }
            _ => return,
        });
    }

    fn output(&mut self, ui: &mut Ui, r: Rect) {
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        ui.c.stroke(rrect(r.x - 0.5, r.y - 0.5, r.w + 1.0, r.h + 1.0, 0.0), &solid(rgba(130, 130, 136, 255)), 1.0);
        let text = self.job.as_ref().map(|j| j.out.lock().unwrap().clone()).unwrap_or_default();
        // Linha nova (de uma thread, sem evento): o quadro inteiro, não só o cursor que pisca.
        if text.len() != self.last_len {
            self.last_len = text.len();
            ui.redraw = true;
        }
        let lines: Vec<&str> = text.lines().collect();
        let px = 11.5;
        let lh = px + 4.0;
        let content = lines.len() as f32 * lh + 12.0;
        if self.follow {
            self.scroll = (content - r.h).max(0.0);
        }
        let before = self.scroll;
        let inner = ui.scroll_area(r, &mut self.scroll, content, lh * 3.0);
        if (self.scroll - before).abs() > 0.5 {
            self.follow = self.scroll >= content - r.h - 2.0;
        }
        let (off, mono) = (self.scroll, self.mono.as_ref());
        ui.clipped(55, inner, |c, fonts| {
            c.clear(Color::WHITE);
            let f = mono.unwrap_or(fonts);
            let first = (off / lh) as usize;
            for (i, l) in lines.iter().enumerate().skip(first).take((inner.h / lh) as usize + 2) {
                f.draw(c, l, 8.0, 6.0 + px + i as f32 * lh - off, px, false, rgba(20, 20, 20, 255));
            }
        });
    }
}

impl App for Util {
    fn tick(&self) -> Option<Duration> {
        self.job.as_ref().filter(|j| j.running.load(Ordering::Relaxed)).map(|_| Duration::from_millis(250))
    }

    fn ui(&mut self, ui: &mut Ui) {
        for a in ui.menu_actions().to_vec() {
            if a == "close" {
                ui.close = true;
            }
        }
        if ui.keys().iter().any(|k| k.combo == "Ctrl+W") {
            ui.close = true;
        }
        let (w, h) = (ui.c.w, ui.c.h);
        let mut tab = self.tab;
        ui.area(Rect::new(16.0, 12.0, w - 32.0, 24.0), |ui| {
            ui.tabs(&mut tab, &TABS.map(tr));
        });
        if tab != self.tab {
            self.tab = tab;
            if let Some(j) = self.job.take() {
                j.stop();
            }
            if tab == 0 {
                self.ifaces = interfaces();
            }
        }
        let ink = rgba(20, 20, 20, 255);
        let top = 52.0;
        // Informações: as interfaces de rede.
        if self.tab == 0 {
            let names: Vec<String> = self.ifaces.iter().map(|(n, _)| n.clone()).collect();
            ui.fonts.draw(ui.c, tr!("Escolha uma interface de rede para ver as informações:"), 20.0, top + 16.0, 12.0, false, ink);
            ui.area(Rect::new(20.0, top + 26.0, 240.0, 24.0), |ui| {
                ui.popup(&mut self.iface, &names, 220.0);
            });
            if let Some((_, rows)) = self.ifaces.get(self.iface) {
                for (k, (l, v)) in rows.iter().enumerate() {
                    let y = top + 80.0 + k as f32 * 22.0;
                    let lw = ui.text_width(l, 12.0, false);
                    ui.fonts.draw(ui.c, &format!("{l}:"), 200.0 - lw, y, 12.0, false, rgba(90, 90, 96, 255));
                    ui.fonts.draw(ui.c, &ui.fonts.ellipsize(v, 12.0, false, w - 230.0), 212.0, y, 12.0, false, ink);
                }
            }
            return;
        }
        // Campo do endereço e as opções de cada aba.
        let prompt = match self.tab {
            5 => tr!("Usuário e máquina (usuario@maquina):"),
            4 => tr!("Domínio ou endereço (ex.: exemplo.org):"),
            _ => tr!("Endereço de rede (ex.: 10.0.0.1 ou exemplo.org):"),
        };
        ui.fonts.draw(ui.c, prompt, 20.0, top + 16.0, 12.0, false, ink);
        let (_, enter) = ui.text_field_in(Rect::new(20.0, top + 24.0, 300.0, 22.0), &mut self.host, false);
        let mut go = enter;
        let running = self.job.as_ref().is_some_and(|j| j.running.load(Ordering::Relaxed));
        match self.tab {
            1 => {
                ui.fonts.draw(ui.c, tr!("Pings:"), 336.0, top + 40.0, 12.0, false, ink);
                ui.text_field_in(Rect::new(380.0, top + 24.0, 50.0, 22.0), &mut self.count, false);
            }
            2 => {
                ui.area(Rect::new(336.0, top + 23.0, 190.0, 24.0), |ui| {
                    ui.popup(&mut self.lookup_type, &LOOKUP_TYPES.iter().map(|t| tr(t.1).to_string()).collect::<Vec<_>>(), 180.0);
                });
            }
            6 => {
                ui.fonts.draw(ui.c, tr!("Portas de"), 336.0, top + 40.0, 12.0, false, ink);
                ui.text_field_in(Rect::new(400.0, top + 24.0, 56.0, 22.0), &mut self.from, false);
                ui.fonts.draw(ui.c, "a", 464.0, top + 40.0, 12.0, false, ink);
                ui.text_field_in(Rect::new(476.0, top + 24.0, 56.0, 22.0), &mut self.to, false);
            }
            _ => {}
        }
        let label = match self.tab {
            1 => "Ping",
            2 => tr!("Procurar"),
            3 => tr!("Rastrear"),
            6 => tr!("Varrer"),
            _ => tr!("Consultar"),
        };
        let mut stop = false;
        ui.area(Rect::new(w - 116.0, top + 23.0, 100.0, 24.0), |ui| {
            if running {
                stop = ui.button(tr!("Parar"));
            } else {
                go |= ui.default_button(label);
            }
        });
        if stop {
            if let Some(j) = &self.job {
                j.stop();
            }
        } else if go {
            self.start();
        }
        self.output(ui, Rect::new(20.0, top + 60.0, w - 40.0, h - top - 76.0));
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(tr!("Arquivo"), vec![MenuItem::new("close", tr!("Fechar"), "Ctrl+W")])]
    }
}

pub fn main() {
    let app = Util {
        tab: 0,
        host: String::new(),
        count: "5".into(),
        lookup_type: 0,
        from: "1".into(),
        to: "1024".into(),
        job: None,
        ifaces: interfaces(),
        iface: 0,
        mono: Fonts::load_mono(),
        scroll: 0.0,
        follow: true,
        last_len: 0,
    };
    let opts = WindowOptions { title: tr!("Utilitário de Rede").into(), app_id: "glove.UtilRede".into(), size: (720, 480), fixed: false, min_size: Some((600, 380)) };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-utilrede: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn servicos() {
        // /etc/services quase sempre tem a 22 (ssh); sem o arquivo, fica vazio.
        let s = super::service(22);
        assert!(s.is_empty() || s == "ssh");
    }
}
