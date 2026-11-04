/// The terminal owns the editable draft, independently of SKK composition.
/// Cursor positions stay on extended grapheme boundaries.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
#[derive(Default, Debug)]
pub struct Draft {
    text: String,
    cursor: usize,
}
impl Draft {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn into_text(self) -> String {
        self.text
    }
    pub fn insert(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
        self.normalize_cursor();
    }
    fn normalize_cursor(&mut self) {
        self.cursor = self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([self.text.len()])
            .find(|i| *i >= self.cursor)
            .unwrap();
    }
    pub fn left(&mut self) {
        self.cursor = self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
    }
    pub fn right(&mut self) {
        if let Some(c) = self.text[self.cursor..].graphemes(true).next() {
            self.cursor += c.len();
        }
    }
    pub fn backspace(&mut self) {
        let end = self.cursor;
        self.left();
        self.text.replace_range(self.cursor..end, "");
        self.normalize_cursor();
    }
    pub fn delete(&mut self) {
        if let Some(c) = self.text[self.cursor..].graphemes(true).next() {
            self.text
                .replace_range(self.cursor..self.cursor + c.len(), "");
            self.normalize_cursor();
        }
    }
    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
    }
    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i)
    }
    pub fn home(&mut self) {
        self.cursor = self.line_start();
    }
    pub fn end(&mut self) {
        self.cursor = self.line_end();
    }
    pub fn vertical(&mut self, down: bool) {
        let start = self.line_start();
        let column = self.text[start..self.cursor]
            .graphemes(true)
            .fold(0, |column, g| {
                column + if g == "\t" { 4 - column % 4 } else { g.width() }
            });
        let next_start = if down {
            let end = self.line_end();
            if end == self.text.len() {
                return;
            }
            end + 1
        } else {
            if start == 0 {
                return;
            }
            self.text[..start - 1].rfind('\n').map_or(0, |i| i + 1)
        };
        self.cursor = next_start;
        let mut used = 0;
        for grapheme in self.text[next_start..].graphemes(true) {
            if grapheme == "\n" {
                break;
            }
            let width = if grapheme == "\t" {
                4 - used % 4
            } else {
                grapheme.width()
            };
            if used + width > column {
                break;
            }
            self.cursor += grapheme.len();
            used += width;
        }
    }
    pub fn kill_to_start(&mut self) {
        let start = self.line_start();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }
    pub fn delete_word(&mut self) {
        while self.text[..self.cursor]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
        {
            self.backspace();
        }
        while self.text[..self.cursor]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace())
        {
            self.backspace();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::Draft;
    #[test]
    fn utf8_editing_and_multiline_navigation() {
        let mut d = Draft::default();
        d.insert("日本\nabc\n語");
        d.home();
        d.vertical(false);
        assert_eq!(d.cursor(), "日本\n".len());
        d.right();
        d.right();
        d.vertical(false);
        assert_eq!(d.cursor(), "日".len());
        d.insert("😀");
        assert_eq!(d.text(), "日😀本\nabc\n語");
        d.backspace();
        assert_eq!(d.text(), "日本\nabc\n語");
        d.delete();
        assert_eq!(d.text(), "日\nabc\n語");
        d.end();
        d.vertical(true);
        d.end();
        d.kill_to_start();
        assert_eq!(d.text(), "日\n\n語");
    }
    #[test]
    fn arbitrary_edits_always_preserve_utf8_boundaries() {
        let mut d = Draft::default();
        for _ in 0..100 {
            d.insert("あa😀\n");
            d.left();
            d.left();
            d.delete();
            d.home();
            d.right();
            d.backspace();
            d.vertical(true);
            d.end();
            d.delete_word();
            assert!(d.text().is_char_boundary(d.cursor()));
        }
    }
}

#[cfg(test)]
mod grapheme_tests {
    use super::*;
    #[test]
    fn combined_kana_emoji_and_flags_are_not_split() {
        let mut d = Draft::default();
        d.insert("か\u{3099}👨‍👩‍👧‍👦🇯🇵");
        d.left();
        assert_eq!(d.cursor(), "か\u{3099}👨‍👩‍👧‍👦".len());
        d.backspace();
        assert_eq!(d.text(), "か\u{3099}🇯🇵");
        d.delete();
        assert_eq!(d.text(), "か\u{3099}");
        d.backspace();
        assert_eq!(d.text(), "");
    }
    #[test]
    fn inserting_a_joiner_keeps_the_cursor_on_a_grapheme_boundary() {
        let mut d = Draft::default();
        d.insert("👨👩");
        d.left();
        d.insert("\u{200d}");
        assert_eq!(d.text(), "👨‍👩");
        assert_eq!(d.cursor(), d.text().len());
        d.backspace();
        assert_eq!(d.text(), "");
    }
}
