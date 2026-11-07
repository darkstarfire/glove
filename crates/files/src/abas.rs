//! Abas no Arquivos: cada aba guarda a sua navegação (pasta, colunas, escolha,
//! voltar/avançar, modo de ver, busca). A aba da frente vive nos campos do
//! próprio `Files`; trocar de aba guarda uma e põe a outra no lugar.
//!
//! Ctrl+T abre uma aba na mesma pasta; Ctrl+W fecha a aba (a janela, se for a
//! última); Ctrl+Page Up/Down trocam. "Abrir em Nova Aba" no menu de uma pasta.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use glove_ui::{Rect, TabAction, Ui, TAB_STRIP_H};

use crate::{display_name, Files, View};

#[derive(Clone)]
pub struct Aba {
    view: View,
    base: PathBuf,
    columns: Vec<PathBuf>,
    selected: Option<PathBuf>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    search: String,
    scroll: HashMap<PathBuf, f32>,
    expanded: HashSet<PathBuf>,
    extra: Vec<PathBuf>,
    search_scope: PathBuf,
}

impl Files {
    fn take_tab(&mut self) -> Aba {
        Aba {
            view: self.view,
            base: self.base.clone(),
            columns: self.columns.clone(),
            selected: self.selected.clone(),
            back: self.back.clone(),
            forward: self.forward.clone(),
            search: self.search.clone(),
            scroll: self.scroll.clone(),
            expanded: self.expanded.clone(),
            extra: self.extra.clone(),
            search_scope: self.search_scope.clone(),
        }
    }

    fn put_tab(&mut self, a: Aba) {
        self.view = a.view;
        self.base = a.base;
        self.columns = a.columns;
        self.selected = a.selected;
        self.back = a.back;
        self.forward = a.forward;
        self.search = a.search;
        self.scroll = a.scroll;
        self.expanded = a.expanded;
        self.extra = a.extra;
        self.search_scope = a.search_scope;
        self.rename = None;
        self.reveal = true;
    }

    /// Quantas abas (1 = sem faixa).
    pub(crate) fn tab_count(&self) -> usize {
        self.abas.len().max(1)
    }

    pub(crate) fn switch_tab(&mut self, i: usize) {
        if self.abas.len() < 2 || i == self.aba || i >= self.abas.len() {
            return;
        }
        let live = self.take_tab();
        self.abas[self.aba] = live;
        let next = self.abas[i].clone();
        self.put_tab(next);
        self.aba = i;
    }

    /// Aba nova em `dir` (ou na pasta atual), já na frente.
    pub(crate) fn new_tab(&mut self, dir: Option<PathBuf>) {
        let live = self.take_tab();
        if self.abas.is_empty() {
            self.abas.push(live.clone());
            self.aba = 0;
        } else {
            self.abas[self.aba] = live.clone();
        }
        let dir = dir.unwrap_or_else(|| self.current());
        let fresh = Aba { base: dir.clone(), columns: vec![dir.clone()], selected: None, back: Vec::new(), forward: Vec::new(), search: String::new(), scroll: HashMap::new(), expanded: HashSet::new(), extra: Vec::new(), search_scope: dir, ..live };
        self.abas.push(fresh.clone());
        self.aba = self.abas.len() - 1;
        self.put_tab(fresh);
    }

    /// Fecha a aba `i`. Falso se era a última (aí quem chama fecha a janela).
    pub(crate) fn close_tab(&mut self, i: usize) -> bool {
        if self.abas.len() < 2 {
            return false;
        }
        if i == self.aba {
            self.abas.remove(i);
            self.aba = self.aba.min(self.abas.len() - 1);
            let next = self.abas[self.aba].clone();
            self.put_tab(next);
        } else {
            self.abas.remove(i);
            if i < self.aba {
                self.aba -= 1;
            }
        }
        if self.abas.len() == 1 {
            self.abas.clear();
            self.aba = 0;
        }
        true
    }

    pub(crate) fn step_tab(&mut self, d: i32) {
        let n = self.abas.len() as i32;
        if n > 1 {
            let i = (self.aba as i32 + d).rem_euclid(n) as usize;
            self.switch_tab(i);
        }
    }

    /// A faixa de abas embaixo da barra de ferramentas (só com mais de uma).
    pub(crate) fn tab_bar(&mut self, ui: &mut Ui, y: f32) -> f32 {
        if self.abas.len() < 2 {
            return 0.0;
        }
        let titles: Vec<String> = (0..self.abas.len()).map(|i| if i == self.aba { display_name(&self.current()) } else { display_name(self.abas[i].columns.last().unwrap_or(&self.abas[i].base)) }).collect();
        match ui.tab_strip(Rect::new(0.0, y, ui.c.w, TAB_STRIP_H), &titles, self.aba) {
            Some(TabAction::Select(i)) => self.switch_tab(i),
            Some(TabAction::Close(i)) => {
                self.close_tab(i);
            }
            Some(TabAction::New) => self.new_tab(None),
            None => {}
        }
        TAB_STRIP_H
    }
}
