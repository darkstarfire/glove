//! A barra de menu e o Dock nos monitores extras (mesa estendida).
//!
//! O monitor principal continua sendo o "dono" da barra e do Dock: é nele que ficam as
//! posições que o resto do compositor usa (menus, ampliação do Dock, área útil). Cada
//! monitor extra ganha uma cópia: a barra pintada na largura dele (com os mesmos menus,
//! extras e relógio) e o Dock com as mesmas peças. O mouse sobre a cópia é traduzido para
//! o ponto equivalente do principal (`chrome_translate`), e os menus que se abrem dali
//! são levados para o monitor em que o clique aconteceu (`shift_menu_level`).
//!
//! A barra dos monitores sem o foco (sem a janela ativa) fica esmaecida, como a do
//! Mac OS X com vários monitores (Preferências ▸ Monitores).

use smithay::utils::{Logical, Point, Rectangle};

use glove_paint::chrome;
use glove_paint::theme::MENUBAR_H;

use crate::shell::{to_tex, BarTarget, MenuLevel, Tex};
use crate::state::Glove;

/// A barra de um monitor extra.
pub struct ExtraBar {
    pub area: Rectangle<i32, Logical>,
    key: Option<BarKey>,
    pub tex: Option<Tex>,
    logo: (f32, f32),
    titles: Vec<(f32, f32)>,
    extras: Vec<(f32, f32)>,
    search: (f32, f32),
}

type BarKey = (Rectangle<i32, Logical>, Vec<String>, Vec<chrome::Extra>, Option<BarTarget>, Option<usize>, bool, u64, u64);

/// O quanto a barra de um monitor sem o foco aparece (o resto é o papel de parede).
const ALPHA_SEM_FOCO: f32 = 0.55;
/// O mesmo para o Dock (ele tem os ícones: apagar tanto quanto a barra some com eles).
const DOCK_SEM_FOCO: f32 = 0.7;

/// Em qual faixa da barra está `x` e o ponto equivalente na outra (pelo item: o mesmo
/// deslocamento dentro dele). Fora dos itens: a parte da esquerda fica onde está; a da
/// direita (extras e relógio), à mesma distância da borda direita.
fn mapear_x(x: f32, de: &ExtraBar, para: (&(f32, f32), &[(f32, f32)], &[(f32, f32)], &(f32, f32)), largura_de: f32, largura_para: f32) -> f32 {
    let dentro = |r: &(f32, f32)| x >= r.0 && x < r.1;
    if dentro(&de.logo) {
        return para.0 .0 + (x - de.logo.0);
    }
    for (r, p) in de.titles.iter().zip(para.1) {
        if dentro(r) {
            return p.0 + (x - r.0);
        }
    }
    for (r, p) in de.extras.iter().zip(para.2) {
        if dentro(r) {
            return p.0 + (x - r.0);
        }
    }
    if dentro(&de.search) {
        return para.3 .0 + (x - de.search.0);
    }
    if x < largura_de / 2.0 {
        x
    } else {
        largura_para - (largura_de - x)
    }
}

impl Glove {
    /// Os monitores extras (mesa estendida): o retângulo e a escala de cada um.
    pub fn extra_areas(&self) -> Vec<(Rectangle<i32, Logical>, f64)> {
        if self.config.monitor.arrangement == "mirror" {
            return Vec::new();
        }
        let principal = self.primary().cloned();
        self.space
            .outputs()
            .filter(|o| Some(*o) != principal.as_ref())
            .filter_map(|o| Some((self.space.output_geometry(o)?, o.current_scale().fractional_scale())))
            .collect()
    }

    /// O retângulo do monitor principal.
    fn primary_rect(&self) -> Option<Rectangle<i32, Logical>> {
        self.primary().and_then(|o| self.space.output_geometry(o))
    }

