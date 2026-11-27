//! Troca de apps no estilo do 10.4: Super+Tab mostra os ícones grandes dos apps
//! abertos (do usado mais recentemente para o mais antigo) numa faixa no meio da
//! tela. Com o Super preso: Tab/→ avançam, Shift+Tab/← voltam, Q encerra, H oculta,
//! Esc cancela. Soltar o Super troca para o app escolhido.
//!
//! Alt+Tab (alternar as janelas do app): a mesma faixa, mas com as miniaturas das
//! janelas do app em uso (a de cima primeiro). Com o Alt preso, Tab/→ e Shift+Tab/←
//! andam, Q fecha a janela escolhida e Esc cancela; soltar o Alt traz a escolhida.

use glove_paint::{chrome, icons::Icon};
use smithay::desktop::Window;

use crate::{
    shell::{app_display_name, to_tex, window_info, Tex},
    state::Glove,
};

/// A faixa das janelas do mesmo app (Alt+Tab).
pub struct WinSwitcher {
    /// As janelas, da de cima da pilha para a de baixo.
    pub windows: Vec<Window>,
    pub selected: usize,
    /// Onde vai cada miniatura, relativo à faixa (de `chrome::window_switcher_layout`).
    pub slots: Vec<[f32; 4]>,
    key: Option<(usize, usize, u64, String)>,
    pub tex: Option<Tex>,
    /// The monitor it opened on (the one with the mouse).
    pub area: smithay::utils::Rectangle<i32, smithay::utils::Logical>,
}

pub struct Switcher {
    /// (pid, ícone, nome), na ordem da faixa.
    pub apps: Vec<(u32, Icon, String)>,
    pub selected: usize,
    key: Option<(Vec<u32>, usize, u64)>,
    pub tex: Option<Tex>,
    /// The monitor it opened on (the one with the mouse).
    pub area: smithay::utils::Rectangle<i32, smithay::utils::Logical>,
}

/// O que fazer com uma tecla enquanto a faixa está aberta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwitcherKey {
    Next,
    Prev,
    Quit,
    Hide,
    Cancel,
    /// O Super foi solto: troca.
    Commit,
}

impl Glove {
    /// Lembra o app usado agora (para a ordem da faixa).
    pub fn note_app_used(&mut self, w: &Window) {
        if let Some(pid) = self.window_pid(w) {
            self.app_mru.retain(|p| *p != pid);
            self.app_mru.insert(0, pid);
            self.app_mru.truncate(64);
        }
    }

    /// Apps com janela (visível, minimizada ou oculta), do mais recente para o mais antigo.
    fn switcher_apps(&self) -> Vec<(u32, Icon, String)> {
        let windows: Vec<Window> = self.space.elements().rev().cloned().chain(self.stashed().cloned()).collect();
        let mut apps: Vec<(u32, Icon, String)> = Vec::new();
        for w in windows {
            let Some(pid) = self.window_pid(&w) else { continue };
            if apps.iter().any(|a| a.0 == pid) {
                continue;
            }
            let app_id = window_info(&w).1;
            let icon = Icon::from_name(&glove_config::app_icon_name(&self.config.dock_apps, &app_id));
            apps.push((pid, icon, app_display_name(&w)));
        }
        let rank = |pid: u32| self.app_mru.iter().position(|p| *p == pid).unwrap_or(usize::MAX);
        // Estável: quem nunca foi usado fica na ordem das janelas.
        apps.sort_by_key(|a| rank(a.0));
        apps
    }

    /// Super+Tab: abre a faixa no próximo app, ou anda se já estiver aberta.
    pub fn switcher_open_or_next(&mut self, back: bool) {
        if self.switcher.is_some() {
            self.switcher_key(if back { SwitcherKey::Prev } else { SwitcherKey::Next });
            return;
        }
        let apps = self.switcher_apps();
        if apps.is_empty() {
            return;
        }
        self.close_menu();
        let n = apps.len();
        let selected = if n > 1 { if back { n - 1 } else { 1 } } else { 0 };
        self.switcher = Some(Switcher { apps, selected, key: None, tex: None, area: self.pointer_area() });
        self.dirty = true;
    }

