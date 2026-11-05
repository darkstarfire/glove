//! Painel Rede: estado das interfaces, Wi-Fi (redes, conectar com senha) e
//! TCP/IP (DHCP ou manual), tudo pelo NetworkManager.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_config::rede::{self, Iface, IpInfo, Wifi};
use glove_kit::{
    paint::{draw::*, Color},
    Rect, Ui,
};

#[derive(Default)]
pub struct Rede {
    /// 0 = Status da Rede; i + 1 = interface i.
    pub show: usize,
    /// None = sem NetworkManager.
    ifaces: Option<Vec<Iface>>,
    nets: Vec<Wifi>,
    wifi_on: Option<bool>,
    sel_net: Option<usize>,
    tab: usize,
    /// Endereços da interface mostrada e o formulário (endereço, máscara, roteador, DNS).
    ip: Option<(String, IpInfo)>,
    form: [String; 4],
    manual: bool,
    form_dirty: bool,
    /// Pedido de senha: (rede, senha).
    pub prompt: Option<(String, String)>,
    /// Trabalho em andamento ("Conectando a “X”…") e a resposta dele.
    busy: Option<String>,
    done: Arc<Mutex<Option<Result<String, String>>>>,
    message: Option<(bool, String)>,
}

/// Linha de status (como no 10.4): (cor da luz, título, frase).
fn status_line(i: &Iface, ip: &str) -> (Color, String, String) {
    let name = i.label();
    if i.connected() {
        let addr = ip.split('/').next().unwrap_or("");
        let text = match (i.kind.as_str(), addr.is_empty()) {
            ("wifi", false) => format!("{name} está conectado à rede “{}” e tem o endereço IP {addr}.", i.connection),
            ("wifi", true) => format!("{name} está conectado à rede “{}”, mas ainda não tem endereço IP.", i.connection),
            (_, false) => format!("{name} está ativo e tem o endereço IP {addr}."),
            _ => format!("{name} está ativo, mas ainda não tem endereço IP."),
        };
        (if addr.is_empty() { rgba(240, 190, 40, 255) } else { rgba(80, 190, 70, 255) }, name.clone(), text)
    } else if i.state.starts_with("connecting") {
        (rgba(240, 190, 40, 255), name.clone(), format!("{name} está conectando…"))
    } else if i.kind == "ethernet" {
        (rgba(220, 70, 60, 255), name, "O cabo não está conectado.".into())
    } else if i.state == "unavailable" {
        (rgba(220, 70, 60, 255), name.clone(), format!("{name} está desligado."))
    } else {
        (rgba(220, 70, 60, 255), name.clone(), format!("{name} está ligado, mas não está conectado a nenhuma rede."))
    }
}

/// Bolinha de estado em gel.
fn light(ui: &mut Ui, cx: f32, cy: f32, c: Color) {
    gel_circle(ui.c, cx, cy, 6.0, c);
}

/// Barrinhas de sinal (0 a 100).
fn signal_bars(ui: &mut Ui, x: f32, y: f32, signal: u8, ink: Color) {
    let lit = match signal {
        0..=24 => 1,
        25..=49 => 2,
        50..=74 => 3,
        _ => 4,
    };
    for k in 0..4 {
        let h = 3.0 + k as f32 * 3.0;
        let c = if k < lit { ink } else { with_alpha(ink, 0.25) };
        ui.c.fill_rect(x + k as f32 * 5.0, y + 12.0 - h, 3.0, h, &solid(c));
    }
}

/// Cadeado pequeno (rede com senha).
fn lock_glyph(ui: &mut Ui, x: f32, y: f32, ink: Color) {
    ui.c.stroke(rrect(x + 2.0, y, 6.0, 7.0, 3.0), &solid(ink), 1.4);
    ui.c.fill(rrect(x, y + 5.0, 10.0, 8.0, 1.5), &solid(ink));
}

