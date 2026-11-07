//! Grade de apps: todos os apps instalados em tela cheia, em páginas, com um
//! campo de busca no alto e a pasta Utilitários. Abre pelo Dock, pela tecla
//! (F4 por padrão) ou por um canto ativo.
//!
//! Digitar já filtra; as setas escolhem, Enter abre, Esc fecha (ou sai da pasta).
//! A roda do mouse e Page Up/Down trocam de página. Clicar fora dos ícones fecha.
//!
//! Desenho: o campo e a grade são duas texturas (a do campo é pequena, para o
//! cursor piscar sem redesenhar os ícones), sobre a tela escurecida.

use glove_config::tr;
use std::time::{Duration, Instant};

use glove_paint::{draw::*, icons, theme::MENUBAR_H};
use smithay::{
    backend::renderer::element::solid::SolidColorBuffer,
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point, Rectangle},
};

use crate::{
    search::{normalize, score, App},
    shell::{to_tex, Tex},
    state::Glove,
};

const FADE: f32 = 0.18;
const FIELD_W: f32 = 240.0;
const FIELD_H: f32 = 26.0;
const DOTS_H: f32 = 34.0;
/// Pasta Utilitários: quantos ícones pequenos aparecem no quadro dela.
const FOLDER_PEEK: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradeKey {
    Char(char),
    Backspace,
    Enter,
    Escape,
    /// Setas: (colunas, linhas).
    Move(i32, i32),
    Page(i32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Folder,
    App(usize),
}

#[derive(Default)]
pub struct Grade {
    pub open: bool,
    pub alpha: f32,
    anim: Option<(f32, f32, Instant)>,
    pub query: String,
    /// Dentro da pasta Utilitários.
    pub folder: bool,
    pub page: usize,
    pub selected: Option<usize>,
    apps: Vec<App>,
    opened: Option<Instant>,
    timer_gen: u64,
    /// Rolagem acumulada e quando chegou a última; quando virou a página.
    scroll: (f64, Option<Instant>),
    flipped: Option<Instant>,
    pub dim: Option<SolidColorBuffer>,
    pub field_tex: Option<(u64, Tex)>,
    pub grid_tex: Option<(u64, Tex)>,
}

/// Tamanhos e posições, a partir da tela (em pontos lógicos).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub cols: usize,
    pub rows: usize,
    pub tile_w: f32,
    pub tile_h: f32,
    pub icon: f32,
    /// Canto da grade (a textura dela começa aqui).
    pub x: f32,
    pub y: f32,
    pub field_x: f32,
    pub field_y: f32,
}

impl Layout {
    pub fn new(sw: i32, sh: i32) -> Self {
        let (sw, sh) = (sw as f32, sh as f32);
        let icon = if sh >= 900.0 { 72.0 } else { 60.0 };
        let tile_w = icon + 64.0;
        let tile_h = icon + 46.0;
        let field_y = MENUBAR_H as f32 + 22.0;
        let top = field_y + FIELD_H + 34.0;
        let cols = (((sw - 120.0) / tile_w) as usize).clamp(3, 8);
        let rows = (((sh - top - DOTS_H - 30.0) / tile_h) as usize).clamp(2, 6);
        let x = ((sw - cols as f32 * tile_w) / 2.0).round();
        // A grade fica no meio do espaço que sobra embaixo do campo.
        let free = sh - top - DOTS_H - 10.0 - rows as f32 * tile_h;
        let y = (top + (free / 2.0).max(0.0)).round();
        Self { cols, rows, tile_w, tile_h, icon, x, y, field_x: ((sw - FIELD_W) / 2.0).round(), field_y }
    }

    pub fn per_page(&self) -> usize {
        self.cols * self.rows
    }

    pub fn size(&self) -> (f32, f32) {
        (self.cols as f32 * self.tile_w, self.rows as f32 * self.tile_h + DOTS_H)
    }

    /// Ladrilho da página sob o ponto (coordenadas da tela).
    pub fn tile_at(&self, px: f32, py: f32) -> Option<usize> {
        let (x, y) = (px - self.x, py - self.y);
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let (c, r) = ((x / self.tile_w) as usize, (y / self.tile_h) as usize);
        (c < self.cols && r < self.rows).then_some(r * self.cols + c)
    }