    pub fn switcher_key(&mut self, k: SwitcherKey) {
        let Some(s) = &mut self.switcher else { return };
        let n = s.apps.len().max(1);
        match k {
            SwitcherKey::Next => s.selected = (s.selected + 1) % n,
            SwitcherKey::Prev => s.selected = (s.selected + n - 1) % n,
            SwitcherKey::Quit | SwitcherKey::Hide => {
                let Some(&(pid, ..)) = s.apps.get(s.selected) else { return };
                if k == SwitcherKey::Quit {
                    for w in self.windows_of_pid(pid) {
                        self.close_window(&w);
                    }
                } else {
                    self.hide_pid(pid);
                }
                // Sai da faixa (o app pode demorar a fechar).
                if let Some(s) = &mut self.switcher {
                    s.apps.retain(|a| a.0 != pid);
                    if s.apps.is_empty() {
                        self.switcher = None;
                    } else {
                        s.selected = s.selected.min(s.apps.len() - 1);
                    }
                }
            }
            SwitcherKey::Cancel => self.switcher = None,
            SwitcherKey::Commit => {
                let pid = s.apps.get(s.selected).map(|a| a.0);
                self.switcher = None;
                if let Some(pid) = pid {
                    self.activate_app(pid);
                }
            }
        }
        self.dirty = true;
    }

    /// Traz o app à frente: mostra as ocultas, todas as janelas dele sobem e a
    /// de cima fica com o foco; só minimizadas: a última volta.
    pub fn activate_app(&mut self, pid: u32) {
        // Só tem janelas em outra mesa virtual: vai até lá.
        let mine = |st: &Self, w: &Window| st.window_pid(w) == Some(pid);
        if !self.space.elements().any(|w| mine(self, w)) {
            let d = self.mesas.away.iter().rev().find(|(w, _, _)| mine(self, w)).map(|(_, _, d)| *d);
            if let Some(d) = d {
                self.switch_desk(d, None);
            }
        }
        let hidden = self.hidden.iter().any(|(w, _)| self.window_pid(w) == Some(pid));
        if hidden {
            let mine: Vec<Window> = self.hidden.iter().filter(|(w, _)| self.window_pid(w) == Some(pid)).map(|(w, _)| w.clone()).collect();
            self.unhide_where(|w| mine.contains(w));
        }
        let visible = self.windows_of_pid(pid);
        if visible.is_empty() {
            if let Some(i) = self.minimized.iter().rposition(|(w, _)| self.window_pid(w) == Some(pid)) {
                self.restore_minimized(i);
            }
            return;
        }
        // Mantém a ordem entre elas; a mais alta do app fica com o foco.
        for w in &visible {
            self.space.raise_element(w, false);
        }
        let top = self.space.elements().rev().find(|w| self.window_pid(w) == Some(pid)).cloned();
        if let Some(top) = top {
            self.focus_window(Some(top));
        }
    }

    pub fn prepare_switcher(&mut self) {
        let sc = self.scale() as f32;
        let (w, _) = self.output_size();
        let tgen = self.theme_gen;
        let Some(s) = &mut self.switcher else { return };
        let key = (s.apps.iter().map(|a| a.0).collect::<Vec<_>>(), s.selected, tgen);
        if s.key.as_ref() != Some(&key) {
            let apps: Vec<(Icon, String)> = s.apps.iter().map(|a| (a.1, a.2.clone())).collect();
            s.tex = chrome::app_switcher(&self.fonts, &apps, s.selected, w as f32 - 80.0, sc).map(to_tex);
            s.key = Some(key);
        }
    }

