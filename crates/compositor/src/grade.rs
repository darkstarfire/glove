//! Grade de apps ("Todos os Apps"): todos os apps instalados em tela cheia, em
//! páginas, sobre o papel de parede desfocado (o Dock continua na frente; a barra
//! de menu, as janelas e os ícones da mesa somem). Abre pelo Dock, pela tecla (F4
//! por padrão) ou por um canto ativo.
//!
//! - Digitar já filtra; as setas escolhem, Enter abre, Esc fecha (ou sai da pasta).
//! - A roda do mouse e Page Up/Down trocam de página. Clicar fora dos ícones fecha.
//! - Pastas: clicar abre uma faixa de tecido escuro embaixo da linha da pasta (a
//!   grade se abre em duas, com um bico apontando para a pasta); o nome, no canto
//!   da faixa, muda com um clique.
//! - Modo de edição: segurar um ícone (ou arrastá-lo) faz todos balançarem.
//!   Arrastar muda a ordem e, na borda, passa de página; soltar um app sobre outro
//!   (parando um instante em cima) cria uma pasta, e sobre uma pasta põe o app nela;
//!   arrastar para fora da faixa tira o app da pasta (a pasta vazia some). Esc ou
//!   um clique no vazio sai do modo. A arrumação fica em `~/.config/glove/grade.json`.
//!
//! Desenho: o campo e a grade são duas texturas (a do campo é pequena, para o
//! cursor piscar sem redesenhar os ícones). Cada ícone com o nome vira uma imagem
//! guardada; a grade só as compõe (girando um pouco no modo de edição), então o
//! balanço custa pouco.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use glove_config::tr;
use glove_paint::tiny_skia::{self, FilterQuality, PathBuilder, Pixmap, PixmapPaint, Transform};
use glove_paint::{draw::*, font::Fonts, icons};
use smithay::{
    reexports::calloop::timer::{TimeoutAction, Timer},
    utils::{Logical, Point, Rectangle},
};

use crate::{
    search::{normalize, score, App},
    shell::{to_tex, Tex},
    state::Glove,
};

const FADE: f32 = 0.18;
/// Troca de página: as duas páginas deslizam de lado (segundos).
/// Duração do deslize entre páginas (segundos; quase nada com "Reduzir efeitos").
fn slide() -> f32 {
    glove_config::anim(std::time::Duration::from_millis(320)).as_secs_f32()
}
const FIELD_W: f32 = 240.0;
const FIELD_H: f32 = 26.0;
const DOTS_H: f32 = 34.0;
/// Quantos ícones pequenos aparecem no quadro de uma pasta.
const FOLDER_PEEK: usize = 9;
/// Segurar o ícone este tempo: modo de edição.
const HOLD: Duration = Duration::from_millis(500);
/// Andar isto (pontos) com o botão apertado: começa a arrastar.
const DRAG_START: f32 = 6.0;
/// Parado sobre um lugar: abre espaço; sobre um ícone: vai virar pasta; na borda: vira a página.
const DWELL_MOVE: Duration = Duration::from_millis(160);
const DWELL_MERGE: Duration = Duration::from_millis(450);
const DWELL_EDGE: Duration = Duration::from_millis(650);
/// Faixa da pasta: altura do título e a margem de baixo.
const BAND_TITLE: f32 = 36.0;
const BAND_PAD: f32 = 12.0;
/// A faixa abre ou fecha neste tempo (s).
const FOLD: f32 = 0.22;
/// Quadros por segundo no modo de edição e nas animações (~30).
const TICK: Duration = Duration::from_millis(33);
const FILE: &str = "grade.json";

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

/// Uma pasta: nome (None = o padrão), se é a dos utilitários (que recebe os
/// utilitários novos) e os apps, na ordem.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Folder {
    id: u64,
    name: Option<String>,
    utilities: bool,
    apps: Vec<usize>,
}

impl Folder {
    fn label(&self) -> String {
        match &self.name {
            Some(n) => n.clone(),
            None if self.utilities => tr!("Utilitários").to_string(),
            None => tr!("Nova Pasta").to_string(),
        }
    }
}

/// Um lugar da grade: um app ou uma pasta.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Entry {
    App(usize),
    Folder(Folder),
}

impl Entry {
    fn item(&self) -> Item {
        match self {
            Entry::App(a) => Item::App(*a),
            Entry::Folder(f) => Item::Folder(f.id),
        }
    }
}

/// O que um ladrilho mostra (a pasta pelo número dela, que não muda ao arrumar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Item {
    App(usize),
    Folder(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    /// Índice na lista da grade.
    Main(usize),
    /// Índice na pasta aberta.
    Band(usize),
    /// O nome da pasta aberta.
    Title,
    /// A faixa, fora dos ícones.
    BandEmpty,
}

