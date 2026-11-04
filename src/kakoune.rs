use std::{
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::Path,
};
use sumi_skk::{Engine, Key, dictionary::Dictionary};
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
fn keys(s: &str) -> String {
    s.replace('<', "<lt>")
}
// A private reply file carries commands. A one-byte FIFO notification cannot be
// split across reads. The ACK prevents replacement until Kakoune consumes it.
pub fn run(dict: &Dictionary, directory: &str) -> io::Result<()> {
    let dir = Path::new(directory);
    let input = OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join("keys"))?;
    let mut input = BufReader::new(input);
    let mut notify = OpenOptions::new().write(true).open(dir.join("notify"))?;
    let mut engine = Engine::default();
    let mut queued = std::collections::VecDeque::new();
    let result = (|| {
        loop {
            let mut line = if let Some(key) = queued.pop_front() {
                key
            } else {
                let mut next = String::new();
                input.read_line(&mut next)?;
                next
            };
            let raw = line.trim_end();
            if raw == "__STOP__" {
                return Ok(());
            }
            let finishing = raw == "__FINISH__";
            if finishing {
                fs::write(dir.join("reply"), "skk-cleanup\nexecute-keys <esc>\n")?;
                notify.write_all(b"\n")?;
                notify.flush()?;
                loop {
                    line.clear();
                    input.read_line(&mut line)?;
                    if line.trim_end() == "__ACK__" {
                        return Ok(());
                    }
                }
            }
            if raw == "__ACK__" || raw.is_empty() {
                continue;
            }
            let key = Key::parse(raw).map_err(io::Error::other)?;
            let result = engine.process_key(key, dict);
            let mut commands = String::new();
            if !result.committed.is_empty() {
                commands.push_str(&format!(
                    "execute-keys -- {}\n",
                    quote(&keys(&result.committed))
                ));
            }
            if let Some(key) = result.passthrough {
                let raw = match key.as_str() {
                    "Enter" => "<ret>",
                    "Backspace" => "<backspace>",
                    _ => "",
                };
                if !raw.is_empty() {
                    commands.push_str(&format!("execute-keys -- {}\n", quote(raw)));
                }
            }
            let status = format!(
                "SKK {:?} {} {}",
                result.mode,
                result.preedit,
                result.error.unwrap_or_default()
            );
            commands.push_str(&format!("echo -- {}\n", quote(&status)));
            fs::write(dir.join("reply.tmp"), commands)?;
            fs::rename(dir.join("reply.tmp"), dir.join("reply"))?;
            notify.write_all(b"\n")?;
            notify.flush()?;
            loop {
                line.clear();
                input.read_line(&mut line)?;
                match line.trim_end() {
                    "__ACK__" => break,
                    "__STOP__" => return Ok(()),
                    other => {
                        // Preserve fast key bursts while waiting for the UI acknowledgement.
                        // The queue is bounded by the FIFO and this pending collection.
                        if other.is_empty() {
                            continue;
                        }
                        if queued.len() >= 4096 {
                            return Err(io::Error::other("input queue overflow"));
                        }
                        queued.push_back(line.clone());
                    }
                }
            }
        }
    })();
    let _ = fs::remove_dir_all(dir);
    result
}
