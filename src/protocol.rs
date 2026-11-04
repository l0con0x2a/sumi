use crate::{Engine, Key, Update, dictionary::Dictionary, session::Session};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: Value,
    op: Option<String>,
    key: Option<String>,
    index: Option<usize>,
}
enum Action {
    Key(Key),
    State,
    Reset,
    Commit,
    Select(usize),
}
impl Request {
    fn action(&self) -> Result<Action, (&'static str, String)> {
        match (
            self.op.as_deref().unwrap_or("key"),
            self.key.as_deref(),
            self.index,
        ) {
            ("key", Some(key), None) => Key::parse(key)
                .map(Action::Key)
                .map_err(|e| ("invalid_key", e)),
            ("state", None, None) => Ok(Action::State),
            ("reset", None, None) => Ok(Action::Reset),
            ("commit", None, None) => Ok(Action::Commit),
            ("select", None, Some(index)) => Ok(Action::Select(index)),
            _ => Err((
                "invalid_request",
                "expected key, state, reset, commit, or select with the matching fields".into(),
            )),
        }
    }
}
fn parse(line: &str) -> Result<(Value, Action), Value> {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Err(
                json!({"id": null, "error": {"code":"invalid_json", "message":e.to_string()}}),
            );
        }
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    let req: Request = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(e) => {
            return Err(
                json!({"id":id,"error":{"code":"invalid_request","message":e.to_string()}}),
            );
        }
    };
    if !req.id.is_string() && !req.id.is_u64() {
        return Err(
            json!({"id":id,"error":{"code":"invalid_request","message":"id must be a string or unsigned integer"}}),
        );
    }
    let action = req
        .action()
        .map_err(|(code, message)| json!({"id":id,"error":{"code":code,"message":message}}))?;
    Ok((req.id, action))
}
fn execute(engine: &mut Engine, dict: &Dictionary, action: Action) -> Update {
    match action {
        Action::Key(key) => engine.process_key(key, dict),
        Action::State => engine.snapshot(),
        Action::Reset => {
            engine.reset();
            engine.snapshot()
        }
        Action::Commit => engine.commit(dict),
        Action::Select(index) => engine.select_candidate(index),
    }
}
// Invalid requests are fully checked before any input state changes.
pub fn respond(engine: &mut Engine, dict: &Dictionary, line: &str) -> Value {
    match parse(line) {
        Ok((id, action)) => json!({"id":id,"result":execute(engine,dict,action)}),
        Err(error) => error,
    }
}
pub fn respond_session(session: &mut Session, line: &str) -> Value {
    match parse(line) {
        Ok((id, action)) => {
            let update = execute(&mut session.engine, &session.dictionary, action);
            json!({"id":id,"result":session.learn(update)})
        }
        Err(error) => error,
    }
}
pub fn serve(input: impl BufRead, mut output: impl Write, dict: &Dictionary) -> io::Result<()> {
    serve_with_engine(input, &mut output, dict, Engine::default())
}
/// Run a session with a caller-configured, independently owned engine.
pub fn serve_with_engine(
    input: impl BufRead,
    mut output: impl Write,
    dict: &Dictionary,
    mut engine: Engine,
) -> io::Result<()> {
    stream(input, &mut output, |line| respond(&mut engine, dict, line))
}
/// Run a session with optional user dictionary learning.
pub fn serve_session(
    input: impl BufRead,
    output: impl Write,
    mut session: Session,
) -> io::Result<()> {
    stream(input, output, |line| respond_session(&mut session, line))
}
fn stream(
    input: impl BufRead,
    mut output: impl Write,
    mut respond: impl FnMut(&str) -> Value,
) -> io::Result<()> {
    // Bound each frame, including malformed input, to avoid unbounded allocation.
    let mut input = input;
    loop {
        let mut frame = Vec::new();
        let mut oversized = false;
        loop {
            let chunk = input.fill_buf()?;
            if chunk.is_empty() {
                break;
            }
            let count = chunk
                .iter()
                .position(|b| *b == b'\n')
                .map_or(chunk.len(), |i| i + 1);
            if frame.len() + count <= 65536 && !oversized {
                frame.extend_from_slice(&chunk[..count]);
            } else {
                oversized = true;
                frame.clear();
            }
            let end = chunk[count - 1] == b'\n';
            input.consume(count);
            if end {
                break;
            }
        }
        if frame.is_empty() && !oversized {
            break;
        }
        let response = if oversized {
            json!({"id":null,"error":{"code":"frame_too_large","message":"maximum frame size is 65536 bytes"}})
        } else {
            match std::str::from_utf8(&frame) {
                Ok(line) => respond(line),
                Err(_) => {
                    json!({"id":null,"error":{"code":"invalid_utf8","message":"expected UTF-8"}})
                }
            }
        };
        serde_json::to_writer(&mut output, &response)?;
        writeln!(output)?;
        output.flush()?;
    }
    Ok(())
}