    /// Bolinha de página sob o ponto.
    pub fn dot_at(&self, px: f32, py: f32, pages: usize) -> Option<usize> {
        let (w, h) = self.size();
        let cy = self.y + h - DOTS_H / 2.0;
        if pages < 2 || (py - cy).abs() > 9.0 {
            return None;
        }
        let x0 = self.x + w / 2.0 - (pages as f32 - 1.0) * 9.0;
        (0..pages).find(|&i| (px - (x0 + i as f32 * 18.0)).abs() <= 8.0)
    }
}

/// Os itens a mostrar: a busca (todos os apps), a pasta aberta ou a tela principal
/// (a pasta Utilitários primeiro, depois os outros apps em ordem alfabética).
fn items(apps: &[App], query: &str, folder: bool) -> Vec<Item> {
    let q = normalize(query.trim());
    if !q.is_empty() {
        let mut v: Vec<(u32, usize)> = apps.iter().enumerate().map(|(i, a)| (score(&normalize(&a.name), &q).max(score(&a.keywords, &q) / 2), i)).filter(|(s, _)| *s > 0).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0).then(apps[a.1].name.len().cmp(&apps[b.1].name.len())));
        return v.into_iter().map(|(_, i)| Item::App(i)).collect();
    }
    let mut out: Vec<Item> = Vec::new();
    if !folder && apps.iter().any(|a| a.utility) {
        out.push(Item::Folder);
    }
    out.extend(apps.iter().enumerate().filter(|(_, a)| a.utility == folder).map(|(i, _)| Item::App(i)));
    out
}

/// Lista ordenada pelo nome, sem acentos (e sem o próprio item da grade).
fn sorted_apps(mut apps: Vec<App>) -> Vec<App> {
    apps.retain(|a| a.glove_id.as_deref() != Some(glove_config::GRADE_ID));
    apps.sort_by_key(|a| normalize(&a.name));
    apps
}

impl Glove {
    pub fn toggle_grade(&mut self) {
        if self.grade.open {
            self.close_grade();
        } else {
            self.open_grade();
        }
    }

