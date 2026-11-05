//! Xadrez contra o computador.

mod engine;

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::{Arc, Mutex};

use engine::{best_move, Mv, Pos, B, K, N, Q, R};
use glove_kit::{
    paint::draw::*,
    run, App, Menu, MenuItem, Rect, Ui, WindowOptions,
};

const SQ: f32 = 54.0;
const MARGIN: f32 = 22.0;
const STATUS_H: f32 = 30.0;

const LEVELS: [(&str, &str, u32); 3] = [("easy", "Fácil", 2), ("medium", "Médio", 4), ("hard", "Difícil", 5)];

struct Chess {
    pos: Pos,
    history: Vec<Pos>,
    selected: Option<u8>,
    last: Option<Mv>,
    /// Lado do jogador (1 brancas, -1 pretas).
    me: i8,
    level: usize,
    /// O computador está pensando (numa tarefa em segundo plano).
    think: bool,
    seed: u64,
    /// Jogada calculada, entregue pela tarefa.
    answer: Arc<Mutex<Option<(u64, Option<Mv>)>>>,
    /// Identifica o cálculo atual (um "Novo jogo" no meio descarta o antigo).
    job: u64,
    wake_r: Arc<OwnedFd>,
    wake_w: Arc<OwnedFd>,
}

fn pipe() -> (Arc<OwnedFd>, Arc<OwnedFd>) {
    let mut fds = [0; 2];
    unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) };
    unsafe { (Arc::new(OwnedFd::from_raw_fd(fds[0])), Arc::new(OwnedFd::from_raw_fd(fds[1]))) }
}

fn glyph(p: i8) -> &'static str {
    // Sempre a forma "cheia"; a cor é pintada por nós.
    match p.abs() {
        K => "♚",
        Q => "♛",
        R => "♜",
        B => "♝",
        N => "♞",
        _ => "♟",
    }
}

