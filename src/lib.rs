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
                    (Some(c), None) if !c.is_control() => {
                        if c == ' ' {
                            Self::Space
                        } else {
                            Self::Char(c)
                        }
                    }
                    _ => return Err("expected a Unicode character or named key".into()),
                }
            }
        })
    }
}
#[derive(Debug, Default)]
enum Composition {
    #[default]
    Direct,
    Reading(String),
    Selecting {
        reading: String,
        candidates: Vec<Candidate>,
        index: usize,
    },
}
#[derive(Debug)]
pub struct Engine {
    mode: Mode,
    composition: Composition,
    pending: String,
}
#[derive(Debug, Serialize)]
pub struct Update {
    pub mode: Mode,
    pub phase: &'static str,
    pub preedit: String,
    pub committed: String,
    pub candidates: Vec<Candidate>,
    pub candidate_index: Option<usize>,
    pub passthrough: Option<String>,
    pub error: Option<String>,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            mode: Mode::Hiragana,
            composition: Composition::Direct,
            pending: String::new(),
        }
    }
}
impl Engine {
    fn render(&self, s: &str) -> String {
        if self.mode == Mode::Katakana {
            romaji::katakana(s)
        } else {
            s.into()
        }
    }
    pub fn snapshot(&self) -> Update {
        let (phase, preedit, candidates, index) = match &self.composition {
            Composition::Direct => ("direct", self.pending.clone(), vec![], None),
            Composition::Reading(s) => (
                "composing",
                format!("▽{}{pending}", self.render(s), pending = self.pending),
                vec![],
                None,
            ),
            Composition::Selecting {
                candidates, index, ..
            } => (
                "selecting_candidates",
                format!("▼{}", candidates[*index].text),
                candidates.clone(),
                Some(*index),
            ),
        };
        Update {
            mode: self.mode,
            phase,
            preedit,
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
    pub fn process_key(&mut self, key: Key, dictionary: &Dictionary) -> Update {
        let mut committed = String::new();
        let mut passthrough = None;
        let mut error = None;
        // Selection has its own cancellation and navigation semantics.
        if let Composition::Selecting {
            reading,
            candidates,
            index,
        } = &mut self.composition
        {
            match key {
                Key::Space => {
                    if *index + 1 < candidates.len() {
                        *index += 1;
                    } else {
                        error = Some(
                            "candidate list exhausted; registration is not implemented".into(),
                        );
                    }
                }
                Key::Char('x') => {
                    if *index > 0 {
                        *index -= 1;
                    } else {
                        self.composition = Composition::Reading(reading.clone());
                    }
                }
                Key::Escape | Key::CtrlG | Key::Backspace => {
                    self.composition = Composition::Reading(reading.clone());
                }
                Key::Enter => {
                    committed = candidates[*index].text.clone();
                    self.composition = Composition::Direct;
                }
                _ => {
                    committed = candidates[*index].text.clone();
                    self.composition = Composition::Direct;
                    let next = self.process_key(key, dictionary);
                    committed.push_str(&next.committed);
                    return Update { committed, ..next };
                }
            }
        } else {
            match key {
                Key::Escape | Key::CtrlG => {
                    if self.is_idle() {
                        passthrough =
                            Some(if key == Key::Escape { "Escape" } else { "C-g" }.into());
                    }
                    self.pending.clear();
                    self.composition = Composition::Direct;
                }
                Key::Backspace => {
                    if !self.pending.is_empty() {
                        self.pending.pop();
                    } else if let Composition::Reading(s) = &mut self.composition {
                        if s.pop().is_none() {
                            self.composition = Composition::Direct;
                        }
                    } else {
                        passthrough = Some("Backspace".into());
                    }
                }
                Key::CtrlJ => {
                    if self.is_idle() {
                        self.mode = Mode::Hiragana;
                    } else {
                        error = Some("mode switch requires idle state".into());
                    }
                }
                Key::Enter => {
                    let tail = romaji::flush(&mut self.pending);
                    if let Composition::Reading(s) = &mut self.composition {
                        s.push_str(&tail);
                        let reading = s.clone();
                        committed = self.render(&reading);
                        self.composition = Composition::Direct;
                    } else if tail.is_empty() {
                        passthrough = Some("Enter".into());
                    } else {
                        committed = self.render(&tail);
                    }
                }
                Key::Space => {
                    let tail = romaji::flush(&mut self.pending);
                    if let Composition::Reading(s) = &mut self.composition {
                        s.push_str(&tail);
                        let candidates = dictionary.lookup(s).to_vec();
                        if candidates.is_empty() {
                            error = Some("no candidates".into());
                        } else {
                            self.composition = Composition::Selecting {
                                reading: s.clone(),
                                candidates,
                                index: 0,
                            };
                        }
                    } else {
                        committed = self.render(&tail);
                        committed.push(' ');
                    }
                }
                Key::Char(c) => {
                    if self.mode == Mode::Ascii {
                        committed.push(c);
                    } else if self.is_idle() && c == 'l' {
                        self.mode = Mode::Ascii;
                    } else if self.is_idle() && c == 'q' {
                        self.mode = if self.mode == Mode::Hiragana {
                            Mode::Katakana
                        } else {
                            Mode::Hiragana
                        };
                    } else if c == 'q'
                        && self.pending.is_empty()
                        && matches!(self.composition, Composition::Reading(_))
                    {
                        if let Composition::Reading(s) = &self.composition {
                            committed = if self.mode == Mode::Katakana {
                                s.clone()
                            } else {
                                romaji::katakana(s)
                            };
                        }
                        self.composition = Composition::Direct;
                    } else {
                        if c.is_ascii_uppercase() {
                            if matches!(self.composition, Composition::Reading(_)) {
                                error = Some("okurigana is not implemented".into());
                            } else {
                                let tail = romaji::flush(&mut self.pending);
                                committed = self.render(&tail);
                                self.composition = Composition::Reading(String::new());
                            }
                        }
                        if error.is_none() {
                            let kana = romaji::feed(&mut self.pending, c.to_ascii_lowercase());
                            if let Composition::Reading(s) = &mut self.composition {
                                s.push_str(&kana);
                            } else {
                                committed.push_str(&self.render(&kana));
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