    /// Alt+Tab: abre a faixa das janelas do app em uso na próxima janela, ou anda se já
    /// estiver aberta. Com uma janela só, não há o que trocar.
    pub fn win_switcher_open_or_next(&mut self, back: bool) {
        if self.win_switcher.is_some() {
            self.win_switcher_key(if back { SwitcherKey::Prev } else { SwitcherKey::Next });
            return;
        }
        let Some(focused) = self.focused_window() else { return };
        let app = window_info(&focused).1;
        let pid = self.window_pid(&focused);
        let windows: Vec<Window> = self
            .space
            .elements()
            .rev()
            .filter(|w| if app.is_empty() { pid.is_some() && self.window_pid(w) == pid } else { window_info(w).1 == app })
            .filter(|w| !self.x11_overlays.contains(w))
            .cloned()
            .collect();
        if windows.len() < 2 {
            return;
        }
        self.close_menu();
        let n = windows.len();
        let selected = if back { n - 1 } else { 1 };
        self.win_switcher = Some(WinSwitcher { windows, selected, slots: Vec::new(), key: None, tex: None, area: self.pointer_area() });
        self.dirty = true;
    }

    pub fn win_switcher_key(&mut self, k: SwitcherKey) {
        let Some(s) = &mut self.win_switcher else { return };
        // Janelas que fecharam enquanto a faixa estava aberta saem dela.
        let alive: Vec<Window> = self.space.elements().cloned().collect();
        s.windows.retain(|w| alive.contains(w));
        if s.windows.is_empty() {
            self.win_switcher = None;
            self.dirty = true;
            return;
        }
        let n = s.windows.len();
        s.selected = s.selected.min(n - 1);
        match k {
            SwitcherKey::Next => s.selected = (s.selected + 1) % n,
            SwitcherKey::Prev => s.selected = (s.selected + n - 1) % n,
            SwitcherKey::Quit => {
                let w = s.windows.remove(s.selected);
                if s.windows.is_empty() {
                    self.win_switcher = None;
                } else {
                    s.selected = s.selected.min(s.windows.len() - 1);
                }
                self.close_window(&w);
            }
            SwitcherKey::Hide => {}
            SwitcherKey::Cancel => self.win_switcher = None,
            SwitcherKey::Commit => {
                let w = s.windows.get(s.selected).cloned();
                self.win_switcher = None;
                if let Some(w) = w {
                    self.space.raise_element(&w, true);
                    self.focus_window(Some(w));
                }
            }
        }
        self.dirty = true;
    }

    pub fn prepare_win_switcher(&mut self) {
        let sc = self.scale() as f32;
        let (w, _) = self.output_size();
        let tgen = self.theme_gen;
        let Some(s) = &self.win_switcher else { return };
        // O tamanho de cada janela com a barra de título (a proporção da miniatura).
        let sizes: Vec<(f32, f32)> = s.windows.iter().map(|w| {
            let g = w.geometry().size;
            (g.w.max(1) as f32, (g.h + crate::shell::deco_top(w)).max(1) as f32)
        }).collect();
        let title = s.windows.get(s.selected).map(|w| {
            let t = window_info(w).0;
            if t.is_empty() { app_display_name(w) } else { t }
        }).unwrap_or_default();
        let key = (s.windows.len(), s.selected, tgen, format!("{title}{sizes:?}"));
        if s.key.as_ref() == Some(&key) {
            return;
        }
        let max_w = w as f32 - 80.0;
        let slots = chrome::window_switcher_layout(&sizes, max_w).1;
        let tex = chrome::window_switcher(&self.fonts, &sizes, s.selected, &title, max_w, sc).map(to_tex);
        if let Some(s) = &mut self.win_switcher {
            s.slots = slots;
            s.tex = tex;
            s.key = Some(key);
        }
    }

    /// In the middle of the monitor it opened on (global points).
    pub fn win_switcher_pos(&self) -> Option<(i32, i32)> {
        let s = self.win_switcher.as_ref()?;
        let t = s.tex.as_ref()?;
        let a = s.area;
        Some((a.loc.x + (a.size.w - t.size.w) / 2, a.loc.y + (a.size.h - t.size.h) / 2))
    }

    pub fn switcher_pos(&self) -> Option<(i32, i32)> {
        let s = self.switcher.as_ref()?;
        let t = s.tex.as_ref()?;
        let a = s.area;
        Some((a.loc.x + (a.size.w - t.size.w) / 2, a.loc.y + (a.size.h - t.size.h) / 2))
    }
}