    pub fn open_grade(&mut self) {
        if self.grade.open || self.greeter || self.lock.is_some() {
            return;
        }
        self.close_menu();
        self.close_search();
        self.close_dashboard();
        self.close_volume_slider();
        // O Dock volta ao tamanho normal (o mouse agora é da grade).
        if self.chrome.dock_hover_x.take().is_some() {
            self.chrome.dock_hover = None;
            self.chrome.animate_dock_zoom(0.0);
        }
        let g = &mut self.grade;
        g.open = true;
        g.query.clear();
        g.folder = false;
        g.page = 0;
        g.selected = None;
        g.apps = sorted_apps(crate::search::installed_apps(&self.config.dock_apps));
        g.opened = Some(Instant::now());
        g.anim = Some((g.alpha, 1.0, Instant::now()));
        // Pisca o cursor do campo enquanto aberta.
        g.timer_gen += 1;
        let generation = g.timer_gen;
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_millis(530)), move |_, _, data| {
            let st = &mut data.state;
            if !st.grade.open || st.grade.timer_gen != generation {
                return TimeoutAction::Drop;
            }
            st.dirty = true;
            TimeoutAction::ToDuration(Duration::from_millis(530))
        });
        self.dirty = true;
    }

    pub fn close_grade(&mut self) {
        if !self.grade.open {
            return;
        }
        self.grade.open = false;
        self.grade.anim = Some((self.grade.alpha, 0.0, Instant::now()));
        self.dirty = true;
    }

    /// A grade cobre a tela (para o mouse e o teclado).
    pub fn grade_active(&self) -> bool {
        self.grade.open
    }

    fn grade_items(&self) -> Vec<Item> {
        items(&self.grade.apps, &self.grade.query, self.grade.folder)
    }

    fn grade_layout(&self) -> Layout {
        let (sw, sh) = self.output_size();
        Layout::new(sw, sh)
    }

    fn grade_pages(&self) -> usize {
        self.grade_items().len().div_ceil(self.grade_layout().per_page()).max(1)
    }

    fn grade_set_page(&mut self, page: usize) {
        let page = page.min(self.grade_pages() - 1);
        if page != self.grade.page {
            self.grade.page = page;
            self.grade.selected = None;
            self.dirty = true;
        }
    }

    fn grade_open_item(&mut self, i: usize) {
        match self.grade_items().get(i).copied() {
            Some(Item::Folder) => {
                self.grade.folder = true;
                self.grade.page = 0;
                self.grade.selected = None;
                self.dirty = true;
            }
            Some(Item::App(a)) => {
                let app = self.grade.apps[a].clone();
                self.close_grade();
                match app.glove_id.as_deref().and_then(|id| self.config.dock_apps.iter().position(|d| d.app_id == id)) {
                    Some(idx) => self.run_dock_action(crate::shell::DockAction::App(idx)),
                    None => self.spawn(&app.exec),
                }
            }
            None => {}
        }
    }

    pub fn grade_key(&mut self, k: GradeKey) {
        let per = self.grade_layout().per_page();
        let cols = self.grade_layout().cols as i32;
        let n = self.grade_items().len();
        match k {
            GradeKey::Char(c) if !c.is_control() && self.grade.query.len() < 100 => {
                self.grade.query.push(c);
                self.grade.page = 0;
                self.grade.selected = (!self.grade_items().is_empty()).then_some(0);
            }
            GradeKey::Backspace => {
                self.grade.query.pop();
                self.grade.page = 0;
                self.grade.selected = (!self.grade.query.is_empty() && !self.grade_items().is_empty()).then_some(0);
            }
            GradeKey::Escape => {
                // Esc apaga a busca, depois sai da pasta, depois fecha.
                if !self.grade.query.is_empty() {
                    self.grade.query.clear();
                    self.grade.selected = None;
                    self.grade.page = 0;
                } else if self.grade.folder {
                    self.grade.folder = false;
                    self.grade.selected = None;
                    self.grade.page = 0;
                } else {
                    self.close_grade();
                }
            }
            GradeKey::Enter => {
                let i = self.grade.selected.or((!self.grade.query.is_empty()).then_some(0));
                if let Some(i) = i {
                    self.grade_open_item(i);
                }
            }
            GradeKey::Move(dx, dy) if n > 0 => {
                let cur = self.grade.selected.map(|s| s as i32).unwrap_or(-1);
                let next = if cur < 0 { (self.grade.page * per) as i32 } else { cur + dx + dy * cols };
                if (0..n as i32).contains(&next) {
                    self.grade.selected = Some(next as usize);
                    self.grade.page = next as usize / per;
                }
            }
            GradeKey::Page(d) => {
                let p = (self.grade.page as i32 + d).max(0) as usize;
                self.grade_set_page(p);
            }
            _ => {}
        }
        self.dirty = true;
    }

    /// Roda do mouse (ou dois dedos no touchpad): uma página por "clique".
    pub fn grade_scroll(&mut self, v: f64) {
        // Soma os movimentos (o touchpad manda muitos pequenos) e vira uma página
        // por gesto: depois de virar, espera um pouco antes da próxima.
        let now = Instant::now();
        let g = &mut self.grade;
        if g.scroll.1.is_some_and(|t| now.duration_since(t) > Duration::from_millis(350)) {
            g.scroll.0 = 0.0;
        }
        g.scroll.0 += v;
        let recent = g.flipped.is_some_and(|t| now.duration_since(t) < Duration::from_millis(400));
        g.scroll.1 = Some(now);
        if g.scroll.0.abs() >= 12.0 && !recent {
            let d = if g.scroll.0 > 0.0 { 1 } else { -1 };
            g.scroll.0 = 0.0;
            g.flipped = Some(now);
            self.grade_key(GradeKey::Page(d));
        }
    }

    pub fn grade_motion(&mut self, pos: Point<f64, Logical>) {
        let l = self.grade_layout();
        let n = self.grade_items().len();
        let sel = l.tile_at(pos.x as f32, pos.y as f32).map(|t| self.grade.page * l.per_page() + t).filter(|&i| i < n);
        if sel.is_some() && sel != self.grade.selected {
            self.grade.selected = sel;
            self.dirty = true;
        }
    }

    pub fn grade_button(&mut self, pos: Point<f64, Logical>) {
        let l = self.grade_layout();
        let (x, y) = (pos.x as f32, pos.y as f32);
        let n = self.grade_items().len();
        if let Some(i) = l.tile_at(x, y).map(|t| self.grade.page * l.per_page() + t).filter(|&i| i < n) {
            self.grade_open_item(i);
            return;
        }
        if let Some(p) = l.dot_at(x, y, self.grade_pages()) {
            self.grade_set_page(p);
            return;
        }
        // O campo de busca não fecha.
        if Rectangle::<f32, Logical>::new((l.field_x, l.field_y).into(), (FIELD_W, FIELD_H).into()).contains((x, y)) {
            return;
        }
        // Clicar fora: sai da pasta ou fecha.
        if self.grade.folder && self.grade.query.is_empty() {
            self.grade.folder = false;
            self.grade.page = 0;
            self.grade.selected = None;
            self.dirty = true;
        } else {
            self.close_grade();
        }
    }

    /// Onde ficam o campo e a grade.
    pub fn grade_field_pos(&self) -> Point<i32, Logical> {
        let l = self.grade_layout();
        (l.field_x as i32, l.field_y as i32).into()
    }

    pub fn grade_grid_pos(&self) -> Point<i32, Logical> {
        let l = self.grade_layout();
        (l.x as i32, l.y as i32).into()
    }

    // -------------------------------------------------------------------
    // Desenho
    // -------------------------------------------------------------------

    pub fn prepare_grade(&mut self) {
        if let Some((from, to, start)) = self.grade.anim {
            let t = (start.elapsed().as_secs_f32() / FADE).min(1.0);
            self.grade.alpha = from + (to - from) * t * t * (3.0 - 2.0 * t);
            if t >= 1.0 {
                self.grade.anim = None;
            } else {
                self.dirty = true;
            }
        }
        if self.grade.alpha <= 0.0 {
            // Fechada: solta a memória das texturas e da lista.
            if !self.grade.open && self.grade.grid_tex.is_some() {
                self.grade.grid_tex = None;
                self.grade.field_tex = None;
                self.grade.apps = Vec::new();
            }
            return;
        }
        let sc = self.scale() as f32;
        let (sw, sh) = self.output_size();
        let a = 0.8 * self.grade.alpha;
        match &mut self.grade.dim {
            Some(d) => {
                d.resize((sw, sh));
                d.set_color([0.02, 0.03, 0.06, a]);
            }
            None => self.grade.dim = Some(SolidColorBuffer::new((sw, sh), [0.02, 0.03, 0.06, a])),
        }
        let l = Layout::new(sw, sh);
        use std::hash::{Hash, Hasher};
        let base = self.theme_gen ^ (sc.to_bits() as u64);

        // Campo de busca (o cursor pisca).
        let caret = self.grade.opened.map(|t| (t.elapsed().as_millis() / 530) % 2 == 0).unwrap_or(true);
        let mut k = std::collections::hash_map::DefaultHasher::new();
        (base, &self.grade.query, caret).hash(&mut k);
        let key = k.finish();
        if self.grade.field_tex.as_ref().map(|t| t.0) != Some(key) {
            self.grade.field_tex = draw_field(&self.fonts, &self.grade.query, caret, sc).map(|c| (key, to_tex(c)));
        }

        // A grade: refeita só quando muda a página, a escolha, a busca ou a tela.
        let items = self.grade_items();
        let pages = items.len().div_ceil(l.per_page()).max(1);
        let page = self.grade.page.min(pages - 1);
        let mut k = std::collections::hash_map::DefaultHasher::new();
        (base, &self.grade.query, self.grade.folder, page, self.grade.selected, sw, sh, self.grade.apps.len()).hash(&mut k);
        let key = k.finish();
        if self.grade.grid_tex.as_ref().map(|t| t.0) != Some(key) {
            let shown: Vec<Item> = items.iter().skip(page * l.per_page()).take(l.per_page()).copied().collect();
            let sel = self.grade.selected.and_then(|s| s.checked_sub(page * l.per_page())).filter(|s| *s < l.per_page());
            let empty = items.is_empty();
            self.grade.grid_tex = draw_grid(&self.fonts, &l, &self.grade.apps, &shown, sel, page, pages, self.grade.folder && self.grade.query.is_empty(), empty, sc).map(|c| (key, to_tex(c)));
        }
    }
}