impl Rede {
    pub fn refresh(&mut self) {
        self.ifaces = rede::devices();
        let has_wifi = self.ifaces.as_ref().is_some_and(|l| l.iter().any(|i| i.kind == "wifi"));
        if has_wifi {
            self.wifi_on = rede::wifi_enabled();
            let sel = self.sel_net.and_then(|i| self.nets.get(i)).map(|w| w.ssid.clone());
            self.nets = rede::wifi_list(false);
            self.sel_net = sel.and_then(|s| self.nets.iter().position(|w| w.ssid == s));
        }
        self.load_ip(false);
    }

    fn shown_iface(&self) -> Option<Iface> {
        self.show.checked_sub(1).and_then(|i| self.ifaces.as_ref()?.get(i).cloned())
    }

    /// Lê os endereços da interface mostrada; o formulário só é refeito se não estiver sendo editado.
    fn load_ip(&mut self, force: bool) {
        let Some(i) = self.shown_iface() else { return };
        let info = rede::ip_info(&i);
        let changed_iface = self.ip.as_ref().map(|(d, _)| d != &i.device).unwrap_or(true);
        if force || changed_iface || !self.form_dirty {
            let (addr, pre) = info.address.split_once('/').map(|(a, p)| (a.to_string(), p.parse().unwrap_or(24))).unwrap_or_default();
            self.form = [addr, if info.address.is_empty() { String::new() } else { rede::mask(pre) }, info.gateway.clone(), info.dns.join(", ")];
            self.manual = info.manual;
            self.form_dirty = false;
        }
        self.ip = Some((i.device.clone(), info));
    }

    /// Roda um trabalho do NetworkManager numa thread (a janela não trava).
    fn start(&mut self, what: String, ok: String, job: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.busy = Some(what);
        self.message = None;
        let done = self.done.clone();
        std::thread::spawn(move || {
            let r = job().map(|_| ok);
            *done.lock().unwrap() = Some(r);
        });
    }

    fn join(&mut self, ssid: String, password: Option<String>) {
        let s = ssid.clone();
        self.start(format!("Conectando a “{ssid}”…"), format!("Conectado a “{ssid}”."), move || rede::connect(&s, password.as_deref()));
    }

