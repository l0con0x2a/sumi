use serde::Serialize;
use std::{collections::HashMap, fs, io, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub text: String,
    pub annotation: Option<String>,
}

#[derive(Default, Debug)]
pub struct Dictionary(HashMap<String, Vec<Candidate>>);

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
        let mut dict = Self::default();
        for (line_no, line) in input.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            let Some((reading, values)) = line.split_once(char::is_whitespace) else {
                return Err(format!("line {}: missing candidate list", line_no + 1));
            };
            // Okuri keys, abbrev and numeric conversion are deliberately unsupported.
            if reading.chars().any(|c| c.is_ascii()) {
                continue;
            }
            let values = values.trim();
            if !values.starts_with('/') || !values.ends_with('/') {
                return Err(format!("line {}: expected /candidate/.../", line_no + 1));
            }
            let entries = dict.0.entry(reading.to_owned()).or_default();
            for value in values[1..values.len() - 1].split('/') {
                // Never evaluate dictionary Lisp or expose okuri blocks as candidates.
                if value.is_empty() || value.starts_with('[') || value.starts_with('(') {
                    continue;
                }
                let (text, annotation) = value
                    .split_once(';')
                    .map_or((value, None), |(t, a)| (t, Some(a.to_owned())));
                if text.is_empty() || text.chars().any(char::is_control) {
                    continue;
                }
                let candidate = Candidate {
                    text: text.to_owned(),
                    annotation,
                };
                if !entries.iter().any(|c| c.text == text) {
                    entries.push(candidate);
                }
            }
        }
        Ok(dict)
    }
    pub fn lookup(&self, reading: &str) -> &[Candidate] {
        self.0.get(reading).map(Vec::as_slice).unwrap_or(&[])
    }
    /// Merge a lower-priority dictionary, preserving order and the first annotation.
    pub fn extend(&mut self, other: Self) {
        for (reading, candidates) in other.0 {
            let existing = self.0.entry(reading).or_default();
            for candidate in candidates {
                if !existing.iter().any(|c| c.text == candidate.text) {
                    existing.push(candidate);
                }
            }
        }
    }
}