fn shadowed(c: &mut Canvas, f: &glove_paint::font::Fonts, t: &str, x: f32, y: f32, px: f32, bold: bool) {
    f.draw(c, t, x, y + 1.0, px, bold, rgba(0, 0, 0, 170));
    f.draw(c, t, x, y, px, bold, rgba(255, 255, 255, 255));
}

fn draw_field(f: &glove_paint::font::Fonts, query: &str, caret: bool, sc: f32) -> Option<Canvas> {
    let (w, h) = (FIELD_W, FIELD_H);
    let mut c = Canvas::new(w, h, sc)?;
    let r = h / 2.0;
    c.fill(rrect(0.5, 0.5, w - 1.0, h - 1.0, r), &vgrad(0.0, h, &[(0.0, rgba(255, 255, 255, 60)), (1.0, rgba(255, 255, 255, 34))]));
    c.stroke(rrect(0.5, 0.5, w - 1.0, h - 1.0, r), &solid(rgba(255, 255, 255, 120)), 1.0);
    // Lupa.
    let (cx, cy) = (16.0, h / 2.0 - 1.0);
    c.stroke(circle(cx, cy, 5.0), &solid(rgba(255, 255, 255, 220)), 1.6);
    // O cabo: bolinhas em diagonal (fica liso nesse tamanho).
    for k in 0..6 {
        let d = 3.8 + k as f32 * 0.75;
        c.fill(circle(cx + d, cy + d, 1.05), &solid(rgba(255, 255, 255, 220)));
    }
    let px = 13.0;
    let tx = 30.0;
    let base = h / 2.0 + 4.5;
    if query.is_empty() {
        f.draw(&mut c, tr!("Buscar"), tx, base, px, false, rgba(255, 255, 255, 150));
    }
    let shown = f.ellipsize(query, px, false, w - tx - 14.0);
    f.draw(&mut c, &shown, tx, base, px, false, rgba(255, 255, 255, 255));
    if caret {
        let cw = f.measure(&shown, px, false);
        c.fill_rect((tx + cw + 1.0).round(), 6.0, 1.0, h - 12.0, &solid(rgba(255, 255, 255, 230)));
    }
    Some(c)
}

