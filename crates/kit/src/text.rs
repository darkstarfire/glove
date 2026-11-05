//! Buffer de texto do editor: cursor, seleção e desfazer/refazer.
//!
//! O texto fica numa única `String`; posições são índices de byte (sempre em
//! fronteira de caractere). O histórico guarda só as edições (o trecho
//! removido e o inserido), nunca cópias do texto inteiro.

#[derive(Clone, Debug)]
struct Edit {
    at: usize,
    removed: String,
    inserted: String,
    cursor_before: usize,
    anchor_before: usize,
}

#[derive(Debug, Default)]
pub struct TextBuffer {
    text: String,
    /// Onde o cursor está.
    pub cursor: usize,
    /// Outra ponta da seleção (igual ao cursor = nada selecionado).
    pub anchor: usize,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Pode juntar a próxima letra digitada à última edição (desfazer por palavra).
    coalesce: bool,
    /// Alterado desde a última vez que foi salvo.
    pub modified: bool,
    /// Posição horizontal desejada ao subir/descer linhas (em pixels lógicos).
    pub want_x: Option<f32>,
    /// Rolagem (pixels lógicos).
    pub scroll_x: f32,
    pub scroll_y: f32,
    /// Pede para rolar até o cursor no próximo desenho.
    pub reveal: bool,
    /// Tamanho da fonte do documento (0 = padrão).
    pub font_px: f32,
    /// Cor do papel (None = branco).
    pub paper: Option<(u8, u8, u8)>,
    /// Alinhamento das linhas (só quando a linha cabe na largura).
    pub align: Align,
}

/// Alinhamento das linhas no `text_area`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

const UNDO_MAX: usize = 1000;