    /// Escolher uma rede: as abertas e as já salvas conectam direto; as outras pedem a senha.
    pub fn choose(&mut self, ssid: &str) {
        let secure = self.nets.iter().find(|w| w.ssid == ssid).is_none_or(|w| w.secure);
        if secure && !rede::saved_connections().iter().any(|c| c == ssid) {
            self.prompt = Some((ssid.to_string(), String::new()));
        } else {
            self.join(ssid.to_string(), None);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, wifi_menu: &mut bool) -> bool {
        let mut save = false;
        let finished = self.done.lock().unwrap().take();
        if let Some(r) = finished {
            self.busy = None;
            self.message = Some(match r {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            });
            self.form_dirty = false;
            self.refresh();
        }
        if self.busy.is_some() {
            ui.wake_after(Duration::from_millis(200));
        }
        let Some(ifaces) = self.ifaces.clone() else {
            ui.small("O NetworkManager não respondeu (o comando nmcli não está disponível).");
            return false;
        };
        let mut names = vec!["Status da Rede".to_string()];
        names.extend(ifaces.iter().map(|i| i.label()));
        self.show = self.show.min(ifaces.len());
        ui.horizontal(|ui| {
            ui.label_right("Mostrar:", 150.0);
            if ui.popup(&mut self.show, &names, 200.0) {
                self.tab = 0;
                self.load_ip(true);
            }
        });
        ui.space(10.0);
        match self.shown_iface() {
            None => self.status(ui, &ifaces),
            Some(i) => self.iface(ui, &i),
        }
        ui.space(8.0);
        // Mensagem do último trabalho, ou o andamento.
        if let Some(b) = &self.busy {
            let b = b.clone();
            ui.horizontal(|ui| {
                ui.spinner(14.0);
                ui.small(&b);
            });
        } else if let Some((ok, m)) = self.message.clone() {
            let t = if ok { m } else { format!("Não deu certo: {m}") };
            ui.small(&t);
        }
        ui.space(4.0);
        if ui.checkbox(wifi_menu, "Mostrar o Wi-Fi na barra de menu") {
            save = true;
        }
        save
    }

    fn status(&mut self, ui: &mut Ui, ifaces: &[Iface]) {
        let w = ui.available_width();
        let row_h = 52.0;
        let h = (ifaces.len().max(1) as f32 * row_h + 8.0).max(80.0);
        let r = ui.alloc(w, h);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        if ifaces.is_empty() {
            ui.fonts.draw(ui.c, "Nenhuma interface de rede encontrada.", r.x + 14.0, r.y + 30.0, 12.0, false, rgba(100, 100, 106, 255));
        }
        let mut open = None;
        for (k, i) in ifaces.iter().enumerate() {
            let y = r.y + 4.0 + k as f32 * row_h;
            let ip = self.ip.as_ref().filter(|(d, _)| *d == i.device).map(|(_, info)| info.address.clone()).unwrap_or_else(|| if i.connected() { rede::ip_info(i).address } else { String::new() });
            let (c, title, text) = status_line(i, &ip);
            light(ui, r.x + 18.0, y + 16.0, c);
            ui.fonts.draw(ui.c, &title, r.x + 34.0, y + 20.0, 13.0, true, rgba(20, 20, 20, 255));
            let t = ui.fonts.ellipsize(&text, 11.0, false, r.w - 50.0);
            ui.fonts.draw(ui.c, &t, r.x + 34.0, y + 38.0, 11.0, false, rgba(80, 80, 86, 255));
            if k + 1 < ifaces.len() {
                ui.c.hline(r.x + 10.0, y + row_h - 1.0, r.w - 20.0, rgba(225, 225, 228, 255));
            }
            if ui.hit(Rect::new(r.x, y, r.w, row_h)).double {
                open = Some(k + 1);
            }
        }
        ui.space(6.0);
        ui.small("Clique duas vezes numa interface para configurá-la.");
        if let Some(s) = open {
            self.show = s;
            self.tab = 0;
            self.load_ip(true);
            ui.redraw = true;
        }
    }

    fn iface(&mut self, ui: &mut Ui, i: &Iface) {
        let wifi = i.kind == "wifi";
        let labels: Vec<&str> = if wifi { vec!["Wi-Fi", "TCP/IP"] } else { vec!["TCP/IP"] };
        let mut tab = self.tab.min(labels.len() - 1);
        let shown = tab;
        ui.tab_box(&mut tab, &labels, 250.0, |ui| {
            if wifi && shown == 0 {
                self.wifi_tab(ui, i);
            } else {
                self.tcpip_tab(ui, i);
            }
        });
        self.tab = tab;
    }

    fn wifi_tab(&mut self, ui: &mut Ui, i: &Iface) {
        let mut on = self.wifi_on.unwrap_or(false);
        ui.horizontal(|ui| {
            if ui.checkbox(&mut on, "Wi-Fi ligado") && self.busy.is_none() {
                self.start(if on { "Ligando o Wi-Fi…".into() } else { "Desligando o Wi-Fi…".into() }, if on { "Wi-Fi ligado.".into() } else { "Wi-Fi desligado.".into() }, move || rede::set_wifi(on));
                self.wifi_on = Some(on);
            }
        });
        ui.space(4.0);
        // Lista de redes: nome, cadeado e sinal.
        let w = ui.available_width();
        let row_h = 20.0;
        let list_h = 128.0;
        let r = ui.alloc(w, list_h);
        ui.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        ui.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        if !on {
            ui.fonts.draw(ui.c, "O Wi-Fi está desligado.", r.x + 12.0, r.y + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        } else if self.nets.is_empty() {
            ui.fonts.draw(ui.c, "Nenhuma rede encontrada.", r.x + 12.0, r.y + 22.0, 11.0, false, rgba(110, 110, 116, 255));
        }
        let mut double = None;
        for (k, n) in self.nets.iter().enumerate().take(((list_h - 2.0) / row_h) as usize).filter(|_| on) {
            let y = r.y + 1.0 + k as f32 * row_h;
            let rr = Rect::new(r.x + 1.0, y, r.w - 2.0, row_h);
            let sel = self.sel_net == Some(k);
            if sel {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(ui.theme.accent));
            } else if k % 2 == 1 {
                ui.c.fill_rect(rr.x, rr.y, rr.w, rr.h, &solid(rgba(237, 243, 254, 255)));
            }
            let ink = if sel { Color::WHITE } else { rgba(20, 20, 20, 255) };
            if n.in_use {
                ui.fonts.draw(ui.c, "✓", rr.x + 6.0, y + 15.0, 12.0, true, ink);
            }
            ui.fonts.draw(ui.c, &n.ssid, rr.x + 22.0, y + 15.0, 12.0, n.in_use, ink);
            if n.secure {
                lock_glyph(ui, rr.x + rr.w - 50.0, y + 4.0, ink);
            }
            signal_bars(ui, rr.x + rr.w - 30.0, y + 4.0, n.signal, ink);
            let h = ui.hit(rr);
            if h.clicked {
                self.sel_net = Some(k);
            }
            if h.double {
                double = Some(n.ssid.clone());
            }
        }
        ui.space(6.0);
        let sel = self.sel_net.and_then(|k| self.nets.get(k)).cloned();
        let idle = self.busy.is_none();
        let (mut join, mut leave, mut scan) = (false, false, false);
        ui.horizontal(|ui| {
            ui.set_enabled(idle && on);
            scan = ui.button("Buscar Redes");
            ui.set_enabled(idle && on && i.connected());
            leave = ui.button("Desconectar");
            ui.set_enabled(idle && on && sel.as_ref().is_some_and(|n| !n.in_use));
            join = ui.default_button("Conectar…");
            ui.set_enabled(true);
        });
        if scan {
            self.start("Buscando redes…".into(), "Busca concluída.".into(), || {
                rede::wifi_list(true);
                Ok(())
            });
        }
        if leave {
            let d = i.device.clone();
            self.start("Desconectando…".into(), "Desconectado.".into(), move || rede::disconnect(&d));
        }
        if let Some(s) = double.or_else(|| join.then(|| sel.map(|n| n.ssid)).flatten()) {
            if idle {
                self.choose(&s);
            }
        }
    }

    fn tcpip_tab(&mut self, ui: &mut Ui, i: &Iface) {
        const LW: f32 = 170.0;
        let mut m = usize::from(self.manual);
        ui.form_row("Configurar IPv4", LW, |ui| {
            if ui.popup(&mut m, &["Usando DHCP".to_string(), "Manualmente".to_string()], 180.0) {
                self.manual = m == 1;
                self.form_dirty = true;
            }
        });
        ui.space(4.0);
        let labels = ["Endereço IP", "Máscara de sub-rede", "Roteador", "Servidores DNS"];
        let manual = self.manual;
        for (k, l) in labels.iter().enumerate() {
            let mut v = self.form[k].clone();
            ui.push_id(l, |ui| {
                ui.form_row(l, LW, |ui| {
                    if manual {
                        if ui.text_field(&mut v, 200.0) {
                            self.form_dirty = true;
                        }
                    } else {
                        let t = if v.is_empty() { "—".to_string() } else { v.clone() };
                        ui.label(&t);
                    }
                });
            });
            self.form[k] = v;
        }
        if let Some((_, info)) = &self.ip {
            if !info.hwaddr.is_empty() {
                let t = format!("Endereço MAC: {}", info.hwaddr);
                ui.form_note(LW, &t);
            }
        }
        ui.space(6.0);
        let can = self.form_dirty && self.busy.is_none() && !i.connection.is_empty();
        let mut apply = false;
        let mut revert = false;
        ui.horizontal(|ui| {
            ui.space(LW);
            ui.set_enabled(can);
            revert = ui.button("Reverter");
            apply = ui.default_button("Aplicar");
            ui.set_enabled(true);
        });
        if revert {
            self.load_ip(true);
        }
        if apply {
            let conn = i.connection.clone();
            if !self.manual {
                self.start("Aplicando…".into(), "Usando DHCP.".into(), move || rede::set_ipv4(&conn, None));
                return;
            }
            let Some(pre) = rede::prefix(&self.form[1]) else {
                self.message = Some((false, "a máscara de sub-rede não é válida (ex.: 255.255.255.0).".into()));
                return;
            };
            if self.form[0].parse::<std::net::Ipv4Addr>().is_err() {
                self.message = Some((false, "o endereço IP não é válido (ex.: 192.168.0.20).".into()));
                return;
            }
            let addr = format!("{}/{pre}", self.form[0].trim());
            let (gw, dns) = (self.form[2].trim().to_string(), self.form[3].replace(' ', ""));
            self.start("Aplicando…".into(), "Endereço manual aplicado.".into(), move || rede::set_ipv4(&conn, Some((&addr, &gw, &dns))));
        }
    }

    /// Folha pedindo a senha da rede (por cima do painel).
    pub fn prompt_sheet(&mut self, ui: &mut Ui) {
        let Some((ssid, mut pw)) = self.prompt.clone() else { return };
        let (w, _) = (ui.c.w, ui.c.h);
        let (sw, sh) = (400.0, 150.0);
        let x = ((w - sw) / 2.0).round();
        let y = 78.0;
        ui.c.fill(rrect(x - 6.0, y, sw + 12.0, sh + 8.0, 6.0), &solid(rgba(0, 0, 0, 40)));
        ui.c.fill_rect(x, y, sw, sh, &vgrad(y, y + sh, &[(0.0, rgba(236, 236, 238, 252)), (1.0, rgba(246, 246, 248, 252))]));
        ui.c.stroke(rrect(x + 0.5, y - 0.5, sw - 1.0, sh, 0.0), &solid(rgba(0, 0, 0, 70)), 1.0);
        let title = format!("A rede “{ssid}” pede uma senha.");
        ui.fonts.draw(ui.c, &title, x + 20.0, y + 26.0, 13.0, true, rgba(20, 20, 20, 255));
        ui.fonts.draw(ui.c, "Senha:", x + 20.0, y + 62.0, 13.0, false, rgba(20, 20, 20, 255));
        let enter = ui.password_field_in(Rect::new(x + 80.0, y + 46.0, sw - 100.0, 22.0), &mut pw, true);
        let (mut cancel, mut ok) = (false, false);
        ui.area(Rect::new(x + sw - 210.0, y + sh - 40.0, 200.0, 30.0), |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button("Cancelar");
                ok = ui.default_button("Conectar");
            });
        });
        let esc = ui.keys().iter().any(|k| k.key == glove_kit::ui::Key::Escape);
        if cancel || esc {
            self.prompt = None;
        } else if (ok || enter) && !pw.is_empty() {
            self.prompt = None;
            self.join(ssid, Some(pw));
        } else {
            self.prompt = Some((ssid, pw));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn frases_de_status() {
        let i = glove_config::rede::Iface { device: "wlp1s0".into(), kind: "wifi".into(), state: "connected".into(), connection: "Casa".into() };
        let (_, t, s) = super::status_line(&i, "192.168.0.5/24");
        assert_eq!(t, "Wi-Fi");
        assert_eq!(s, "Wi-Fi está conectado à rede “Casa” e tem o endereço IP 192.168.0.5.");
        let e = glove_config::rede::Iface { device: "eth0".into(), kind: "ethernet".into(), state: "unavailable".into(), connection: String::new() };
        assert_eq!(super::status_line(&e, "").2, "O cabo não está conectado.");
    }
}