#[allow(clippy::too_many_arguments)]
fn draw_grid(f: &glove_paint::font::Fonts, l: &Layout, apps: &[App], shown: &[Item], sel: Option<usize>, page: usize, pages: usize, in_folder: bool, empty: bool, sc: f32) -> Option<Canvas> {
    let (w, h) = l.size();
    let mut c = Canvas::new(w, h, sc)?;
    let grid_h = h - DOTS_H;
    if in_folder {
        // A pasta aberta: uma faixa de vidro escuro com o nome.
        c.fill(rrect(4.0, 0.0, w - 8.0, grid_h, 18.0), &solid(rgba(255, 255, 255, 22)));
        c.stroke(rrect(4.5, 0.5, w - 9.0, grid_h - 1.0, 18.0), &solid(rgba(255, 255, 255, 50)), 1.0);
    }
    if empty {
        let t = tr!("Nenhum app encontrado");
        let tw = f.measure(t, 16.0, true);
        shadowed(&mut c, f, t, (w - tw) / 2.0, grid_h / 2.0, 16.0, true);
    }
    let s = l.icon;
    for (i, item) in shown.iter().enumerate() {
        let (col, row) = (i % l.cols, i / l.cols);
        let (tx, ty) = (col as f32 * l.tile_w, row as f32 * l.tile_h);
        if sel == Some(i) {
            c.fill(rrect(tx + 6.0, ty + 2.0, l.tile_w - 12.0, l.tile_h - 4.0, 12.0), &solid(rgba(255, 255, 255, 46)));
        }
        let (ix, iy) = (tx + (l.tile_w - s) / 2.0, ty + 10.0);
        let name = match *item {
            Item::Folder => {
                draw_folder(&mut c, f, apps, ix, iy, s);
                tr!("Utilitários").to_string()
            }
            Item::App(a) => {
                let app = &apps[a];
                icons::draw(&mut c, f, app.icon, ix, iy, s, app.name.chars().next());
                app.name.clone()
            }
        };
        let px = 12.5;
        let label = f.ellipsize(&name, px, false, l.tile_w - 10.0);
        let lw = f.measure(&label, px, false);
        shadowed(&mut c, f, &label, tx + (l.tile_w - lw) / 2.0, iy + s + 18.0, px, false);
    }
    if pages > 1 {
        let cy = h - DOTS_H / 2.0;
        let x0 = w / 2.0 - (pages as f32 - 1.0) * 9.0;
        for p in 0..pages {
            let a = if p == page { 240 } else { 90 };
            c.fill(circle(x0 + p as f32 * 18.0, cy, 3.5), &solid(rgba(255, 255, 255, a)));
        }
    }
    Some(c)
}

