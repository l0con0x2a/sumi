use super::{editor::Draft, sys::Terminal};
use std::{
    borrow::Cow,
    io::{self, Write},
};
use sumi_skk::Update;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn visible(text: &str, replacement: char) -> Cow<'_, str> {
    if text.chars().any(char::is_control) {
        Cow::Owned(
            text.chars()
                .map(|c| if c.is_control() { replacement } else { c })
                .collect(),
        )
    } else {
        Cow::Borrowed(text)
    }
}

fn clip(text: &str, columns: usize) -> String {
    let mut result = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let grapheme = visible(grapheme, ' ');
        let width = grapheme.width();
        if used + width > columns {
            break;
        }
        result.push_str(&grapheme);
        used += width;
    }
    result
}
fn layout(text: &str, caret: usize, columns: usize) -> (Vec<String>, (usize, usize)) {
    let mut lines = vec![String::new()];
    let mut column = 0;
    let mut cursor = (0, 0);
    for (offset, original) in text.grapheme_indices(true).chain([(text.len(), "")]) {
        let grapheme = if original == "\n" || original == "\t" {
            Cow::Borrowed(original)
        } else {
            visible(original, '�')
        };
        let width = if grapheme == "\t" {
            4 - column % 4
        } else {
            grapheme.width()
        };
        if grapheme != "\n" && grapheme != "\t" && (column >= columns || column + width > columns) {
            lines.push(String::new());
            column = 0;
        }
        if offset == caret {
            cursor = (lines.len() - 1, column);
        }
        if offset == text.len() {
            break;
        }
        if grapheme == "\n" {
            lines.push(String::new());
            column = 0;
        } else if grapheme == "\t" {
            for _ in 0..width {
                if column >= columns {
                    lines.push(String::new());
                    column = 0;
                }
                lines.last_mut().unwrap().push(' ');
                column += 1;
            }
        } else {
            lines.last_mut().unwrap().push_str(&grapheme);
            column += width;
        }
        if offset < caret && caret < offset + original.len() {
            cursor = (lines.len() - 1, column);
        }
    }
    (lines, cursor)
}
pub fn draw(
    terminal: &mut Terminal,
    draft: &Draft,
    state: &Update,
    message: &str,
) -> io::Result<()> {
    let (columns, rows) = terminal.size();
    let available = columns.saturating_sub(1).max(1);
    let header = usize::from(rows >= 3);
    let candidate_rows = if rows >= 5 {
        state.candidates.len().min(6).min(rows / 3)
    } else {
        0
    };
    let body_rows = rows - header - candidate_rows - 1;
    let mut display = vec![String::new(); rows];
    if header > 0 {
        display[0] = "SKK  Ctrl-D: send  Esc/Ctrl-C: cancel  Ctrl-G: undo conversion".into();
    }
    let mut text = draft.text().to_owned();
    text.insert_str(draft.cursor(), &state.preedit);
    let (lines, cursor) = layout(
        &text,
        draft.cursor() + state.preedit.len(),
        available.max(2),
    );
    let start = cursor.0.saturating_sub(body_rows - 1);
    for (row, line) in lines.iter().skip(start).take(body_rows).enumerate() {
        display[header + row] = line.clone();
    }
    let selected = state.candidate_index.unwrap_or(0);
    let candidate_start = selected
        .saturating_sub(candidate_rows.saturating_sub(1) / 2)
        .min(state.candidates.len().saturating_sub(candidate_rows));
    for row in 0..candidate_rows {
        let index = candidate_start + row;
        let candidate = &state.candidates[index];
        display[header + body_rows + row] = format!(
            "{} {}. {}{}",
            if index == selected { ">" } else { " " },
            index + 1,
            candidate.text,
            candidate
                .annotation
                .as_deref()
                .map_or(String::new(), |a| format!(" ; {a}"))
        );
    }
    display[rows - 1] = format!(
        "{:?} {:?} {}{}",
        state.mode,
        state.phase,
        state.candidate_index.map_or(String::new(), |i| format!(
            "{}/{} ",
            i + 1,
            state.candidates.len()
        )),
        message
    );
    write!(terminal.tty, "\x1b[?25l")?;
    for (row, line) in display.iter().enumerate() {
        write!(
            terminal.tty,
            "\x1b[{};1H{}\x1b[K",
            row + 1,
            clip(line, available)
        )?;
    }
    write!(
        terminal.tty,
        "\x1b[{};{}H\x1b[?25h",
        header + cursor.0 - start + 1,
        cursor.1.min(available - 1) + 1
    )?;
    terminal.tty.flush()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn japanese_width_and_wrapping() {
        let (lines, caret) = layout("日本abc", "日本a".len(), 5);
        assert_eq!(lines, ["日本a", "bc"]);
        assert_eq!(caret, (1, 0));
        let (_, caret) = layout("日本abc", "日本abc".len(), 5);
        assert_eq!(caret, (1, 2));
        assert_eq!(clip("日本語", 5), "日本");
    }
    #[test]
    fn tabs_and_control_text_cannot_inject_terminal_commands() {
        assert_eq!(clip("a\x1b[31m", 20), "a [31m");
        let (lines, _) = layout("a\t日\nb", 0, 6);
        assert_eq!(lines, ["a   日", "b"]);
    }
}

#[cfg(test)]
mod grapheme_tests {
    use super::*;
    #[test]
    fn joined_emoji_have_one_display_width() {
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(clip(&format!("a{family}b"), 3), format!("a{family}"));
        let (_, caret) = layout(&format!("a{family}b"), 1 + family.len(), 5);
        assert_eq!(caret, (0, 3));
    }
}
