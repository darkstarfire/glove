//! Tabelas: inserir, ir de célula em célula, linhas, colunas, juntar e separar.
//!
//! Uma tabela é uma sequência de parágrafos marcados com a célula (linha,
//! coluna). As operações leem a tabela numa grade, mudam a grade e devolvem tudo
//! de uma vez (um passo só do Desfazer, nunca uma estrutura pela metade).

use std::rc::Rc;

use crate::rich::{Cell, CharStyle, Doc, Para, ParaStyle, Pos, Role, TableProps};

/// O bloco da tabela do parágrafo `p`: (número da tabela, primeiro, fim exclusivo).
pub fn block(doc: &Doc, p: usize) -> Option<(u32, usize, usize)> {
    let id = doc.paras.get(p)?.style.cell?.table;
    let same = |i: usize| doc.paras[i].style.cell.is_some_and(|c| c.table == id);
    let mut a = p;
    while a > 0 && same(a - 1) {
        a -= 1;
    }
    let mut b = p + 1;
    while b < doc.paras.len() && same(b) {
        b += 1;
    }
    Some((id, a, b))
}

/// Uma célula da grade: quantas colunas ocupa e os parágrafos dela.
#[derive(Clone)]
struct GridCell {
    span: u16,
    paras: Vec<Para>,
}

/// A tabela em linhas de células (na ordem do documento).
fn grid(doc: &Doc, a: usize, b: usize) -> Vec<Vec<GridCell>> {
    let mut rows: Vec<Vec<GridCell>> = Vec::new();
    let mut last: Option<(u16, u16)> = None;
    for p in &doc.paras[a..b] {
        let c = p.style.cell.expect("parágrafo de tabela");
        if last.map(|l| l.0) != Some(c.row) {
            rows.push(Vec::new());
        }
        let row = rows.last_mut().unwrap();
        if last != Some((c.row, c.col)) {
            row.push(GridCell { span: c.span.max(1), paras: Vec::new() });
        }
        row.last_mut().unwrap().paras.push(p.clone());
        last = Some((c.row, c.col));
    }
    rows
}

/// Parágrafo vazio de célula, no estilo de `like`.
fn empty_like(like: &Para) -> Para {
    let st = like.runs.last().map(|r| r.style.clone()).unwrap_or_default();
    Para::new("", Rc::new(CharStyle { note: None, image: None, ..(*st).clone() }), ParaStyle { list: None, page_break_before: false, ..like.style.clone() })
}

/// Grava a grade de volta no lugar do bloco [a, b) (um passo só do Desfazer).
fn put(doc: &mut Doc, id: u32, a: usize, b: usize, rows: Vec<Vec<GridCell>>, cursor: (usize, usize)) {
    let mut out: Vec<Para> = Vec::new();
    let mut target = a;
    for (r, row) in rows.iter().enumerate() {
        let mut col = 0u16;
        for (ci, cell) in row.iter().enumerate() {
            if (r, ci) == cursor {
                target = a + out.len();
            }
            for p in &cell.paras {
                let mut p = p.clone();
                p.style.cell = Some(Cell { table: id, row: r as u16, col, span: cell.span });
                out.push(p);
            }
            col += cell.span;
        }
    }
    if out.is_empty() {
        // Tabela apagada inteira: fica um parágrafo comum no lugar.
        let body = doc.body.clone();
        out.push(Para::new("", body.0, body.1));
    }
    let styles: Vec<ParaStyle> = out.iter().map(|p| p.style.clone()).collect();
    let n = out.len();
    doc.group(|d| {
        d.anchor = Pos::new(a, 0);
        d.cursor = Pos::new(b - 1, d.paras[b - 1].text.len());
        d.insert_frag(out);
        d.set_para_styles(a, a + n - 1, styles);
        d.set_cursor(Pos::new(target.min(a + n - 1), 0), false);
    });
}