    /// Pinta (ou reaproveita) a barra de cada monitor extra. Chamado depois da barra do
    /// principal, com os mesmos títulos, extras e menu aberto.
    pub fn prepare_extra_bars(&mut self, titles: &[String], extras: &[chrome::Extra], tgen: u64) {
        if !self.config.monitor.menubar_all {
            self.chrome.extra_bars.clear();
            return;
        }
        let areas = self.extra_areas();
        let dona = self.bar_owner();
        let open_all = self.chrome.menu.as_ref().and_then(|m| match m.target {
            BarTarget::Logo => Some(0),
            BarTarget::Title(i) => Some(i + 1),
            _ => None,
        });
        let open_extra_all = match self.chrome.menu.as_ref().map(|m| m.target) {
            Some(BarTarget::Extra(k)) => Some(k),
            _ => self.control.as_ref().map(|c| c.extra).or(self.extras.slider.as_ref().and(self.volume_extra_index())),
        };
        let mut velhas = std::mem::take(&mut self.chrome.extra_bars);
        let mut novas = Vec::new();
        for (area, escala) in areas {
            // O item aceso só na barra do monitor em que o menu ou painel foi aberto.
            let nesta = dona == Some(area);
            let (open, open_extra, search_open) = if nesta { (open_all, open_extra_all, self.search.open) } else { (None, None, false) };
            let open_t = self.chrome.menu.as_ref().map(|m| m.target).filter(|_| nesta);
            let key: BarKey = (area, titles.to_vec(), extras.to_vec(), open_t, open_extra, search_open, tgen, escala.to_bits());
            let mut barra = match velhas.iter().position(|b| b.area == area) {
                Some(i) => velhas.remove(i),
                None => ExtraBar { area, key: None, tex: None, logo: (0.0, 0.0), titles: Vec::new(), extras: Vec::new(), search: (0.0, 0.0) },
            };
            if barra.key.as_ref() != Some(&key) {
                if let Some((pm, layout)) = chrome::menubar(&self.fonts, &self.theme, area.size.w as f32, titles, extras, "", open, open_extra, search_open, escala as f32) {
                    barra.tex = Some(to_tex(pm));
                    barra.logo = layout.logo;
                    barra.titles = layout.titles;
                    barra.extras = layout.extras;
                    barra.search = layout.search;
                }
                barra.key = Some(key);
            }
            novas.push(barra);
        }
        self.chrome.extra_bars = novas;
    }

    /// O monitor com o foco: o da janela ativa (pelo meio dela) ou, sem janela, o do mouse.
    pub fn active_area(&self) -> Option<Rectangle<i32, Logical>> {
        if let Some(g) = self.focused_window().and_then(|w| self.space.element_geometry(&w)) {
            let meio: Point<f64, Logical> = ((g.loc.x + g.size.w / 2) as f64, (g.loc.y + g.size.h / 2) as f64).into();
            if let Some(r) = self.output_rect_at(meio) {
                return Some(r);
            }
        }
        self.output_rect_at(self.pointer_pos)
    }

    /// O monitor em que está aberto um menu ou painel da barra (menu, central de
    /// controle, volume, Busca Rápida); None sem nenhum aberto.
    pub fn bar_owner(&self) -> Option<Rectangle<i32, Logical>> {
        let ponto = |p: Point<i32, Logical>| self.output_rect_at(p.to_f64());
        if let Some(m) = &self.chrome.menu {
            return m.levels.first().and_then(|l| ponto(l.origin));
        }
        if let Some(c) = &self.control {
            return ponto(c.origin);
        }
        if let Some(s) = &self.extras.slider {
            return ponto(s.origin);
        }
        if self.search.open {
            return ponto(self.search_origin());
        }
        None
    }

    /// O monitor que está "em uso": o do menu ou painel aberto; sem nenhum, o da janela
    /// ativa ou do mouse.
    fn foco_area(&self) -> Option<Rectangle<i32, Logical>> {
        self.bar_owner().or_else(|| self.active_area())
    }

