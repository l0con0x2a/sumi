//! Incremental romaji transliteration without SKK commands or a dictionary.
use crate::romaji;

/// The output script for literal romaji conversion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Script {
    #[default]
    Hiragana,
    Katakana,
}

/// A streaming converter. Unfinished romaji remains pending across chunks.
///
/// ```
/// use sumi_skk::kana::{Converter, Script};
/// let mut converter = Converter::new(Script::Hiragana);
/// assert_eq!(converter.push("ko"), "こ");
/// assert_eq!(converter.push("n"), "");
/// assert_eq!(converter.push("nichiha"), "んにちは");
/// assert_eq!(converter.finish(), "");
/// ```
#[derive(Debug, Default)]
pub struct Converter {
    pending: String,
    script: Script,
}
impl Converter {
    pub fn new(script: Script) -> Self {
        Self {
            script,
            ..Self::default()
        }
    }
    fn render(&self, text: &str) -> String {
        match self.script {
            Script::Hiragana => text.into(),
            Script::Katakana => romaji::katakana(text),
        }
    }
    /// Convert a chunk. Uppercase letters are romaji, not SKK commands.
    /// Other Unicode text, spaces and line endings are preserved.
    pub fn push(&mut self, input: &str) -> String {
        let mut output = String::new();
        for c in input.chars() {
            output.push_str(&romaji::feed(&mut self.pending, c.to_ascii_lowercase()));
        }
        self.render(&output)
    }
    /// Flush a trailing n as ん and preserve other incomplete romaji literally.
    /// The converter can be reused for a new stream after this call.
    pub fn finish(&mut self) -> String {
        let tail = romaji::flush(&mut self.pending);
        self.render(&tail)
    }
    pub fn pending(&self) -> &str {
        &self.pending
    }
}