/// Linha e célula (índice na linha) do parágrafo `p`.
fn locate(doc: &Doc, a: usize, p: usize) -> (usize, usize) {
    let mut row = 0usize;
    let mut ci = 0usize;
    let mut last: Option<(u16, u16)> = None;
    for q in &doc.paras[a..=p] {
        let c = q.style.cell.unwrap();
        match last {
            Some((r, _)) if r != c.row => {
                row += 1;
                ci = 0;
            }
            Some((_, col)) if col != c.col => ci += 1,
            _ => {}
        }
        last = Some((c.row, c.col));
    }
    (row, ci)
}

/// Número novo de tabela.
fn new_id(doc: &Doc) -> u32 {
    doc.paras.iter().filter_map(|p| p.style.cell.map(|c| c.table)).max().unwrap_or(0).max(doc.tables.keys().copied().max().unwrap_or(0)) + 1
}

/// Insere uma tabela de `rows` × `cols` no cursor (numa linha própria).
pub fn insert(doc: &mut Doc, rows: usize, cols: usize) {
    let (rows, cols) = (rows.clamp(1, 200), cols.clamp(1, 30));
    let id = new_id(doc);
    doc.tables.insert(id, TableProps::default());
    let cs = Rc::new(CharStyle { note: None, image: None, ..(*doc.body.0).clone() });
    let cell_ps = ParaStyle { role: Role::Body, first_indent: 0.0, line_spacing: 1.0, align: crate::rich::Align::Left, ..ParaStyle::default() };
    let mut frag: Vec<Para> = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let ps = ParaStyle { cell: Some(Cell { table: id, row: r as u16, col: c as u16, span: 1 }), ..cell_ps.clone() };
            frag.push(Para::new("", cs.clone(), ps));
        }
    }
    let styles: Vec<ParaStyle> = frag.iter().map(|p| p.style.clone()).collect();
    let n = frag.len();
    let body = doc.body.clone();
    doc.group(|d| {
        d.set_cursor(d.selection().1, false);
        let p = d.cursor.p;
        // A tabela fica entre parágrafos comuns (dá para escrever antes e depois).
        if !d.paras[p].text.is_empty() || d.paras[p].style.cell.is_some() {
            d.set_cursor(Pos::new(p, d.paras[p].text.len()), false);
            d.insert_text("\n");
        }
        let p0 = d.cursor.p;
        let mut f = frag;
        f.push(Para::new("", body.0.clone(), body.1.clone()));
        d.insert_frag(f);
        d.set_para_styles(p0, p0 + n - 1, styles);
        let after = p0 + n;
        if let Some(ps) = d.paras.get(after).map(|p| ParaStyle { cell: None, ..p.style.clone() }) {
            d.set_para_styles(after, after, vec![ps]);
        }
        d.set_cursor(Pos::new(p0, 0), false);
    });
}

/// A primeira posição da célula seguinte (ou anterior). `None` = já é a última (ou primeira).
pub fn next_cell(doc: &Doc, p: usize, forward: bool) -> Option<Pos> {
    let (_, a, b) = block(doc, p)?;
    let here = doc.paras[p].style.cell?;
    let key = |i: usize| doc.paras[i].style.cell.map(|c| (c.row, c.col));
    if forward {
        let k = (p + 1..b).find(|&i| key(i) != Some((here.row, here.col)))?;
        Some(Pos::new(k, 0))
    } else {
        let k = (a..p).rev().find(|&i| key(i) != Some((here.row, here.col)))?;
        // O começo daquela célula.
        let c = key(k);
        let mut s = k;
        while s > a && key(s - 1) == c {
            s -= 1;
        }
        Some(Pos::new(s, 0))
    }
}

/// O cursor está numa tabela? Devolve o bloco e onde (linha, célula).
fn here(doc: &Doc) -> Option<(u32, usize, usize, usize, usize)> {
    let p = doc.cursor.p;
    let (id, a, b) = block(doc, p)?;
    let (r, c) = locate(doc, a, p);
    Some((id, a, b, r, c))
}

pub fn in_table(doc: &Doc) -> bool {
    doc.paras[doc.cursor.p].style.cell.is_some()
}

