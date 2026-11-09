//! As fontes do documento: cada família em normal, negrito, itálico e
//! negrito-itálico, achadas pelo fontconfig ("Helvetica" vira a Nimbus Sans,
//! "Times New Roman" a Liberation Serif…) e lidas uma vez só.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use glove_ui::paint::Fonts;

use crate::rich::CharStyle;

type Face = (String, bool, bool);

#[derive(Default)]
pub struct FontBook {
    faces: RefCell<HashMap<Face, Rc<Fonts>>>,
    /// Arquivo e índice de cada face (o PDF embute a fonte).
    files: RefCell<HashMap<Face, (std::path::PathBuf, u32)>>,
    /// (face, tamanho) → (subida, descida).
    metrics: RefCell<HashMap<(Face, u32), (f32, f32)>>,
}

/// Arquivo e índice da face (fc-match), se houver.
fn fc_match(family: &str, bold: bool, italic: bool) -> Option<(std::path::PathBuf, u32)> {
    let mut pat = family.replace([':', ','], " ");
    if bold {
        pat.push_str(":weight=bold");
    }
    if italic {
        pat.push_str(":slant=italic");
    }
    let out = std::process::Command::new("fc-match").args(["-f", "%{file}\n%{index}", &pat]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut it = s.lines();
    let file = std::path::PathBuf::from(it.next()?.trim());
    let index = it.next().and_then(|i| i.trim().parse().ok()).unwrap_or(0);
    file.is_file().then_some((file, index))
}

impl FontBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tamanho em que o trecho é desenhado (sobrescrito e subscrito ficam menores).
    pub fn eff_size(s: &CharStyle) -> f32 {
        if s.script != 0 { s.size * 0.66 } else { s.size }
    }

    /// A face do estilo (família, negrito, itálico).
    pub fn face(&self, s: &CharStyle) -> Rc<Fonts> {
        let key = (s.family.clone(), s.bold, s.italic);
        if let Some(f) = self.faces.borrow().get(&key) {
            return f.clone();
        }
        let file = fc_match(&s.family, s.bold, s.italic);
        if let Some(f) = &file {
            self.files.borrow_mut().insert(key.clone(), f.clone());
        }
        let f = file
            .and_then(|(p, i)| Fonts::load_file(&p, i))
            .or_else(Fonts::load_document)
            .map(Rc::new)
            .unwrap_or_else(|| Rc::new(Fonts::load_system().expect("nenhuma fonte")));
        self.faces.borrow_mut().insert(key, f.clone());
        f
    }

    /// Subida e descida (acima e abaixo da linha de base) no tamanho do trecho.
    pub fn metrics(&self, s: &CharStyle) -> (f32, f32) {
        let size = Self::eff_size(s);
        let key = ((s.family.clone(), s.bold, s.italic), size.to_bits());
        if let Some(m) = self.metrics.borrow().get(&key) {
            return *m;
        }
        let (a, d, _, _) = self.face(s).v_metrics(size);
        let m = (a, d);
        self.metrics.borrow_mut().insert(key, m);
        m
    }

    /// Arquivo e índice da face do estilo (para embutir no PDF).
    pub fn file(&self, s: &CharStyle) -> Option<(std::path::PathBuf, u32)> {
        self.face(s);
        self.files.borrow().get(&(s.family.clone(), s.bold, s.italic)).cloned()
    }

    pub fn measure(&self, s: &CharStyle, text: &str) -> f32 {
        self.face(s).measure(text, Self::eff_size(s), false)
    }
}
