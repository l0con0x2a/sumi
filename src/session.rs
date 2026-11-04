//! Optional dictionary learning around the pure input engine.
use crate::{Engine, Key, Update, dictionary::Dictionary, user_dictionary::UserDictionary};
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
        let (user, mut dictionary) = UserDictionary::open(path)?;
        dictionary.extend(system);
        Ok(Self {
            engine,
            dictionary,
            user: Some(user),
        })
    }
    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
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
}