impl TextBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        let mut text: String = text.into();
        if text.contains('\r') {
            text = text.replace("\r\n", "\n").replace('\r', "\n");
        }
        Self { text, ..Default::default() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    // -----------------------------------------------------------------------
    // Linhas
    // -----------------------------------------------------------------------

    pub fn line_count(&self) -> usize {
        self.text.bytes().filter(|b| *b == b'\n').count() + 1
    }

    /// Início (byte) de cada linha. Calculado sob demanda; barato para textos comuns.
    pub fn line_starts(&self) -> Vec<usize> {
        let mut v = Vec::with_capacity(self.text.len() / 30 + 1);
        v.push(0);
        for (i, b) in self.text.bytes().enumerate() {
            if b == b'\n' {
                v.push(i + 1);
            }
        }
        v
    }

    /// Linha que contém `pos`, e o início dela.
    pub fn line_of(&self, pos: usize) -> (usize, usize) {
        let before = &self.text[..pos.min(self.text.len())];
        let line = before.bytes().filter(|b| *b == b'\n').count();
        let start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        (line, start)
    }

    /// Fim da linha que começa em `start` (sem o '\n').
    pub fn line_end(&self, start: usize) -> usize {
        self.text[start..].find('\n').map(|i| start + i).unwrap_or(self.text.len())
    }

    /// (linha, coluna) do cursor, contando a partir de 1 (para a barra de status).
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let (line, start) = self.line_of(self.cursor);
        (line + 1, self.text[start..self.cursor].chars().count() + 1)
    }

    // -----------------------------------------------------------------------
    // Seleção
    // -----------------------------------------------------------------------

    pub fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    pub fn selected_text(&self) -> &str {
        let (a, b) = self.selection();
        &self.text[a..b]
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    /// Move o cursor; com `extend`, aumenta a seleção.
    pub fn set_cursor(&mut self, pos: usize, extend: bool) {
        self.cursor = pos.min(self.text.len());
        if !extend {
            self.anchor = self.cursor;
        }
        self.coalesce = false;
        self.reveal = true;
    }

    fn prev_char(&self, pos: usize) -> usize {
        self.text[..pos].char_indices().next_back().map(|(i, _)| i).unwrap_or(0)
    }

    fn next_char(&self, pos: usize) -> usize {
        self.text[pos..].chars().next().map(|c| pos + c.len_utf8()).unwrap_or(pos)
    }

    pub fn left(&mut self, extend: bool, word: bool) {
        let to = if !extend && self.has_selection() {
            self.selection().0
        } else if word {
            self.word_left(self.cursor)
        } else {
            self.prev_char(self.cursor)
        };
        self.want_x = None;
        self.set_cursor(to, extend);
    }

    pub fn right(&mut self, extend: bool, word: bool) {
        let to = if !extend && self.has_selection() {
            self.selection().1
        } else if word {
            self.word_right(self.cursor)
        } else {
            self.next_char(self.cursor)
        };
        self.want_x = None;
        self.set_cursor(to, extend);
    }

    fn word_left(&self, mut p: usize) -> usize {
        while p > 0 && !self.text[..p].chars().next_back().unwrap().is_alphanumeric() {
            p = self.prev_char(p);
        }
        while p > 0 && self.text[..p].chars().next_back().unwrap().is_alphanumeric() {
            p = self.prev_char(p);
        }
        p
    }

    fn word_right(&self, mut p: usize) -> usize {
        while p < self.text.len() && !self.text[p..].chars().next().unwrap().is_alphanumeric() {
            p = self.next_char(p);
        }
        while p < self.text.len() && self.text[p..].chars().next().unwrap().is_alphanumeric() {
            p = self.next_char(p);
        }
        p
    }

    /// Seleciona a palavra sob `pos` (clique duplo).
    pub fn select_word(&mut self, pos: usize) {
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut a = pos;
        while a > 0 && self.text[..a].chars().next_back().is_some_and(is_word) {
            a = self.prev_char(a);
        }
        let mut b = pos;
        while b < self.text.len() && self.text[b..].chars().next().is_some_and(is_word) {
            b = self.next_char(b);
        }
        self.anchor = a;
        self.cursor = b;
        self.coalesce = false;
    }

    pub fn home(&mut self, extend: bool, doc: bool) {
        let to = if doc { 0 } else { self.line_of(self.cursor).1 };
        self.want_x = None;
        self.set_cursor(to, extend);
    }

    pub fn end(&mut self, extend: bool, doc: bool) {
        let to = if doc { self.text.len() } else { self.line_end(self.line_of(self.cursor).1) };
        self.want_x = None;
        self.set_cursor(to, extend);
    }

    // -----------------------------------------------------------------------
    // Edição
    // -----------------------------------------------------------------------

    /// Troca a seleção (ou nada, se vazia) por `s`.
    pub fn insert(&mut self, s: &str) {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let (a, b) = self.selection();
        let typing = s.chars().count() == 1 && !s.contains('\n') && a == b;
        self.replace(a, b, &s, typing);
    }

    pub fn backspace(&mut self) {
        let (a, b) = self.selection();
        if a != b {
            self.replace(a, b, "", false);
        } else if a > 0 {
            let p = self.prev_char(a);
            self.replace(p, a, "", false);
        }
    }

    pub fn delete(&mut self) {
        let (a, b) = self.selection();
        if a != b {
            self.replace(a, b, "", false);
        } else if a < self.text.len() {
            let n = self.next_char(a);
            self.replace(a, n, "", false);
        }
    }

    fn replace(&mut self, a: usize, b: usize, s: &str, typing: bool) {
        let removed = self.text[a..b].to_string();
        // Junta letras digitadas em sequência numa única entrada do histórico.
        let joined = typing
            && self.coalesce
            && self.undo.last().is_some_and(|e| e.removed.is_empty() && e.at + e.inserted.len() == a && !s.starts_with(' '));
        if joined {
            self.undo.last_mut().unwrap().inserted.push_str(s);
        } else {
            self.undo.push(Edit { at: a, removed, inserted: s.to_string(), cursor_before: self.cursor, anchor_before: self.anchor });
            if self.undo.len() > UNDO_MAX {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.text.replace_range(a..b, s);
        self.cursor = a + s.len();
        self.anchor = self.cursor;
        self.coalesce = typing;
        self.modified = true;
        self.want_x = None;
        self.reveal = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        let Some(e) = self.undo.pop() else { return };
        self.text.replace_range(e.at..e.at + e.inserted.len(), &e.removed);
        self.cursor = e.cursor_before.min(self.text.len());
        self.anchor = e.anchor_before.min(self.text.len());
        self.redo.push(e);
        self.after_history();
    }

    pub fn redo(&mut self) {
        let Some(e) = self.redo.pop() else { return };
        self.text.replace_range(e.at..e.at + e.removed.len(), &e.inserted);
        self.cursor = e.at + e.inserted.len();
        self.anchor = self.cursor;
        self.undo.push(e);
        self.after_history();
    }

    fn after_history(&mut self) {
        self.coalesce = false;
        self.modified = true;
        self.want_x = None;
        self.reveal = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digitar_apagar_desfazer() {
        let mut b = TextBuffer::new("");
        for c in "olá mundo".chars() {
            b.insert(&c.to_string());
        }
        assert_eq!(b.text(), "olá mundo");
        b.backspace();
        assert_eq!(b.text(), "olá mund");
        b.undo();
        assert_eq!(b.text(), "olá mundo");
        b.undo(); // "mundo" digitado de uma vez
        assert_eq!(b.text(), "olá");
        b.redo();
        assert_eq!(b.text(), "olá mundo");
    }

    #[test]
    fn selecao_e_linhas() {
        let mut b = TextBuffer::new("um\ndois\ntrês");
        assert_eq!(b.line_count(), 3);
        b.set_cursor(4, false);
        assert_eq!(b.cursor_line_col(), (2, 2));
        b.end(true, false);
        assert_eq!(b.selected_text(), "ois");
        b.insert("X");
        assert_eq!(b.text(), "um\ndX\ntrês");
        b.select_all();
        b.delete();
        assert!(b.is_empty());
        b.undo();
        assert_eq!(b.text(), "um\ndX\ntrês");
    }

    #[test]
    fn acentos_e_palavras() {
        let mut b = TextBuffer::new("ação rápida");
        b.end(false, true);
        b.left(false, true);
        assert_eq!(&b.text()[b.cursor..], "rápida");
        b.left(false, false);
        b.left(false, false);
        assert_eq!(&b.text()[b.cursor..], "o rápida");
        b.select_word(b.cursor);
        assert_eq!(b.selected_text(), "ação");
        let mut c = TextBuffer::new("a\r\nb");
        assert_eq!(c.text(), "a\nb");
        c.end(false, true);
        c.insert("\r\nc");
        assert_eq!(c.text(), "a\nb\nc");
    }
}