struct Press {
    at: (f32, f32),
    hit: Hit,
    serial: u64,
    moved: bool,
    /// Virou modo de edição por segurar: soltar não abre nada.
    held: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Origin {
    Main(usize),
    Folder(u64, usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cand {
    /// Abrir espaço neste lugar (índice da lista com o espaço).
    Move(usize),
    /// Juntar com o item deste índice (da lista sem o arrastado).
    Merge(usize),
}

struct Drag {
    entry: Entry,
    origin: Origin,
    /// Dentro da faixa da pasta aberta.
    in_folder: bool,
    pos: (f32, f32),
    /// Onde o ladrilho foi pego (relativo ao canto dele).
    grab: (f32, f32),
    /// Onde ele entra (índice na lista sem ele).
    gap: usize,
    cand: Option<(Cand, Instant)>,
    merge: Option<usize>,
    edge: Option<(i32, Instant)>,
}

#[derive(Default)]
pub struct Grade {
    pub open: bool,
    pub alpha: f32,
    anim: Option<(f32, f32, Instant)>,
    pub query: String,
    pub page: usize,
    /// Escolhido pelo teclado ou pelo mouse: índice na pasta aberta ou na grade.
    pub selected: Option<usize>,
    apps: Vec<App>,
    top: Vec<Entry>,
    next_id: u64,
    /// Pasta aberta, quanto a faixa abriu (0 a 1) e a animação; fechando.
    folder: Option<u64>,
    fold: f32,
    fold_anim: Option<(f32, f32, Instant)>,
    closing: bool,
    edit: bool,
    edit_since: Option<Instant>,
    press: Option<Press>,
    press_gen: u64,
    drag: Option<Drag>,
    /// Renomeando a pasta aberta: o texto e se o próximo caractere troca tudo.
    rename: Option<(String, bool)>,
    opened: Option<Instant>,
    timer_gen: u64,
    ticking: bool,
    /// Rolagem acumulada e quando chegou a última; quando virou a página.
    scroll: (f64, Option<Instant>),
    flipped: Option<Instant>,
    /// Ícone + nome de cada item, prontos para compor (com a chave do conteúdo).
    tiles: HashMap<Item, (u64, Canvas)>,
    tiles_base: u64,
    /// Posição desenhada de cada item (desliza até o lugar no modo de edição).
    pos: HashMap<Item, (f32, f32)>,
    easing: bool,
    last_frame: Option<Instant>,
    /// Muda a cada arrumação (para refazer a grade).
    ver: u64,
    pub bg: Option<((i32, i32, u64), Tex)>,
    pub field_tex: Option<(u64, Tex)>,
    pub grid_tex: Option<(u64, Tex)>,
    /// Bolinhas das páginas (paradas enquanto as páginas deslizam).
    pub dots_tex: Option<(u64, Tex)>,
    /// Página saindo: a imagem dela, o lado (1 = foi para a próxima) e o início.
    pub slide: Option<(Tex, f32, Instant)>,
}

/// Tamanhos e posições, a partir da área livre (sem o Dock), em pontos lógicos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub cols: usize,
    pub rows: usize,
    pub tile_w: f32,
    pub tile_h: f32,
    pub icon: f32,
    /// Canto da grade (na tela).
    pub x: f32,
    pub y: f32,
    pub field_x: f32,
    pub field_y: f32,
    /// A textura da grade (na tela): x, y, largura, altura.
    pub tex: (f32, f32, f32, f32),
}

impl Layout {
    pub fn new(area: Rectangle<i32, Logical>) -> Self {
        let (ax, ay, aw, ah) = (area.loc.x as f32, area.loc.y as f32, area.size.w as f32, area.size.h as f32);
        let icon = if ah >= 860.0 { 72.0 } else { 60.0 };
        let tile_w = icon + 64.0;
        let tile_h = icon + 46.0;
        let field_y = ay + 22.0;
        let tex_y = (field_y + FIELD_H + 8.0).round();
        let top = field_y + FIELD_H + 34.0;
        let bottom = ay + ah;
        let cols = (((aw - 120.0) / tile_w) as usize).clamp(3, 8);
        let rows = (((bottom - top - DOTS_H - 20.0) / tile_h) as usize).clamp(2, 6);
        let x = (ax + (aw - cols as f32 * tile_w) / 2.0).round();
        // A grade fica no meio do espaço que sobra embaixo do campo.
        let free = bottom - top - DOTS_H - 10.0 - rows as f32 * tile_h;
        let y = (top + (free / 2.0).max(0.0)).round();
        Self { cols, rows, tile_w, tile_h, icon, x, y, field_x: (ax + (aw - FIELD_W) / 2.0).round(), field_y, tex: (ax, tex_y, aw, (bottom - tex_y).max(1.0)) }
    }

    pub fn per_page(&self) -> usize {
        self.cols * self.rows
    }

    fn slot_pos(&self, s: usize) -> (f32, f32) {
        (self.x + (s % self.cols) as f32 * self.tile_w, self.y + (s / self.cols) as f32 * self.tile_h)
    }

    /// Centro do ícone de um ladrilho que começa em (x, y).
    fn icon_center(&self, x: f32, y: f32) -> (f32, f32) {
        (x + self.tile_w / 2.0, y + 10.0 + self.icon / 2.0)
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

    fn dots_y(&self) -> f32 {
        self.y + self.rows as f32 * self.tile_h + DOTS_H / 2.0
    }

    /// Bolinha de página sob o ponto.
    pub fn dot_at(&self, px: f32, py: f32, pages: usize) -> Option<usize> {
        if pages < 2 || (py - self.dots_y()).abs() > 9.0 {
            return None;
        }
        let x0 = self.x + self.cols as f32 * self.tile_w / 2.0 - (pages as f32 - 1.0) * 9.0;
        (0..pages).find(|&i| (px - (x0 + i as f32 * 18.0)).abs() <= 8.0)
    }
}

/// A faixa da pasta aberta (na tela, já com o quanto abriu).
#[derive(Clone, Copy, Debug)]
struct Band {
    top: f32,
    h: f32,
    full_h: f32,
    /// Quanto a parte de cima sobe (quando a faixa não cabe embaixo).
    up: f32,
    /// Onde a grade se abre (antes de subir).
    split: f32,
    notch_x: f32,
}

impl Band {
    fn shift(&self, y: f32) -> f32 {
        if y < self.split { y - self.up } else { y + self.h - self.up }
    }
}

/// O que está na tela: os lugares da página (None = o espaço do arrastado) e os da faixa.
struct Slots {
    main: Vec<(Option<Item>, f32, f32)>,
    band: Vec<(Option<usize>, f32, f32)>,
    band_geo: Option<Band>,
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn in_tile(l: &Layout, x: f32, y: f32, tx: f32, ty: f32) -> bool {
    x >= tx && x < tx + l.tile_w && y >= ty && y < ty + l.tile_h
}

fn app_key(a: &App) -> String {
    a.glove_id.clone().unwrap_or_else(|| format!("exec:{}", a.exec))
}

/// A arrumação: a guardada (sem os apps que saíram) mais os apps novos; sem nada
/// guardado, a pasta Utilitários e os outros apps em ordem alfabética. Utilitário
/// novo vai para a pasta dos utilitários.
fn arrange(apps: &[App], saved: Option<&serde_json::Value>, next_id: &mut u64) -> Vec<Entry> {
    let mut id = || {
        *next_id += 1;
        *next_id
    };
    let index: HashMap<String, usize> = apps.iter().enumerate().map(|(i, a)| (app_key(a), i)).collect();
    let mut used = vec![false; apps.len()];
    let mut take = |k: &str| index.get(k).copied().filter(|&i| !std::mem::replace(&mut used[i], true));
    let mut out = Vec::new();
    match saved.and_then(|v| v.get("itens")).and_then(|v| v.as_array()) {
        Some(list) => {
            for it in list {
                if let Some(i) = it.as_str().and_then(&mut take) {
                    out.push(Entry::App(i));
                } else if let Some(keys) = it.get("apps").and_then(|v| v.as_array()) {
                    let name = it.get("nome").and_then(|v| v.as_str()).map(String::from);
                    let utilities = it.get("utilitarios").and_then(|v| v.as_bool()).unwrap_or(false);
                    let apps = keys.iter().filter_map(|k| k.as_str()).filter_map(&mut take).collect();
                    out.push(Entry::Folder(Folder { id: id(), name, utilities, apps }));
                }
            }
        }
        None if apps.iter().any(|a| a.utility) => out.push(Entry::Folder(Folder { id: id(), name: None, utilities: true, apps: Vec::new() })),
        None => {}
    }
    for (i, a) in apps.iter().enumerate() {
        if take(&app_key(a)).is_none() {
            continue;
        }
        let home = if a.utility {
            out.iter_mut().find_map(|e| match e {
                Entry::Folder(f) if f.utilities => Some(f),
                _ => None,
            })
        } else {
            None
        };
        match home {
            Some(f) => f.apps.push(i),
            None => out.push(Entry::App(i)),
        }
    }
    out.retain(|e| !matches!(e, Entry::Folder(f) if f.apps.is_empty()));
    out
}

fn to_json(top: &[Entry], apps: &[App]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = top
        .iter()
        .map(|e| match e {
            Entry::App(i) => serde_json::Value::String(app_key(&apps[*i])),
            Entry::Folder(f) => serde_json::json!({
                "nome": f.name,
                "utilitarios": f.utilities,
                "apps": f.apps.iter().map(|i| app_key(&apps[*i])).collect::<Vec<_>>(),
            }),
        })
        .collect();
    serde_json::json!({ "itens": items })
}

/// Os apps de uma busca, do mais parecido para o menos.
fn search(apps: &[App], query: &str) -> Vec<usize> {
    let q = normalize(query.trim());
    let mut v: Vec<(u32, usize)> = apps.iter().enumerate().map(|(i, a)| (score(&normalize(&a.name), &q).max(score(&a.keywords, &q) / 2), i)).filter(|(s, _)| *s > 0).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(apps[a.1].name.len().cmp(&apps[b.1].name.len())));
    v.into_iter().map(|(_, i)| i).collect()
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
        let saved = std::fs::read_to_string(glove_config::dir().join(FILE)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let g = &mut self.grade;
        g.open = true;
        g.query.clear();
        g.page = 0;
        g.selected = None;
        g.folder = None;
        g.fold = 0.0;
        g.fold_anim = None;
        g.closing = false;
        g.edit = false;
        g.press = None;
        g.drag = None;
        g.rename = None;
        g.pos.clear();
        g.apps = sorted_apps(crate::search::installed_apps(&self.config.dock_apps));
        g.top = arrange(&g.apps, saved.as_ref(), &mut g.next_id);
        g.ver += 1;
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
        self.grade_cancel_drag();
        self.grade_commit_rename();
        self.grade.edit = false;
        self.grade.press = None;
        self.grade.open = false;
        self.grade.anim = Some((self.grade.alpha, 0.0, Instant::now()));
        self.dirty = true;
    }

    /// A grade cobre a tela (para o mouse e o teclado).
    pub fn grade_active(&self) -> bool {
        self.grade.open
    }

    /// Botão apertado ou arrastando na grade (o mouse não vai para o Dock no meio).
    pub fn grade_busy(&self) -> bool {
        self.grade.press.is_some() || self.grade.drag.is_some()
    }

    fn searching(&self) -> bool {
        !self.grade.query.trim().is_empty()
    }

    /// A grade agora: a busca ou a arrumação.
    fn main_list(&self) -> Vec<Item> {
        if self.searching() {
            return search(&self.grade.apps, &self.grade.query).into_iter().map(Item::App).collect();
        }
        self.grade.top.iter().map(Entry::item).collect()
    }

    fn folder_ref(&self) -> Option<&Folder> {
        let id = self.grade.folder?;
        self.grade.top.iter().find_map(|e| match e {
            Entry::Folder(f) if f.id == id => Some(f),
            _ => None,
        })
    }

    fn folder_mut(&mut self) -> Option<&mut Folder> {
        let id = self.grade.folder?;
        self.grade.top.iter_mut().find_map(|e| match e {
            Entry::Folder(f) if f.id == id => Some(f),
            _ => None,
        })
    }

    /// A pasta está aberta (e não fechando)?
    fn folder_open(&self) -> bool {
        self.grade.folder.is_some() && !self.grade.closing && !self.searching()
    }

    fn grade_layout(&self) -> Layout {
        Layout::new(self.work_area())
    }

    fn grade_pages(&self) -> usize {
        let extra = usize::from(self.grade.drag.as_ref().is_some_and(|d| !d.in_folder));
        (self.main_list().len() + extra).div_ceil(self.grade_layout().per_page()).max(1)
    }

    fn grade_set_page(&mut self, page: usize) {
        let page = page.min(self.grade_pages() - 1);
        if page != self.grade.page {
            if self.grade.folder.is_some() {
                self.grade_close_folder_now();
            }
            // A página que sai desliza para um lado e a nova entra pelo outro
            // (só as imagens prontas andam; nada é redesenhado).
            if self.grade.open && self.grade.drag.is_none() {
                if let Some((_, old)) = self.grade.grid_tex.take() {
                    let dir = if page > self.grade.page { 1.0 } else { -1.0 };
                    self.grade.slide = Some((old, dir, Instant::now()));
                }
            }
            self.grade.page = page;
            self.grade.selected = None;
            self.grade.pos.clear();
            self.dirty = true;
        }
    }

    fn grade_save(&mut self) {
        self.grade.ver += 1;
        let json = to_json(&self.grade.top, &self.grade.apps);
        let dir = glove_config::dir();
        let _ = std::fs::create_dir_all(&dir);
        let tmp = dir.join(format!("{FILE}.tmp"));
        if std::fs::write(&tmp, json.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(FILE));
        }
        self.dirty = true;
    }

    /// Redesenha a ~30 quadros por segundo enquanto houver animação.
    fn grade_tick(&mut self) {
        if self.grade.ticking {
            return;
        }
        self.grade.ticking = true;
        let _ = self.loop_handle.insert_source(Timer::from_duration(TICK), |_, _, data| {
            let st = &mut data.state;
            let g = &st.grade;
            if g.alpha <= 0.0 || !(g.edit || g.drag.is_some() || g.fold_anim.is_some() || g.easing) {
                st.grade.ticking = false;
                return TimeoutAction::Drop;
            }
            st.dirty = true;
            TimeoutAction::ToDuration(TICK)
        });
    }

    fn grade_enter_edit(&mut self) {
        if !self.grade.edit {
            self.grade.edit = true;
            self.grade.edit_since = Some(Instant::now());
            self.grade.selected = None;
            self.grade_tick();
            self.dirty = true;
        }
    }

    fn grade_exit_edit(&mut self) {
        self.grade_cancel_drag();
        self.grade_commit_rename();
        self.grade.edit = false;
        self.grade.pos.clear();
        self.dirty = true;
    }

    fn grade_open_folder(&mut self, id: u64) {
        self.grade_commit_rename();
        self.grade.folder = Some(id);
        self.grade.closing = false;
        self.grade.fold_anim = Some((self.grade.fold, 1.0, Instant::now()));
        self.grade.selected = None;
        self.grade_tick();
        self.dirty = true;
    }

    fn grade_close_folder(&mut self) {
        self.grade_commit_rename();
        if self.grade.folder.is_some() {
            self.grade.closing = true;
            self.grade.fold_anim = Some((self.grade.fold, 0.0, Instant::now()));
            self.grade.selected = None;
            self.grade_tick();
            self.dirty = true;
        }
    }

    fn grade_close_folder_now(&mut self) {
        self.grade_commit_rename();
        self.grade.folder = None;
        self.grade.closing = false;
        self.grade.fold = 0.0;
        self.grade.fold_anim = None;
        self.grade.selected = None;
        self.dirty = true;
    }

    fn grade_commit_rename(&mut self) {
        let Some((t, _)) = self.grade.rename.take() else { return };
        let t = t.trim().to_string();
        let changed = match self.folder_mut() {
            Some(f) if !t.is_empty() && f.label() != t => {
                f.name = Some(t);
                true
            }
            _ => false,
        };
        if changed {
            self.grade_save();
        }
        self.dirty = true;
    }

    fn grade_launch(&mut self, a: usize) {
        let Some(app) = self.grade.apps.get(a).cloned() else { return };
        self.close_grade();
        match app.glove_id.as_deref().and_then(|id| self.config.dock_apps.iter().position(|d| d.app_id == id)) {
            Some(idx) => self.run_dock_action(crate::shell::DockAction::App(idx)),
            None => self.spawn(&app.exec),
        }
    }

    /// Clique (ou Enter) num item da grade: a pasta abre ou fecha; o app abre.
    fn grade_open_main(&mut self, i: usize) {
        match self.main_list().get(i).copied() {
            Some(Item::Folder(id)) => {
                if self.grade.folder == Some(id) && !self.grade.closing {
                    self.grade_close_folder();
                } else {
                    self.grade_open_folder(id);
                }
            }
            Some(Item::App(a)) if !self.grade.edit => self.grade_launch(a),
            _ => {}
        }
    }

    fn grade_open_band(&mut self, j: usize) {
        if self.grade.edit {
            return;
        }
        if let Some(a) = self.folder_ref().and_then(|f| f.apps.get(j).copied()) {
            self.grade_launch(a);
        }
    }

    pub fn grade_key(&mut self, k: GradeKey) {
        // Renomeando a pasta: o teclado é do nome.
        if let Some((t, fresh)) = &mut self.grade.rename {
            match k {
                GradeKey::Char(c) if !c.is_control() => {
                    if std::mem::take(fresh) {
                        t.clear();
                    }
                    if t.chars().count() < 40 {
                        t.push(c);
                    }
                }
                GradeKey::Backspace => {
                    if std::mem::take(fresh) {
                        t.clear();
                    } else {
                        t.pop();
                    }
                }
                GradeKey::Enter => self.grade_commit_rename(),
                GradeKey::Escape => self.grade.rename = None,
                _ => {}
            }
            self.dirty = true;
            return;
        }
        if k == GradeKey::Escape && self.grade.drag.is_some() {
            self.grade_cancel_drag();
            self.dirty = true;
            return;
        }
        let in_folder = self.folder_open();
        let l = self.grade_layout();
        let per = l.per_page();
        let cols = l.cols as i32;
        let n = if in_folder { self.folder_ref().map_or(0, |f| f.apps.len()) } else { self.main_list().len() };
        match k {
            GradeKey::Char(c) if !c.is_control() && self.grade.query.len() < 100 => {
                if self.grade.edit {
                    self.grade_exit_edit();
                }
                if self.grade.folder.is_some() {
                    self.grade_close_folder_now();
                }
                self.grade.query.push(c);
                self.grade.page = 0;
                self.grade.selected = (!self.main_list().is_empty()).then_some(0);
            }
            GradeKey::Backspace => {
                self.grade.query.pop();
                self.grade.page = 0;
                self.grade.selected = (self.searching() && !self.main_list().is_empty()).then_some(0);
            }
            GradeKey::Escape => {
                // Esc apaga a busca, depois fecha a pasta, sai do modo de edição e fecha.
                if !self.grade.query.is_empty() {
                    self.grade.query.clear();
                    self.grade.selected = None;
                    self.grade.page = 0;
                } else if self.grade.folder.is_some() && !self.grade.closing {
                    self.grade_close_folder();
                } else if self.grade.edit {
                    self.grade_exit_edit();
                } else {
                    self.close_grade();
                }
            }
            GradeKey::Enter if !self.grade.edit => {
                let i = self.grade.selected.or(self.searching().then_some(0));
                match i {
                    Some(i) if in_folder => self.grade_open_band(i),
                    Some(i) => self.grade_open_main(i),
                    None => {}
                }
            }
            GradeKey::Move(dx, dy) if n > 0 && !self.grade.edit => {
                let cur = self.grade.selected.map(|s| s as i32).unwrap_or(-1);
                let first = if in_folder { 0 } else { (self.grade.page * per) as i32 };
                let next = if cur < 0 { first } else { cur + dx + dy * cols };
                if (0..n as i32).contains(&next) {
                    self.grade.selected = Some(next as usize);
                    if !in_folder {
                        self.grade.page = next as usize / per;
                    }
                }
            }
            GradeKey::Page(d) if !in_folder => {
                let p = (self.grade.page as i32 + d).max(0) as usize;
                self.grade_set_page(p);
            }
            _ => {}
        }
        self.dirty = true;
    }

    /// Roda do mouse (ou dois dedos no touchpad): uma página por "clique".
    pub fn grade_scroll(&mut self, v: f64) {
        if self.folder_open() {
            return;
        }
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

    // -------------------------------------------------------------------
    // Onde está cada coisa
    // -------------------------------------------------------------------

    fn grade_band(&self, l: &Layout) -> Option<Band> {
        let id = self.grade.folder?;
        if self.grade.fold <= 0.0 || self.searching() {
            return None;
        }
        let per = l.per_page();
        let k = self.main_list().iter().position(|it| *it == Item::Folder(id))?;
        if k / per != self.grade.page {
            return None;
        }
        let (sx, sy) = l.slot_pos(k % per);
        let split = sy + l.tile_h;
        let dragging_in = usize::from(self.grade.drag.as_ref().is_some_and(|d| d.in_folder));
        let n = self.folder_ref().map_or(0, |f| f.apps.len()) + dragging_in;
        let rows = n.max(1).div_ceil(l.cols);
        let full_h = BAND_TITLE + rows as f32 * l.tile_h + BAND_PAD;
        let limit = l.tex.1 + l.tex.3 - 6.0;
        let up = (split + full_h - limit).max(0.0).min((split - l.tex.1).max(0.0));
        let t = smooth(self.grade.fold);
        Some(Band { top: split - up * t, h: full_h * t, full_h, up: up * t, split, notch_x: sx + l.tile_w / 2.0 })
    }

    fn grade_slots(&self, l: &Layout) -> Slots {
        let g = &self.grade;
        let mut list: Vec<Option<Item>> = self.main_list().into_iter().map(Some).collect();
        if let Some(d) = g.drag.as_ref().filter(|d| !d.in_folder) {
            list.insert(d.gap.min(list.len()), None);
        }
        let per = l.per_page();
        let band = self.grade_band(l);
        let main = list
            .iter()
            .enumerate()
            .skip(g.page * per)
            .take(per)
            .map(|(k, it)| {
                let (x, y) = l.slot_pos(k % per);
                (*it, x, band.map_or(y, |b| b.shift(y)))
            })
            .collect();
        let mut out = Slots { main, band: Vec::new(), band_geo: band };
        if let Some(b) = band {
            let mut apps: Vec<Option<usize>> = self.folder_ref().map(|f| f.apps.iter().map(|a| Some(*a)).collect()).unwrap_or_default();
            if let Some(d) = g.drag.as_ref().filter(|d| d.in_folder) {
                apps.insert(d.gap.min(apps.len()), None);
            }
            out.band = apps.into_iter().enumerate().map(|(j, a)| (a, l.x + (j % l.cols) as f32 * l.tile_w, b.top + BAND_TITLE + (j / l.cols) as f32 * l.tile_h)).collect();
        }
        out
    }

    /// O nome da pasta, no canto da faixa: (x, y, largura, altura) na tela.
    fn grade_title_rect(&self, l: &Layout, b: &Band) -> (f32, f32, f32, f32) {
        let label = match &self.grade.rename {
            Some((t, _)) => t.clone(),
            None => self.folder_ref().map(|f| f.label()).unwrap_or_default(),
        };
        let w = (self.fonts.measure(&label, 13.0, true) + 24.0).max(150.0);
        (l.x + 4.0, b.top + 7.0, w, 24.0)
    }

    fn grade_hit(&self, l: &Layout, x: f32, y: f32) -> Option<Hit> {
        let s = self.grade_slots(l);
        if self.grade.folder.is_some() && !self.searching() {
            let b = s.band_geo.filter(|_| !self.grade.closing)?;
            if y < b.top || y >= b.top + b.h {
                // Com a pasta aberta, o resto da grade não responde (o clique fecha a pasta).
                return None;
            }
            let (tx, ty, tw, th) = self.grade_title_rect(l, &b);
            if x >= tx && x < tx + tw && y >= ty && y < ty + th {
                return Some(Hit::Title);
            }
            return Some(s.band.iter().position(|(a, sx, sy)| a.is_some() && in_tile(l, x, y, *sx, *sy)).map_or(Hit::BandEmpty, Hit::Band));
        }
        let per = l.per_page();
        s.main.iter().position(|(it, sx, sy)| it.is_some() && in_tile(l, x, y, *sx, *sy)).map(|k| Hit::Main(self.grade.page * per + k))
    }

    // -------------------------------------------------------------------
    // Mouse
    // -------------------------------------------------------------------

    pub fn grade_press(&mut self, pos: Point<f64, Logical>) {
        let l = self.grade_layout();
        let (x, y) = (pos.x as f32, pos.y as f32);
        let hit = self.grade_hit(&l, x, y);
        if self.grade.rename.is_some() && hit != Some(Hit::Title) {
            self.grade_commit_rename();
        }
        match hit {
            Some(h @ (Hit::Main(_) | Hit::Band(_))) => {
                self.grade.press_gen += 1;
                let serial = self.grade.press_gen;
                self.grade.press = Some(Press { at: (x, y), hit: h, serial, moved: false, held: false });
                if !self.grade.edit && !self.searching() {
                    // Segurar: modo de edição.
                    let _ = self.loop_handle.insert_source(Timer::from_duration(HOLD), move |_, _, data| {
                        let st = &mut data.state;
                        let still = st.grade.press.as_ref().is_some_and(|p| p.serial == serial && !p.moved);
                        if still && st.grade.open && st.grade.drag.is_none() {
                            st.grade_enter_edit();
                            if let Some(p) = &mut st.grade.press {
                                p.held = true;
                            }
                        }
                        TimeoutAction::Drop
                    });
                }
            }
            Some(Hit::Title) => {
                if self.grade.rename.is_none() {
                    let label = self.folder_ref().map(|f| f.label()).unwrap_or_default();
                    self.grade.rename = Some((label, true));
                }
            }
            Some(Hit::BandEmpty) => {}
            None => {
                let field = Rectangle::<f32, Logical>::new((l.field_x, l.field_y).into(), (FIELD_W, FIELD_H).into());
                if let Some(p) = l.dot_at(x, y, self.grade_pages()).filter(|_| self.grade.folder.is_none()) {
                    self.grade_set_page(p);
                } else if field.contains((x, y)) {
                    // O campo de busca não fecha.
                } else if self.grade.folder.is_some() && !self.grade.closing {
                    self.grade_close_folder();
                } else if self.grade.edit {
                    self.grade_exit_edit();
                } else {
                    self.close_grade();
                }
            }
        }
        self.dirty = true;
    }

    pub fn grade_release(&mut self, _pos: Point<f64, Logical>) {
        if self.grade.drag.is_some() {
            self.grade_drop();
            return;
        }
        let Some(p) = self.grade.press.take() else { return };
        if p.moved || p.held {
            return;
        }
        match p.hit {
            Hit::Main(i) => self.grade_open_main(i),
            Hit::Band(j) => self.grade_open_band(j),
            _ => {}
        }
        self.dirty = true;
    }

    pub fn grade_motion(&mut self, pos: Point<f64, Logical>) {
        let (x, y) = (pos.x as f32, pos.y as f32);
        if self.grade.drag.is_some() {
            self.grade_drag_to(x, y);
            return;
        }
        if let Some(p) = &mut self.grade.press {
            if !p.moved && (x - p.at.0).hypot(y - p.at.1) > DRAG_START {
                p.moved = true;
                let (hit, at) = (p.hit, p.at);
                if !self.searching() {
                    self.grade_start_drag(hit, at, (x, y));
                }
            }
            return;
        }
        if self.grade.edit {
            return;
        }
        let l = self.grade_layout();
        let sel = match self.grade_hit(&l, x, y) {
            Some(Hit::Main(i)) | Some(Hit::Band(i)) => Some(i),
            _ => None,
        };
        if sel.is_some() && sel != self.grade.selected {
            self.grade.selected = sel;
            self.dirty = true;
        }
    }

    fn grade_start_drag(&mut self, hit: Hit, at: (f32, f32), now: (f32, f32)) {
        let l = self.grade_layout();
        let s = self.grade_slots(&l);
        let per = l.per_page();
        let (entry, origin, in_folder, slot) = match hit {
            Hit::Main(i) if self.grade.folder.is_none() && i < self.grade.top.len() => {
                let Some(&(_, sx, sy)) = s.main.get(i - self.grade.page * per) else { return };
                (self.grade.top.remove(i), Origin::Main(i), false, (sx, sy))
            }
            Hit::Band(j) => {
                let Some(&(_, sx, sy)) = s.band.get(j) else { return };
                let Some(f) = self.folder_mut() else { return };
                if j >= f.apps.len() {
                    return;
                }
                let id = f.id;
                (Entry::App(f.apps.remove(j)), Origin::Folder(id, j), true, (sx, sy))
            }
            _ => return,
        };
        self.grade_enter_edit();
        let gap = match origin {
            Origin::Main(i) => i,
            Origin::Folder(_, j) => j,
        };
        self.grade.pos.insert(entry.item(), slot);
        self.grade.drag = Some(Drag { entry, origin, in_folder, pos: now, grab: (at.0 - slot.0, at.1 - slot.1), gap, cand: None, merge: None, edge: None });
        self.grade.selected = None;
        self.grade_tick();
        self.dirty = true;
    }

    /// O arrastado anda: abre espaço, marca a pasta a formar, vira a página na
    /// borda ou sai da pasta. Chamado também a cada quadro (o tempo parado conta).
    fn grade_drag_to(&mut self, x: f32, y: f32) {
        let l = self.grade_layout();
        let now = Instant::now();
        let Some(d) = self.grade.drag.as_mut() else { return };
        d.pos = (x, y);
        if d.in_folder {
            let band = self.grade_band(&l);
            let inside = band.is_some_and(|b| y >= b.top - 16.0 && y < b.top + b.h + 16.0);
            let Some(b) = band.filter(|_| inside) else {
                // Saiu da faixa: a pasta fecha e o app vai para a grade, logo depois dela.
                let id = self.grade.folder;
                let fi = self.grade.top.iter().position(|e| matches!(e, Entry::Folder(f) if Some(f.id) == id)).unwrap_or(self.grade.top.len());
                self.grade_close_folder_now();
                let mut gap = fi + 1;
                if matches!(self.grade.top.get(fi), Some(Entry::Folder(f)) if f.apps.is_empty()) {
                    self.grade.top.remove(fi);
                    gap = fi;
                }
                if let Some(d) = self.grade.drag.as_mut() {
                    d.in_folder = false;
                    d.gap = gap.min(self.grade.top.len());
                    d.cand = None;
                    d.merge = None;
                }
                self.grade.ver += 1;
                self.dirty = true;
                return;
            };
            let n = self.folder_ref().map_or(0, |f| f.apps.len());
            let col = ((x - l.x) / l.tile_w).floor().clamp(0.0, l.cols as f32 - 1.0) as usize;
            let row = ((y - b.top - BAND_TITLE) / l.tile_h).floor().max(0.0) as usize;
            let g = (row * l.cols + col).min(n);
            let d = self.grade.drag.as_mut().unwrap();
            set_cand(d, (g != d.gap).then_some(Cand::Move(g)), now);
            self.dirty = true;
            return;
        }
        // Na grade: na borda, vira a página.
        let gw = l.cols as f32 * l.tile_w;
        let edge = if x < l.x - 10.0 {
            -1
        } else if x > l.x + gw + 10.0 {
            1
        } else {
            0
        };
        if edge != 0 {
            let flip = match d.edge {
                Some((e, t)) if e == edge => now.duration_since(t) >= DWELL_EDGE,
                _ => {
                    d.edge = Some((edge, now));
                    false
                }
            };
            d.cand = None;
            d.merge = None;
            if flip {
                d.edge = Some((edge, now));
                let p = (self.grade.page as i32 + edge).max(0) as usize;
                self.grade_set_page(p);
            }
            self.dirty = true;
            return;
        }
        d.edge = None;
        let len = self.grade.top.len();
        let per = l.per_page();
        let Some(s) = l.tile_at(x, y) else {
            set_cand(d, None, now);
            return;
        };
        let g = self.grade.page * per + s;
        let gap = d.gap;
        let cand = if g == gap {
            None
        } else if g > len {
            Some(Cand::Move(len))
        } else {
            let li = if g < gap { g } else { g - 1 };
            let (sx, sy) = l.slot_pos(s);
            let (cx, cy) = l.icon_center(sx, sy);
            let near = (x - cx).hypot(y - cy) < l.icon * 0.34;
            if near && matches!(d.entry, Entry::App(_)) && li < len { Some(Cand::Merge(li)) } else { Some(Cand::Move(g.min(len))) }
        };
        set_cand(d, cand, now);
        self.dirty = true;
    }

    fn grade_drop(&mut self) {
        let Some(d) = self.grade.drag.take() else { return };
        self.grade.press = None;
        let item = d.entry.item();
        self.grade.pos.insert(item, (d.pos.0 - d.grab.0, d.pos.1 - d.grab.1));
        match (d.in_folder, d.merge, d.entry) {
            (true, _, Entry::App(a)) if self.folder_ref().is_some() => {
                let f = self.folder_mut().unwrap();
                let g = d.gap.min(f.apps.len());
                f.apps.insert(g, a);
            }
            (false, Some(t), Entry::App(a)) if t < self.grade.top.len() => match &mut self.grade.top[t] {
                Entry::Folder(f) => f.apps.push(a),
                Entry::App(b) => {
                    // Um app sobre outro: nasce uma pasta, já aberta e com o nome para escrever.
                    let b = *b;
                    self.grade.next_id += 1;
                    let id = self.grade.next_id;
                    let both_util = self.grade.apps.get(a).is_some_and(|x| x.utility) && self.grade.apps.get(b).is_some_and(|x| x.utility);
                    let name = both_util.then(|| tr!("Utilitários").to_string());
                    self.grade.top[t] = Entry::Folder(Folder { id, name, utilities: false, apps: vec![b, a] });
                    self.grade_save();
                    self.grade_open_folder(id);
                    let label = self.folder_ref().map(|f| f.label()).unwrap_or_default();
                    self.grade.rename = Some((label, true));
                    return;
                }
            },
            (_, _, entry) => {
                let g = d.gap.min(self.grade.top.len());
                self.grade.top.insert(g, entry);
            }
        }
        self.grade_save();
    }

    /// Desfaz o arrasto: o item volta para onde estava.
    fn grade_cancel_drag(&mut self) {
        let Some(d) = self.grade.drag.take() else { return };
        self.grade.press = None;
        match d.origin {
            Origin::Folder(id, j) => {
                let home = self.grade.top.iter_mut().find_map(|e| match e {
                    Entry::Folder(f) if f.id == id => Some(f),
                    _ => None,
                });
                match (home, d.entry) {
                    (Some(f), Entry::App(a)) => f.apps.insert(j.min(f.apps.len()), a),
                    (_, entry) => self.grade.top.push(entry),
                }
            }
            Origin::Main(i) => {
                let i = i.min(self.grade.top.len());
                self.grade.top.insert(i, d.entry);
            }
        }
        self.grade_save();
    }

    /// Onde ficam o campo e a grade.
    pub fn grade_field_pos(&self) -> Point<i32, Logical> {
        let l = self.grade_layout();
        (l.field_x as i32, l.field_y as i32).into()
    }

    pub fn grade_grid_pos(&self) -> Point<i32, Logical> {
        let l = self.grade_layout();
        (l.tex.0 as i32, l.tex.1 as i32).into()
    }

    // -------------------------------------------------------------------
    // Desenho
    // -------------------------------------------------------------------

    pub fn prepare_grade(&mut self) {
        if let Some((from, to, start)) = self.grade.anim {
            let t = (start.elapsed().as_secs_f32() / FADE).min(1.0);
            self.grade.alpha = from + (to - from) * smooth(t);
            if t >= 1.0 {
                self.grade.anim = None;
            } else {
                self.dirty = true;
            }
        }
        if let Some((from, to, start)) = self.grade.fold_anim {
            let t = (start.elapsed().as_secs_f32() / FOLD).min(1.0);
            self.grade.fold = from + (to - from) * t;
            if t >= 1.0 {
                self.grade.fold_anim = None;
                if self.grade.closing {
                    self.grade.folder = None;
                    self.grade.closing = false;
                }
            } else {
                self.dirty = true;
            }
        }
        if self.grade.alpha <= 0.0 {
            // Fechada: solta a memória das texturas e das listas.
            if !self.grade.open && self.grade.grid_tex.is_some() {
                let g = &mut self.grade;
                g.grid_tex = None;
                g.field_tex = None;
                g.bg = None;
                g.tiles.clear();
                g.pos.clear();
                g.apps = Vec::new();
                g.top = Vec::new();
                g.folder = None;
                g.fold = 0.0;
            }
            return;
        }
        // Parado com o arrastado: o tempo conta (abrir espaço, formar pasta, virar página).
        if let Some((x, y)) = self.grade.drag.as_ref().map(|d| d.pos) {
            self.grade_drag_to(x, y);
        }
        let sc = self.scale() as f32;
        let (sw, sh) = self.output_size();
        let l = self.grade_layout();
        use std::hash::{Hash, Hasher};
        let base = self.theme_gen ^ (sc.to_bits() as u64) ^ ((l.icon as u64) << 40);

        // Fundo: o papel de parede desfocado e escurecido.
        let bg_key = (sw, sh, self.theme_gen);
        if self.grade.bg.as_ref().map(|b| b.0) != Some(bg_key) {
            self.grade.bg = blurred_wallpaper(&self.theme, &self.config.wallpaper, sw, sh).map(|t| (bg_key, t));
        }

        // Campo de busca (o cursor pisca).
        let caret = self.grade.opened.map(|t| (t.elapsed().as_millis() / 530) % 2 == 0).unwrap_or(true);
        let field_caret = caret && self.grade.rename.is_none();
        let mut k = std::collections::hash_map::DefaultHasher::new();
        (base, &self.grade.query, field_caret).hash(&mut k);
        let key = k.finish();
        if self.grade.field_tex.as_ref().map(|t| t.0) != Some(key) {
            self.grade.field_tex = draw_field(&self.fonts, &self.grade.query, field_caret, sc).map(|c| (key, to_tex(c)));
        }

        // Ícones prontos (refeitos quando muda a escala, o tema ou o conteúdo).
        if self.grade.tiles_base != base {
            self.grade.tiles.clear();
            self.grade.tiles_base = base;
        }
        let slots = self.grade_slots(&l);
        let mut need: Vec<Item> = slots.main.iter().filter_map(|s| s.0).collect();
        need.extend(slots.band.iter().filter_map(|s| s.0.map(Item::App)));
        if let Some(d) = &self.grade.drag {
            need.push(d.entry.item());
        }
        for it in &need {
            self.grade_ensure_tile(*it, &l, sc);
        }

        // Deslizar até o lugar (só no modo de edição).
        let now = Instant::now();
        let dt = self.grade.last_frame.map_or(0.0, |t| now.duration_since(t).as_secs_f32().min(0.1));
        self.grade.last_frame = Some(now);
        let dragged = self.grade.drag.as_ref().map(|d| d.entry.item());
        if self.grade.edit || self.grade.drag.is_some() {
            let kf = 1.0 - (-dt / 0.07).exp();
            let mut moving = false;
            let targets = slots.main.iter().filter_map(|s| s.0.map(|it| (it, s.1, s.2))).chain(slots.band.iter().filter_map(|s| s.0.map(|a| (Item::App(a), s.1, s.2))));
            for (it, tx, ty) in targets {
                if Some(it) == dragged {
                    continue;
                }
                let p = self.grade.pos.entry(it).or_insert((tx, ty));
                p.0 += (tx - p.0) * kf;
                p.1 += (ty - p.1) * kf;
                if (tx - p.0).abs() > 0.5 || (ty - p.1).abs() > 0.5 {
                    moving = true;
                } else {
                    *p = (tx, ty);
                }
            }
            self.grade.easing = moving;
        } else {
            self.grade.pos.clear();
            self.grade.easing = false;
        }

        // A grade: a cada quadro quando há movimento; senão, só quando algo muda.
        let animating = self.grade.edit || self.grade.drag.is_some() || self.grade.fold_anim.is_some() || self.grade.easing;
        let mut k = std::collections::hash_map::DefaultHasher::new();
        (base, &self.grade.query, self.grade.folder, self.grade.page, self.grade.selected, sw, sh, self.grade.ver, self.grade.fold.to_bits()).hash(&mut k);
        (&self.grade.rename, caret && self.grade.rename.is_some()).hash(&mut k);
        let key = k.finish();
        if animating || self.grade.grid_tex.as_ref().map(|t| t.0) != Some(key) {
            self.grade.grid_tex = self.draw_grade_grid(&l, &slots, caret).map(|c| (key, to_tex(c)));
        }
        if animating {
            self.grade_tick();
        }
        // Bolinhas das páginas, numa imagem à parte.
        let pages = self.grade_pages();
        let mut k = std::collections::hash_map::DefaultHasher::new();
        (pages, self.grade.page, self.grade.fold.to_bits(), l.dots_y().to_bits(), l.x.to_bits(), self.theme_gen).hash(&mut k);
        let key = k.finish();
        if pages < 2 {
            self.grade.dots_tex = None;
        } else if self.grade.dots_tex.as_ref().map(|t| t.0) != Some(key) {
            let fold = smooth(self.grade.fold);
            let sc = self.scale() as f32;
            let page = self.grade.page;
            self.grade.dots_tex = Canvas::new((pages as f32 - 1.0) * 18.0 + 16.0, 16.0, sc)
                .map(|mut c| {
                    for p in 0..pages {
                        let a = if p == page { 240.0 } else { 90.0 } * (1.0 - fold);
                        c.fill(circle(8.0 + p as f32 * 18.0, 8.0, 3.5), &solid(rgba(255, 255, 255, a as u8)));
                    }
                    c
                })
                .map(|c| (key, to_tex(c)));
        }
        // Páginas deslizando: um quadro novo a cada vez até acabar.
        if let Some((_, _, start)) = &self.grade.slide {
            if start.elapsed().as_secs_f32() >= slide() {
                self.grade.slide = None;
            }
            self.dirty = true;
        }
    }

    /// Canto da imagem das bolinhas das páginas.
    pub fn grade_dots_pos(&self) -> Point<i32, Logical> {
        let l = self.grade_layout();
        let pages = self.grade_pages();
        let x0 = l.x + l.cols as f32 * l.tile_w / 2.0 - (pages as f32 - 1.0) * 9.0;
        ((x0 - 8.0).round() as i32, (l.dots_y() - 8.0).round() as i32).into()
    }

    /// Deslocamento lateral da página nova e da que sai, durante a troca de página.
    pub fn grade_slide_offsets(&self) -> Option<(i32, i32)> {
        let (_, dir, start) = self.grade.slide.as_ref()?;
        let t = (start.elapsed().as_secs_f32() / slide()).min(1.0);
        // Começa rápido e freia no fim (como o Launchpad).
        let e = 1.0 - (1.0 - t).powi(3);
        let w = self.output_size().0 as f32;
        Some(((dir * (1.0 - e) * w).round() as i32, (-dir * e * w).round() as i32))
    }

    fn grade_ensure_tile(&mut self, it: Item, l: &Layout, sc: f32) {
        use std::hash::{Hash, Hasher};
        let folder = match it {
            Item::Folder(id) => self.grade.top.iter().find_map(|e| match e {
                Entry::Folder(f) if f.id == id => Some(f),
                _ => None,
            }),
            Item::App(_) => None,
        };
        let mut k = std::collections::hash_map::DefaultHasher::new();
        it.hash(&mut k);
        if let Some(f) = folder {
            (f.label(), f.apps.iter().take(FOLDER_PEEK).collect::<Vec<_>>()).hash(&mut k);
        }
        let key = k.finish();
        if self.grade.tiles.get(&it).is_some_and(|t| t.0 == key) {
            return;
        }
        if let Some(c) = draw_tile(&self.fonts, l, &self.grade.apps, it, folder, sc) {
            self.grade.tiles.insert(it, (key, c));
        }
    }

    fn draw_grade_grid(&self, l: &Layout, slots: &Slots, caret: bool) -> Option<Canvas> {
        let g = &self.grade;
        let f = &self.fonts;
        let sc = self.scale() as f32;
        let (ox, oy, w, h) = l.tex;
        let mut c = Canvas::new(w, h, sc)?;
        let t = g.edit_since.map_or(0.0, |t| t.elapsed().as_secs_f32());
        let jiggle = |it: Item| -> f32 {
            if !g.edit {
                return 0.0;
            }
            let seed = match it {
                Item::App(a) => a as u32,
                Item::Folder(id) => id as u32 ^ 0x9e37,
            };
            let phase = hash(seed) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
            (t * std::f32::consts::TAU * 3.0 + phase).sin() * 1.8
        };
        let fold = smooth(g.fold);
        let open = slots.band_geo.and(g.folder).map(Item::Folder);
        let per = l.per_page();
        let dragged = g.drag.as_ref().map(|d| d.entry.item());
        let merge_at = g.drag.as_ref().and_then(|d| d.merge.map(|m| if m < d.gap { m } else { m + 1 }));

        if self.searching() && slots.main.is_empty() {
            let msg = tr!("Nenhum app encontrado");
            let tw = f.measure(msg, 16.0, true);
            shadowed(&mut c, f, msg, (w - tw) / 2.0, l.y - oy + l.rows as f32 * l.tile_h / 2.0, 16.0, true);
        }
        let place = |it: Item, tx: f32, ty: f32| -> (f32, f32) { if g.edit || g.drag.is_some() { g.pos.get(&it).copied().unwrap_or((tx, ty)) } else { (tx, ty) } };
        for (k, (it, tx, ty)) in slots.main.iter().enumerate() {
            let Some(it) = *it else { continue };
            let (x, y) = place(it, *tx, *ty);
            let (x, y) = (x - ox, y - oy);
            let gi = g.page * per + k;
            if g.selected == Some(gi) && !g.edit && open.is_none() {
                c.fill(rrect(x + 6.0, y + 2.0, l.tile_w - 12.0, l.tile_h - 4.0, 12.0), &solid(rgba(255, 255, 255, 46)));
            }
            if merge_at == Some(gi) {
                // Vai virar pasta: o quadro de vidro em volta do ícone.
                let (ix, iy) = (x + (l.tile_w - l.icon) / 2.0, y + 10.0);
                c.fill(rrect(ix - 10.0, iy - 10.0, l.icon + 20.0, l.icon + 20.0, l.icon * 0.26), &solid(rgba(225, 232, 245, 90)));
                c.stroke(rrect(ix - 9.5, iy - 9.5, l.icon + 19.0, l.icon + 19.0, l.icon * 0.26), &solid(rgba(255, 255, 255, 160)), 1.0);
            }
            let dim = if open.is_some_and(|o| o != it) { 1.0 - 0.78 * fold } else { 1.0 };
            if let Some((_, tile)) = g.tiles.get(&it) {
                blit(&mut c, tile, l, x, y, jiggle(it), 1.0, dim);
            }
        }
        // A faixa da pasta aberta.
        if let Some(b) = slots.band_geo {
            let label = self.folder_ref().map(|f| f.label()).unwrap_or_default();
            let title = self.grade_title_rect(l, &b);
            draw_band(&mut c, f, l, &b, (ox, oy), &label, g.rename.as_ref(), caret, title);
            let s = c.scale;
            let (top, bot) = (b.top - oy, b.top - oy + b.h);
            c.set_clip(Some([0, (top * s).floor() as i32, (w * s).ceil() as i32, (bot * s).ceil() as i32]));
            for (a, tx, ty) in &slots.band {
                let Some(a) = *a else { continue };
                let it = Item::App(a);
                let (x, y) = place(it, *tx, *ty);
                let j = slots.band.iter().position(|s| s.0 == Some(a)).unwrap_or(0);
                if g.selected == Some(j) && !g.edit {
                    c.fill(rrect(x - ox + 6.0, y - oy + 2.0, l.tile_w - 12.0, l.tile_h - 4.0, 12.0), &solid(rgba(255, 255, 255, 40)));
                }
                if let Some((_, tile)) = g.tiles.get(&it) {
                    blit(&mut c, tile, l, x - ox, y - oy, jiggle(it), 1.0, 1.0);
                }
            }
            c.set_clip(None);
        }
        // O arrastado, por cima de tudo, um pouco maior.
        if let (Some(d), Some(it)) = (&g.drag, dragged) {
            if let Some((_, tile)) = g.tiles.get(&it) {
                blit(&mut c, tile, l, d.pos.0 - d.grab.0 - ox, d.pos.1 - d.grab.1 - oy, 0.0, 1.12, 0.92);
            }
        }
        Some(c)
    }
}

/// Guarda a nova possibilidade; se ela ficou o tempo pedido, vale.
fn set_cand(d: &mut Drag, c: Option<Cand>, now: Instant) {
    match (c, d.cand) {
        (None, _) => {
            d.cand = None;
            d.merge = None;
        }
        (Some(c), Some((old, t))) if old == c => {
            let dwell = match c {
                Cand::Move(_) => DWELL_MOVE,
                Cand::Merge(_) => DWELL_MERGE,
            };
            if now.duration_since(t) >= dwell {
                match c {
                    Cand::Move(g) => {
                        d.gap = g;
                        d.cand = None;
                        d.merge = None;
                    }
                    Cand::Merge(m) => d.merge = Some(m),
                }
            }
        }
        (Some(c), _) => {
            d.cand = Some((c, now));
            d.merge = None;
        }
    }
}

/// Compõe um ladrilho pronto, girando em volta do ícone (balanço) e aumentando.
#[allow(clippy::too_many_arguments)]
fn blit(c: &mut Canvas, tile: &Canvas, l: &Layout, x: f32, y: f32, angle: f32, zoom: f32, opacity: f32) {
    if opacity <= 0.01 {
        return;
    }
    let s = c.scale;
    let (cx, cy) = (l.tile_w / 2.0 * s, (10.0 + l.icon / 2.0) * s);
    let mut t = Transform::from_translate((x * s).round(), (y * s).round());
    let turned = angle != 0.0 || zoom != 1.0;
    if turned {
        t = t.pre_concat(Transform::from_translate(cx, cy)).pre_concat(Transform::from_rotate(angle)).pre_concat(Transform::from_scale(zoom, zoom)).pre_concat(Transform::from_translate(-cx, -cy));
    }
    let quality = if turned { FilterQuality::Bilinear } else { FilterQuality::Nearest };
    c.draw_pixmap_t(tile.pm.as_ref(), &PixmapPaint { opacity, quality, ..Default::default() }, t);
}

fn shadowed(c: &mut Canvas, f: &Fonts, t: &str, x: f32, y: f32, px: f32, bold: bool) {
    f.draw(c, t, x, y + 1.0, px, bold, rgba(0, 0, 0, 170));
    f.draw(c, t, x, y, px, bold, rgba(255, 255, 255, 255));
}

/// Um ladrilho: o ícone (ou o quadro da pasta) e o nome embaixo.
fn draw_tile(f: &Fonts, l: &Layout, apps: &[App], it: Item, folder: Option<&Folder>, sc: f32) -> Option<Canvas> {
    let mut c = Canvas::new(l.tile_w, l.tile_h, sc)?;
    let s = l.icon;
    let (ix, iy) = ((l.tile_w - s) / 2.0, 10.0);
    let name = match it {
        Item::App(a) => {
            let app = apps.get(a)?;
            icons::draw(&mut c, f, app.icon, ix, iy, s, app.name.chars().next());
            app.name.clone()
        }
        Item::Folder(_) => {
            let fo = folder?;
            draw_folder(&mut c, f, apps, &fo.apps, ix, iy, s);
            fo.label()
        }
    };
    let px = 12.5;
    let label = f.ellipsize(&name, px, false, l.tile_w - 10.0);
    let lw = f.measure(&label, px, false);
    shadowed(&mut c, f, &label, (l.tile_w - lw) / 2.0, iy + s + 18.0, px, false);
    Some(c)
}

/// O quadro de uma pasta: vidro arredondado com os ícones dela em miniatura.
fn draw_folder(c: &mut Canvas, f: &Fonts, apps: &[App], members: &[usize], x: f32, y: f32, s: f32) {
    c.fill(rrect(x + 2.0, y + 4.0, s - 4.0, s - 4.0, s * 0.2), &solid(rgba(0, 0, 0, 50)));
    c.fill(rrect(x + 2.0, y + 2.0, s - 4.0, s - 4.0, s * 0.2), &vgrad(y, y + s, &[(0.0, rgba(225, 230, 240, 120)), (1.0, rgba(170, 180, 200, 90))]));
    c.stroke(rrect(x + 2.5, y + 2.5, s - 5.0, s - 5.0, s * 0.2), &solid(rgba(255, 255, 255, 140)), 1.0);
    let m = s * 0.14;
    let cell = (s - 2.0 * m) / 3.0;
    for (k, a) in members.iter().filter_map(|i| apps.get(*i)).take(FOLDER_PEEK).enumerate() {
        let (cx, cy) = (x + m + (k % 3) as f32 * cell, y + m + (k / 3) as f32 * cell);
        icons::draw_bare(c, f, a.icon, cx + cell * 0.08, cy + cell * 0.08, cell * 0.84, a.name.chars().next());
    }
}

/// Tecido escuro (fios finos com um pouco de ruído), para a faixa da pasta.
fn linen() -> &'static Pixmap {
    static P: OnceLock<Pixmap> = OnceLock::new();
    P.get_or_init(|| {
        let n = 96u32;
        let mut pm = Pixmap::new(n, n).expect("tecido");
        for (i, px) in pm.data_mut().chunks_exact_mut(4).enumerate() {
            let (x, y) = (i as u32 % n, i as u32 / n);
            let noise = (hash(x.wrapping_mul(7919) ^ y.wrapping_mul(104_729)) % 13) as i32 - 6;
            let warp = if y % 2 == 0 { 3 } else { -2 };
            let weft = if (x + (hash(y) % 3)) % 3 == 0 { 2 } else { 0 };
            let v = (54 + noise + warp + weft).clamp(0, 255) as u8;
            px.copy_from_slice(&[v, v.saturating_add(1), v.saturating_add(4), 255]);
        }
        pm
    })
}

/// O brilho azul das bordas da faixa (esticado ao desenhar).
fn glow() -> &'static Canvas {
    static G: OnceLock<GlowCanvas> = OnceLock::new();
    &G.get_or_init(|| {
        let mut c = Canvas::new(64.0, 64.0, 1.0).expect("brilho");
        c.fill(circle(32.0, 32.0, 32.0), &radial(32.0, 32.0, 32.0, &[(0.0, rgba(60, 200, 255, 230)), (0.4, rgba(0, 130, 235, 120)), (1.0, rgba(0, 60, 170, 0))]));
        GlowCanvas(c)
    })
    .0
}

/// O Canvas só é lido depois de pronto (a imagem não muda).
struct GlowCanvas(Canvas);
// SAFETY: o Canvas guardado é feito uma vez e depois só lido.
unsafe impl Sync for GlowCanvas {}
unsafe impl Send for GlowCanvas {}

/// A faixa da pasta aberta: tecido escuro com o bico apontando para a pasta, os
/// brilhos azuis nas bordas e o nome (ou o campo de renomear) no canto.
#[allow(clippy::too_many_arguments)]
fn draw_band(c: &mut Canvas, f: &Fonts, l: &Layout, b: &Band, (ox, oy): (f32, f32), label: &str, rename: Option<&(String, bool)>, caret: bool, title: (f32, f32, f32, f32)) {
    let s = c.scale;
    let w = c.w;
    let (top, bot) = (b.top - oy, b.top - oy + b.h);
    let nx = b.notch_x - ox;
    let open = (b.h / b.full_h).clamp(0.0, 1.0);
    // Brilhos: em cima, saindo do bico; embaixo, na borda de baixo.
    let g = glow();
    let (gw, gh) = ((w * 0.6).min(820.0), 120.0 * open);
    for (gx, gy) in [(nx, top), (w / 2.0, bot)] {
        if gh < 1.0 {
            break;
        }
        let t = Transform::from_translate(((gx - gw / 2.0) * s).round(), ((gy - gh / 2.0) * s).round()).pre_scale(gw * s / 64.0, gh * s / 64.0);
        c.draw_pixmap_t(g.pm.as_ref(), &PixmapPaint { quality: FilterQuality::Bilinear, opacity: open, ..Default::default() }, t);
    }
    // O tecido, com o bico.
    let notch = 11.0_f32.min(b.h);
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, top);
    pb.line_to(nx - notch, top);
    pb.line_to(nx, top - notch);
    pb.line_to(nx + notch, top);
    pb.line_to(w, top);
    pb.line_to(w, bot);
    pb.line_to(0.0, bot);
    pb.close();
    let shape = pb.finish();
    let pattern = tiny_skia::Pattern::new(linen().as_ref(), tiny_skia::SpreadMode::Repeat, FilterQuality::Nearest, 1.0, Transform::from_scale(1.0 / s, 1.0 / s));
    let paint = tiny_skia::Paint { shader: pattern, anti_alias: true, ..Default::default() };
    c.fill(shape, &paint);
    // Sombra de dentro, em cima e embaixo, e as linhas claras das bordas.
    let sh = 10.0_f32.min(b.h / 2.0);
    c.fill_rect(0.0, top, w, sh, &vgrad(top, top + sh, &[(0.0, rgba(0, 0, 0, 110)), (1.0, rgba(0, 0, 0, 0))]));
    c.fill_rect(0.0, bot - sh, w, sh, &vgrad(bot - sh, bot, &[(0.0, rgba(0, 0, 0, 0)), (1.0, rgba(0, 0, 0, 90))]));
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, top + 0.5);
    pb.line_to(nx - notch, top + 0.5);
    pb.line_to(nx, top - notch + 0.5);
    pb.line_to(nx + notch, top + 0.5);
    pb.line_to(w, top + 0.5);
    c.stroke(pb.finish(), &solid(rgba(255, 255, 255, 60)), 1.0);
    c.hline(0.0, bot - 1.0, w, rgba(255, 255, 255, 40));
    if b.h < BAND_TITLE {
        return;
    }
    // O nome: no canto; renomeando, num campo branco.
    let (tx, ty, tw, th) = (title.0 - ox, title.1 - oy, title.2, title.3);
    let px = 13.0;
    match rename {
        Some((text, fresh)) => {
            c.fill(rrect(tx, ty, tw, th, 4.0), &solid(rgba(250, 250, 252, 255)));
            c.stroke(rrect(tx + 0.5, ty + 0.5, tw - 1.0, th - 1.0, 4.0), &solid(rgba(80, 140, 230, 255)), 1.0);
            let tw2 = f.measure(text, px, true);
            if *fresh && !text.is_empty() {
                c.fill_rect(tx + 9.0, ty + 4.0, tw2 + 2.0, th - 8.0, &solid(rgba(170, 205, 255, 255)));
            }
            f.draw(c, text, tx + 10.0, ty + 17.0, px, true, rgba(20, 20, 24, 255));
            if caret && !*fresh {
                c.fill_rect((tx + 10.0 + tw2 + 1.0).round(), ty + 5.0, 1.0, th - 10.0, &solid(rgba(20, 20, 24, 255)));
            }
        }
        None => {
            let lw = f.measure(label, px, true);
            c.fill(rrect(tx, ty, lw + 20.0, th, th / 2.0), &solid(rgba(0, 0, 0, 60)));
            shadowed(c, f, label, tx + 10.0, ty + 17.0, px, true);
        }
    }
    let _ = l;
}