/// Linha nova acima ou abaixo da do cursor (mesmas colunas).
pub fn add_row(doc: &mut Doc, below: bool) {
    let Some((id, a, b, r, c)) = here(doc) else { return };
    let mut rows = grid(doc, a, b);
    let new: Vec<GridCell> = rows[r].iter().map(|cell| GridCell { span: cell.span, paras: vec![empty_like(&cell.paras[0])] }).collect();
    let at = if below { r + 1 } else { r };
    rows.insert(at, new);
    put(doc, id, a, b, rows, (at, c));
}

/// Coluna nova à esquerda ou à direita da do cursor.
pub fn add_col(doc: &mut Doc, right: bool) {
    let Some((id, a, b, r, c)) = here(doc) else { return };
    let mut rows = grid(doc, a, b);
    // A coluna (na grade de colunas) onde entra a nova.
    let gcol: u16 = rows[r][..c].iter().map(|x| x.span).sum::<u16>() + if right { rows[r][c].span } else { 0 };
    for row in rows.iter_mut() {
        let mut acc = 0u16;
        let mut idx = row.len();
        for (i, cell) in row.iter().enumerate() {
            if acc >= gcol {
                idx = i;
                break;
            }
            acc += cell.span;
        }
        let like = row.get(idx.min(row.len() - 1)).map(|x| x.paras[0].clone()).unwrap();
        row.insert(idx, GridCell { span: 1, paras: vec![empty_like(&like)] });
    }
    let props = doc.tables.entry(id).or_default();
    let cols = rows.iter().map(|row| row.iter().map(|x| x.span as usize).sum::<usize>()).max().unwrap_or(1);
    let mut w = props.fractions(cols - 1);
    w.insert(gcol as usize, 1.0 / (cols - 1).max(1) as f32);
    props.widths = w;
    let cc = if right { c + 1 } else { c };
    put(doc, id, a, b, rows, (r, cc));
}

/// Apaga a linha do cursor (a última apaga a tabela).
pub fn del_row(doc: &mut Doc) {
    let Some((id, a, b, r, _)) = here(doc) else { return };
    let mut rows = grid(doc, a, b);
    rows.remove(r);
    let nr = r.min(rows.len().saturating_sub(1));
    put(doc, id, a, b, rows, (nr, 0));
}

/// Apaga a coluna do cursor.
pub fn del_col(doc: &mut Doc) {
    let Some((id, a, b, r, c)) = here(doc) else { return };
    let mut rows = grid(doc, a, b);
    let gcol: u16 = rows[r][..c].iter().map(|x| x.span).sum();
    for row in rows.iter_mut() {
        let mut acc = 0u16;
        let mut hit = None;
        for (i, cell) in row.iter().enumerate() {
            if gcol >= acc && gcol < acc + cell.span {
                hit = Some(i);
                break;
            }
            acc += cell.span;
        }
        if let Some(i) = hit {
            if row[i].span > 1 {
                row[i].span -= 1;
            } else {
                row.remove(i);
            }
        }
    }
    rows.retain(|row| !row.is_empty());
    if let Some(p) = doc.tables.get_mut(&id) {
        if (gcol as usize) < p.widths.len() {
            p.widths.remove(gcol as usize);
        }
    }
    put(doc, id, a, b, rows, (r, c.saturating_sub(1)));
}

/// Junta as células selecionadas da mesma linha numa só (o texto vai junto).
pub fn merge(doc: &mut Doc) {
    let (sa, sb) = doc.selection();
    let Some((id, a, b)) = block(doc, sa.p) else { return };
    if sb.p >= b {
        return;
    }
    let (r0, c0) = locate(doc, a, sa.p);
    let (r1, c1) = locate(doc, a, sb.p);
    if r0 != r1 || c0 == c1 {
        return;
    }
    let mut rows = grid(doc, a, b);
    let row = &mut rows[r0];
    let taken: Vec<GridCell> = row.drain(c0 + 1..=c1).collect();
    for t in taken {
        row[c0].span += t.span;
        // Só os parágrafos com texto entram (célula vazia não deixa linha em branco).
        row[c0].paras.extend(t.paras.into_iter().filter(|p| !p.text.is_empty()));
    }
    put(doc, id, a, b, rows, (r0, c0));
}

