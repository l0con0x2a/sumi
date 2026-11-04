//! UTF-8 SKK dictionaries. Dictionary expressions are parsed as data, never run.
use serde::Serialize;
use std::{collections::HashMap, fs, io, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub text: String,
    pub annotation: Option<String>,
}
#[derive(Default, Debug)]
struct Entry {
    candidates: Vec<Candidate>,
    okuri: HashMap<String, Vec<Candidate>>,
}
impl Entry {
    fn extend(&mut self, lower: Self) {
        for (suffix, candidates) in lower.okuri {
            let specific = self.okuri.entry(suffix).or_default();
            // A lower-priority hint must not jump ahead of the higher
            // dictionary's general candidates for the same okuri key.
            append_unique(specific, self.candidates.iter().cloned());
            append_unique(specific, candidates);
        }
        append_unique(&mut self.candidates, lower.candidates);
    }
}
#[derive(Default, Debug)]
pub struct Dictionary {
    entries: HashMap<String, Entry>,
    skipped_entries: usize,
    skipped_candidates: usize,
}
/// Counts include merged entries; skipped counts describe unsupported input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Stats {
    pub entries: usize,
    pub candidates: usize,
    pub okuri_blocks: usize,
    pub skipped_entries: usize,
    pub skipped_candidates: usize,
}
fn append_unique(target: &mut Vec<Candidate>, candidates: impl IntoIterator<Item = Candidate>) {
    for candidate in candidates {
        if !target.iter().any(|c| c.text == candidate.text) {
            target.push(candidate);
        }
    }
}
fn supported_reading(reading: &str) -> bool {
    let stem = if reading.ends_with(|c: char| c.is_ascii_lowercase()) {
        &reading[..reading.len() - 1]
    } else {
        reading
    };
    !stem.is_empty() && stem.chars().all(|c| !c.is_ascii() && !c.is_control())
}

