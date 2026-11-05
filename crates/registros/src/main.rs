//! Registros (como o Console da era): as fontes à esquerda (a sessão do
//! Glove, o sistema e o kernel pelo journalctl) e as linhas à direita, com
//! filtro e atualização a cada poucos segundos.

use std::process::Command;
use std::time::Duration;

use glove_kit::{
    paint::{draw::*, Color},
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions, SEARCH_H,
};

const SOURCES: [&str; 4] = ["Sessão do Glove", "Sessão anterior", "Sistema (desde a partida)", "Kernel"];
const ROW: f32 = 15.0;
const MAX_LINES: usize = 3000;

fn state_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_STATE_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")).join("glove")
}

/// Tira as cores de terminal (ESC[…m) que o log do Glove tem.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' && it.peek() == Some(&'[') {
            for d in it.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn load(source: usize) -> Vec<String> {
    let text = match source {
        0 => std::fs::read_to_string(state_dir().join("sessao.log")).unwrap_or_else(|_| "Ainda não há registro da sessão do Glove (só existe ao entrar pela tela de login).".into()),
        1 => std::fs::read_to_string(state_dir().join("sessao.log.anterior")).unwrap_or_else(|_| "Não há registro da sessão anterior.".into()),
        2 => Command::new("journalctl").args(["-b", "--no-pager", "-o", "short", "-n", "3000"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default(),
        _ => Command::new("journalctl").args(["-k", "-b", "--no-pager", "-o", "short", "-n", "3000"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default(),
    };
    let mut v: Vec<String> = text.lines().map(strip_ansi).collect();
    if v.len() > MAX_LINES {
        v.drain(..v.len() - MAX_LINES);
    }
    if v.is_empty() {
        v.push("Sem linhas (talvez falte permissão para ler o registro do sistema).".into());
    }
    v
}

struct Registros {
    source: Option<usize>,
    lines: Vec<String>,
    filter: String,
    scroll: f32,
    /// Acompanha o fim (como o "rolar até o fim" automático).
    follow: bool,
}

impl App for Registros {
    fn ui(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.c.w, ui.c.h);
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+W" => acts.push("close".into()),
                "Ctrl+R" => acts.push("reload".into()),
                "Ctrl+K" => acts.push("clear-filter".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "close" => ui.close = true,
                "reload" => self.lines = load(self.source.unwrap_or(0)),
                "clear-filter" => self.filter.clear(),
                _ => {}
            }
        }
        // Barra: filtro.
        let bar = 40.0;
        ui.c.fill_rect(0.0, 0.0, w, bar, &vgrad(0.0, bar, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(206, 206, 210, 255))]));
        ui.c.hline(0.0, bar - 1.0, w, rgba(140, 140, 146, 255));
        ui.search_field_in(Rect::new(w - 230.0, (bar - SEARCH_H) / 2.0, 216.0, SEARCH_H), &mut self.filter, "Filtrar");
        let mut follow = self.follow;
        ui.area(Rect::new(12.0, 10.0, 220.0, 22.0), |ui| {
            ui.checkbox(&mut follow, "Acompanhar o fim");
        });
        self.follow = follow;
        // Fontes.
        let names: Vec<String> = SOURCES.iter().map(|s| s.to_string()).collect();
        let before = self.source;
        ui.area(Rect::new(8.0, bar + 8.0, 190.0, h - bar - 16.0), |ui| {
            ui.list(&mut self.source, &names, 190.0, h - bar - 16.0);
        });
        if self.source != before {
            self.lines = load(self.source.unwrap_or(0));
            self.scroll = f32::MAX;
        }
        // Linhas (com o filtro), em fonte de largura fixa no papel branco.
        let f = self.filter.trim().to_lowercase();
        let shown: Vec<&String> = self.lines.iter().filter(|l| f.is_empty() || l.to_lowercase().contains(&f)).collect();
        let r = Rect::new(206.0, bar + 8.0, w - 214.0, h - bar - 16.0);
        let content = shown.len() as f32 * ROW + 8.0;
        if self.follow {
            self.scroll = f32::MAX;
        }
        let area = ui.scroll_area(r, &mut self.scroll, content, ROW * 3.0);
        let first = (self.scroll / ROW) as usize;
        let count = (area.h / ROW) as usize + 2;
        let off = self.scroll;
        ui.clipped(5, area, |c, fonts| {
            c.clear(Color::WHITE);
            for (i, l) in shown.iter().enumerate().skip(first).take(count) {
                let y = 4.0 + i as f32 * ROW - off;
                let low = l.to_lowercase();
                let ink = if low.contains("error") || low.contains("erro") || low.contains("fail") || low.contains("falh") {
                    rgba(180, 40, 30, 255)
                } else if low.contains("warn") || low.contains("aviso") {
                    rgba(160, 110, 0, 255)
                } else {
                    rgba(30, 30, 34, 255)
                };
                let t = fonts.ellipsize(l, 11.0, false, area.w - 12.0);
                fonts.draw(c, &t, 6.0, y + 11.0, 11.0, false, ink);
            }
        });
        ui.c.stroke(rrect(r.x - 0.5, r.y - 0.5, r.w + 1.0, r.h + 1.0, 0.0), &solid(rgba(150, 150, 156, 255)), 1.0);
    }

    fn tick(&self) -> Option<Duration> {
        Some(Duration::from_secs(3))
    }

    fn on_tick(&mut self) {
        if self.follow {
            self.lines = load(self.source.unwrap_or(0));
        }
    }

    fn menus(&self) -> Vec<Menu> {
        vec![Menu::new(
            "Arquivo",
            vec![MenuItem::new("reload", "Recarregar", "Ctrl+R"), MenuItem::new("clear-filter", "Limpar Filtro", "Ctrl+K"), MenuItem::Separator, MenuItem::new("close", "Fechar", "Ctrl+W")],
        )]
    }
}

fn main() {
    let app = Registros { source: Some(2), lines: load(2), filter: String::new(), scroll: f32::MAX, follow: true };
    let opts = WindowOptions { title: "Registros".into(), app_id: "glove.Registros".into(), size: (820, 500), fixed: false, min_size: None };
    if let Err(e) = run(opts, app) {
        eprintln!("glove-registros: {e}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sem_cores() {
        assert_eq!(super::strip_ansi("\u{1b}[2m2026\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m ok"), "2026  INFO ok");
    }
}
