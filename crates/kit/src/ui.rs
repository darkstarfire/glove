//! Widgets em modo imediato: o app descreve a tela a cada quadro e o kit
//! desenha e responde ao mouse/teclado. Só redesenha quando há evento.

use std::collections::HashMap;

use crate::text::TextBuffer;
use glove_paint::{
    draw::*,
    icons::{self, Glyph, Icon},
    Color, Fonts, Theme,
};

pub type Id = u64;

/// Tipo de alerta: o ícone grande à esquerda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    /// Aviso comum: só o ícone do app.
    Note,
    /// Cuidado (ex.: perder alterações): triângulo amarelo com o ícone do app no canto.
    Caution,
    /// Problema grave: placa vermelha.
    Stop,
}

/// Quebra `text` em linhas que cabem em `max` (respeitando quebras de linha do texto).
fn wrap_lines(fonts: &Fonts, text: &str, px: f32, bold: bool, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let test = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if fonts.measure(&test, px, bold) > max && !line.is_empty() {
                out.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = test;
            }
        }
        out.push(line);
    }
    out
}

/// Marcas de um slider: quantas, rótulos (posição de 0 a 1, texto) e se o valor
/// pula de marca em marca.
pub struct Ticks<'a> {
    pub count: usize,
    pub labels: &'a [(f32, &'a str)],
    pub snap: bool,
}

/// Trilho fino e afundado dos sliders.
fn slider_track(c: &mut Canvas, t: Rect) {
    c.fill(rrect(t.x, t.y, t.w, t.h, t.h / 2.0), &vgrad(t.y, t.y + t.h, &[(0.0, rgba(120, 120, 126, 255)), (1.0, rgba(190, 190, 196, 255))]));
    c.hline(t.x + 2.0, t.y + t.h, t.w - 4.0, rgba(255, 255, 255, 200));
}

/// Polegar em escudo apontando para baixo (15×18 pt), centrado em `cx`, topo em `y`.
fn slider_thumb_down(c: &mut Canvas, cx: f32, y: f32, base: Color) {
    let shape = |inset: f32| {
        let (l, r, t) = (cx - 7.5 + inset, cx + 7.5 - inset, y + inset);
        let (shoulder, tip) = (y + 11.0, y + 18.0 - inset * 1.4);
        let mut pb = glove_paint::tiny_skia::PathBuilder::new();
        pb.move_to(l + 2.5, t);
        pb.line_to(r - 2.5, t);
        pb.quad_to(r, t, r, t + 2.5);
        pb.line_to(r, shoulder);
        pb.line_to(cx, tip);
        pb.line_to(l, shoulder);
        pb.line_to(l, t + 2.5);
        pb.quad_to(l, t, l + 2.5, t);
        pb.close();
        pb.finish()
    };
    c.fill(shape(0.0), &solid(darken(base, 0.45)));
    c.fill(shape(1.0), &vgrad(y, y + 18.0, &[(0.0, darken(base, 0.04)), (0.5, base), (1.0, lighten(base, 0.5))]));
    c.fill_clipped(rrect(cx - 7.5, y, 15.0, 7.0, 0.0), &vgrad(y + 1.0, y + 7.0, &[(0.0, rgba(255, 255, 255, 220)), (1.0, rgba(255, 255, 255, 50))]), shape(1.5));
}

/// Conteúdo de uma parte do segmentado.
enum SegLabel<'a> {
    Glyph(Glyph),
    Text(&'a str),
}

/// Altura do cabeçalho das tabelas e recuo de cada nível da árvore.
pub const TABLE_HEADER_H: f32 = 17.0;
pub const TABLE_INDENT: f32 = 16.0;

/// Coluna de uma tabela. `width` é relativa (as larguras dividem a área na proporção).
/// `right` alinha à direita (números) e faz a primeira ordenação ser decrescente.
pub struct Column<'a> {
    pub title: &'a str,
    pub width: f32,
    pub right: bool,
    pub sortable: bool,
}

impl<'a> Column<'a> {
    pub fn new(title: &'a str, width: f32) -> Self {
        Self { title, width, right: false, sortable: true }
    }
    /// Alinhada à direita (números).
    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }
    pub fn fixed(mut self) -> Self {
        self.sortable = false;
        self
    }
}

/// Linha de uma tabela. `children`: `None` = folha, `Some(aberta)` = tem filhos
/// (mostra o triângulo). `depth` é o nível na árvore (recuo).
#[derive(Clone, Default)]
pub struct TableRow {
    pub cells: Vec<String>,
    pub icon: Option<Icon>,
    pub depth: u16,
    pub children: Option<bool>,
}

/// Estado da tabela guardado pelo app: rolagem e ordenação (coluna, crescente).
#[derive(Clone, Debug, Default)]
pub struct TableState {
    pub scroll: f32,
    pub sort: Option<(usize, bool)>,
    view_h: f32,
    row_h: f32,
}

impl TableState {
    pub fn sorted_by(col: usize, ascending: bool) -> Self {
        Self { sort: Some((col, ascending)), ..Self::default() }
    }
    /// Rola o mínimo para a linha `i` ficar visível (ex.: depois das setas).
    pub fn reveal(&mut self, i: usize) {
        if self.row_h <= 0.0 {
            return;
        }
        let y = i as f32 * self.row_h;
        if y < self.scroll {
            self.scroll = y;
        } else if y + self.row_h > self.scroll + self.view_h {
            self.scroll = y + self.row_h - self.view_h;
        }
    }
}

/// Aparência da tabela.
pub struct TableStyle {
    pub row_h: f32,
    pub px: f32,
    /// Linhas alternadas em azul-claro (Monitor); o Arquivos não usa.
    pub zebra: bool,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self { row_h: 18.0, px: 11.0, zebra: true }
    }
}

/// O que aconteceu na tabela neste quadro.
#[derive(Clone, Copy, Debug, Default)]
pub struct TableResponse {
    /// Linha clicada (selecionar).
    pub pressed: Option<usize>,
    /// Clique duplo numa linha (abrir).
    pub double: Option<usize>,
    /// Triângulo clicado (abrir/fechar).
    pub toggled: Option<usize>,
    /// A ordenação mudou (o app reordena as linhas).
    pub sorted: bool,
}

/// Triângulo de abrir/fechar (▸ fechado, ▾ aberto), centrado em (cx, cy).
pub fn disclosure_triangle(c: &mut Canvas, cx: f32, cy: f32, open: bool, color: Color) {
    let mut pb = glove_paint::tiny_skia::PathBuilder::new();
    if open {
        pb.move_to(cx - 4.5, cy - 2.5);
        pb.line_to(cx + 4.5, cy - 2.5);
        pb.line_to(cx, cy + 3.5);
    } else {
        pb.move_to(cx - 2.5, cy - 4.5);
        pb.line_to(cx + 3.5, cy);
        pb.line_to(cx - 2.5, cy + 4.5);
    }
    pb.close();
    c.fill(pb.finish(), &solid(color));
}

/// Altura do campo de busca.
pub const SEARCH_H: f32 = 22.0;
/// Tamanho de cada parte do controle segmentado.
pub const SEGMENT_W: f32 = 28.0;
pub const SEGMENT_H: f32 = 22.0;

/// Altura da barra de progresso.
pub const PROGRESS_H: f32 = 12.0;

/// Fase de uma animação de `period_ms` (0 a 1), pelo relógio: todas andam juntas
/// e não precisam guardar estado.
fn anim_phase(period_ms: u64) -> f32 {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    (ms % period_ms) as f32 / period_ms as f32
}

/// Largura da barra de rolagem (15 pt, como na era).
pub const SCROLLBAR_W: f32 = 15.0;
const SCROLL_ARROW_H: f32 = 15.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, (px, py): (f32, f32)) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// Resultado de `Ui::hit`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Response {
    pub hover: bool,
    /// Botão pressionado sobre a área (segurando).
    pub down: bool,
    /// Acabou de pressionar neste quadro.
    pub pressed: bool,
    /// Soltou sobre a área (clique completo).
    pub clicked: bool,
    /// Segundo clique rápido (clique duplo), no momento de pressionar.
    pub double: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Backspace,
    Delete,
    Enter,
    Escape,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// Qualquer outra tecla (para capturar atalhos): nome do keysym.
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

/// Uma tecla com modificadores, já formatada como atalho ("Super+Shift+Q").
#[derive(Clone, Debug)]
pub struct KeyPress {
    pub key: Key,
    pub combo: String,
}

/// Estado de entrada acumulado entre dois quadros.
#[derive(Default)]
pub struct Input {
    /// A janela tem o foco do teclado (é a janela ativa).
    pub active: bool,
    pub pos: (f32, f32),
    pub inside: bool,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    /// O mouse se moveu neste quadro.
    pub moved: bool,
    pub scroll: f32,
    /// 1 = clique simples, 2 = duplo, 3 = triplo (no quadro do clique).
    pub clicks: u8,
    pub keys: Vec<KeyPress>,
    /// Itens de menu (sem atalho) escolhidos na barra de menu.
    pub menu_actions: Vec<String>,
    pub text: String,
    pub mods: Mods,
    /// Botão direito pressionado neste quadro (não conta como clique comum).
    pub right_pressed: bool,
}

impl Input {
    /// Houve clique ou tecla neste quadro?
    pub fn has_events(&self) -> bool {
        self.pressed || self.released || self.right_pressed || self.scroll != 0.0 || !self.keys.is_empty() || !self.text.is_empty() || !self.menu_actions.is_empty()
    }

    pub fn end_frame(&mut self) {
        self.pressed = false;
        self.right_pressed = false;
        self.released = false;
        self.scroll = 0.0;
        self.moved = false;
        self.clicks = 0;
        self.keys.clear();
        self.menu_actions.clear();
        self.text.clear();
    }
}

struct PopupState {
    /// Aberto neste quadro: ignora o clique que o abriu.
    fresh: bool,
    id: Id,
    rect: Rect,
    items: Vec<String>,
    selected: usize,
}

/// Memória que sobrevive entre quadros.
#[derive(Default)]
pub struct Memory {
    active: Option<Id>,
    focus: Option<Id>,
    popup: Option<PopupState>,
    /// Item escolhido no pop-up no quadro anterior: (id, índice).
    popup_choice: Option<(Id, usize)>,
    scroll: HashMap<Id, f32>,
    cursor: HashMap<Id, usize>,
    slider_grab: Option<Id>,
    capture: Option<Id>,
    /// Texto copiado neste quadro (o kit publica na área de transferência).
    pub(crate) clip_out: Option<String>,
    /// Widget que pediu para colar.
    pub(crate) paste_req: Option<Id>,
    /// Texto colado chegando para um widget.
    pub(crate) paste_in: Option<(Id, String)>,
    /// Novo título da janela.
    pub(crate) title: Option<String>,
    /// Pedidos para a janela: redimensionar pelo puxador, minimizar.
    pub(crate) resize_req: bool,
    /// Comandos para o Glove (pelo canal dos menus).
    pub(crate) glove_cmds: Vec<String>,
    pub(crate) minimize_req: bool,
    /// Novo tamanho da janela pedido pelo app: (largura, altura, animar).
    pub(crate) size_req: Option<(u32, u32, bool)>,
    /// Pedido para redesenhar daqui a pouco (animações, repetição de setas).
    pub(crate) wake_after: Option<std::time::Duration>,
    /// Onde estão as animações que pediram o próximo quadro (redesenho parcial:
    /// se só elas mudam, só essa área é refeita).
    pub(crate) anim_rects: Vec<Rect>,
    /// Algum pedido de quadro sem área conhecida: o próximo quadro é inteiro.
    pub(crate) anim_full: bool,
    /// Ícone do app (alertas), descoberto pelo app_id.
    pub(crate) app_icon: Option<Icon>,
    /// Sombra do alerta já desfocada: (largura, altura, escala) → canvas.
    alert_shadow: Option<((u32, u32, u32), Canvas)>,
    /// Título do último alerta que já tocou o som (toca uma vez só).
    alert_sounded: Option<String>,
    /// Seta de rolagem apertada: (id, próxima repetição).
    scroll_repeat: Option<(Id, std::time::Instant)>,
    /// Superfícies de rascunho reaproveitadas (recorte de áreas roláveis).
    scratch: HashMap<Id, Canvas>,
    /// Quais foram usadas neste quadro (as outras são liberadas no fim).
    scratch_used: Vec<Id>,
}

impl Memory {
    pub(crate) fn with_app_icon(icon: Icon) -> Self {
        Self { app_icon: Some(icon), ..Self::default() }
    }
}

struct Layout {
    horizontal: bool,
    x0: f32,
    y0: f32,
    cx: f32,
    cy: f32,
    width: f32,
    used_w: f32,
    used_h: f32,
    spacing: f32,
}

