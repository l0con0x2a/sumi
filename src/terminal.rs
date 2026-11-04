//! Standalone terminal composition. POSIX handling stays outside the SKK engine.
mod editor;
mod input;
mod screen;
mod sys;
use editor::Draft;
use input::Event;
use std::io;
use sumi_skk::{Key, Phase, Update, session::Session};
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
fn confirm(session: &mut Session, draft: &mut Draft) -> String {
    apply(draft, session.commit())
}
pub fn run(mut session: Session) -> io::Result<Option<String>> {
    let mut terminal = Terminal::open()?;
    let mut draft = Draft::default();
    let mut message = String::new();
    let mut dirty = true;
    loop {
        if dirty {
            screen::draw(&mut terminal, &draft, &session.snapshot(), &message)?;
        }
        dirty = true;
        let event = terminal.event()?;
        match event {
            Event::Cancel => return Ok(None),
            Event::Submit => {
                if session.is_idle() {
                    return Ok(Some(draft.into_text()));
                }
                message = "Confirm pending input with Enter before sending".into();
            }
            Event::Key(key) => message = apply(&mut draft, session.process_key(key)),
            Event::Paste(text) => {
                if text
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
                {
                    message = "Paste contains unsupported control characters".into();
                } else {
                    message = confirm(&mut session, &mut draft);
                    draft.insert(&text);
                }
            }
            Event::Up | Event::Down if session.snapshot().phase == Phase::SelectingCandidates => {
                message = apply(
                    &mut draft,
                    session.process_key(if event == Event::Up {
                        Key::Char('x')
                    } else {
                        Key::Space
                    }),
                );
            }
            Event::Left
            | Event::Right
            | Event::Home
            | Event::End
            | Event::Up
            | Event::Down
            | Event::Delete
            | Event::KillToStart
            | Event::DeleteWord => {
                message = confirm(&mut session, &mut draft);
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
            }
            Event::Suspend => terminal.suspend()?,
            Event::Redraw => (),
            Event::Idle => dirty = false,
            Event::Unknown => message = "Unsupported key or invalid UTF-8 input".into(),
        }
    }
}