/// Separa a célula juntada do cursor (as partes novas ficam vazias).
pub fn split(doc: &mut Doc) {
    let Some((id, a, b, r, c)) = here(doc) else { return };
    let mut rows = grid(doc, a, b);
    let span = rows[r][c].span;
    if span < 2 {
        return;
    }
    rows[r][c].span = 1;
    let like = rows[r][c].paras[0].clone();
    for k in 0..span - 1 {
        rows[r].insert(c + 1 + k as usize, GridCell { span: 1, paras: vec![empty_like(&like)] });
    }
    put(doc, id, a, b, rows, (r, c));
}

/// Liga ou desliga as bordas da tabela do cursor.
pub fn toggle_borders(doc: &mut Doc) {
    let Some((id, ..)) = here(doc) else { return };
    let p = doc.tables.entry(id).or_default();
    p.borders = !p.borders;
    doc.modified = true;
    doc.version = doc.version.wrapping_add(1);
}

/// Colunas todas da mesma largura.
pub fn equal_widths(doc: &mut Doc) {
    let Some((id, ..)) = here(doc) else { return };
    doc.tables.entry(id).or_default().widths.clear();
    doc.modified = true;
    doc.version = doc.version.wrapping_add(1);
}

/// A tabela do cursor tem bordas?
pub fn borders(doc: &Doc) -> bool {
    here(doc).map(|(id, ..)| doc.tables.get(&id).is_none_or(|p| p.borders)).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(d: &Doc) -> Vec<(u16, u16, u16, String)> {
        d.paras.iter().filter_map(|p| p.style.cell.map(|c| (c.row, c.col, c.span, p.text.clone()))).collect()
    }

    #[test]
    fn inserir_e_navegar() {
        let mut d = Doc::from_plain("antes", Rc::new(CharStyle::default()), ParaStyle::default());
        d.set_cursor(Pos::new(0, 5), false);
        insert(&mut d, 2, 3);
        assert_eq!(d.paras.len(), 1 + 6 + 1);
        assert_eq!(d.cursor, Pos::new(1, 0));
        assert_eq!(cells(&d)[5], (1, 2, 1, String::new()));
        assert!(d.paras[7].style.cell.is_none());
        assert_eq!(next_cell(&d, 1, true), Some(Pos::new(2, 0)));
        assert_eq!(next_cell(&d, 6, true), None);
        assert_eq!(next_cell(&d, 4, false), Some(Pos::new(3, 0)));
        d.undo();
        assert_eq!(d.plain_text(), "antes");
    }

    #[test]
    fn linhas_colunas_juntar_separar() {
        let mut d = Doc::new();
        insert(&mut d, 2, 2);
        // Escreve em cada célula: a, b / c, d.
        for (i, t) in ["a", "b", "c", "d"].iter().enumerate() {
            d.set_cursor(Pos::new(i, 0), false);
            d.insert_text(t);
        }
        d.set_cursor(Pos::new(0, 0), false);
        add_row(&mut d, true);
        assert_eq!(cells(&d).iter().map(|c| c.3.as_str()).collect::<Vec<_>>(), ["a", "b", "", "", "c", "d"]);
        assert_eq!(cells(&d)[4].0, 2);
        d.set_cursor(Pos::new(0, 0), false);
        add_col(&mut d, true);
        assert_eq!(cells(&d).len(), 9);
        assert_eq!(cells(&d)[2], (0, 2, 1, "b".into()));
        // Junta a, (nova) e b na primeira linha.
        d.anchor = Pos::new(0, 0);
        d.cursor = Pos::new(2, 1);
        merge(&mut d);
        assert_eq!(cells(&d)[0].2, 3);
        assert_eq!(cells(&d).iter().filter(|c| c.0 == 0).map(|c| c.3.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        d.set_cursor(Pos::new(0, 0), false);
        split(&mut d);
        assert_eq!(cells(&d).iter().filter(|c| c.0 == 0).count(), 4);
        d.set_cursor(Pos::new(0, 0), false);
        del_row(&mut d);
        assert_eq!(cells(&d).iter().map(|c| c.0).max(), Some(1));
        d.set_cursor(Pos::new(0, 0), false);
        del_col(&mut d);
        assert!(cells(&d).iter().all(|c| c.1 < 2));
    }
}
