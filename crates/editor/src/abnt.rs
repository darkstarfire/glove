//! Modelo ABNT (trabalhos acadêmicos) e o sumário gerado dos títulos.

use std::rc::Rc;

use crate::para::CM;
use crate::rich::{Align, CharStyle, Doc, HeaderFooter, PageSetup, Para, ParaStyle, Pos, Role, NOTE_CHAR};
use crate::view::DocView;

/// Família do modelo ABNT (Times New Roman, que o sistema troca pela Liberation Serif).
pub const FAMILY: &str = "Times New Roman";

/// A4 com margens de 3 cm em cima e à esquerda e 2 cm embaixo e à direita.
fn page() -> PageSetup {
    PageSetup { width: 595.28, height: 841.89, margin_top: 3.0 * CM, margin_left: 3.0 * CM, margin_bottom: 2.0 * CM, margin_right: 2.0 * CM }
}

/// Parágrafo no padrão: corpo justificado, 1,5 entre linhas, recuo de 1,25 cm.
fn para(ps: &ParaStyle) -> ParaStyle {
    let mut ps = ps.clone();
    match ps.role {
        Role::Body if ps.list.is_none() && ps.cell.is_none() => {
            ps.align = Align::Justify;
            ps.line_spacing = 1.5;
            ps.first_indent = 1.25 * CM;
            ps.space_before = 0.0;
            ps.space_after = 0.0;
        }
        Role::Quote => {
            ps.left_indent = 4.0 * CM;
            ps.first_indent = 0.0;
            ps.line_spacing = 1.0;
            ps.align = Align::Justify;
            ps.space_before = 12.0;
            ps.space_after = 12.0;
        }
        Role::Chapter | Role::Section | Role::Subsection => {
            ps.line_spacing = 1.5;
            ps.first_indent = 0.0;
            ps.align = Align::Left;
        }
        _ => {}
    }
    ps
}

/// Documento novo já no modelo.
pub fn new_doc() -> Doc {
    let cs = Rc::new(CharStyle { family: FAMILY.into(), size: 12.0, ..CharStyle::default() });
    let mut d = Doc::with_style(cs, para(&ParaStyle::default()));
    d.page = page();
    d.header = HeaderFooter { right: "{página}".into(), ..HeaderFooter::default() };
    d.title_page = true;
    d
}

/// Aplica o modelo a um documento que já existe (um passo só do Desfazer para o texto).
pub fn apply(doc: &mut Doc) {
    doc.page = page();
    if doc.header.is_empty() {
        doc.header.right = "{página}".into();
    }
    doc.title_page = true;
    doc.body = (Rc::new(CharStyle { family: FAMILY.into(), size: 12.0, ..CharStyle::default() }), para(&ParaStyle::default()));
    let (cur, anc) = (doc.cursor, doc.anchor);
    doc.group(|d| {
        let n = d.paras.len();
        let styles: Vec<ParaStyle> = d.paras.iter().map(|p| para(&p.style)).collect();
        d.set_para_styles(0, n - 1, styles);
        d.select_all();
        if d.has_selection() {
            d.set_char_style(&|cs| cs.family = FAMILY.into());
        }
        // Corpo em 12 pontos; citação longa em 10.
        for i in 0..n {
            let (role, len) = (d.paras[i].style.role, d.paras[i].text.len());
            let size = match role {
                Role::Body => 12.0,
                Role::Quote => 10.0,
                _ => continue,
            };
            if len > 0 {
                d.anchor = Pos::new(i, 0);
                d.cursor = Pos::new(i, len);
                d.set_char_style(&move |cs| cs.size = size);
            }
        }
    });
    doc.cursor = doc.clamp(cur);
    doc.anchor = doc.clamp(anc);
    doc.modified = true;
}

/// Insere o sumário no cursor (ou refaz o que existe) com as páginas da paginação atual.
pub fn toc(doc: &mut Doc, view: &DocView, update: bool) {
    let family = doc.paras[0].runs[0].style.family.clone();
    let heads: Vec<(usize, String, usize)> = doc
        .paras
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.style.role.outline().map(|l| (l, p.text.replace(NOTE_CHAR, ""), view.page_of(i))))
        .collect();
    let title_cs = Rc::new(CharStyle { family: family.clone(), size: 14.0, bold: true, ..CharStyle::default() });
    let mut frag = vec![Para::new(glove_config::tr!("Sumário"), title_cs, ParaStyle { role: Role::Toc, align: Align::Center, space_after: 12.0, ..ParaStyle::default() })];
    for (level, text, page) in heads {
        let cs = Rc::new(CharStyle { family: family.clone(), size: 12.0, bold: level == 0, ..CharStyle::default() });
        let ps = ParaStyle { role: Role::Toc, left_indent: level as f32 * 18.0, space_after: 3.0, ..ParaStyle::default() };
        frag.push(Para::new(&format!("{}\t{}", text.trim(), page), cs, ps));
    }
    let styles: Vec<ParaStyle> = frag.iter().map(|p| p.style.clone()).collect();
    let n = frag.len();
    if update {
        // Troca o bloco de linhas do sumário que já existe.
        let Some(first) = doc.paras.iter().position(|p| p.style.role == Role::Toc) else { return };
        let last = (first..doc.paras.len()).take_while(|&i| doc.paras[i].style.role == Role::Toc).last().unwrap_or(first);
        doc.group(|d| {
            d.anchor = Pos::new(first, 0);
            d.cursor = Pos::new(last, d.paras[last].text.len());
            d.insert_frag(frag);
            d.set_para_styles(first, first + n - 1, styles);
        });
        return;
    }
    doc.group(|d| {
        d.set_cursor(d.selection().1, false);
        let p = d.cursor.p;
        if !d.paras[p].text.is_empty() {
            if d.cursor.o == 0 {
                // No começo de um parágrafo: o sumário entra antes dele.
                d.insert_text("\n");
                d.set_cursor(Pos::new(p, 0), false);
            } else {
                d.set_cursor(Pos::new(p, d.paras[p].text.len()), false);
                d.insert_text("\n");
            }
        }
        let p0 = d.cursor.p;
        let body = d.paras[p0].style.clone();
        let mut f = frag;
        f.push(Para::new("", Rc::new(CharStyle { family: family.clone(), ..CharStyle::default() }), ParaStyle { role: Role::Body, ..body }));
        d.insert_frag(f);
        d.set_para_styles(p0, p0 + n - 1, styles);
    });
}
