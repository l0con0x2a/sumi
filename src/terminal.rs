//! Standalone terminal composition. POSIX handling stays outside the SKK engine.
mod editor;
mod input;
mod screen;
mod sys;
use editor::Draft;
use input::Event;
use std::io;
use sumi_skk::{
    Engine, Key, Mode, Phase, Update,
    dictionary::{Candidate, Dictionary},
    session::Session,
};
use sys::Terminal;
pub use sys::interrupted_signal;

fn apply(draft: &mut Draft, update: Update) -> String {
    draft.insert(&update.committed);
    match update.passthrough.as_deref() {
        Some("Backspace") => draft.backspace(),
        Some("Enter") => draft.insert("\n"),
        _ => (),
    }
    match (update.error, update.learning_error) {
        (Some(error), Some(learning)) => format!("{error}; learning failed: {learning}"),
        (Some(error), None) => error,
        (None, Some(learning)) => format!("Learning failed: {learning}"),
        (None, None) => String::new(),
    }
}
// Registration has a separate engine, while borrowing the same dictionary.
// Its temporary conversions do not learn until the complete word is registered.
enum Input<'a> {
    Session(&'a mut Session),
    Registration(&'a mut Engine, &'a Dictionary),
}
impl Input<'_> {
    fn snapshot(&self) -> Update {
        match self {
            Self::Session(s) => s.snapshot(),
            Self::Registration(e, _) => e.snapshot(),
        }
    }
    fn key(&mut self, key: Key) -> Update {
        match self {
            Self::Session(s) => s.process_key(key),
            Self::Registration(e, d) => e.process_key(key, d),
        }
    }
    fn commit(&mut self) -> Update {
        match self {
            Self::Session(s) => s.commit(),
            Self::Registration(e, d) => e.commit(d),
        }
    }
}
fn edit(event: Event, input: &mut Input<'_>, draft: &mut Draft) -> String {
    match event {
        Event::Key(key) => apply(draft, input.key(key)),
        Event::Paste(text) => {
            if text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
            {
                "Paste contains unsupported control characters".into()
            } else {
                let message = apply(draft, input.commit());
                draft.insert(&text);
                message
            }
        }
        Event::Up | Event::Down if input.snapshot().phase == Phase::SelectingCandidates => apply(
            draft,
            input.key(if event == Event::Up {
                Key::Char('x')
            } else {
                Key::Space
            }),
        ),
        Event::Left
        | Event::Right
        | Event::Home
        | Event::End
        | Event::Up
        | Event::Down
        | Event::Delete
        | Event::KillToStart
        | Event::DeleteWord => {
            let message = apply(draft, input.commit());
            match event {
                Event::Left => draft.left(),
                Event::Right => draft.right(),
                Event::Home => draft.home(),
                Event::End => draft.end(),
                Event::Up => draft.vertical(false),
                Event::Down => draft.vertical(true),
                Event::Delete => draft.delete(),
                Event::KillToStart => draft.kill_to_start(),
                Event::DeleteWord => draft.delete_word(),
                _ => unreachable!(),
            }
            message
        }
        Event::Unknown => "Unsupported key or invalid UTF-8 input".into(),
        _ => String::new(),
    }
}
struct Registration {
    engine: Engine,
    draft: Draft,
}
pub fn run(mut session: Session) -> io::Result<Option<String>> {
    let mut terminal = Terminal::open()?;
    let mut draft = Draft::default();
    let mut registration: Option<Registration> = None;
    let mut message = String::new();
    let mut dirty = true;
    loop {
        if dirty {
            if let Some(registration) = &registration {
                let state = session.snapshot();
                let title = format!(
                    "Register {}{}  Enter: register  Esc: back",
                    state.dictionary_key.unwrap_or_default(),
                    if session.has_user_dictionary() {
                        ""
                    } else {
                        " (session only)"
                    }
                );
                screen::draw(
                    &mut terminal,
                    &registration.draft,
                    &registration.engine.snapshot(),
                    &message,
                    &title,
                )?;
            } else {
                screen::draw(
                    &mut terminal,
                    &draft,
                    &session.snapshot(),
                    &message,
                    "SKK  Ctrl-D: send  Esc/Ctrl-C: cancel  Ctrl-R: register",
                )?;
            }
        }
        dirty = true;
        let event = terminal.event()?;
        match event {
            Event::Abort => return Ok(None),
            Event::Idle => {
                dirty = false;
                continue;
            }
            Event::Redraw => continue,
            Event::Suspend => {
                terminal.suspend()?;
                continue;
            }
            Event::Cancel => {
                if registration.take().is_some() {
                    message = "Registration cancelled".into();
                    continue;
                }
                return Ok(None);
            }
            Event::Register => {
                if registration.is_some() {
                    message = "Nested registration is not supported".into();
                    continue;
                }
                let state = session.snapshot();
                if state.phase != Phase::Composing
                    || !state.pending.is_empty()
                    || state.reading.as_ref().is_none_or(String::is_empty)
                    || state.okuri.as_ref().is_some_and(String::is_empty)
                {
                    message = "Complete the reading, then press Ctrl-R to register".into();
                    continue;
                }
                registration = Some(Registration {
                    engine: Engine::with_romaji_rule(Mode::Hiragana, state.romaji_rule),
                    draft: Draft::default(),
                });
                message = "Enter the word stem; the original okuri is added automatically".into();
            }
            Event::Submit | Event::Key(Key::Enter) if registration.is_some() => {
                let current = registration.as_mut().unwrap();
                apply(
                    &mut current.draft,
                    current.engine.commit(session.dictionary()),
                );
                let update = session.register_current(Candidate {
                    text: current.draft.text().into(),
                    annotation: None,
                });
                if let Some(error) = update.error {
                    message = error;
                } else {
                    message = apply(&mut draft, update);
                    registration = None;
                }
            }
            Event::Submit => {
                if session.is_idle() {
                    return Ok(Some(draft.into_text()));
                }
                message = "Confirm pending input with Enter before sending".into();
            }
            event => {
                message = if let Some(current) = &mut registration {
                    edit(
                        event,
                        &mut Input::Registration(&mut current.engine, session.dictionary()),
                        &mut current.draft,
                    )
                } else {
                    edit(event, &mut Input::Session(&mut session), &mut draft)
                };
            }
        }
    }
}
