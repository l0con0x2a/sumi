//! Optional dictionary learning around the pure input engine.
use crate::{
    Confirmed, Engine, Key, Phase, Update,
    dictionary::{Candidate, Dictionary},
    user_dictionary::UserDictionary,
};
use std::{io, path::Path};
#[derive(Debug)]
pub struct Session {
    pub(crate) engine: Engine,
    pub(crate) dictionary: Dictionary,
    user: Option<UserDictionary>,
}
impl Session {
    pub fn new(engine: Engine, dictionary: Dictionary) -> Self {
        Self {
            engine,
            dictionary,
            user: None,
        }
    }
    /// Explicit opt-in: no files are created until a dictionary choice is confirmed.
    pub fn with_user_dictionary(
        engine: Engine,
        system: Dictionary,
        path: impl AsRef<Path>,
    ) -> io::Result<Self> {
        let (user, higher) = UserDictionary::open(path)?;
        let mut dictionary = system;
        dictionary.prepend(higher);
        Ok(Self {
            engine,
            dictionary,
            user: Some(user),
        })
    }
    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
    }
    pub fn has_user_dictionary(&self) -> bool {
        self.user.is_some()
    }
    pub fn snapshot(&self) -> Update {
        self.engine.snapshot()
    }
    pub fn is_idle(&self) -> bool {
        self.engine.is_idle()
    }
    pub fn reset(&mut self) {
        self.engine.reset();
    }
    pub(crate) fn learn(&mut self, mut update: Update) -> Update {
        if let (Some(user), Some(confirmed)) = (&self.user, &update.confirmed) {
            // Preferences in the active session still work if disk persistence
            // fails. The committed text is never lost because of a save error.
            let result = self.dictionary.promote(
                &confirmed.dictionary_key,
                confirmed.candidate.clone(),
                confirmed.okuri.as_deref(),
            );
            if let Err(error) = result
                .map_err(io::Error::other)
                .and_then(|()| user.record(confirmed))
            {
                update.learning_error = Some(error.to_string());
            }
        }
        update
    }
    pub fn process_key(&mut self, key: Key) -> Update {
        let update = self.engine.process_key(key, &self.dictionary);
        self.learn(update)
    }
    pub fn commit(&mut self) -> Update {
        let update = self.engine.commit(&self.dictionary);
        self.learn(update)
    }
    pub fn select_candidate(&mut self, index: usize) -> Update {
        self.engine.select_candidate(index)
    }
    /// Add a word without changing composition. It is session-local unless a
    /// user dictionary was explicitly configured.
    pub fn register(&mut self, reading: &str, candidate: Candidate, okuri: Option<&str>) -> Update {
        if let Err(error) = self.dictionary.promote(reading, candidate.clone(), okuri) {
            return Update {
                error: Some(error),
                ..self.snapshot()
            };
        }
        let confirmed = Confirmed {
            dictionary_key: reading.into(),
            candidate,
            okuri: okuri.map(str::to_owned),
        };
        let learning_error = self
            .user
            .as_ref()
            .and_then(|user| user.record(&confirmed).err())
            .map(|e| e.to_string());
        Update {
            learning_error,
            ..self.snapshot()
        }
    }
    /// Register a stem for the current completed reading and confirm it, adding
    /// the existing okuri. The parent reading is retained on validation errors.
    pub fn register_current(&mut self, candidate: Candidate) -> Update {
        let state = self.snapshot();
        if state.phase != Phase::Composing
            || !state.pending.is_empty()
            || state.reading.as_ref().is_none_or(String::is_empty)
            || state.okuri.as_ref().is_some_and(String::is_empty)
        {
            return Update {
                error: Some("registration requires a completed reading".into()),
                ..state
            };
        }
        if let Err(error) = self.dictionary.promote(
            state.dictionary_key.as_deref().unwrap(),
            candidate,
            state.okuri.as_deref(),
        ) {
            return Update {
                error: Some(error),
                ..state
            };
        }
        let selected = self.engine.process_key(Key::Space, &self.dictionary);
        if selected.error.is_some() {
            return selected;
        }
        self.commit()
    }
}