fn draw_field(f: &Fonts, query: &str, caret: bool, sc: f32) -> Option<Canvas> {
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

/// O papel de parede pequeno, desfocado e escurecido, esticado para a tela.
fn blurred_wallpaper(theme: &glove_paint::theme::Theme, style: &str, sw: i32, sh: i32) -> Option<Tex> {
    let (w, h) = ((sw as f32 / 12.0).ceil().max(8.0), (sh as f32 / 12.0).ceil().max(8.0));
    let mut c = glove_paint::wallpaper::render_style(theme, style, w, h, 1.0)?;
    for _ in 0..3 {
        box_blur(&mut c.pm, 2);
    }
    c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 105)));
    let mut t = to_tex(c);
    t.size = (sw, sh).into();
    Some(t)
}

/// Desfoque de caixa (horizontal e vertical) nos pixels pré-multiplicados.
fn box_blur(pm: &mut Pixmap, r: usize) {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let src = pm.data().to_vec();
    let mut tmp = vec![0u8; src.len()];
    let pass = |from: &[u8], to: &mut [u8], horizontal: bool| {
        let (n, m) = if horizontal { (h, w) } else { (w, h) };
        for line in 0..n {
            for i in 0..m {
                let mut acc = [0u32; 4];
                let mut cnt = 0u32;
                for k in i.saturating_sub(r)..(i + r + 1).min(m) {
                    let idx = if horizontal { (line * w + k) * 4 } else { (k * w + line) * 4 };
                    for ch in 0..4 {
                        acc[ch] += from[idx + ch] as u32;
                    }
                    cnt += 1;
                }
                let idx = if horizontal { (line * w + i) * 4 } else { (i * w + line) * 4 };
                for ch in 0..4 {
                    to[idx + ch] = (acc[ch] / cnt) as u8;
                }
            }
        }
    };
    pass(&src, &mut tmp, true);
    pass(&tmp, pm.data_mut(), false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glove_paint::icons::Icon;

    fn app(name: &str, utility: bool) -> App {
        App { name: name.into(), keywords: String::new(), exec: name.to_lowercase(), icon: Icon::Window, glove_id: None, utility }
    }

    #[test]
    fn arrumacao_padrao_e_guardada() {
        let apps = sorted_apps(vec![app("Zeta", false), app("Monitor", true), app("Alfa", false)]);
        assert_eq!(apps[0].name, "Alfa");
        let mut id = 0;
        // Sem nada guardado: a pasta dos utilitários e os outros em ordem alfabética.
        let top = arrange(&apps, None, &mut id);
        assert!(matches!(&top[0], Entry::Folder(f) if f.utilities && f.apps == vec![1]));
        assert_eq!(&top[1..], &[Entry::App(0), Entry::App(2)]);
        // Guarda e lê de volta igual.
        let json = to_json(&top, &apps);
        let again = arrange(&apps, Some(&json), &mut id);
        assert_eq!(again.iter().map(|e| e.item()).count(), 3);
        assert!(matches!(&again[0], Entry::Folder(f) if f.utilities && f.apps == vec![1]));
        // Uma pasta do usuário com nome, um app que saiu e um app novo (vai para o fim).
        let saved = serde_json::json!({"itens": ["exec:zeta", {"nome": "Jogos", "apps": ["exec:alfa", "exec:sumiu"]}]});
        let apps2 = sorted_apps(vec![app("Zeta", false), app("Monitor", true), app("Alfa", false), app("Beta", false)]);
        let top = arrange(&apps2, Some(&saved), &mut id);
        // Alfa, Beta, Monitor, Zeta: Zeta = 3, Alfa = 0, Beta = 1, Monitor = 2.
        assert_eq!(top[0], Entry::App(3));
        assert!(matches!(&top[1], Entry::Folder(f) if f.name.as_deref() == Some("Jogos") && f.apps == vec![0]));
        // Sem pasta de utilitários guardada: o utilitário fica solto, no fim, com o novo.
        assert_eq!(&top[2..], &[Entry::App(1), Entry::App(2)]);
        // Pasta que ficou vazia some.
        let saved = serde_json::json!({"itens": [{"nome": "Vazia", "apps": ["exec:sumiu"]}]});
        assert!(arrange(&apps2, Some(&saved), &mut id).iter().all(|e| matches!(e, Entry::App(_))));
    }

    #[test]
    fn busca() {
        let apps = sorted_apps(vec![app("Zeta", false), app("Monitor", true), app("Alfa", false)]);
        assert_eq!(search(&apps, "mon"), vec![1]);
        assert!(search(&apps, "xyz").is_empty());
    }

    #[test]
    fn possibilidades_do_arrasto() {
        let t0 = Instant::now();
        let mut d = Drag { entry: Entry::App(0), origin: Origin::Main(0), in_folder: false, pos: (0.0, 0.0), grab: (0.0, 0.0), gap: 0, cand: None, merge: None, edge: None };
        // Parado pouco tempo sobre um lugar: ainda não abre espaço.
        set_cand(&mut d, Some(Cand::Move(3)), t0);
        set_cand(&mut d, Some(Cand::Move(3)), t0 + Duration::from_millis(50));
        assert_eq!(d.gap, 0);
        set_cand(&mut d, Some(Cand::Move(3)), t0 + DWELL_MOVE);
        assert_eq!(d.gap, 3);
        // Sobre um ícone: vira pasta só depois de parar um pouco; mudar de ideia desfaz.
        set_cand(&mut d, Some(Cand::Merge(1)), t0);
        assert_eq!(d.merge, None);
        set_cand(&mut d, Some(Cand::Merge(1)), t0 + DWELL_MERGE);
        assert_eq!(d.merge, Some(1));
        set_cand(&mut d, None, t0 + DWELL_MERGE);
        assert_eq!(d.merge, None);
    }

    #[test]
    fn grade() {
        let l = Layout::new(Rectangle::new((0, 22).into(), (1280, 700).into()));
        assert!(l.cols >= 3 && l.rows >= 2);
        assert!(l.x >= 0.0 && l.x + l.cols as f32 * l.tile_w <= 1280.0);
        assert!(l.dots_y() <= 722.0);
        assert_eq!(l.tile_at(l.x + 1.0, l.y + 1.0), Some(0));
        assert_eq!(l.tile_at(l.x + l.tile_w + 1.0, l.y + l.tile_h + 1.0), Some(l.cols + 1));
        assert_eq!(l.tile_at(l.x - 1.0, l.y), None);
        let b = Band { top: 300.0, h: 100.0, full_h: 100.0, up: 0.0, split: 300.0, notch_x: 0.0 };
        assert_eq!((b.shift(200.0), b.shift(300.0)), (200.0, 400.0));
    }
}
