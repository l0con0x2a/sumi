//! Editor-independent SKK state machine and dictionary-backed input.
//!
//! [`Engine`] owns input state. A shared [`dictionary::Dictionary`] is supplied
//! for each event; no files, processes or terminal operations occur in the engine.
pub mod dictionary;
pub mod kana;
pub mod protocol;
mod romaji;

use dictionary::{Candidate, Dictionary};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Ascii,
    Hiragana,
    Katakana,
}
impl Mode {
    fn render(self, text: &str) -> String {
        if self == Self::Katakana {
            romaji::katakana(text)
        } else {
            text.into()
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Direct,
    Composing,
    SelectingCandidates,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Space,
    Enter,
    Escape,
    Backspace,
    CtrlJ,
    CtrlG,
}
impl Key {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(match s {
            "Space" => Self::Space,
            "Apostrophe" => Self::Char('\''),
            "Enter" => Self::Enter,
            "Escape" => Self::Escape,
            "Backspace" => Self::Backspace,
            "C-j" => Self::CtrlJ,
            "C-g" => Self::CtrlG,
            _ => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(' '), None) => Self::Space,
                    (Some(c), None) if !c.is_control() => Self::Char(c),
                    _ => return Err("expected a Unicode character or named key".into()),
                }
            }
        })
    }
}
#[derive(Clone, Debug)]
struct Okuri {
    key: char,
    text: String,
}
#[derive(Clone, Debug, Default)]
struct Reading {
    kana: String,
    okuri: Option<Okuri>,
}
impl Reading {
    fn push(&mut self, text: &str) {
        match &mut self.okuri {
            Some(okuri) => okuri.text.push_str(text),
            None => self.kana.push_str(text),
        }
    }
    fn suffix(&self) -> &str {
        self.okuri.as_ref().map_or("", |o| o.text.as_str())
    }
    fn plain(&self) -> String {
        format!("{}{}", self.kana, self.suffix())
    }
    fn key(&self) -> String {
        let mut key = self.kana.clone();
        if let Some(okuri) = &self.okuri {
            key.push(okuri.key);
        }
        key
    }
    fn preedit(&self, mode: Mode, pending: &str) -> String {
        let marker = if self.okuri.is_some() { "*" } else { "" };
        format!(
            "▽{}{marker}{}{pending}",
            mode.render(&self.kana),
            mode.render(self.suffix())
        )
    }
}
#[derive(Debug, Default)]
enum Composition {
    #[default]
    Direct,
    Reading(Reading),
    Selecting {
        reading: Reading,
        candidates: Vec<Candidate>,
        index: usize,
    },
}
/// An isolated input session. Multiple engines may share an immutable dictionary.
#[derive(Debug)]
pub struct Engine {
    mode: Mode,
    composition: Composition,
    pending: String,
}
/// One event's committed delta plus the complete new preedit and structured state.
#[derive(Debug, Serialize)]
pub struct Update {
    pub mode: Mode,
    pub phase: Phase,
    pub preedit: String,
    pub pending: String,
    /// Hiragana stem, without okuri; None during direct input.
    pub reading: Option<String>,
    /// Completed okuri kana; Some("") while entering the first okuri syllable.
    pub okuri: Option<String>,
    pub dictionary_key: Option<String>,
    pub committed: String,
    pub candidates: Vec<Candidate>,
    pub candidate_index: Option<usize>,
    pub passthrough: Option<String>,
    pub error: Option<String>,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(Mode::Hiragana)
    }
}
impl Engine {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            composition: Composition::Direct,
            pending: String::new(),
        }
    }
    /// Discard uncommitted input, preserving the current input mode.
    pub fn reset(&mut self) {
        self.composition = Composition::Direct;
        self.pending.clear();
    }
    pub fn snapshot(&self) -> Update {
        let (phase, preedit, reading, candidates, index) = match &self.composition {
            Composition::Direct => (Phase::Direct, self.pending.clone(), None, vec![], None),
            Composition::Reading(r) => (
                Phase::Composing,
                r.preedit(self.mode, &self.pending),
                Some(r),
                vec![],
                None,
            ),
            Composition::Selecting {
                reading,
                candidates,
                index,
            } => (
                Phase::SelectingCandidates,
                format!(
                    "▼{}{}",
                    candidates[*index].text,
                    self.mode.render(reading.suffix())
                ),
                Some(reading),
                candidates.clone(),
                Some(*index),
            ),
        };
        Update {
            mode: self.mode,
            phase,
            preedit,
            pending: self.pending.clone(),
            reading: reading.map(|r| r.kana.clone()),
            okuri: reading.and_then(|r| r.okuri.as_ref().map(|o| o.text.clone())),
            dictionary_key: reading.map(Reading::key),
            committed: String::new(),
            candidates,
            candidate_index: index,
            passthrough: None,
            error: None,
        }
    }
    pub fn is_idle(&self) -> bool {
        matches!(self.composition, Composition::Direct) && self.pending.is_empty()
    }
    fn lookup(&mut self, dictionary: &Dictionary) -> Option<String> {
        let Composition::Reading(reading) = std::mem::take(&mut self.composition) else {
            unreachable!("lookup is only called during composition");
        };
        let candidates = if reading.okuri.is_some() {
            dictionary.lookup_okuri(&reading.key(), reading.suffix())
        } else {
            dictionary.lookup(&reading.kana).to_vec()
        };
        if candidates.is_empty() {
            self.composition = Composition::Reading(reading);
            Some("no candidates".into())
        } else {
            self.composition = Composition::Selecting {
                reading,
                candidates,
                index: 0,
            };
            None
        }
    }
    pub fn process_key(&mut self, key: Key, dictionary: &Dictionary) -> Update {
        let key = if key == Key::Char(' ') {
            Key::Space
        } else {
            key
        };
        if matches!(key, Key::Char(c) if c.is_control()) {
            return Update {
                error: Some("control characters require named keys".into()),
                ..self.snapshot()
            };
        }
        let mut committed = String::new();
        let mut passthrough = None;
        let mut error = None;
        let composition = std::mem::take(&mut self.composition);
        if let Composition::Selecting {
            reading,
            candidates,
            mut index,
        } = composition
        {
            match key {
                Key::Space => {
                    if index + 1 < candidates.len() {
                        index += 1;
                    } else {
                        error = Some(
                            "candidate list exhausted; registration is not implemented".into(),
                        );
                    }
                    self.composition = Composition::Selecting {
                        reading,
                        candidates,
                        index,
                    };
                }
                Key::Char('x') if index > 0 => {
                    index -= 1;
                    self.composition = Composition::Selecting {
                        reading,
                        candidates,
                        index,
                    };
                }
                Key::Char('x') | Key::Escape | Key::CtrlG | Key::Backspace => {
                    self.composition = Composition::Reading(reading)
                }
                Key::Enter | Key::CtrlJ => {
                    committed = format!(
                        "{}{}",
                        candidates[index].text,
                        self.mode.render(reading.suffix())
                    );
                    if key == Key::CtrlJ {
                        self.mode = Mode::Hiragana;
                    }
                }
                _ => {
                    committed = format!(
                        "{}{}",
                        candidates[index].text,
                        self.mode.render(reading.suffix())
                    );
                    let next = self.process_key(key, dictionary);
                    committed.push_str(&next.committed);
                    return Update { committed, ..next };
                }
            }
        } else {
            self.composition = composition;
            match key {
                Key::Escape | Key::CtrlG => {
                    if self.is_idle() {
                        passthrough =
                            Some(if key == Key::Escape { "Escape" } else { "C-g" }.into());
                    }
                    self.reset();
                }
                Key::Backspace => {
                    if self.pending.pop().is_some() {
                        if self.pending.is_empty()
                            && let Composition::Reading(r) = &mut self.composition
                            && r.okuri.as_ref().is_some_and(|o| o.text.is_empty())
                        {
                            r.okuri = None;
                        }
                    } else if let Composition::Reading(r) = &mut self.composition {
                        if let Some(okuri) = &mut r.okuri {
                            okuri.text.pop();
                            if okuri.text.is_empty() {
                                r.okuri = None;
                            }
                        } else if r.kana.pop().is_none() {
                            self.composition = Composition::Direct;
                        }
                    } else {
                        passthrough = Some("Backspace".into());
                    }
                }
                Key::Enter | Key::CtrlJ => {
                    let tail = romaji::flush(&mut self.pending);
                    if let Composition::Reading(r) = &mut self.composition {
                        r.push(&tail);
                        committed = self.mode.render(&r.plain());
                        self.composition = Composition::Direct;
                    } else if key == Key::Enter && tail.is_empty() {
                        passthrough = Some("Enter".into());
                    } else {
                        committed = self.mode.render(&tail);
                    }
                    if key == Key::CtrlJ {
                        self.mode = Mode::Hiragana;
                    }
                }
                Key::Space => {
                    let tail = romaji::flush(&mut self.pending);
                    if let Composition::Reading(r) = &mut self.composition {
                        r.push(&tail);
                        error = self.lookup(dictionary);
                    } else {
                        committed = self.mode.render(&tail);
                        committed.push(' ');
                    }
                }
                Key::Char(c) => {
                    if self.mode == Mode::Ascii {
                        committed.push(c);
                    } else if matches!(self.composition, Composition::Direct)
                        && (c == 'q' || c == 'l')
                    {
                        committed = self.mode.render(&romaji::flush(&mut self.pending));
                        self.mode = if c == 'l' {
                            Mode::Ascii
                        } else if self.mode == Mode::Hiragana {
                            Mode::Katakana
                        } else {
                            Mode::Hiragana
                        };
                    } else if c == 'q' && matches!(self.composition, Composition::Reading(_)) {
                        let tail = romaji::flush(&mut self.pending);
                        if let Composition::Reading(r) = &mut self.composition {
                            r.push(&tail);
                            committed = if self.mode == Mode::Katakana {
                                r.plain()
                            } else {
                                romaji::katakana(&r.plain())
                            };
                        }
                        self.composition = Composition::Direct;
                    } else {
                        if c.is_ascii_uppercase() && matches!(self.composition, Composition::Direct)
                        {
                            committed = self.mode.render(&romaji::flush(&mut self.pending));
                            self.composition = Composition::Reading(Reading::default());
                        }
                        if c != 'Q' {
                            let had_pending = !self.pending.is_empty();
                            if let Composition::Reading(r) = &mut self.composition
                                && c.is_ascii_uppercase()
                                && !had_pending
                                && r.okuri.is_none()
                                && !r.kana.is_empty()
                            {
                                r.okuri = Some(Okuri {
                                    key: c.to_ascii_lowercase(),
                                    text: String::new(),
                                });
                            }
                            let kana = romaji::feed(&mut self.pending, c.to_ascii_lowercase());
                            if let Composition::Reading(r) = &mut self.composition {
                                r.push(&kana);
                                // Uppercase completing a pending syllable is still part of the stem.
                                // If a consonant remains (KanJ, SasS), it starts the okuri instead.
                                if c.is_ascii_uppercase()
                                    && had_pending
                                    && !self.pending.is_empty()
                                    && r.okuri.is_none()
                                    && !r.kana.is_empty()
                                {
                                    r.okuri = Some(Okuri {
                                        key: c.to_ascii_lowercase(),
                                        text: String::new(),
                                    });
                                }
                                if r.okuri.as_ref().is_some_and(|o| !o.text.is_empty())
                                    && self.pending.is_empty()
                                {
                                    error = self.lookup(dictionary);
                                }
                            } else {
                                committed.push_str(&self.mode.render(&kana));
                            }
                        }
                    }
                }
            }
        }
        Update {
            committed,
            passthrough,
            error,
            ..self.snapshot()
        }
    }
}