impl Chess {
    fn new(me: i8, level: usize, wake: (Arc<OwnedFd>, Arc<OwnedFd>), job: u64) -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
        let mut c = Self {
            pos: Pos::start(),
            history: Vec::new(),
            selected: None,
            last: None,
            me,
            level,
            think: false,
            seed,
            answer: Arc::default(),
            job,
            wake_r: wake.0,
            wake_w: wake.1,
        };
        if me == -1 {
            c.start_thinking();
        }
        c
    }

    fn restart(&mut self, me: i8) {
        let wake = (self.wake_r.clone(), self.wake_w.clone());
        *self = Chess::new(me, self.level, wake, self.job + 1);
    }

    fn start_thinking(&mut self) {
        self.think = true;
        self.job += 1;
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let (pos, depth, seed, job) = (self.pos.clone(), LEVELS[self.level].2, self.seed, self.job);
        let (answer, wake) = (self.answer.clone(), self.wake_w.clone());
        std::thread::spawn(move || {
            let m = best_move(&pos, depth, seed);
            *answer.lock().unwrap() = Some((job, m));
            unsafe { libc::write(wake.as_raw_fd(), b"x".as_ptr() as *const _, 1) };
        });
    }

    fn over(&self) -> bool {
        self.pos.legal().is_empty() || self.pos.half >= 100
    }

    fn status(&self) -> String {
        let moves = self.pos.legal();
        if moves.is_empty() {
            if self.pos.in_check() {
                return if self.pos.side == self.me { "Xeque-mate. O computador venceu.".into() } else { "Xeque-mate! Você venceu.".into() };
            }
            return "Empate por afogamento.".into();
        }
        if self.pos.half >= 100 {
            return "Empate: 50 lances sem captura nem peão.".into();
        }
        if self.think {
            return "O computador está pensando…".into();
        }
        if self.pos.in_check() { "Xeque! Sua vez.".into() } else { "Sua vez.".into() }
    }

    /// Casa da tela → índice do tabuleiro (invertido se jogo de pretas).
    fn square_at(&self, col: usize, row: usize) -> u8 {
        let (f, r) = if self.me == 1 { (col, 7 - row) } else { (7 - col, row) };
        (r * 8 + f) as u8
    }

    fn screen_of(&self, sq: u8) -> (f32, f32) {
        let (f, r) = ((sq % 8) as f32, (sq / 8) as f32);
        let (col, row) = if self.me == 1 { (f, 7.0 - r) } else { (7.0 - f, r) };
        (MARGIN + col * SQ, MARGIN + row * SQ)
    }

    fn play(&mut self, m: Mv) {
        self.history.push(self.pos.clone());
        self.pos = self.pos.make(m);
        self.last = Some(m);
        self.selected = None;
    }

    fn click(&mut self, sq: u8) {
        if self.think || self.over() || self.pos.side != self.me {
            return;
        }
        let p = self.pos.b[sq as usize];
        if let Some(from) = self.selected {
            // Promoção sempre vira dama (a escolha mais comum).
            let m = self.pos.legal().into_iter().find(|m| m.from == from && m.to == sq && (m.promo == 0 || m.promo == Q));
            if let Some(m) = m {
                self.play(m);
                if !self.over() {
                    self.start_thinking();
                }
                return;
            }
        }
        self.selected = (p != 0 && p.signum() == self.me).then_some(sq);
    }

    fn undo(&mut self) {
        // Volta o lance do computador e o seu.
        while let Some(prev) = self.history.pop() {
            self.pos = prev;
            if self.pos.side == self.me {
                break;
            }
        }
        self.last = None;
        self.selected = None;
        self.think = false;
        self.job += 1; // descarta um cálculo em andamento
    }

    fn draw_board(&self, ui: &mut Ui) {
        let c = &mut ui.c;
        let size = SQ * 8.0;
        // Moldura de madeira escura
        c.fill(rrect(MARGIN - 14.0, MARGIN - 14.0, size + 28.0, size + 28.0, 6.0), &linear(0.0, 0.0, size, size, &[(0.0, rgba(120, 72, 38, 255)), (1.0, rgba(70, 40, 20, 255))]));
        c.stroke(rrect(MARGIN - 14.0, MARGIN - 14.0, size + 28.0, size + 28.0, 6.0), &solid(rgba(40, 22, 10, 255)), 1.0);
        let light = rgba(238, 216, 176, 255);
        let dark = rgba(170, 118, 72, 255);
        let legal: Vec<Mv> = self.selected.map(|s| self.pos.legal().into_iter().filter(|m| m.from == s).collect()).unwrap_or_default();
        let king_sq = self.pos.in_check().then(|| self.pos.king(self.pos.side)).flatten();
        for row in 0..8 {
            for col in 0..8 {
                let sq = self.square_at(col, row);
                let (x, y) = (MARGIN + col as f32 * SQ, MARGIN + row as f32 * SQ);
                let base = if (col + row) % 2 == 0 { light } else { dark };
                c.fill_rect(x, y, SQ, SQ, &vgrad(y, y + SQ, &[(0.0, lighten(base, 0.08)), (1.0, base)]));
                if self.last.is_some_and(|m| m.from == sq || m.to == sq) {
                    c.fill_rect(x, y, SQ, SQ, &solid(rgba(250, 220, 60, 90)));
                }
                if self.selected == Some(sq) {
                    c.fill_rect(x, y, SQ, SQ, &solid(with_alpha(ui.theme.accent, 0.45)));
                }
                if king_sq == Some(sq) {
                    c.fill_rect(x, y, SQ, SQ, &radial(x + SQ / 2.0, y + SQ / 2.0, SQ * 0.7, &[(0.0, rgba(255, 40, 30, 200)), (1.0, rgba(255, 40, 30, 0))]));
                }
            }
        }
        // Peças
        for sq in 0..64u8 {
            let p = self.pos.b[sq as usize];
            if p == 0 {
                continue;
            }
            let (x, y) = self.screen_of(sq);
            let g = glyph(p);
            let px = SQ * 0.82;
            let tw = ui.fonts.measure(g, px, false);
            let (gx, gy) = (x + (SQ - tw) / 2.0, y + SQ * 0.82);
            ui.fonts.draw(c, g, gx + 1.5, gy + 2.0, px, false, rgba(0, 0, 0, 80));
            if p > 0 {
                // Peça branca: contorno escuro + corpo claro.
                for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, 0.7), (0.7, -0.7), (-0.7, 0.7)] {
                    ui.fonts.draw(c, g, gx + dx * 1.3, gy + dy * 1.3, px, false, rgba(50, 40, 30, 255));
                }
                ui.fonts.draw(c, g, gx, gy, px, false, rgba(252, 250, 244, 255));
            } else {
                for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                    ui.fonts.draw(c, g, gx + dx, gy + dy, px, false, rgba(200, 190, 175, 200));
                }
                ui.fonts.draw(c, g, gx, gy, px, false, rgba(28, 24, 22, 255));
            }
        }
        // Casas possíveis
        for m in legal {
            let (x, y) = self.screen_of(m.to);
            let (cx, cy) = (x + SQ / 2.0, y + SQ / 2.0);
            if self.pos.b[m.to as usize] != 0 {
                c.stroke(circle(cx, cy, SQ * 0.45), &solid(rgba(30, 80, 30, 160)), 3.0);
            } else {
                c.fill(circle(cx, cy, SQ * 0.14), &solid(rgba(30, 80, 30, 140)));
            }
        }
        // Coordenadas
        for i in 0..8 {
            let file = if self.me == 1 { i } else { 7 - i };
            let rank = if self.me == 1 { 8 - i } else { i + 1 };
            let f = ((b'a' + file as u8) as char).to_string();
            let r = rank.to_string();
            ui.fonts.draw(c, &f, MARGIN + i as f32 * SQ + SQ / 2.0 - 3.0, MARGIN + size + 11.0, 10.0, true, rgba(235, 215, 185, 255));
            ui.fonts.draw(c, &r, MARGIN - 10.0, MARGIN + i as f32 * SQ + SQ / 2.0 + 4.0, 10.0, true, rgba(235, 215, 185, 255));
        }
    }
}