// Split only outside parenthesized expressions, including quoted slash/semicolon.
fn split_outside(input: &str, delimiter: char) -> Result<Vec<&str>, String> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    let mut parts = Vec::new();
    for (i, c) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && c == '\\' {
            escaped = true;
            continue;
        }
        if depth > 0 && c == '"' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match c {
            '(' if depth > 0
                || (i == start && (delimiter == '/' || start == 0))
                || (delimiter == '/'
                    && input[..i].ends_with(';')
                    && input[i..].starts_with("(concat")) =>
            {
                depth += 1
            }
            ')' if depth > 0 => depth -= 1,
            c if c == delimiter && depth == 0 => {
                parts.push(&input[start..i]);
                start = i + c.len_utf8();
            }
            _ => (),
        }
    }
    if depth > 0 || quoted {
        return Err("unterminated dictionary expression".into());
    }
    parts.push(&input[start..]);
    Ok(parts)
}
// Decode only the SKK concat representation, not arbitrary Lisp. Octal escapes
// are bytes; validating the resulting UTF-8 also permits escaped Japanese text.
fn text(input: &str) -> Option<String> {
    if !input.starts_with('(') {
        return Some(input.to_owned());
    }
    let mut rest = input.strip_prefix("(concat")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mut output = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest == ")" {
            break;
        }
        rest = rest.strip_prefix('"')?;
        let mut end = None;
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    end = Some(i + 1);
                    break;
                }
                b'\\' => {
                    i += 1;
                    match *bytes.get(i)? {
                        b'0'..=b'7' => {
                            let mut value = 0u16;
                            let mut count = 0;
                            while count < 3
                                && bytes.get(i).is_some_and(|b| (b'0'..=b'7').contains(b))
                            {
                                value = value * 8 + (bytes[i] - b'0') as u16;
                                count += 1;
                                i += 1;
                            }
                            output.push(u8::try_from(value).ok()?);
                            continue;
                        }
                        b'"' | b'\\' => output.push(bytes[i]),
                        _ => return None,
                    }
                }
                byte => output.push(byte),
            }
            i += 1;
        }
        rest = &rest[end?..];
    }
    String::from_utf8(output).ok()
}
fn candidate(input: &str) -> Result<Option<Candidate>, String> {
    let parts = split_outside(input, ';')?;
    let Some(body) = text(parts[0]) else {
        return Ok(None);
    };
    if body.is_empty() || body.chars().any(char::is_control) {
        return Ok(None);
    }
    let annotation = if parts.len() > 1 {
        // The first semicolon separates annotation; later semicolons are data.
        let annotation = &input[parts[0].len() + 1..];
        let annotation = if annotation.starts_with("(concat") {
            text(annotation)
        } else {
            Some(annotation.to_owned())
        };
        annotation.filter(|s| !s.chars().any(char::is_control))
    } else {
        None
    };
    Ok(Some(Candidate {
        text: body,
        annotation,
    }))
}
impl Dictionary {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let input = fs::read_to_string(path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        Self::parse(&input).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        })
    }
    pub fn demo() -> Self {
        Self::parse(include_str!("../data/demo.skk")).expect("valid bundled dictionary")
    }
    pub fn parse(input: &str) -> Result<Self, String> {
        let mut dictionary = Self::default();
        for (line_no, line) in input.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            let error = |message: &str| format!("line {}: {message}", line_no + 1);
            let (reading, values) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| error("missing candidate list"))?;
            if !supported_reading(reading) {
                dictionary.skipped_entries += 1;
                continue;
            }
            let values = values.trim();
            if values.len() < 2 || !values.starts_with('/') || !values.ends_with('/') {
                return Err(error("expected /candidate/.../"));
            }
            let values = split_outside(&values[1..values.len() - 1], '/').map_err(|e| error(&e))?;
            let entry = dictionary.entries.entry(reading.to_owned()).or_default();
            let mut okuri: Option<String> = None;
            for value in values {
                if value.is_empty() {
                    continue;
                }
                if let Some(suffix) = value.strip_prefix('[') {
                    if okuri.is_some() || suffix.is_empty() {
                        return Err(error("invalid okuri block"));
                    }
                    okuri = Some(suffix.to_owned());
                    continue;
                }
                if value == "]" {
                    if okuri.take().is_none() {
                        return Err(error("unexpected okuri block end"));
                    }
                    continue;
                }
                let Some(candidate) = candidate(value).map_err(|e| error(&e))? else {
                    dictionary.skipped_candidates += 1;
                    continue;
                };
                let target = match &okuri {
                    Some(suffix) => entry.okuri.entry(suffix.clone()).or_default(),
                    None => &mut entry.candidates,
                };
                append_unique(target, [candidate]);
            }
            if okuri.is_some() {
                return Err(error("unterminated okuri block"));
            }
        }
        Ok(dictionary)
    }
    /// Return candidates in dictionary order, excluding okuri-specific hints.
    pub fn lookup(&self, reading: &str) -> &[Candidate] {
        self.entries
            .get(reading)
            .map(|e| e.candidates.as_slice())
            .unwrap_or(&[])
    }
    /// Prioritize a matching okuri block, followed by the general candidates.
    /// Blocks for other suffixes never appear as candidates.
    pub fn lookup_okuri(&self, reading: &str, suffix: &str) -> Vec<Candidate> {
        let Some(entry) = self.entries.get(reading) else {
            return Vec::new();
        };
        let mut result = entry.okuri.get(suffix).cloned().unwrap_or_default();
        append_unique(&mut result, entry.candidates.iter().cloned());
        result
    }
    /// Merge a lower-priority dictionary; preserve order and the first annotation.
    pub fn extend(&mut self, other: Self) {
        self.skipped_entries += other.skipped_entries;
        self.skipped_candidates += other.skipped_candidates;
        if self.entries.is_empty() {
            self.entries = other.entries;
            return;
        }
        for (reading, source) in other.entries {
            let entry = self.entries.entry(reading).or_default();
            entry.extend(source);
        }
    }
    /// Overlay a higher-priority dictionary, visiting only its entries. This
    /// avoids rebuilding a large system dictionary for a small user dictionary.
    pub fn prepend(&mut self, higher: Self) {
        self.skipped_entries += higher.skipped_entries;
        self.skipped_candidates += higher.skipped_candidates;
        for (reading, mut source) in higher.entries {
            let entry = self.entries.entry(reading).or_default();
            source.extend(std::mem::take(entry));
            *entry = source;
        }
    }
    pub fn stats(&self) -> Stats {
        Stats {
            entries: self.entries.len(),
            candidates: self
                .entries
                .values()
                .map(|e| e.candidates.len() + e.okuri.values().map(Vec::len).sum::<usize>())
                .sum(),
            okuri_blocks: self.entries.values().map(|e| e.okuri.len()).sum(),
            skipped_entries: self.skipped_entries,
            skipped_candidates: self.skipped_candidates,
        }
    }
    /// Remember a preferred candidate without touching the filesystem.
    pub fn promote(
        &mut self,
        reading: &str,
        candidate: Candidate,
        okuri: Option<&str>,
    ) -> Result<(), String> {
        if !supported_reading(reading)
            || (okuri.is_some() && !reading.ends_with(|c: char| c.is_ascii_lowercase()))
            || candidate.text.is_empty()
            || candidate.text.chars().any(char::is_control)
            || candidate
                .annotation
                .as_ref()
                .is_some_and(|s| s.chars().any(char::is_control))
            || okuri.is_some_and(|s| {
                s.is_empty() || s.chars().any(|c| c.is_control() || "/[]".contains(c))
            })
        {
            return Err("invalid reading, candidate or okuri".into());
        }
        let entry = self.entries.entry(reading.into()).or_default();
        let promote = |list: &mut Vec<Candidate>| {
            list.retain(|c| c.text != candidate.text);
            list.insert(0, candidate.clone());
        };
        promote(&mut entry.candidates);
        if let Some(suffix) = okuri {
            promote(entry.okuri.entry(suffix.into()).or_default());
        }
        Ok(())
    }
    /// Deterministic SKK serialization of supported data. Comments are managed
    /// separately by UserDictionary so a save can retain copyright/user notes.
    pub fn to_skk(&self) -> String {
        let mut output = String::new();
        let mut keys: Vec<_> = self.entries.keys().collect();
        keys.sort();
        for okuri in [true, false] {
            output.push_str(if okuri {
                ";; okuri-ari entries.\n"
            } else {
                ";; okuri-nasi entries.\n"
            });
            let mut group: Vec<_> = keys
                .iter()
                .filter(|k| k.ends_with(|c: char| c.is_ascii_lowercase()) == okuri)
                .collect();
            if okuri {
                group.reverse();
            }
            for key in group {
                let entry = &self.entries[*key];
                output.push_str(key);
                output.push_str(" /");
                for candidate in &entry.candidates {
                    write_candidate(&mut output, candidate);
                }
                let mut suffixes: Vec<_> = entry.okuri.keys().collect();
                suffixes.sort();
                for suffix in suffixes {
                    output.push('[');
                    output.push_str(suffix);
                    output.push('/');
                    for candidate in &entry.okuri[suffix] {
                        write_candidate(&mut output, candidate);
                    }
                    output.push_str("]/");
                }
                output.push('\n');
            }
        }
        output
    }
}
fn encode(value: &str) -> String {
    if !value.contains(['/', ';']) && !value.starts_with(['(', '[', ']']) {
        return value.into();
    }
    let mut output = String::from("(concat \"");
    for c in value.chars() {
        match c {
            '/' => output.push_str("\\057"),
            ';' => output.push_str("\\073"),
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            c => output.push(c),
        }
    }
    output.push_str("\")");
    output
}
fn write_candidate(output: &mut String, candidate: &Candidate) {
    output.push_str(&encode(&candidate.text));
    if let Some(annotation) = &candidate.annotation {
        output.push(';');
        output.push_str(&encode(annotation));
    }
    output.push('/');
}
