//! Standalone terminal composition. POSIX handling stays outside the SKK engine.
mod editor;
mod input;
mod screen;
mod sys;
use editor::Draft;
use input::Event;
use std::io;
use sumi_skk::{Engine, Key, Mode, Phase, RomajiRule, Update, dictionary::Dictionary};
use sys::Terminal;
pub use sys::interrupted_signal;

fn apply(draft: &mut Draft, update: Update) -> String {
    draft.insert(&update.committed);
    match update.passthrough.as_deref() {
        Some("Backspace") => draft.backspace(),
        Some("Enter") => draft.insert("\n"),
        _ => (),
    }
    update.error.unwrap_or_default()
}
fn confirm(engine: &mut Engine, draft: &mut Draft, dictionary: &Dictionary) {
    if !engine.is_idle() {
        apply(draft, engine.process_key(Key::Enter, dictionary));
    }
}
pub fn run(dictionary: &Dictionary, rule: RomajiRule) -> io::Result<Option<String>> {
    let mut terminal = Terminal::open()?;
    let mut engine = Engine::with_romaji_rule(Mode::Hiragana, rule);
    let mut draft = Draft::default();
    let mut message = String::new();
    let mut dirty = true;
    loop {
        if dirty {
            screen::draw(&mut terminal, &draft, &engine.snapshot(), &message)?;
        }
        dirty = true;
        let event = terminal.event()?;
        match event {
            Event::Cancel => return Ok(None),
            Event::Submit => {
                if engine.is_idle() {
                    return Ok(Some(draft.into_text()));
                }
                message = "Confirm pending input with Enter before sending".into();
            }
            Event::Key(key) => message = apply(&mut draft, engine.process_key(key, dictionary)),
            Event::Paste(text) => {
                if text
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
                {
                    message = "Paste contains unsupported control characters".into();
                } else {
                    confirm(&mut engine, &mut draft, dictionary);
                    draft.insert(&text);
                    message.clear();
                }
            }
            Event::Up | Event::Down if engine.snapshot().phase == Phase::SelectingCandidates => {
                message = apply(
                    &mut draft,
                    engine.process_key(
                        if event == Event::Up {
                            Key::Char('x')
                        } else {
                            Key::Space
                        },
                        dictionary,
                    ),
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
                confirm(&mut engine, &mut draft, dictionary);
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
                message.clear();
            }
            Event::Suspend => terminal.suspend()?,
            Event::Redraw => (),
            Event::Idle => dirty = false,
            Event::Unknown => message = "Unsupported key or invalid UTF-8 input".into(),
        }
    }
}