/// O quadro da pasta Utilitários: vidro arredondado com os ícones dela em miniatura.
fn draw_folder(c: &mut Canvas, f: &glove_paint::font::Fonts, apps: &[App], x: f32, y: f32, s: f32) {
    c.fill(rrect(x + 2.0, y + 4.0, s - 4.0, s - 4.0, s * 0.2), &solid(rgba(0, 0, 0, 50)));
    c.fill(rrect(x + 2.0, y + 2.0, s - 4.0, s - 4.0, s * 0.2), &vgrad(y, y + s, &[(0.0, rgba(225, 230, 240, 120)), (1.0, rgba(170, 180, 200, 90))]));
    c.stroke(rrect(x + 2.5, y + 2.5, s - 5.0, s - 5.0, s * 0.2), &solid(rgba(255, 255, 255, 140)), 1.0);
    let m = s * 0.14;
    let cell = (s - 2.0 * m) / 3.0;
    for (k, a) in apps.iter().filter(|a| a.utility).take(FOLDER_PEEK).enumerate() {
        let (cx, cy) = (x + m + (k % 3) as f32 * cell, y + m + (k / 3) as f32 * cell);
        icons::draw_bare(c, f, a.icon, cx + cell * 0.08, cy + cell * 0.08, cell * 0.84, a.name.chars().next());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glove_paint::icons::Icon;

    fn app(name: &str, utility: bool) -> App {
        App { name: name.into(), keywords: String::new(), exec: name.to_lowercase(), icon: Icon::Window, glove_id: None, utility }
    }

    #[test]
    fn itens() {
        let apps = sorted_apps(vec![app("Zeta", false), app("Monitor", true), app("Alfa", false)]);
        assert_eq!(apps[0].name, "Alfa");
        // Tela principal: a pasta e os apps comuns; dentro da pasta, só os utilitários.
        assert_eq!(items(&apps, "", false), vec![Item::Folder, Item::App(0), Item::App(2)]);
        assert_eq!(items(&apps, "", true), vec![Item::App(1)]);
        // A busca procura em todos, utilitários inclusive.
        assert_eq!(items(&apps, "mon", false), vec![Item::App(1)]);
        assert!(items(&apps, "xyz", false).is_empty());
    }

    #[test]
    fn grade() {
        let l = Layout::new(1280, 800);
        assert!(l.cols >= 3 && l.rows >= 2);
        assert!(l.x >= 0.0 && l.x + l.size().0 <= 1280.0);
        assert!(l.y + l.size().1 <= 800.0);
        assert_eq!(l.tile_at(l.x + 1.0, l.y + 1.0), Some(0));
        assert_eq!(l.tile_at(l.x + l.tile_w + 1.0, l.y + l.tile_h + 1.0), Some(l.cols + 1));
        assert_eq!(l.tile_at(l.x - 1.0, l.y), None);
    }
}