impl App for Chess {
    fn ui(&mut self, ui: &mut Ui) {
        let mut acts: Vec<String> = ui.menu_actions().to_vec();
        for k in ui.keys() {
            match k.combo.as_str() {
                "Ctrl+N" => acts.push("new".into()),
                "Ctrl+Z" => acts.push("undo".into()),
                "Ctrl+W" => acts.push("close".into()),
                _ => {}
            }
        }
        for a in acts {
            match a.as_str() {
                "new" => self.restart(self.me),
                "undo" => self.undo(),
                "close" => ui.close = true,
                "flip" => self.restart(-self.me),
                l => {
                    if let Some(i) = LEVELS.iter().position(|(id, _, _)| *id == l) {
                        self.level = i;
                    }
                }
            }
        }

        // Jogada do computador chegou?
        let got = self.answer.lock().unwrap().take();
        if let Some((job, m)) = got {
            if job == self.job && self.think {
                if let Some(m) = m {
                    self.play(m);
                }
                self.think = false;
            }
        }

        for row in 0..8 {
            for col in 0..8 {
                let r = Rect::new(MARGIN + col as f32 * SQ, MARGIN + row as f32 * SQ, SQ, SQ);
                if ui.hit(r).pressed {
                    let sq = self.square_at(col, row);
                    self.click(sq);
                }
            }
        }
        self.draw_board(ui);
        let (w, h) = (ui.c.w, ui.c.h);
        let sy = h - STATUS_H;
        ui.c.fill_rect(0.0, sy, w, STATUS_H, &vgrad(sy, h, &[(0.0, rgba(236, 236, 238, 255)), (1.0, rgba(216, 216, 220, 255))]));
        ui.c.hline(0.0, sy, w, rgba(150, 150, 156, 255));
        let st = self.status();
        let tw = ui.text_width(&st, 13.0, true);
        ui.fonts.draw(ui.c, &st, (w - tw) / 2.0, sy + 20.0, 13.0, true, rgba(30, 30, 30, 255));
    }

    fn watch_fd(&self) -> Option<RawFd> {
        Some(self.wake_r.as_raw_fd())
    }

    fn fd_ready(&mut self) -> bool {
        let mut buf = [0u8; 64];
        while unsafe { libc::read(self.wake_r.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) } > 0 {}
        true
    }

    fn menus(&self) -> Vec<Menu> {
        let mut nivel: Vec<MenuItem> = LEVELS.iter().enumerate().map(|(i, (id, label, _))| MenuItem::new(id, label, "").checked(i == self.level)).collect();
        nivel.push(MenuItem::Separator);
        nivel.push(MenuItem::new("flip", if self.me == 1 { "Jogar com as pretas" } else { "Jogar com as brancas" }, ""));
        vec![
            Menu::new(
                "Jogo",
                vec![
                    MenuItem::new("new", "Novo jogo", "Ctrl+N"),
                    MenuItem::new("undo", "Desfazer jogada", "Ctrl+Z").enabled(!self.history.is_empty()),
                    MenuItem::Separator,
                    MenuItem::new("close", "Fechar", "Ctrl+W"),
                ],
            ),
            Menu::new("Nível", nivel),
        ]
    }
}

fn main() {
    let side = (MARGIN * 2.0 + SQ * 8.0) as u32;
    let opts = WindowOptions { title: "Xadrez".into(), app_id: "glove.Chess".into(), size: (side, side + STATUS_H as u32 + 4), fixed: true, min_size: None };
    if let Err(e) = run(opts, Chess::new(1, 1, pipe(), 0)) {
        eprintln!("glove-chess: {e}");
    }
}