    /// A transparência da barra do monitor `area`: esmaecida se ele não tem o foco (com
    /// mais de um monitor e a opção ligada); None = normal. Com um menu aberto, vale o
    /// monitor dele (antes todas voltavam ao normal e piscavam).
    pub fn bar_alpha(&self, area: Option<Rectangle<i32, Logical>>) -> Option<f32> {
        if !self.config.monitor.dim_inactive || !self.config.monitor.menubar_all || self.extra_areas().is_empty() {
            return None;
        }
        (area.is_some() && self.foco_area() != area).then_some(ALPHA_SEM_FOCO)
    }

    /// O mesmo para o Dock do monitor `area` (com o Dock em todos os monitores): um pouco
    /// menos apagado que a barra, mas o de um monitor sem o foco fica para trás.
    pub fn dock_alpha(&self, area: Option<Rectangle<i32, Logical>>) -> Option<f32> {
        if !self.config.monitor.dim_inactive || !self.config.monitor.dock_all || self.extra_areas().is_empty() {
            return None;
        }
        (area.is_some() && self.foco_area() != area && self.chrome.dock_zoom_area != area).then_some(DOCK_SEM_FOCO)
    }

    /// O Dock aparece neste monitor? Sem ocultar automaticamente, sempre (onde ele
    /// existe); ocultando, só no monitor em que foi revelado.
    pub fn dock_shown_on(&self, area: Option<Rectangle<i32, Logical>>) -> bool {
        if !self.config.dock.autohide {
            return true;
        }
        match self.chrome.dock_reveal {
            Some(r) => Some(r) == area,
            None => area.is_none() || area == self.primary_rect(),
        }
    }

    /// Onde fica o Dock no monitor extra `area` (coordenadas globais), ou None sem o Dock
    /// nos extras.
    pub fn extra_dock_origin(&self, area: Rectangle<i32, Logical>) -> Option<Point<i32, Logical>> {
        if !self.config.monitor.dock_all || self.chrome.dock_layout.is_none() {
            return None;
        }
        Some(self.chrome.dock_pos_for((area.size.w, area.size.h)) + area.loc)
    }

    /// Há uma janela em tela cheia neste monitor (a barra e o Dock da cópia somem).
    pub fn area_fullscreen(&self, area: Rectangle<i32, Logical>) -> bool {
        self.space.elements().any(|w| {
            crate::shell::is_fullscreen(w)
                && self.space.element_geometry(w).is_some_and(|g| {
                    let meio: Point<f64, Logical> = ((g.loc.x + g.size.w / 2) as f64, (g.loc.y + g.size.h / 2) as f64).into();
                    area.to_f64().contains(meio)
                })
        })
    }

    /// O mouse sobre a barra ou o Dock de um monitor extra: o ponto equivalente na barra
    /// ou no Dock do principal e o deslocamento (o ponto real menos o equivalente), que
    /// leva para lá os menus abertos dali. None fora deles.
    pub fn chrome_translate(&self, pos: Point<f64, Logical>) -> Option<(Point<f64, Logical>, Point<i32, Logical>)> {
        if self.greeter || self.lock.is_some() || self.fullscreen_cover() {
            return None;
        }
        let area = self.output_rect_at(pos)?;
        let principal = self.primary_rect()?;
        if area == principal || self.area_fullscreen(area) {
            return None;
        }
        let local: Point<f64, Logical> = pos - area.loc.to_f64();
        if self.config.monitor.menubar_all && local.y < MENUBAR_H as f64 {
            let barra = self.chrome.extra_bars.iter().find(|b| b.area == area)?;
            let c = &self.chrome;
            let x = mapear_x(local.x as f32, barra, (&c.logo_range, &c.title_ranges, &c.extra_ranges, &c.search_range), area.size.w as f32, principal.size.w as f32);
            let q: Point<f64, Logical> = (principal.loc.x as f64 + x as f64, principal.loc.y as f64 + local.y).into();
            let d = pos - q;
            return Some((q, (d.x.round() as i32, d.y.round() as i32).into()));
        }
        if let (Some(o), Some(l)) = (self.extra_dock_origin(area).filter(|_| self.dock_shown_on(Some(area))), self.chrome.dock_layout.as_ref()) {
            let r = Rectangle::<i32, Logical>::new(o, (l.width as i32, l.height as i32).into());
            if r.to_f64().contains(pos) {
                let p = self.chrome.dock_pos();
                let d = o - p;
                return Some((pos - d.to_f64(), d));
            }
        }
        None
    }