pub struct Ui<'a> {
    pub c: &'a mut Canvas,
    pub theme: &'a Theme,
    pub fonts: &'a Fonts,
    /// Fonte dos documentos (Editor etc.).
    pub doc_fonts: &'a Fonts,
    /// Janela redimensionável: desenha o puxador no canto inferior direito.
    pub(crate) resizable: bool,
    input: &'a Input,
    mem: &'a mut Memory,
    stack: Vec<Layout>,
    seed: u64,
    counter: u64,
    /// Clique engolido por um pop-up aberto.
    captured: bool,
    /// Pede outro quadro logo em seguida.
    pub redraw: bool,
    /// Pede para fechar a janela.
    pub close: bool,
}

const TEXT: f32 = 13.0;
const CONTROL_H: f32 = 22.0;

fn ink() -> Color {
    rgba(20, 20, 20, 255)
}

impl<'a> Ui<'a> {
    pub(crate) fn new(c: &'a mut Canvas, theme: &'a Theme, fonts: &'a Fonts, doc_fonts: &'a Fonts, resizable: bool, input: &'a Input, mem: &'a mut Memory) -> Self {
        let (w, h) = (c.w, c.h);
        let captured = mem.popup.as_ref().is_some_and(|p| p.rect.contains(input.pos)) || mem.popup.is_some() && input.pressed;
        let mut ui = Self { c, theme, fonts, doc_fonts, resizable, input, mem, stack: Vec::new(), seed: 0, counter: 0, captured, redraw: false, close: false };
        ui.stack.push(Layout { horizontal: false, x0: 0.0, y0: 0.0, cx: 0.0, cy: 0.0, width: w, used_w: 0.0, used_h: 0.0, spacing: 8.0 });
        let _ = h;
        ui
    }

    // -----------------------------------------------------------------------
    // Identificadores e layout
    // -----------------------------------------------------------------------

    fn next_id(&mut self) -> Id {
        self.counter += 1;
        let mut h = self.seed ^ 0x9e37_79b9_7f4a_7c15;
        h = h.wrapping_mul(31).wrapping_add(self.counter);
        h ^ (h >> 29)
    }

