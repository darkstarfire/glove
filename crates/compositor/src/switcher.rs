//! Troca de apps no estilo do 10.4: Super+Tab mostra os ícones grandes dos apps
//! abertos (do usado mais recentemente para o mais antigo) numa faixa no meio da
//! tela. Com o Super preso: Tab/→ avançam, Shift+Tab/← voltam, Q encerra, H oculta,
//! Esc cancela. Soltar o Super troca para o app escolhido.

use glove_paint::{chrome, icons::Icon};
use smithay::desktop::Window;

use crate::{
    shell::{app_display_name, to_tex, window_info, Tex},
    state::Glove,
};

pub struct Switcher {
    /// (pid, ícone, nome), na ordem da faixa.
    pub apps: Vec<(u32, Icon, String)>,
    pub selected: usize,
    key: Option<(Vec<u32>, usize, u64)>,
    pub tex: Option<Tex>,
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
        let windows: Vec<Window> = self.space.elements().rev().cloned().chain(self.minimized.iter().chain(&self.hidden).map(|(w, _)| w.clone())).collect();
        let mut apps: Vec<(u32, Icon, String)> = Vec::new();
        for w in windows {
            let Some(pid) = self.window_pid(&w) else { continue };
            if apps.iter().any(|a| a.0 == pid) {
                continue;
            }
            let app_id = window_info(&w).1;
            let icon = glove_config::known_app(&self.config.dock_apps, &app_id).map(|a| Icon::from_name(&a.icon)).unwrap_or(Icon::Window);
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
        self.switcher = Some(Switcher { apps, selected, key: None, tex: None });
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

    pub fn switcher_pos(&self) -> Option<(i32, i32)> {
        let t = self.switcher.as_ref()?.tex.as_ref()?;
        let (w, h) = self.output_size();
        Some(((w - t.size.w) / 2, (h - t.size.h) / 2))
    }
}