    /// Um menu que se abre pela barra ou pelo Dock de um monitor extra vai para lá: o
    /// mesmo deslocamento do clique, e dentro do monitor.
    pub fn shift_menu_level(&self, level: &mut MenuLevel) {
        let Some((_, d)) = self.chrome_translate(self.pointer_pos) else { return };
        let Some(area) = self.output_rect_at(self.pointer_pos) else { return };
        let x = (level.origin.x + d.x).clamp(area.loc.x, (area.loc.x + area.size.w - level.size.w).max(area.loc.x));
        let y = (level.origin.y + d.y).clamp(area.loc.y, (area.loc.y + area.size.h - level.size.h).max(area.loc.y));
        level.origin = (x, y).into();
    }

    /// Um painel da barra (central de controle, volume) aberto pela barra de um monitor
    /// extra vai para lá: o deslocamento do clique, com a largura `w` dentro do monitor.
    pub fn shift_point(&self, p: Point<i32, Logical>, w: i32) -> Point<i32, Logical> {
        let Some((_, d)) = self.chrome_translate(self.pointer_pos) else { return p };
        let Some(area) = self.output_rect_at(self.pointer_pos) else { return p };
        let x = (p.x + d.x).clamp(area.loc.x, (area.loc.x + area.size.w - w).max(area.loc.x));
        (x, p.y + d.y).into()
    }

    /// A área útil de um monitor: no principal, a de sempre; num extra, sem a barra e o
    /// Dock dele (quando estão lá).
    pub fn work_area_of(&self, rect: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        if Some(rect) == self.primary_rect() || self.greeter {
            return self.work_area();
        }
        let mut r = rect;
        if self.config.monitor.menubar_all && self.config.monitor.arrangement != "mirror" {
            r.loc.y += MENUBAR_H;
            r.size.h -= MENUBAR_H;
        }
        if self.config.monitor.dock_all && !self.config.dock.autohide {
            let reserva = match &self.chrome.dock_layout {
                Some(l) if l.side.vertical() => l.shelf.2,
                Some(l) => l.shelf.3,
                None => chrome::dock_shelf_thickness(self.config.dock.size.clamp(24, 128) as f32),
            }
            .ceil() as i32;
            match chrome::DockSide::from_config(&self.config.dock.position) {
                chrome::DockSide::Bottom => r.size.h -= reserva,
                chrome::DockSide::Left => {
                    r.loc.x += reserva;
                    r.size.w -= reserva;
                }
                chrome::DockSide::Right => r.size.w -= reserva,
            }
        }
        r.size.w = r.size.w.max(100);
        r.size.h = r.size.h.max(100);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapeia_os_itens_da_barra() {
        let de = ExtraBar {
            area: Rectangle::new((1280, 0).into(), (1920, 1080).into()),
            key: None,
            tex: None,
            logo: (0.0, 30.0),
            titles: vec![(30.0, 90.0), (90.0, 140.0)],
            extras: vec![(1800.0, 1830.0)],
            search: (1890.0, 1920.0),
        };
        let p_logo = (0.0, 30.0);
        let p_titles = vec![(30.0, 90.0), (90.0, 140.0)];
        let p_extras = vec![(1160.0, 1190.0)];
        let p_search = (1250.0, 1280.0);
        let m = |x| mapear_x(x, &de, (&p_logo, &p_titles, &p_extras, &p_search), 1920.0, 1280.0);
        assert_eq!(m(100.0), 100.0);
        assert_eq!(m(1810.0), 1170.0);
        assert_eq!(m(1900.0), 1260.0);
        // Entre os itens da direita: à mesma distância da borda.
        assert_eq!(m(1700.0), 1060.0);
    }
}