    /// Dá a um trecho da interface uma identidade estável (ex.: linhas de uma lista).
    pub fn push_id(&mut self, key: impl std::hash::Hash, f: impl FnOnce(&mut Self)) {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hs);
        self.seed.hash(&mut hs);
        let (old_seed, old_counter) = (self.seed, self.counter);
        self.seed = hs.finish();
        self.counter = 0;
        f(self);
        self.seed = old_seed;
        self.counter = old_counter;
    }

    fn top(&mut self) -> &mut Layout {
        self.stack.last_mut().unwrap()
    }

    pub fn available_width(&self) -> f32 {
        let l = self.stack.last().unwrap();
        if l.horizontal { l.width - (l.cx - l.x0) } else { l.width }
    }

    pub fn cursor(&self) -> (f32, f32) {
        let l = self.stack.last().unwrap();
        (l.cx, l.cy)
    }

    /// Reserva um espaço e devolve onde desenhar.
    pub fn alloc(&mut self, w: f32, h: f32) -> Rect {
        let l = self.top();
        let r = Rect::new(l.cx, l.cy, w, h);
        if l.horizontal {
            l.cx += w + l.spacing;
            l.used_w = l.cx - l.x0 - l.spacing;
            l.used_h = l.used_h.max(h);
        } else {
            l.cy += h + l.spacing;
            l.used_h = l.cy - l.y0 - l.spacing;
            l.used_w = l.used_w.max(w);
        }
        r
    }

    pub fn space(&mut self, s: f32) {
        let l = self.top();
        if l.horizontal { l.cx += s } else { l.cy += s }
    }

    pub fn spacing(&mut self, s: f32) {
        self.top().spacing = s;
    }

    fn child(&mut self, horizontal: bool, x: f32, y: f32, width: f32, f: impl FnOnce(&mut Self)) -> (f32, f32) {
        let spacing = self.top().spacing;
        self.stack.push(Layout { horizontal, x0: x, y0: y, cx: x, cy: y, width, used_w: 0.0, used_h: 0.0, spacing });
        f(self);
        let l = self.stack.pop().unwrap();
        (l.used_w.max(0.0), l.used_h.max(0.0))
    }

    /// Coloca os widgets lado a lado.
    pub fn horizontal(&mut self, f: impl FnOnce(&mut Self)) {
        let (x, y) = self.cursor();
        let width = self.available_width();
        let (w, h) = self.child(true, x, y, width, f);
        self.alloc(w, h);
    }

    /// Bloco com recuo à esquerda.
    pub fn indent(&mut self, by: f32, f: impl FnOnce(&mut Self)) {
        let (x, y) = self.cursor();
        let width = self.available_width() - by;
        let (w, h) = self.child(false, x + by, y, width, f);
        self.alloc(w + by, h);
    }

    /// Área com posição e largura fixas (não ocupa espaço no layout atual).
    pub fn area(&mut self, r: Rect, f: impl FnOnce(&mut Self)) {
        self.child(false, r.x, r.y, r.w, f);
    }

    /// Como `area`, mas devolve o tamanho usado pelo conteúdo (para a janela se
    /// ajustar a ele com `set_window_size`).
    pub fn area_size(&mut self, r: Rect, f: impl FnOnce(&mut Self)) -> (f32, f32) {
        self.child(false, r.x, r.y, r.w, f)
    }

    /// Coluna com largura fixa (para formulários "rótulo: controle").
    pub fn column(&mut self, width: f32, f: impl FnOnce(&mut Self)) {
        let (x, y) = self.cursor();
        let (_, h) = self.child(false, x, y, width, f);
        self.alloc(width, h);
    }

    /// Muda o título da janela.
    pub fn set_title(&mut self, t: &str) {
        self.mem.title = Some(t.to_string());
    }

    /// Desliga (ou religa) a interação com os widgets seguintes (ex.: atrás de uma folha).
    pub fn set_enabled(&mut self, on: bool) {
        self.captured = !on;
    }

    pub fn mods(&self) -> &Mods {
        &self.input.mods
    }

    /// Itens de menu (sem atalho) escolhidos neste quadro.
    pub fn menu_actions(&self) -> &[String] {
        &self.input.menu_actions
    }

    /// Muda o tamanho da janela (em pontos). Com `animate`, ela cresce ou encolhe
    /// suave, com o topo parado, como as Preferências da era ao trocar de painel.
    /// Pode ser chamado a cada quadro: só age quando o tamanho pedido muda.
    pub fn set_window_size(&mut self, w: f32, h: f32, animate: bool) {
        self.mem.size_req = Some((w.round().max(1.0) as u32, h.round().max(1.0) as u32, animate));
    }

    pub fn minimize(&mut self) {
        self.mem.minimize_req = true;
    }

    /// Pede ao Glove uma captura da tela inteira daqui a `delay_ms`.
    pub fn request_screenshot(&mut self, delay_ms: u64, open: bool) {
        self.mem.glove_cmds.push(format!("SHOT\t{delay_ms}\t{}", open as u8));
    }

    /// Pede ao Glove o fim da sessão: "encerrar", "reiniciar" ou "desligar".
    pub fn request_end_session(&mut self, kind: &str) {
        self.mem.glove_cmds.push(format!("SAIR\t{kind}"));
    }

    /// Pede ao Glove para mostrar um protetor de tela agora (botão Testar).
    pub fn request_screensaver(&mut self, kind: &str) {
        self.mem.glove_cmds.push(format!("PROTETOR\t{kind}"));
    }

    /// Põe `text` na área de transferência.
    pub fn copy(&mut self, text: &str) {
        self.mem.clip_out = Some(text.to_string());
    }

    /// Pede o conteúdo da área de transferência; chega depois em `take_paste`.
    pub fn request_paste(&mut self) {
        self.mem.paste_req = Some(0);
    }

    /// Texto colado que chegou (pedido com `request_paste`).
    pub fn take_paste(&mut self) -> Option<String> {
        match self.mem.paste_in.take() {
            Some((0, t)) => Some(t),
            other => {
                self.mem.paste_in = other;
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Interação básica
    // -----------------------------------------------------------------------

    fn hovered(&self, r: Rect) -> bool {
        !self.captured && self.input.inside && r.contains(self.input.pos)
    }

    /// Lógica de clique comum: devolve (hover, pressionado agora, clicado).
    fn interact(&mut self, id: Id, r: Rect) -> (bool, bool, bool) {
        let hover = self.hovered(r);

        if hover && self.input.pressed {
            self.mem.active = Some(id);
            if self.mem.focus != Some(id) {
                self.mem.focus = None;
            }
        }
        let active = self.mem.active == Some(id);
        let mut clicked = false;
        if active && self.input.released {
            clicked = hover;
            self.mem.active = None;
        }
        (hover, active && self.input.down || (active && self.input.pressed), clicked)
    }

    // -----------------------------------------------------------------------
    // Áreas livres (para apps que desenham a própria interface)
    // -----------------------------------------------------------------------

    /// Interação com um retângulo qualquer.
    pub fn hit(&mut self, r: Rect) -> Response {
        let id = self.next_id();
        let (hover, down, clicked) = self.interact(id, r);
        let pressed = hover && self.input.pressed;
        Response { hover, down, pressed, clicked, double: pressed && self.input.clicks >= 2 }
    }

    /// Rolagem do mouse sobre `r` neste quadro (positivo = para baixo).
    pub fn scroll_in(&self, r: Rect) -> f32 {
        if self.hovered(r) { self.input.scroll } else { 0.0 }
    }

    pub fn pointer(&self) -> (f32, f32) {
        self.input.pos
    }

    /// O botão do mouse foi pressionado neste quadro (em qualquer lugar da janela).
    pub fn mouse_pressed(&self) -> bool {
        self.input.pressed
    }

    /// Botão direito pressionado dentro de `r` neste quadro (menus de contexto).
    pub fn right_clicked(&self, r: Rect) -> bool {
        self.input.right_pressed && !self.captured && r.contains(self.input.pos)
    }

    /// O botão do mouse foi solto neste quadro.
    pub fn mouse_released(&self) -> bool {
        self.input.released
    }

    /// O botão do mouse está pressionado.
    pub fn mouse_down(&self) -> bool {
        self.input.down
    }

    /// O mouse se moveu neste quadro.
    pub fn moved(&self) -> bool {
        self.input.moved
    }

    /// Desenha dentro de `r` com recorte: a função recebe um canvas local
    /// (0,0 = canto de `r`). O canvas é reaproveitado entre quadros.
    pub fn clipped(&mut self, key: u64, r: Rect, f: impl FnOnce(&mut Canvas, &Fonts)) {
        let scale = self.c.scale;
        let id = key ^ 0xc1_1b_ed;
        if !self.visible(r) {
            // Fora da área redesenhada: guarda o rascunho para o próximo quadro.
            if self.mem.scratch.contains_key(&id) {
                self.mem.scratch_used.push(id);
            }
            return;
        }
        let mut sc = match self.mem.scratch.remove(&id) {
            Some(c) if c.w == r.w && c.h == r.h && c.scale == scale => c,
            _ => match Canvas::new(r.w, r.h, scale) {
                Some(c) => c,
                None => return,
            },
        };
        sc.clear(Color::TRANSPARENT);
        f(&mut sc, self.fonts);
        self.c.draw_canvas(&sc, r.x, r.y);
        self.mem.scratch.insert(id, sc);
        self.mem.scratch_used.push(id);
    }

    // -----------------------------------------------------------------------
    // Texto
    // -----------------------------------------------------------------------

    pub fn text_width(&self, t: &str, px: f32, bold: bool) -> f32 {
        self.fonts.measure(t, px, bold)
    }

    pub fn label_ex(&mut self, t: &str, px: f32, bold: bool, color: Color) {
        let w = self.fonts.measure(t, px, bold).ceil();
        let h = (px * 1.35).ceil();
        let r = self.alloc(w, h);
        self.fonts.draw(self.c, t, r.x, r.y + px, px, bold, color);
    }

    pub fn label(&mut self, t: &str) {
        self.label_ex(t, TEXT, false, ink());
    }

    pub fn heading(&mut self, t: &str) {
        self.label_ex(t, TEXT, true, ink());
    }

    pub fn small(&mut self, t: &str) {
        self.label_ex(t, 11.0, false, rgba(90, 90, 90, 255));
    }

    /// Rótulo alinhado à direita numa largura fixa (formulários).
    pub fn label_right(&mut self, t: &str, width: f32) {
        let w = self.fonts.measure(t, TEXT, false);
        let r = self.alloc(width, CONTROL_H);
        self.fonts.draw(self.c, t, r.x + width - w, r.y + 15.0, TEXT, false, ink());
    }

    /// Texto em várias linhas, quebrado na largura disponível.
    pub fn paragraph(&mut self, t: &str) {
        let max = self.available_width();
        let mut line = String::new();
        for word in t.split_whitespace() {
            let test = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if self.fonts.measure(&test, 12.0, false) > max && !line.is_empty() {
                self.label_ex(&line, 12.0, false, rgba(50, 50, 50, 255));
                line = word.to_string();
            } else {
                line = test;
            }
        }
        if !line.is_empty() {
            self.label_ex(&line, 12.0, false, rgba(50, 50, 50, 255));
        }
    }

    pub fn separator(&mut self) {
        let w = self.available_width();
        let r = self.alloc(w, 2.0);
        self.c.hline(r.x, r.y, w, rgba(0, 0, 0, 50));
        self.c.hline(r.x, r.y + 1.0, w, rgba(255, 255, 255, 200));
    }

    // -----------------------------------------------------------------------
    // Botões
    // -----------------------------------------------------------------------

    fn push_button(&mut self, t: &str, default: bool, min_w: f32) -> bool {
        let id = self.next_id();
        let w = (self.fonts.measure(t, TEXT, false) + 32.0).max(min_w).ceil();
        let r = self.alloc(w, CONTROL_H);
        let (_, down, clicked) = self.interact(id, r);
        let base = if down || default { self.theme.accent } else { rgba(238, 238, 240, 255) };
        let base = if down { darken(base, 0.2) } else { base };
        gel_rect(self.c, r.x, r.y, r.w, r.h, r.h / 2.0, base);
        let tw = self.fonts.measure(t, TEXT, false);
        self.fonts.draw(self.c, t, r.x + (r.w - tw) / 2.0, r.y + 15.5, TEXT, false, ink());
        clicked
    }

    pub fn button(&mut self, t: &str) -> bool {
        self.push_button(t, false, 70.0)
    }

    /// Botão principal (azul).
    pub fn default_button(&mut self, t: &str) -> bool {
        self.push_button(t, true, 70.0)
    }

    pub fn small_button(&mut self, t: &str) -> bool {
        self.push_button(t, false, 0.0)
    }

    /// Botão de gel numa posição livre (grades, teclados). `lit` = aceso (ex.: operação ativa).
    /// Tecla neutra (calculadora, teclados): gel claro, texto normal, escurece ao apertar.
    pub fn key_button(&mut self, r: Rect, t: &str, px: f32, lit: bool) -> bool {
        let id = self.next_id();
        let (_, down, clicked) = self.interact(id, r);
        let base = rgba(242, 242, 244, 255);
        let b = if down { darken(base, 0.22) } else if lit { darken(base, 0.12) } else { base };
        gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, b);
        self.c.stroke(rrect(r.x + 0.5, r.y + 0.5, r.w - 1.0, r.h - 1.0, 5.0), &solid(rgba(120, 120, 126, 255)), 1.0);
        let tw = self.fonts.measure(t, px, false);
        self.fonts.draw(self.c, t, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + px * 0.36, px, false, rgba(25, 25, 25, 255));
        clicked
    }

    pub fn gel_button(&mut self, r: Rect, t: &str, base: Color, px: f32, lit: bool) -> bool {
        let id = self.next_id();
        let (_, down, clicked) = self.interact(id, r);
        let b = if down { darken(base, 0.25) } else if lit { lighten(base, 0.35) } else { base };
        gel_rect(self.c, r.x, r.y, r.w, r.h, (r.h / 2.0).min(10.0), b);
        let tw = self.fonts.measure(t, px, true);
        let ink = if base.red() + base.green() + base.blue() < 1.6 { Color::WHITE } else { rgba(25, 25, 25, 255) };
        self.fonts.draw(self.c, t, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + px * 0.36, px, true, ink);
        clicked
    }

    /// Texto digitado neste quadro.
    pub fn typed(&self) -> &str {
        &self.input.text
    }

    /// Teclas especiais pressionadas neste quadro.
    pub fn keys(&self) -> &[KeyPress] {
        &self.input.keys
    }

    // -----------------------------------------------------------------------
    // Caixas de seleção e rádio
    // -----------------------------------------------------------------------

    pub fn checkbox(&mut self, value: &mut bool, t: &str) -> bool {
        let id = self.next_id();
        let tw = self.fonts.measure(t, TEXT, false);
        let r = self.alloc(20.0 + tw, 18.0);
        let (_, down, clicked) = self.interact(id, r);
        if clicked {
            *value = !*value;
        }
        let (bx, by, s) = (r.x + 1.0, r.y + 2.0, 14.0);
        let base = if *value { self.theme.accent } else { rgba(240, 240, 242, 255) };
        gel_rect(self.c, bx, by, s, s, 3.5, if down { darken(base, 0.2) } else { base });
        if *value {
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            pb.move_to(bx + 3.5, by + 7.5);
            pb.line_to(bx + 6.3, by + 10.5);
            pb.line_to(bx + 11.0, by + 3.5);
            self.c.stroke(pb.finish(), &solid(rgba(20, 30, 60, 240)), 2.0);
        }
        self.fonts.draw(self.c, t, r.x + 20.0, r.y + 14.0, TEXT, false, ink());
        clicked
    }

    pub fn radio(&mut self, selected: bool, t: &str) -> bool {
        let id = self.next_id();
        let tw = self.fonts.measure(t, TEXT, false);
        let r = self.alloc(20.0 + tw, 18.0);
        let (_, down, clicked) = self.interact(id, r);
        let (cx, cy) = (r.x + 8.0, r.y + 9.0);
        let base = if selected { self.theme.accent } else { rgba(240, 240, 242, 255) };
        gel_circle(self.c, cx, cy, 7.0, if down { darken(base, 0.2) } else { base });
        if selected {
            self.c.fill(circle(cx, cy + 0.5, 2.8), &solid(rgba(20, 30, 60, 230)));
        }
        self.fonts.draw(self.c, t, r.x + 20.0, r.y + 14.0, TEXT, false, ink());
        clicked && !selected
    }

    /// Grupo de rádios; devolve verdadeiro se mudou.
    pub fn radio_group(&mut self, value: &mut usize, options: &[&str]) -> bool {
        let mut changed = false;
        for (i, o) in options.iter().enumerate() {
            if self.radio(*value == i, o) {
                *value = i;
                changed = true;
            }
        }
        changed
    }

    // -----------------------------------------------------------------------
    // Slider
    // -----------------------------------------------------------------------

    /// Slider horizontal. Devolve verdadeiro enquanto o valor muda.
    pub fn slider(&mut self, value: &mut f32, min: f32, max: f32, width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, 20.0);
        let track = Rect::new(r.x + 8.0, r.y + 8.0, r.w - 16.0, 5.0);
        let hover = self.hovered(r);
        if hover && self.input.pressed {
            self.mem.slider_grab = Some(id);
        }
        let mut changed = false;
        if self.mem.slider_grab == Some(id) {
            if self.input.down || self.input.pressed {
                let t = ((self.input.pos.0 - track.x) / track.w).clamp(0.0, 1.0);
                let v = min + t * (max - min);
                if (v - *value).abs() > f32::EPSILON {
                    *value = v;
                    changed = true;
                }
            }
            if self.input.released {
                self.mem.slider_grab = None;
            }
        }
        // Trilho fino afundado, sem preenchimento (10.4) e polegar redondo em gel.
        slider_track(self.c, track);
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let kx = track.x + track.w * t;
        let grabbed = self.mem.slider_grab == Some(id);
        let a = self.theme.accent;
        gel_circle(self.c, kx, r.y + 10.5, 7.5, if grabbed { lighten(a, 0.2) } else { lighten(a, 0.5) });
        changed
    }

    /// Slider com marcas de graduação e rótulos embaixo (10.3/10.4): polegar azul em
    /// forma de escudo apontando para as marcas. `labels` são (posição de 0 a 1, texto).
    /// Com `snap`, o valor pula de marca em marca. Apagado (`enabled` falso) não responde.
    pub fn slider_ticks(&mut self, value: &mut f32, min: f32, max: f32, width: f32, ticks: &Ticks, enabled: bool) -> bool {
        let id = self.next_id();
        // O trilho fica na altura do texto do rótulo do formulário.
        let h = if ticks.labels.is_empty() { 29.0 } else { 41.0 };
        let r = self.alloc(width, h);
        let track = Rect::new(r.x + 8.0, r.y + 9.0, r.w - 16.0, 4.0);
        let mut changed = false;
        if enabled {
            if self.hovered(Rect::new(r.x, r.y, r.w, 25.0)) && self.input.pressed {
                self.mem.slider_grab = Some(id);
            }
            if self.mem.slider_grab == Some(id) {
                if self.input.down || self.input.pressed {
                    let mut t = ((self.input.pos.0 - track.x) / track.w).clamp(0.0, 1.0);
                    if ticks.snap && ticks.count > 1 {
                        let n = (ticks.count - 1) as f32;
                        t = (t * n).round() / n;
                    }
                    let v = min + t * (max - min);
                    if (v - *value).abs() > f32::EPSILON {
                        *value = v;
                        changed = true;
                    }
                }
                if self.input.released {
                    self.mem.slider_grab = None;
                }
            }
        }
        slider_track(self.c, track);
        // Marcas
        let mark = if enabled { rgba(110, 110, 116, 255) } else { rgba(170, 170, 176, 255) };
        if ticks.count > 1 {
            for i in 0..ticks.count {
                let x = (track.x + track.w * i as f32 / (ticks.count - 1) as f32).round() + 0.5;
                self.c.fill_rect(x - 0.5, r.y + 22.0, 1.0, 4.0, &solid(mark));
            }
        }
        // Rótulos (presos dentro da largura do controle)
        let ink_l = if enabled { rgba(40, 40, 44, 255) } else { rgba(150, 150, 156, 255) };
        for (pos, label) in ticks.labels {
            let tw = self.fonts.measure(label, 10.0, false);
            let x = (track.x + track.w * pos - tw / 2.0).clamp(r.x, r.x + r.w - tw);
            self.fonts.draw(self.c, label, x, r.y + 37.0, 10.0, false, ink_l);
        }
        // Polegar em escudo
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let kx = (track.x + track.w * t).round();
        let grabbed = self.mem.slider_grab == Some(id);
        let a = self.theme.accent;
        let base = if !enabled { rgba(238, 238, 240, 255) } else if grabbed { lighten(a, 0.2) } else { lighten(a, 0.45) };
        slider_thumb_down(self.c, kx, r.y + 3.0, base);
        changed
    }

    // -----------------------------------------------------------------------
    // Pop-up (lista suspensa)
    // -----------------------------------------------------------------------

    pub fn popup(&mut self, value: &mut usize, options: &[String], width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, CONTROL_H);
        let mut changed = false;
        if let Some((pid, idx)) = self.mem.popup_choice {
            if pid == id {
                self.mem.popup_choice = None;
                if idx != *value && idx < options.len() {
                    *value = idx;
                    changed = true;
                }
            }
        }
        let open = self.mem.popup.as_ref().is_some_and(|p| p.id == id);
        let (_, _, clicked) = self.interact(id, r);
        if clicked && !open {
            let item_h = 20.0;
            let h = options.len() as f32 * item_h + 8.0;
            let y = (r.y - (*value as f32) * item_h - 4.0).clamp(2.0, (self.c.h - h - 2.0).max(2.0));
            self.mem.popup = Some(PopupState { fresh: true, id, rect: Rect::new(r.x, y, r.w, h), items: options.to_vec(), selected: *value });
            self.redraw = true;
        }

        // Corpo branco + ponta azul com setas
        gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, rgba(244, 244, 246, 255));
        let cap = 20.0;
        self.c.fill(rrect4(r.x + r.w - cap, r.y + 1.0, cap - 1.0, r.h - 2.0, [0.0, 4.0, 4.0, 0.0]), &vgrad(r.y, r.y + r.h, &[(0.0, darken(self.theme.accent, 0.05)), (0.5, self.theme.accent), (1.0, lighten(self.theme.accent, 0.5))]));
        self.c.fill(rrect4(r.x + r.w - cap + 1.0, r.y + 2.0, cap - 3.0, r.h * 0.45, [0.0, 3.0, 1.0, 0.0]), &solid(rgba(255, 255, 255, 110)));
        let ax = r.x + r.w - cap / 2.0;
        let arrow = |c: &mut Canvas, y: f32, up: bool| {
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            let d = if up { -1.0 } else { 1.0 };
            pb.move_to(ax - 3.5, y);
            pb.line_to(ax + 3.5, y);
            pb.line_to(ax, y + 3.5 * d);
            pb.close();
            c.fill(pb.finish(), &solid(rgba(20, 30, 60, 230)));
        };
        arrow(self.c, r.y + 9.5, true);
        arrow(self.c, r.y + 12.5, false);
        let label = options.get(*value).map(|s| s.as_str()).unwrap_or("");
        let t = self.fonts.ellipsize(label, TEXT, false, r.w - cap - 16.0);
        self.fonts.draw(self.c, &t, r.x + 10.0, r.y + 15.5, TEXT, false, ink());
        changed
    }

    // -----------------------------------------------------------------------
    // Campo de texto
    // -----------------------------------------------------------------------

    /// Edição de uma linha de texto em `r` (foco pelo clique, digitação, setas).
    /// Devolve (mudou, Enter pressionado).
    fn edit_line(&mut self, id: Id, r: Rect, value: &mut String) -> (bool, bool) {
        let hover = self.hovered(r);
        if hover && self.input.pressed {
            self.mem.focus = Some(id);
            self.mem.cursor.insert(id, value.chars().count());
        } else if self.input.pressed && !hover && self.mem.focus == Some(id) {
            self.mem.focus = None;
        }
        let focused = self.mem.focus == Some(id);
        let mut changed = false;
        let mut enter = false;
        if focused {
            let mut cur = (*self.mem.cursor.get(&id).unwrap_or(&value.chars().count())).min(value.chars().count());
            let byte = |s: &String, ci: usize| s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len());
            for ch in self.input.text.chars().filter(|c| !c.is_control()) {
                let b = byte(value, cur);
                value.insert(b, ch);
                cur += 1;
                changed = true;
            }
            for k in &self.input.keys {
                match k.key {
                    Key::Backspace if cur > 0 => {
                        let b = byte(value, cur - 1);
                        value.remove(b);
                        cur -= 1;
                        changed = true;
                    }
                    Key::Delete if cur < value.chars().count() => {
                        let b = byte(value, cur);
                        value.remove(b);
                        changed = true;
                    }
                    Key::Left => cur = cur.saturating_sub(1),
                    Key::Right => cur = (cur + 1).min(value.chars().count()),
                    Key::Home => cur = 0,
                    Key::End => cur = value.chars().count(),
                    Key::Enter => {
                        enter = true;
                        self.mem.focus = None;
                    }
                    Key::Escape => self.mem.focus = None,
                    _ => {}
                }
            }
            self.mem.cursor.insert(id, cur);
        }
        (changed, enter)
    }

    /// Campo de texto num retângulo livre (ex.: renomear no lugar). `autofocus` põe
    /// o cursor nele (no fim). Devolve (mudou, Enter).
    pub fn text_field_in(&mut self, r: Rect, value: &mut String, autofocus: bool) -> (bool, bool) {
        let id = self.next_id();
        if autofocus {
            self.mem.focus = Some(id);
            self.mem.cursor.insert(id, value.chars().count());
        }
        let (changed, enter) = self.edit_line(id, r, value);
        let focused = self.mem.focus == Some(id);
        self.c.fill(rrect(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, 3.0), &solid(with_alpha(self.theme.accent, if focused { 0.5 } else { 0.25 })));
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(Color::WHITE));
        let px = (r.h * 0.62).min(TEXT);
        let base = r.y + r.h / 2.0 + px * 0.36;
        let t = value.clone();
        self.fonts.draw(self.c, &t, r.x + 4.0, base, px, false, ink());
        if focused {
            let cur = (*self.mem.cursor.get(&id).unwrap_or(&0)).min(t.chars().count());
            let before: String = t.chars().take(cur).collect();
            let cx = r.x + 4.0 + self.fonts.measure(&before, px, false);
            self.c.fill_rect(cx, r.y + 2.0, 1.0, r.h - 4.0, &solid(ink()));
        }
        (changed, enter)
    }

    /// Algum campo de texto está com o cursor?
    pub fn has_focus(&self) -> bool {
        self.mem.focus.is_some()
    }

    pub fn text_field(&mut self, value: &mut String, width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, CONTROL_H);
        let (changed, _) = self.edit_line(id, r, value);
        let focused = self.mem.focus == Some(id);

        // Moldura afundada + anel azul quando em foco
        if focused {
            self.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 5.0), &solid(with_alpha(self.theme.accent, 0.45)));
        }
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, 3.0, &vgrad(r.y + 1.0, r.y + 4.0, &[(0.0, rgba(0, 0, 0, 40)), (1.0, rgba(0, 0, 0, 0))]));
        let t = value.clone();
        self.fonts.draw(self.c, &t, r.x + 6.0, r.y + 15.5, TEXT, false, ink());
        if focused {
            let cur = *self.mem.cursor.get(&id).unwrap_or(&0);
            let before: String = t.chars().take(cur).collect();
            let cx = r.x + 6.0 + self.fonts.measure(&before, TEXT, false);
            self.c.fill_rect(cx, r.y + 4.0, 1.0, r.h - 8.0, &solid(ink()));
        }
        changed
    }

    /// Campo de senha: bolinhas no lugar das letras (nada de copiar o texto).
    /// `autofocus` põe o cursor nele. Devolve verdadeiro quando o Enter é pressionado.
    pub fn password_field_in(&mut self, r: Rect, value: &mut String, autofocus: bool) -> bool {
        let id = self.next_id();
        if autofocus {
            self.mem.focus = Some(id);
        }
        let (_, enter) = self.edit_line(id, r, value);
        let focused = self.mem.focus == Some(id);
        if focused {
            self.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 5.0), &solid(with_alpha(self.theme.accent, 0.45)));
        }
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, 3.0, &vgrad(r.y + 1.0, r.y + 4.0, &[(0.0, rgba(0, 0, 0, 40)), (1.0, rgba(0, 0, 0, 0))]));
        let n = value.chars().count();
        let cur = (*self.mem.cursor.get(&id).unwrap_or(&n)).min(n);
        let step = 8.0;
        let max = ((r.w - 16.0) / step) as usize;
        let first = n.saturating_sub(max);
        let cy = r.y + r.h / 2.0;
        for i in first..n {
            self.c.fill(circle(r.x + 9.0 + (i - first) as f32 * step, cy, 3.0), &solid(ink()));
        }
        if focused {
            let cx = r.x + 5.5 + (cur.saturating_sub(first)) as f32 * step;
            self.c.fill_rect(cx, r.y + 4.0, 1.0, r.h - 8.0, &solid(ink()));
        }
        enter
    }

    // -----------------------------------------------------------------------
    // Busca, segmentado e itens de barra de ferramentas
    // -----------------------------------------------------------------------

    /// Campo de busca em cápsula com lupa (como no 10.3/10.4). Mostra `placeholder`
    /// em cinza quando vazio; o "x" à direita e o Esc limpam. Devolve verdadeiro
    /// quando o Enter é pressionado.
    pub fn search_field(&mut self, value: &mut String, placeholder: &str, width: f32) -> bool {
        let r = self.alloc(width, SEARCH_H);
        self.search_field_in(r, value, placeholder)
    }

    /// Campo de busca num retângulo livre (altura recomendada: `SEARCH_H`).
    pub fn search_field_in(&mut self, r: Rect, value: &mut String, placeholder: &str) -> bool {
        let id = self.next_id();
        let clear_id = self.next_id();
        let clear = Rect::new(r.x + r.w - 19.0, r.y + (r.h - 14.0) / 2.0, 14.0, 14.0);
        // O "x" vem antes do campo, para o clique nele não mover o cursor.
        let mut cleared = false;
        if !value.is_empty() {
            let (_, _, clicked) = self.interact(clear_id, clear);
            if clicked || (self.hovered(clear) && self.input.pressed) {
                cleared = true;
            }
        }
        let field = Rect::new(r.x, r.y, r.w - if value.is_empty() { 0.0 } else { 20.0 }, r.h);
        let (_, enter) = if cleared { (false, false) } else { self.edit_line(id, field, value) };
        let focused = self.mem.focus == Some(id);
        if cleared || focused && self.input.keys.iter().any(|k| k.key == Key::Escape) {
            value.clear();
            self.mem.cursor.insert(id, 0);
            self.mem.focus = Some(id);
        }

        let rad = r.h / 2.0;
        if self.mem.focus == Some(id) {
            self.c.fill(rrect(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, rad + 3.0), &solid(with_alpha(self.theme.accent, 0.45)));
        }
        self.c.fill(rrect(r.x, r.y, r.w, r.h, rad), &solid(rgba(118, 118, 124, 255)));
        let inner = || rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, rad - 1.0);
        self.c.fill(inner(), &solid(Color::WHITE));
        self.c.fill_clipped(rrect(r.x, r.y, r.w, 4.0, 0.0), &vgrad(r.y + 1.0, r.y + 4.0, &[(0.0, rgba(0, 0, 0, 55)), (1.0, rgba(0, 0, 0, 0))]), inner());
        icons::magnifier(self.c, r.x + 12.0, r.y + r.h / 2.0, 12.0, rgba(70, 70, 74, 255));

        let tx = r.x + 22.0;
        let avail = r.w - 22.0 - if value.is_empty() { 8.0 } else { 24.0 };
        let base = r.y + r.h / 2.0 + 4.5;
        let px = 12.0;
        if value.is_empty() {
            if self.mem.focus != Some(id) {
                let t = self.fonts.ellipsize(placeholder, px, false, avail);
                self.fonts.draw(self.c, &t, tx, base, px, false, rgba(150, 150, 154, 255));
            }
        } else {
            // Texto longo: mostra o final (onde se digita).
            let mut shown: String = value.clone();
            while self.fonts.measure(&shown, px, false) > avail && !shown.is_empty() {
                shown.remove(0);
            }
            self.fonts.draw(self.c, &shown, tx, base, px, false, ink());
            // Botão de limpar: bolinha cinza com "x" branco.
            let (cx, cy) = (clear.x + 7.0, clear.y + 7.0);
            self.c.fill(circle(cx, cy, 6.5), &solid(rgba(150, 150, 156, 255)));
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            pb.move_to(cx - 2.5, cy - 2.5);
            pb.line_to(cx + 2.5, cy + 2.5);
            pb.move_to(cx + 2.5, cy - 2.5);
            pb.line_to(cx - 2.5, cy + 2.5);
            self.c.stroke(pb.finish(), &solid(Color::WHITE), 1.4);
        }
        if self.mem.focus == Some(id) {
            let cur = (*self.mem.cursor.get(&id).unwrap_or(&0)).min(value.chars().count());
            let full_w = self.fonts.measure(value, px, false);
            let before: String = value.chars().take(cur).collect();
            let off = (full_w - avail).max(0.0);
            let cx = tx + self.fonts.measure(&before, px, false) - off;
            self.c.fill_rect(cx.max(tx), r.y + 4.0, 1.0, r.h - 8.0, &solid(ink()));
        }
        enter
    }

    /// Controle segmentado com glifos (10.3/10.4): cápsula clara com divisórias e o
    /// segmento escolhido em azul claro. `selected` = None para botões momentâneos
    /// (ex.: Voltar/Avançar); `enabled` vazio = todos ativos. Devolve o segmento clicado.
    pub fn segmented(&mut self, glyphs: &[Glyph], selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
        let r = self.alloc(glyphs.len() as f32 * SEGMENT_W, SEGMENT_H);
        self.segmented_in(r, glyphs, selected, enabled)
    }

    /// Segmentado num retângulo livre (as partes dividem a largura igualmente).
    pub fn segmented_in(&mut self, r: Rect, glyphs: &[Glyph], selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
        let n = glyphs.len().max(1);
        let widths = vec![r.w / n as f32; n];
        let labels: Vec<SegLabel> = glyphs.iter().map(|g| SegLabel::Glyph(*g)).collect();
        self.segments(r, &labels, &widths, selected, enabled)
    }

    /// Segmentado com textos (abas do 10.3/10.4), centrado na largura disponível.
    pub fn segmented_text(&mut self, labels: &[&str], selected: Option<usize>) -> Option<usize> {
        let avail = self.available_width();
        let (x0, y0) = self.cursor();
        self.alloc(avail, SEGMENT_H);
        let widths: Vec<f32> = labels.iter().map(|l| (self.fonts.measure(l, 12.0, false) + 24.0).round()).collect();
        let total: f32 = widths.iter().sum();
        let r = Rect::new((x0 + (avail - total) / 2.0).round(), y0, total, SEGMENT_H);
        let segs: Vec<SegLabel> = labels.iter().map(|l| SegLabel::Text(l)).collect();
        self.segments(r, &segs, &widths, selected, &[])
    }

    fn segments(&mut self, r: Rect, labels: &[SegLabel], widths: &[f32], selected: Option<usize>, enabled: &[bool]) -> Option<usize> {
        let rad = 4.0;
        self.c.fill(rrect(r.x, r.y, r.w, r.h, rad), &solid(rgba(132, 132, 138, 255)));
        let inner = || rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, rad - 1.0);
        let mut out = None;
        let a = self.theme.accent;
        let mut x = r.x;
        for (i, label) in labels.iter().enumerate() {
            let id = self.next_id();
            let seg = Rect::new(x.round(), r.y, (x + widths[i]).round() - x.round(), r.h);
            x += widths[i];
            let on = enabled.get(i).copied().unwrap_or(true);
            let (_, down, clicked) = if on { self.interact(id, seg) } else { (false, false, false) };
            if clicked {
                out = Some(i);
            }
            let sel = selected == Some(i);
            let stops: [(f32, Color); 4] = if sel {
                [(0.0, lighten(a, 0.72)), (0.48, lighten(a, 0.52)), (0.52, lighten(a, 0.4)), (1.0, lighten(a, 0.64))]
            } else {
                [(0.0, rgba(253, 253, 253, 255)), (0.48, rgba(242, 242, 242, 255)), (0.52, rgba(232, 232, 232, 255)), (1.0, rgba(246, 246, 246, 255))]
            };
            let stops = stops.map(|(t, c)| (t, if down { darken(c, 0.15) } else { c }));
            self.c.fill_clipped(rrect(seg.x, r.y, seg.w, r.h, 0.0), &vgrad(r.y, r.y + r.h, &stops), inner());
            if i > 0 {
                self.c.vline(seg.x, r.y + 1.0, r.h - 2.0, rgba(150, 150, 156, 255));
            }
            let color = if on { rgba(30, 30, 34, 255) } else { rgba(165, 165, 170, 255) };
            match label {
                SegLabel::Glyph(g) => icons::glyph(self.c, *g, (seg.x + seg.w / 2.0).round(), (r.y + r.h / 2.0).round(), 12.0, color),
                SegLabel::Text(t) => {
                    let tw = self.fonts.measure(t, 12.0, false);
                    self.fonts.draw(self.c, t, seg.x + (seg.w - tw) / 2.0, r.y + 15.5, 12.0, false, color);
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Formulário e caixa com abas
    // -----------------------------------------------------------------------

    /// Linha de formulário da era: rótulo alinhado à direita numa coluna de `label_w`
    /// (termina em ":" se tiver texto) e os controles à esquerda numa coluna comum.
    pub fn form_row(&mut self, label: &str, label_w: f32, f: impl FnOnce(&mut Self)) {
        self.horizontal(|ui| {
            let t = if label.is_empty() || label.ends_with(':') { label.to_string() } else { format!("{label}:") };
            ui.label_right(&t, label_w);
            ui.column(ui.available_width(), f);
        });
    }

    /// Nota pequena embaixo de um controle de formulário, alinhada com a coluna dos controles.
    pub fn form_note(&mut self, label_w: f32, text: &str) {
        let sp = self.stack.last().map(|l| l.spacing).unwrap_or(8.0);
        self.indent(label_w + sp, |ui| ui.small(text));
    }

    /// Caixa em relevo com abas (segmentado de texto) centradas na borda de cima.
    /// Devolve verdadeiro quando a aba muda.
    pub fn tab_box(&mut self, value: &mut usize, labels: &[&str], height: f32, f: impl FnOnce(&mut Self)) -> bool {
        let w = self.available_width();
        let (x0, y0) = self.cursor();
        let r = self.alloc(w, height);
        let b = Rect::new(r.x, r.y + SEGMENT_H / 2.0, r.w, r.h - SEGMENT_H / 2.0);
        self.c.fill(rrect(b.x, b.y, b.w, b.h, 6.0), &solid(rgba(0, 0, 0, 12)));
        self.c.stroke(rrect(b.x + 0.5, b.y + 0.5, b.w - 1.0, b.h - 1.0, 6.0), &solid(rgba(0, 0, 0, 55)), 1.0);
        self.c.fill_clipped(rrect(b.x, b.y, b.w, 5.0, 0.0), &vgrad(b.y + 1.0, b.y + 5.0, &[(0.0, rgba(0, 0, 0, 22)), (1.0, rgba(0, 0, 0, 0))]), rrect(b.x + 1.0, b.y + 1.0, b.w - 2.0, b.h - 2.0, 5.0));
        let mut changed = false;
        self.area(Rect::new(x0, y0, w, SEGMENT_H), |ui| {
            if let Some(i) = ui.segmented_text(labels, Some(*value)) {
                if i != *value {
                    *value = i;
                    changed = true;
                }
            }
        });
        self.area(Rect::new(b.x + 18.0, b.y + SEGMENT_H / 2.0 + 14.0, b.w - 36.0, b.h - SEGMENT_H / 2.0 - 24.0), f);
        changed
    }

    /// Item de barra de ferramentas: ícone com o nome embaixo (Arquivos, Preferências).
    /// Realce ao passar o mouse e ao apertar; `selected` deixa marcado.
    pub fn toolbar_item_in(&mut self, r: Rect, icon: Icon, label: &str, selected: bool) -> bool {
        let id = self.next_id();
        let (hover, down, clicked) = self.interact(id, r);
        if selected || down {
            self.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(0, 0, 0, if down { 45 } else { 30 })));
        } else if hover {
            self.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(255, 255, 255, 60)));
        }
        let size = (r.h - 22.0).clamp(16.0, 34.0);
        icons::draw(self.c, self.fonts, icon, r.x + (r.w - size) / 2.0, r.y + 3.0, size, label.chars().next());
        let t = self.fonts.ellipsize(label, 11.0, false, r.w - 4.0);
        let tw = self.fonts.measure(&t, 11.0, false);
        self.fonts.draw(self.c, &t, r.x + (r.w - tw) / 2.0, r.y + r.h - 6.0, 11.0, false, ink());
        clicked
    }

    // -----------------------------------------------------------------------
    // Captura de atalho
    // -----------------------------------------------------------------------

    /// Campo que grava a próxima combinação de teclas.
    pub fn shortcut_field(&mut self, value: &mut String, width: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, CONTROL_H);
        let (_, _, clicked) = self.interact(id, r);
        if clicked {
            self.mem.capture = Some(id);
        }
        let capturing = self.mem.capture == Some(id);
        let mut changed = false;
        if capturing {
            if let Some(k) = self.input.keys.iter().find(|k| !k.combo.is_empty()) {
                if k.key == Key::Escape {
                    self.mem.capture = None;
                } else {
                    *value = k.combo.clone();
                    self.mem.capture = None;
                    changed = true;
                }
            }
        }
        let base = if capturing { lighten(self.theme.accent, 0.6) } else { rgba(244, 244, 246, 255) };
        gel_rect(self.c, r.x, r.y, r.w, r.h, 5.0, base);
        let shown = if capturing { "Pressione as teclas…".to_string() } else { pretty_combo(value) };
        let tw = self.fonts.measure(&shown, 12.0, false);
        self.fonts.draw(self.c, &shown, r.x + (r.w - tw) / 2.0, r.y + 15.0, 12.0, false, ink());
        changed
    }

    // -----------------------------------------------------------------------
    // Abas (segmentadas) e caixa de grupo
    // -----------------------------------------------------------------------

    /// Abas soltas (segmentado de texto centrado). Para abas sobre uma caixa, use `tab_box`.
    pub fn tabs(&mut self, value: &mut usize, labels: &[&str]) -> bool {
        match self.segmented_text(labels, Some(*value)) {
            Some(i) if i != *value => {
                *value = i;
                true
            }
            _ => false,
        }
    }

    /// Seção que abre e fecha (10.2–10.4): triângulo e título em negrito; clicar em
    /// qualquer um dos dois alterna. Devolve verdadeiro quando muda. O app desenha o
    /// conteúdo logo depois, se `open`.
    pub fn disclosure(&mut self, open: &mut bool, title: &str) -> bool {
        let id = self.next_id();
        let tw = self.fonts.measure(title, 12.0, true);
        let r = self.alloc(18.0 + tw, 18.0);
        let (hover, down, clicked) = self.interact(id, r);
        let _ = hover;
        if clicked {
            *open = !*open;
            self.redraw = true;
        }
        let color = if down { rgba(30, 30, 34, 255) } else { rgba(90, 90, 96, 255) };
        disclosure_triangle(self.c, r.x + 6.0, r.y + 9.0, *open, color);
        self.fonts.draw(self.c, title, r.x + 16.0, r.y + 13.5, 12.0, true, ink());
        clicked
    }

    /// Caixa clara com borda suave, de altura fixa, para agrupar controles.
    pub fn group(&mut self, height: f32, f: impl FnOnce(&mut Self)) {
        let w = self.available_width();
        let r = self.alloc(w, height);
        self.c.fill(rrect(r.x, r.y, r.w, r.h, 6.0), &solid(rgba(0, 0, 0, 22)));
        self.c.fill(rrect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 5.0), &solid(rgba(255, 255, 255, 70)));
        self.area(Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0), f);
    }

    /// Pede um novo quadro daqui a `d` (sem eventos), para animações.
    /// O quadro pedido assim é refeito por inteiro; se só uma área muda, use
    /// [`Ui::wake_after_in`].
    pub fn wake_after(&mut self, d: std::time::Duration) {
        self.mem.wake_after = Some(self.mem.wake_after.map_or(d, |w| w.min(d)));
        self.mem.anim_full = true;
    }

    /// Pede um novo quadro daqui a `d` em que só `r` muda: o kit redesenha e
    /// manda para a tela só essa área (pulso do botão, listras do progresso…).
    pub fn wake_after_in(&mut self, d: std::time::Duration, r: Rect) {
        self.mem.wake_after = Some(self.mem.wake_after.map_or(d, |w| w.min(d)));
        self.mem.anim_rects.push(r);
    }

    /// O retângulo cai na área que está sendo redesenhada? (Fora dela, o
    /// desenho é descartado; pular o trabalho caro é opcional.)
    pub fn visible(&self, r: Rect) -> bool {
        self.c.visible(r.x, r.y, r.w, r.h)
    }

    // -----------------------------------------------------------------------
    // Alerta
    // -----------------------------------------------------------------------

    /// Alerta da era como folha que desce do topo da janela (o resto escurece e fica
    /// bloqueado: o app deve chamar `set_enabled(false)` antes de desenhar o resto
    /// enquanto o alerta estiver aberto). `buttons` vão da esquerda para a direita: o
    /// último é o padrão (Enter, azul pulsante), "Cancelar" responde ao Esc e, com três
    /// botões, o primeiro fica afastado à esquerda (ex.: "Não salvar").
    /// Devolve o índice do botão escolhido.
    pub fn alert(&mut self, kind: AlertKind, title: &str, text: &str, buttons: &[&str]) -> Option<usize> {
        self.alert_impl(kind, title, text, buttons, true)
    }

    /// Alerta que ocupa a janela inteira (janela própria de alerta, como a de
    /// Desligar): sem folha nem fundo escurecido, e a janela toma a altura do texto.
    pub fn alert_window(&mut self, kind: AlertKind, title: &str, text: &str, buttons: &[&str]) -> Option<usize> {
        self.alert_impl(kind, title, text, buttons, false)
    }

    fn alert_impl(&mut self, kind: AlertKind, title: &str, text: &str, buttons: &[&str], sheet: bool) -> Option<usize> {
        // Avisos e erros tocam o alerta do painel Som uma vez ao aparecer.
        if matches!(kind, AlertKind::Caution | AlertKind::Stop) && self.mem.alert_sounded.as_deref() != Some(title) {
            self.mem.alert_sounded = Some(title.to_string());
            glove_config::sons::alert();
        }
        use std::time::Duration;
        self.set_enabled(true);
        let (w, h) = (self.c.w, self.c.h);
        let (sx, sw) = if sheet {
            let sw = (w - 24.0).min(440.0).max(200.0);
            (((w - sw) / 2.0).round(), sw)
        } else {
            (0.0, w)
        };
        const ICON: f32 = 64.0;
        let tx = sx + 20.0 + ICON + 16.0;
        let tw = sx + sw - 20.0 - tx;
        let title_lines = wrap_lines(self.fonts, title, 13.0, true, tw);
        let text_lines = if text.is_empty() { Vec::new() } else { wrap_lines(self.fonts, text, 11.0, false, tw) };
        let text_h = title_lines.len() as f32 * 17.0 + if text_lines.is_empty() { 0.0 } else { 6.0 + text_lines.len() as f32 * 15.0 };
        let natural = (16.0 + text_h.max(ICON - 4.0) + 20.0 + CONTROL_H + 16.0).round();
        if !sheet {
            self.set_window_size(w, natural, false);
            self.c.fill_rect(0.0, 0.0, w, h, &solid(self.theme.body_b));
            self.c.pinstripes(0.0, 0.0, w, h, self.theme.body_a, self.theme.body_b);
        }
        let sh = natural.min(h);
        if sheet {
            // Fundo escurecido, sombra e a folha listrada encostada no topo.
            self.c.fill_rect(0.0, 0.0, w, h, &solid(rgba(0, 0, 0, 40)));
            // O desfoque da sombra é caro: só é refeito quando o tamanho muda (o pulso redesenha).
            let key = (sw as u32, sh as u32, self.c.scale.to_bits());
            if self.mem.alert_shadow.as_ref().map(|(k, _)| *k) != Some(key) {
                self.mem.alert_shadow = Canvas::shadow(-14.0, 0.0, sw + 28.0, sh + 18.0, sw, sh, &ShadowSpec { blur: 14.0, offset_y: 4.0, radius: 6.0, alpha: 0.45 }, self.c.scale).map(|c| (key, c));
            }
            if let Some((_, shadow)) = &self.mem.alert_shadow {
                self.c.draw_canvas(shadow, sx - 14.0, 0.0);
            }
            let shape = || rrect4(sx, 0.0, sw, sh, [0.0, 0.0, 6.0, 6.0]);
            self.c.fill(shape(), &solid(self.theme.body_b));
            self.c.pinstripes(sx, 0.0, sw, sh - 6.0, self.theme.body_a, self.theme.body_b);
            self.c.stroke(shape(), &solid(rgba(0, 0, 0, 80)), 1.0 / self.c.scale);
        }

        // Ícone
        let (ix, iy) = (sx + 20.0, 14.0);
        let app = self.mem.app_icon.unwrap_or(Icon::Window);
        match kind {
            AlertKind::Note => icons::draw(self.c, self.fonts, app, ix, iy, ICON, None),
            AlertKind::Caution => {
                icons::alert_caution(self.c, ix, iy, ICON);
                icons::draw(self.c, self.fonts, app, ix + ICON - 30.0, iy + ICON - 30.0, 32.0, None);
            }
            AlertKind::Stop => icons::alert_stop(self.c, ix, iy, ICON),
        }

        // Textos
        let mut y = 16.0 + 13.0;
        for l in &title_lines {
            self.fonts.draw(self.c, l, tx, y, 13.0, true, ink());
            y += 17.0;
        }
        y += 6.0 - 17.0 + 11.0 + 4.0;
        for l in &text_lines {
            self.fonts.draw(self.c, l, tx, y, 11.0, false, rgba(30, 30, 30, 255));
            y += 15.0;
        }

        // Botões (o padrão pulsa: ~1,6 s por ciclo a 5 quadros/s, só com o alerta aberto)
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        // Só a janela ativa pulsa (como no original); nas outras o botão fica parado
        // e o app não gasta nada.
        let pulse = if self.input.active {
            0.5 + 0.5 * ((ms % 1600) as f32 / 1600.0 * std::f32::consts::TAU).sin()
        } else {
            0.5
        };
        let by = sh - 16.0 - CONTROL_H;
        let widths: Vec<f32> = buttons.iter().map(|b| (self.fonts.measure(b, TEXT, false) + 32.0).max(80.0).ceil()).collect();
        let mut x = sx + sw - 20.0;
        let mut rects = vec![Rect::new(0.0, 0.0, 0.0, 0.0); buttons.len()];
        for i in (0..buttons.len()).rev() {
            if buttons.len() == 3 && i == 0 {
                rects[0] = Rect::new(tx, by, widths[0], CONTROL_H);
                break;
            }
            x -= widths[i];
            rects[i] = Rect::new(x, by, widths[i], CONTROL_H);
            x -= 12.0;
        }
        let mut chosen = None;
        let last = buttons.len().saturating_sub(1);
        if self.input.active && !buttons.is_empty() {
            // Só o botão padrão muda a cada quadro do pulso.
            self.wake_after_in(Duration::from_millis(200), rects[last]);
        }
        for (i, b) in buttons.iter().enumerate() {
            let id = self.next_id();
            let r = rects[i];
            let (_, down, clicked) = self.interact(id, r);
            let base = if i == last {
                let a = self.theme.accent;
                if down { darken(a, 0.2) } else { mix(darken(a, 0.08), lighten(a, 0.3), pulse) }
            } else if down {
                darken(rgba(238, 238, 240, 255), 0.2)
            } else {
                rgba(238, 238, 240, 255)
            };
            gel_rect(self.c, r.x, r.y, r.w, r.h, r.h / 2.0, base);
            let bw = self.fonts.measure(b, TEXT, false);
            self.fonts.draw(self.c, b, r.x + (r.w - bw) / 2.0, r.y + 15.5, TEXT, false, ink());
            if clicked {
                chosen = Some(i);
            }
        }
        let keys = &self.input.keys;
        if keys.iter().any(|k| k.key == Key::Enter) && !buttons.is_empty() {
            chosen = Some(last);
        }
        if keys.iter().any(|k| k.key == Key::Escape) {
            if let Some(i) = buttons.iter().position(|b| b.eq_ignore_ascii_case("Cancelar")) {
                chosen = Some(i);
            } else if buttons.len() == 1 {
                chosen = Some(0);
            }
        }
        // Nada embaixo responde enquanto o alerta está aberto.
        self.set_enabled(false);
        chosen
    }

    // -----------------------------------------------------------------------
    // Progresso e espera
    // -----------------------------------------------------------------------

    /// Barra de progresso (12 pt de altura). `value` de 0 a 1; `None` = indeterminada,
    /// com listras que andam (só pede novos quadros enquanto aparece).
    pub fn progress(&mut self, value: Option<f32>, width: f32) {
        let r = self.alloc(width, PROGRESS_H);
        self.progress_in(r, value);
    }

    /// Barra de progresso num retângulo livre.
    pub fn progress_in(&mut self, r: Rect, value: Option<f32>) {
        let phase = if value.is_none() {
            self.wake_after_in(std::time::Duration::from_millis(50), r);
            anim_phase(900)
        } else {
            0.0
        };
        progress_bar(self.c, r.x, r.y, r.w, r.h, value, phase, self.theme.accent);
    }

    /// Giratório de espera de `size` pt (12 raios, uma volta por segundo).
    pub fn spinner(&mut self, size: f32) {
        let r = self.alloc(size, size);
        self.spinner_at(r.x + size / 2.0, r.y + size / 2.0, size);
    }

    /// Giratório centrado em (cx, cy).
    pub fn spinner_at(&mut self, cx: f32, cy: f32, size: f32) {
        let h = size / 2.0 + 1.0;
        self.wake_after_in(std::time::Duration::from_millis(1000 / 12), Rect::new(cx - h, cy - h, h * 2.0, h * 2.0));
        spinner(self.c, cx, cy, size / 2.0, anim_phase(1000), rgba(60, 60, 64, 255));
    }

    // -----------------------------------------------------------------------
    // Rolagem
    // -----------------------------------------------------------------------

    /// Área rolável na vertical com a barra de rolagem em gel à direita, sempre visível
    /// como na era (sem polegar quando tudo cabe). Aplica a roda do mouse, as setas, o
    /// clique no trilho e o arrasto do polegar a `offset` e devolve a área do conteúdo.
    pub fn scroll_area(&mut self, area: Rect, offset: &mut f32, content: f32, line: f32) -> Rect {
        let id = self.next_id();
        if self.hovered(area) && self.input.scroll != 0.0 {
            *offset += self.input.scroll;
        }
        // A barra para acima do puxador de redimensionar.
        let at_corner = self.resizable && area.x + area.w >= self.c.w - 0.5 && area.y + area.h >= self.c.h - 0.5;
        let sb = Rect::new(area.x + area.w - SCROLLBAR_W, area.y, SCROLLBAR_W, if at_corner { area.h - SCROLLBAR_W } else { area.h });
        self.vscrollbar(id, sb, offset, content, area.h, line);
        Rect::new(area.x, area.y, area.w - SCROLLBAR_W, area.h)
    }

    /// Barra de rolagem vertical: trilho em relevo, polegar em cápsula de gel e as duas
    /// setas juntas embaixo (padrão da era).
    fn vscrollbar(&mut self, id: Id, sb: Rect, offset: &mut f32, content: f32, view: f32, line: f32) {
        use std::time::{Duration, Instant};
        let max = (content - view).max(0.0);
        let arrows = sb.h >= 2.0 * SCROLL_ARROW_H + 30.0;
        let track = Rect::new(sb.x, sb.y, sb.w, sb.h - if arrows { 2.0 * SCROLL_ARROW_H } else { 0.0 });
        let up = Rect::new(sb.x, track.y + track.h, sb.w, SCROLL_ARROW_H);
        let down = Rect::new(sb.x, up.y + SCROLL_ARROW_H, sb.w, SCROLL_ARROW_H);
        let thumb_h = if content > 0.0 { (track.h * view / content).clamp(24.0_f32.min(track.h), track.h) } else { track.h };
        let thumb_y = |o: f32| track.y + if max > 0.0 { o / max * (track.h - thumb_h) } else { 0.0 };
        let (grab, up_id, down_id) = (id ^ 0x5c5c, id ^ 0x5c01, id ^ 0x5c02);

        if max > 0.0 && self.input.pressed {
            let my = self.input.pos.1;
            if self.hovered(track) {
                let ty = thumb_y(*offset);
                if my >= ty && my < ty + thumb_h {
                    self.mem.slider_grab = Some(grab);
                    self.mem.scroll.insert(grab, my - ty);
                } else if crate::platform::SCROLL_JUMP.load(std::sync::atomic::Ordering::Relaxed) {
                    // "Rolar até aqui": o meio do puxador vai para o clique e já pode arrastar.
                    let room = (track.h - thumb_h).max(1.0);
                    *offset = ((my - track.y - thumb_h / 2.0) / room * max).clamp(0.0, max);
                    self.mem.slider_grab = Some(grab);
                    self.mem.scroll.insert(grab, thumb_h / 2.0);
                } else {
                    let page = (view - line).max(line);
                    *offset += if my < ty { -page } else { page };
                }
            } else if arrows && (self.hovered(up) || self.hovered(down)) {
                let (aid, d) = if self.hovered(up) { (up_id, -line) } else { (down_id, line) };
                *offset += d;
                self.mem.active = Some(aid);
                // Segurar a seta repete, depois de uma pausa.
                self.mem.scroll_repeat = Some((aid, Instant::now() + Duration::from_millis(350)));
                self.wake_after(Duration::from_millis(350));
            }
        }
        if self.mem.slider_grab == Some(grab) {
            if self.input.down {
                let off = *self.mem.scroll.get(&grab).unwrap_or(&0.0);
                let t = ((self.input.pos.1 - off - track.y) / (track.h - thumb_h).max(1.0)).clamp(0.0, 1.0);
                *offset = t * max;
            }
            if self.input.released {
                self.mem.slider_grab = None;
            }
        }
        let held = match self.mem.scroll_repeat {
            Some((aid, _)) if aid == up_id || aid == down_id => Some(aid),
            _ => None,
        };
        if let Some(aid) = held {
            if !self.input.down || self.input.released {
                self.mem.scroll_repeat = None;
                if self.mem.active == Some(aid) {
                    self.mem.active = None;
                }
            } else {
                let r = if aid == up_id { up } else { down };
                let now = Instant::now();
                if let Some((_, at)) = self.mem.scroll_repeat.filter(|(_, at)| now >= *at) {
                    let _ = at;
                    if self.hovered(r) {
                        *offset += if aid == up_id { -line } else { line };
                    }
                    self.mem.scroll_repeat = Some((aid, now + Duration::from_millis(50)));
                }
                self.wake_after(Duration::from_millis(50));
            }
        }
        *offset = offset.clamp(0.0, max);

        // Seta apertada (fica azul enquanto o mouse está sobre ela).
        let lit_up = self.input.down && held == Some(up_id) && self.hovered(up);
        let lit_down = self.input.down && held == Some(down_id) && self.hovered(down);
        let (accent, grabbed) = (self.theme.accent, self.mem.slider_grab == Some(grab));

        // Trilho em relevo com listras leves
        let c = &mut *self.c;
        c.fill_rect(sb.x, sb.y, sb.w, sb.h, &linear(sb.x, 0.0, sb.x + sb.w, 0.0, &[(0.0, rgba(200, 200, 204, 255)), (0.35, rgba(238, 238, 240, 255)), (1.0, rgba(226, 226, 230, 255))]));
        c.pinstripes(sb.x + 1.0, track.y, sb.w - 1.0, track.h, rgba(255, 255, 255, 0), rgba(255, 255, 255, 60));
        c.vline(sb.x, sb.y, sb.h, rgba(150, 150, 156, 255));
        if max > 0.0 {
            let ty = thumb_y(*offset);
            let base = if grabbed { darken(accent, 0.15) } else { accent };
            gel_rect(c, sb.x + 2.0, ty + 1.0, sb.w - 3.0, thumb_h - 2.0, (sb.w - 3.0) / 2.0, base);
        }
        if arrows {
            for (r, is_up, lit) in [(up, true, lit_up), (down, false, lit_down)] {
                let fill = if lit {
                    linear(r.x, 0.0, r.x + r.w, 0.0, &[(0.0, darken(accent, 0.1)), (0.5, lighten(accent, 0.25)), (1.0, accent)])
                } else {
                    linear(r.x, 0.0, r.x + r.w, 0.0, &[(0.0, rgba(222, 222, 226, 255)), (0.5, rgba(252, 252, 252, 255)), (1.0, rgba(236, 236, 240, 255))])
                };
                c.fill_rect(r.x + 1.0, r.y, r.w - 1.0, r.h, &fill);
                c.hline(r.x, r.y, r.w, rgba(150, 150, 156, 255));
                let ink = if lit { Color::WHITE } else if max > 0.0 { rgba(70, 70, 74, 255) } else { rgba(170, 170, 174, 255) };
                let (cx, cy) = (r.x + r.w / 2.0 + 0.5, r.y + r.h / 2.0);
                let d = if is_up { -1.0 } else { 1.0 };
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                pb.move_to(cx - 3.5, cy - 2.0 * d);
                pb.line_to(cx + 3.5, cy - 2.0 * d);
                pb.line_to(cx, cy + 2.5 * d);
                pb.close();
                c.fill(pb.finish(), &solid(ink));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Lista
    // -----------------------------------------------------------------------

    pub fn list(&mut self, selected: &mut Option<usize>, rows: &[String], width: f32, height: f32) -> bool {
        let id = self.next_id();
        let r = self.alloc(width, height);
        let row_h = 19.0;
        let content = rows.len() as f32 * row_h;
        let mut off = *self.mem.scroll.get(&id).unwrap_or(&0.0);
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(130, 130, 135, 255)));
        let inner = self.scroll_area(Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0), &mut off, content, row_h);
        self.mem.scroll.insert(id, off);
        let r = Rect::new(r.x, r.y, inner.w + 2.0, r.h);

        let mut changed = false;
        let hover = self.hovered(r);
        if hover && self.input.pressed {
            let idx = ((self.input.pos.1 - r.y - 1.0 + off) / row_h).floor() as isize;
            if idx >= 0 && (idx as usize) < rows.len() && *selected != Some(idx as usize) {
                *selected = Some(idx as usize);
                changed = true;
            }
        }

        self.c.fill_rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, &solid(Color::WHITE));
        let first = (off / row_h).floor() as usize;
        let visible = (height / row_h).ceil() as usize + 1;
        for i in first..(first + visible).min(rows.len()) {
            let y = r.y + 1.0 + i as f32 * row_h - off;
            let (top, bot) = (y.max(r.y + 1.0), (y + row_h).min(r.y + r.h - 1.0));
            if bot <= top {
                continue;
            }
            let sel = *selected == Some(i);
            if sel {
                self.c.fill_rect(r.x + 1.0, top, r.w - 2.0, bot - top, &vgrad(y, y + row_h, &[(0.0, lighten(self.theme.accent, 0.15)), (1.0, darken(self.theme.accent, 0.1))]));
            } else if i % 2 == 1 {
                self.c.fill_rect(r.x + 1.0, top, r.w - 2.0, bot - top, &solid(rgba(237, 243, 254, 255)));
            }
            if y >= r.y && y + row_h <= r.y + r.h + 4.0 {
                let t = self.fonts.ellipsize(&rows[i], 12.0, false, r.w - 16.0);
                self.fonts.draw(self.c, &t, r.x + 8.0, y + 14.0, 12.0, false, if sel { Color::WHITE } else { ink() });
            }
        }
        changed
    }

    // -----------------------------------------------------------------------
    // Tabela
    // -----------------------------------------------------------------------

    /// Tabela da era (10.4): cabeçalho em gel claro com divisórias, a coluna ordenada
    /// em azul com ▲/▼, linhas alternadas opcionais, seleção chapada (cinza quando a
    /// janela não está ativa), recuo e triângulos de expansão para árvores e barra de
    /// rolagem em gel. O app guarda as linhas já ordenadas: o kit só troca
    /// `state.sort` quando um cabeçalho é clicado e avisa em `sorted`.
    pub fn table(&mut self, area: Rect, cols: &[Column], rows: &[TableRow], selected: Option<usize>, state: &mut TableState, style: &TableStyle) -> TableResponse {
        let id = self.next_id();
        let mut resp = TableResponse::default();
        let hh = TABLE_HEADER_H;
        let rh = style.row_h;

        // Corpo com a barra de rolagem
        let content = rows.len() as f32 * rh;
        let body = self.scroll_area(Rect::new(area.x, area.y + hh, area.w, area.h - hh), &mut state.scroll, content, rh);
        state.view_h = body.h;
        state.row_h = rh;
        let total_w: f32 = cols.iter().map(|c| c.width).sum::<f32>().max(1.0);
        let xs: Vec<f32> = cols.iter().scan(0.0, |acc, c| {
            let x = *acc;
            *acc += c.width / total_w * body.w;
            Some(x.round())
        }).collect();
        let col_end = |j: usize| xs.get(j + 1).copied().unwrap_or(body.w);
        let tree = rows.iter().any(|r| r.children.is_some() || r.depth > 0);

        // Cabeçalho: clique ordena (de novo na mesma coluna inverte).
        for (j, col) in cols.iter().enumerate() {
            let cid = self.next_id();
            let r = Rect::new(area.x + xs[j], area.y, col_end(j) - xs[j], hh);
            let (_, _, clicked) = if col.sortable { self.interact(cid, r) } else { (false, false, false) };
            if clicked {
                state.sort = Some(match state.sort {
                    Some((c, asc)) if c == j => (j, !asc),
                    _ => (j, !col.right),
                });
                resp.sorted = true;
                state.scroll = 0.0;
            }
        }

        // Linhas: clique seleciona, duplo ativa, triângulo abre/fecha.
        let first = (state.scroll / rh).floor().max(0.0) as usize;
        let count = (body.h / rh).ceil() as usize + 1;
        if self.hovered(body) && self.input.pressed {
            let i = ((self.input.pos.1 - body.y + state.scroll) / rh).floor();
            if i >= 0.0 && (i as usize) < rows.len() {
                let i = i as usize;
                let row = &rows[i];
                let tx = body.x + xs.first().copied().unwrap_or(0.0) + 4.0 + row.depth as f32 * TABLE_INDENT;
                if tree && row.children.is_some() && self.input.pos.0 >= tx - 2.0 && self.input.pos.0 < tx + TABLE_INDENT {
                    resp.toggled = Some(i);
                } else {
                    resp.pressed = Some(i);
                    if self.input.clicks >= 2 {
                        resp.double = Some(i);
                    }
                }
                self.redraw = true;
            }
        }

        // Desenho do cabeçalho (inclui o canto sobre a barra de rolagem).
        let a = self.theme.accent;
        let normal = [(0.0, rgba(255, 255, 255, 255)), (0.4, rgba(245, 245, 245, 255)), (0.5, rgba(236, 236, 236, 255)), (0.56, rgba(243, 243, 243, 255)), (1.0, rgba(255, 255, 255, 255))];
        let lit = [(0.0, lighten(a, 0.82)), (0.4, lighten(a, 0.58)), (0.5, lighten(a, 0.42)), (0.56, lighten(a, 0.5)), (1.0, lighten(a, 0.8))];
        let (hy, hb) = (area.y, area.y + hh);
        self.c.fill_rect(area.x, hy, area.w, hh, &vgrad(hy, hb, &normal));
        let sorted_col = state.sort.map(|(c, _)| c);
        for (j, col) in cols.iter().enumerate() {
            let (x0, x1) = (area.x + xs[j], area.x + col_end(j));
            let on = sorted_col == Some(j);
            if on {
                self.c.fill_rect(x0, hy, x1 - x0, hh, &vgrad(hy, hb, &lit));
            }
            if j > 0 {
                self.c.vline(x0, hy + 1.0, hh - 2.0, rgba(196, 196, 200, 255));
            }
            let ink = rgba(25, 25, 28, 255);
            let tw = self.fonts.measure(col.title, 11.0, false);
            let arrow_w = if on { 13.0 } else { 0.0 };
            let avail = x1 - x0 - 12.0 - arrow_w;
            let t = self.fonts.ellipsize(col.title, 11.0, false, avail);
            let tw = tw.min(avail);
            let base = hy + 12.5;
            // Seta de ordenação do lado oposto ao texto.
            let (tx, ax) = if col.right { (x1 - 6.0 - tw, x0 + 9.0) } else { (x0 + 6.0, x1 - 9.0) };
            self.fonts.draw(self.c, &t, tx, base, 11.0, false, ink);
            if on {
                let asc = state.sort.is_some_and(|(_, asc)| asc);
                let cy = hy + hh / 2.0;
                let d = if asc { -1.0 } else { 1.0 };
                let mut pb = glove_paint::tiny_skia::PathBuilder::new();
                pb.move_to(ax - 3.5, cy - 2.0 * d);
                pb.line_to(ax + 3.5, cy - 2.0 * d);
                pb.line_to(ax, cy + 2.5 * d);
                pb.close();
                self.c.fill(pb.finish(), &solid(darken(a, 0.35)));
            }
        }
        if let Some(j) = sorted_col.filter(|&j| j < cols.len()) {
            self.c.hline(area.x + xs[j], hy, col_end(j) - xs[j], darken(a, 0.2));
        }
        self.c.hline(area.x, hb - 1.0, area.w, rgba(160, 160, 166, 255));
        if let Some(j) = sorted_col.filter(|&j| j < cols.len()) {
            self.c.hline(area.x + xs[j], hb - 1.0, col_end(j) - xs[j], darken(a, 0.05));
        }
        self.c.vline(body.x + body.w, hy + 1.0, hh - 2.0, rgba(196, 196, 200, 255));

        // Corpo, desenhado numa superfície recortada.
        let active = self.input.active;
        let off = state.scroll;
        let fonts = self.fonts;
        let px = style.px;
        let zebra = style.zebra;
        self.clipped(id, body, |c, _| {
            c.clear(Color::WHITE);
            if zebra {
                // Listras também abaixo da última linha, como na era.
                let n = (off / rh).floor() as usize;
                let mut i = n;
                loop {
                    let y = i as f32 * rh - off;
                    if y >= body.h {
                        break;
                    }
                    if i % 2 == 1 {
                        c.fill_rect(0.0, y, body.w, rh, &solid(rgba(241, 246, 254, 255)));
                    }
                    i += 1;
                }
            }
            for (i, row) in rows.iter().enumerate().skip(first).take(count) {
                let y = i as f32 * rh - off;
                let sel = selected == Some(i);
                if sel {
                    c.fill_rect(0.0, y, body.w, rh, &solid(if active { a } else { rgba(212, 212, 216, 255) }));
                }
                let ink = if sel && active { Color::WHITE } else { rgba(20, 20, 20, 255) };
                let base = y + (rh + px) / 2.0 - 1.5;
                for (j, col) in cols.iter().enumerate() {
                    let Some(cell) = row.cells.get(j) else { continue };
                    let (mut x0, x1) = (xs[j] + 6.0, col_end(j) - 6.0);
                    if j == 0 {
                        if tree {
                            x0 += row.depth as f32 * TABLE_INDENT;
                            if let Some(open) = row.children {
                                let tc = if sel && active { Color::WHITE } else { rgba(100, 100, 106, 255) };
                                disclosure_triangle(c, x0 + 4.0, y + rh / 2.0, open, tc);
                            }
                            x0 += TABLE_INDENT;
                        }
                        if let Some(icon) = row.icon {
                            let s = (rh - 2.0).min(16.0);
                            icons::draw(c, fonts, icon, x0, y + (rh - s) / 2.0, s, None);
                            x0 += s + 4.0;
                        }
                    }
                    if x1 <= x0 {
                        continue;
                    }
                    let t = fonts.ellipsize(cell, px, false, x1 - x0);
                    let x = if col.right { x1 - fonts.measure(&t, px, false) } else { x0 };
                    fonts.draw(c, &t, x, base, px, false, ink);
                }
            }
        });
        resp
    }

    // -----------------------------------------------------------------------
    // Ícones
    // -----------------------------------------------------------------------

    pub fn icon(&mut self, icon: Icon, size: f32) {
        let r = self.alloc(size, size);
        icons::draw(self.c, self.fonts, icon, r.x, r.y, size, None);
    }

    /// Ícone grande com nome embaixo (barra de painéis das Preferências).
    pub fn icon_tile(&mut self, icon: Icon, t: &str, selected: bool) -> bool {
        let w = (self.fonts.measure(t, 11.0, false) + 12.0).max(64.0);
        let r = self.alloc(w, 58.0);
        self.toolbar_item_in(r, icon, t, selected)
    }

    // -----------------------------------------------------------------------
    // Área de texto (várias linhas)
    // -----------------------------------------------------------------------

    /// Editor de texto com seleção, rolagem e área de transferência.
    /// Devolve verdadeiro se o texto mudou neste quadro.
    pub fn text_area(&mut self, buf: &mut TextBuffer, r: Rect) -> bool {
        let px_doc = if buf.font_px > 0.0 { buf.font_px } else { 13.0 };
        let px_size: f32 = px_doc;
        let lh: f32 = (px_doc * 1.3).round();
        const PAD: f32 = 4.0;
        const SB: f32 = 15.0;
        let id = self.next_id();
        let tr = Rect::new(r.x, r.y, r.w - SB, r.h);
        // A barra de rolagem para acima do puxador de redimensionar.
        let at_corner = self.resizable && r.x + r.w >= self.c.w - 0.5 && r.y + r.h >= self.c.h - 0.5;
        let sb = Rect::new(r.x + r.w - SB, r.y, SB, if at_corner { r.h - SB } else { r.h });
        let fonts = self.doc_fonts;
        let expand = |s: &str| s.replace('\t', "    ");
        let width = |s: &str| fonts.measure(&expand(s), px_size, false);

        if self.mem.focus.is_none() && !self.captured {
            self.mem.focus = Some(id);
        }
        let before_len = buf.len();
        let before_text_changed = buf.modified;
        let mut changed = false;

        // Posição (relativa ao conteúdo) → byte no texto.
        let hit = |buf: &TextBuffer, starts: &[usize], x: f32, y: f32| -> usize {
            let line = ((y / lh).floor().max(0.0) as usize).min(starts.len() - 1);
            let start = starts[line];
            let end = buf.line_end(start);
            let s = &buf.text()[start..end];
            let mut best = start;
            let mut prev_w = 0.0;
            for (i, ch) in s.char_indices() {
                let w = width(&s[..i + ch.len_utf8()]);
                if x < (prev_w + w) / 2.0 {
                    return start + i;
                }
                prev_w = w;
                best = start + i + ch.len_utf8();
            }
            best
        };

        let mut starts = buf.line_starts();
        let content_h = starts.len() as f32 * lh + 2.0 * PAD;
        let max_scroll = (content_h - r.h).max(0.0);

        // Mouse no texto
        let hover = self.hovered(tr);
        let local = |ui: &Self, buf: &TextBuffer| (ui.input.pos.0 - tr.x - PAD + buf.scroll_x, ui.input.pos.1 - tr.y - PAD + buf.scroll_y);
        if hover && self.input.pressed {
            self.mem.focus = Some(id);
            self.mem.active = Some(id);
            let (x, y) = local(self, buf);
            let p = hit(buf, &starts, x, y);
            if self.input.clicks >= 3 {
                let (_, ls) = buf.line_of(p);
                let le = buf.line_end(ls);
                buf.anchor = ls;
                buf.cursor = le;
            } else if self.input.clicks == 2 {
                buf.select_word(p);
            } else {
                buf.set_cursor(p, self.input.mods.shift);
            }
            buf.want_x = None;
        } else if self.mem.active == Some(id) && self.input.down && self.input.moved && !self.input.pressed {
            let (x, y) = local(self, buf);
            let p = hit(buf, &starts, x, y);
            if p != buf.cursor {
                buf.set_cursor(p, true);
            }
        }
        if self.hovered(r) && self.input.scroll != 0.0 {
            buf.scroll_y = (buf.scroll_y + self.input.scroll).clamp(0.0, max_scroll);
        }

        // Barra de rolagem (desenhada aqui; o texto fica à esquerda dela)
        self.vscrollbar(id, sb, &mut buf.scroll_y, content_h, r.h, lh);

        // Teclado
        let focused = self.mem.focus == Some(id) && !self.captured;
        if focused {
            if let Some((pid, text)) = self.mem.paste_in.take() {
                if pid == id {
                    buf.insert(&text);
                } else {
                    self.mem.paste_in = Some((pid, text));
                }
            }
            for ch in self.input.text.chars().filter(|c| !c.is_control()) {
                buf.insert(&ch.to_string());
            }
            let shift = self.input.mods.shift;
            let word = self.input.mods.ctrl || self.input.mods.alt;
            let page_lines = ((r.h / lh) as isize - 1).max(1);
            for k in &self.input.keys {
                match k.combo.as_str() {
                    "Ctrl+A" => {
                        buf.select_all();
                        continue;
                    }
                    "Ctrl+C" | "Ctrl+X" => {
                        if buf.has_selection() {
                            self.mem.clip_out = Some(buf.selected_text().to_string());
                            if k.combo == "Ctrl+X" {
                                buf.delete();
                            }
                        }
                        continue;
                    }
                    "Ctrl+V" => {
                        self.mem.paste_req = Some(id);
                        continue;
                    }
                    "Ctrl+Z" => {
                        buf.undo();
                        continue;
                    }
                    "Ctrl+Shift+Z" | "Ctrl+Y" => {
                        buf.redo();
                        continue;
                    }
                    _ => {}
                }
                let vertical = |buf: &mut TextBuffer, starts: &[usize], delta: isize| {
                    let (line, start) = buf.line_of(buf.cursor);
                    let x = buf.want_x.unwrap_or_else(|| width(&buf.text()[start..buf.cursor]));
                    let target = (line as isize + delta).clamp(0, starts.len() as isize - 1) as usize;
                    let p = if delta < 0 && line == 0 {
                        0
                    } else if delta > 0 && line + 1 == starts.len() {
                        buf.len()
                    } else {
                        hit(buf, starts, x, target as f32 * lh + lh / 2.0)
                    };
                    buf.set_cursor(p, shift);
                    buf.want_x = Some(x);
                };
                match k.key {
                    Key::Backspace => buf.backspace(),
                    Key::Delete => buf.delete(),
                    Key::Enter => buf.insert("\n"),
                    Key::Tab => buf.insert("\t"),
                    Key::Left => buf.left(shift, word),
                    Key::Right => buf.right(shift, word),
                    Key::Up => vertical(buf, &starts, -1),
                    Key::Down => vertical(buf, &starts, 1),
                    Key::PageUp => vertical(buf, &starts, -page_lines),
                    Key::PageDown => vertical(buf, &starts, page_lines),
                    Key::Home => buf.home(shift, self.input.mods.ctrl),
                    Key::End => buf.end(shift, self.input.mods.ctrl),
                    _ => {}
                }
                starts = buf.line_starts();
            }
        }
        if buf.len() != before_len || (buf.modified && !before_text_changed) {
            changed = true;
            starts = buf.line_starts();
        }

        // Mantém o cursor visível
        let content_h = starts.len() as f32 * lh + 2.0 * PAD;
        let max_scroll = (content_h - r.h).max(0.0);
        if buf.reveal {
            buf.reveal = false;
            let (line, start) = buf.line_of(buf.cursor);
            let cy = line as f32 * lh;
            if cy < buf.scroll_y {
                buf.scroll_y = cy;
            } else if cy + lh + 2.0 * PAD > buf.scroll_y + r.h {
                buf.scroll_y = cy + lh + 2.0 * PAD - r.h;
            }
            let cx = width(&buf.text()[start..buf.cursor]);
            let vis = tr.w - 2.0 * PAD - 8.0;
            if cx < buf.scroll_x {
                buf.scroll_x = (cx - 40.0).max(0.0);
            } else if cx > buf.scroll_x + vis {
                buf.scroll_x = cx - vis + 40.0;
            }
        }
        buf.scroll_y = buf.scroll_y.clamp(0.0, max_scroll);

        // Desenho numa superfície de rascunho (recorta o que sai da área)
        let scale = self.c.scale;
        let mut sc = match self.mem.scratch.remove(&id) {
            Some(c) if c.w == tr.w && c.h == tr.h && c.scale == scale => c,
            _ => match Canvas::new(tr.w, tr.h, scale) {
                Some(c) => c,
                None => return changed,
            },
        };
        sc.clear(buf.paper.map(|(r, g, b)| rgba(r, g, b, 255)).unwrap_or(Color::WHITE));
        let (sel_a, sel_b) = buf.selection();
        let sel_color = if focused { self.theme.highlight } else { rgba(212, 212, 216, 255) };
        let first = (buf.scroll_y / lh).floor() as usize;
        let count = (r.h / lh).ceil() as usize + 2;
        for line in first..(first + count).min(starts.len()) {
            let start = starts[line];
            let end = buf.line_end(start);
            let y = PAD + line as f32 * lh - buf.scroll_y;
            let x0 = PAD - buf.scroll_x;
            if sel_a != sel_b && sel_a <= end && sel_b >= start {
                let a = sel_a.max(start);
                let b = sel_b.min(end);
                let xa = width(&buf.text()[start..a]);
                let mut xb = width(&buf.text()[start..b]);
                if sel_b > end {
                    xb += 6.0;
                }
                sc.fill_rect(x0 + xa, y, (xb - xa).max(0.0), lh, &solid(sel_color));
            }
            let s = expand(&buf.text()[start..end]);
            fonts.draw(&mut sc, &s, x0, y + px_size, px_size, false, ink());
            if focused && sel_a == sel_b && buf.cursor >= start && buf.cursor <= end {
                let cx = x0 + width(&buf.text()[start..buf.cursor]);
                sc.fill_rect(cx.round(), y + 1.0, 1.0, lh - 2.0, &solid(ink()));
            }
        }
        self.c.draw_canvas(&sc, tr.x, tr.y);
        self.mem.scratch.insert(id, sc);
        self.mem.scratch_used.push(id);

        // Moldura
        self.c.hline(r.x, r.y, r.w, rgba(120, 120, 126, 255));
        self.c.fill_rect(r.x, r.y, r.w, 3.0, &vgrad(r.y, r.y + 3.0, &[(0.0, rgba(0, 0, 0, 35)), (1.0, rgba(0, 0, 0, 0))]));

        changed
    }

    // -----------------------------------------------------------------------
    // Fim do quadro: desenha o pop-up aberto por cima de tudo
    // -----------------------------------------------------------------------

    /// Puxador no canto inferior direito (redimensionar arrastando).
    fn resize_grip(&mut self) {
        let (w, h) = (self.c.w, self.c.h);
        let r = Rect::new(w - 15.0, h - 15.0, 15.0, 15.0);
        self.c.fill_rect(r.x, r.y, r.w, r.h, &solid(rgba(236, 236, 238, 255)));
        self.c.vline(r.x, r.y, r.h, rgba(150, 150, 156, 255));
        for i in 0..3 {
            let d = 4.0 + i as f32 * 3.5;
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            pb.move_to(w - d, h - 2.0);
            pb.line_to(w - 2.0, h - d);
            self.c.stroke(pb.finish(), &solid(rgba(120, 120, 126, 255)), 1.0);
            let mut pb = glove_paint::tiny_skia::PathBuilder::new();
            pb.move_to(w - d + 1.0, h - 2.0);
            pb.line_to(w - 2.0, h - d + 1.0);
            self.c.stroke(pb.finish(), &solid(rgba(255, 255, 255, 220)), 1.0);
        }
        if !self.captured && self.input.pressed && r.contains(self.input.pos) {
            self.mem.resize_req = true;
        }
    }

    pub(crate) fn finish(mut self) -> (bool, bool) {
        if self.resizable {
            self.resize_grip();
        }
        if let Some(mut p) = self.mem.popup.take() {
            let r = p.rect;
            let fresh = std::mem::replace(&mut p.fresh, false);
            let item_h = 20.0;
            let hover = if r.contains(self.input.pos) { Some(((self.input.pos.1 - r.y - 4.0) / item_h).floor() as isize) } else { None };
            let mut keep = true;
            if !fresh && (self.input.released || self.input.pressed) {
                if let Some(h) = hover {
                    if self.input.released && h >= 0 && (h as usize) < p.items.len() {
                        self.mem.popup_choice = Some((p.id, h as usize));
                        keep = false;
                        self.redraw = true;
                    }
                } else if self.input.pressed {
                    keep = false;
                    self.redraw = true;
                }
            }
            if keep {
                if let Some(sh) = Canvas::shadow(-10.0, 0.0, r.w + 20.0, r.h + 12.0, r.w, r.h, &ShadowSpec { blur: 10.0, offset_y: 3.0, radius: 5.0, alpha: 0.35 }, self.c.scale) {
                    self.c.draw_canvas(&sh, r.x - 10.0, r.y);
                }
                self.c.fill(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(rgba(255, 255, 255, 245)));
                self.c.stroke(rrect(r.x, r.y, r.w, r.h, 5.0), &solid(rgba(0, 0, 0, 60)), 1.0 / self.c.scale);
                for (i, it) in p.items.iter().enumerate() {
                    let y = r.y + 4.0 + i as f32 * item_h;
                    let hot = hover == Some(i as isize);
                    if hot {
                        self.c.fill_rect(r.x, y, r.w, item_h, &vgrad(y, y + item_h, &[(0.0, lighten(self.theme.accent, 0.2)), (1.0, darken(self.theme.accent, 0.1))]));
                    }
                    let color = if hot { Color::WHITE } else { ink() };
                    if i == p.selected {
                        self.fonts.draw(self.c, "✓", r.x + 6.0, y + 15.0, 13.0, false, color);
                    }
                    self.fonts.draw(self.c, it, r.x + 22.0, y + 15.0, 13.0, false, color);
                }
                self.mem.popup = Some(p);
            }
        }
        if self.input.released {
            self.mem.active = None;
        }
        // Libera superfícies de recorte que não apareceram neste quadro.
        let used = std::mem::take(&mut self.mem.scratch_used);
        self.mem.scratch.retain(|k, _| used.contains(k));
        (self.redraw, self.close)
    }
}

/// "Super+Return" → "Super + Enter" (mais legível).
pub fn pretty_combo(s: &str) -> String {
    s.split('+')
        .map(|p| match p {
            "Return" => "Enter",
            "BackSpace" => "Backspace",
            "Comma" | "comma" => ",",
            "Up" => "↑",
            "Down" => "↓",
            "Left" => "←",
            "Right" => "→",
            "Print" => "Print Screen",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}
