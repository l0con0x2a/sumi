use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    process::{Command, Stdio},
};
use sumi_skk::{Engine, Key, dictionary::Dictionary};

// stty is called only on entry/exit; no terminal framework or per-key process.
struct Terminal {
    tty: File,
    saved: String,
}
impl Terminal {
    fn open() -> io::Result<Self> {
        let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let output = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::from(tty.try_clone()?))
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("stty -g failed"));
        }
        let saved = String::from_utf8(output.stdout)
            .map_err(io::Error::other)?
            .trim()
            .to_owned();
        let mut term = Self { tty, saved };
        let status = Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::from(term.tty.try_clone()?))
            .status()?;
        if !status.success() {
            return Err(io::Error::other("stty raw failed"));
        }
        write!(
            term.tty,
            "\x1b[?1049h\x1b[2J\x1b[HSKK  Enter: confirm | Ctrl-D: send | Esc/Ctrl-C: cancel\r\n"
        )?;
        Ok(term)
    }
    fn character(&mut self, first: u8) -> io::Result<char> {
        let width = match first {
            0..=0x7f => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return Err(io::Error::other("invalid UTF-8 input")),
        };
        let mut bytes = [0; 4];
        bytes[0] = first;
        self.tty.read_exact(&mut bytes[1..width])?;
        std::str::from_utf8(&bytes[..width])
            .map_err(io::Error::other)?
            .chars()
            .next()
            .ok_or_else(|| io::Error::other("empty input"))
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = write!(self.tty, "\x1b[?1049l");
        let _ = self.tty.flush();
        if let Ok(tty) = self.tty.try_clone() {
            let _ = Command::new("stty")
                .arg(&self.saved)
                .stdin(Stdio::from(tty))
                .status();
        }
    }
}
fn display_safe(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '�' } else { c })
        .collect()
}
pub fn run(dict: &Dictionary) -> io::Result<Option<String>> {
    let mut terminal = Terminal::open()?;
    let mut engine = Engine::default();
    let mut text = String::new();
    let mut message = String::new();
    loop {
        let state = engine.snapshot();
        write!(
            terminal.tty,
            "\x1b[2;1H\x1b[J{}{}\r\n{:?} {}",
            display_safe(&text),
            display_safe(&state.preedit),
            state.mode,
            display_safe(&message)
        )?;
        terminal.tty.flush()?;
        let mut byte = [0];
        terminal.tty.read_exact(&mut byte)?;
        let key = match byte[0] {
            3 | 27 => return Ok(None),
            4 => {
                if engine.is_idle() {
                    return Ok(Some(text));
                }
                message = "Confirm pending input with Enter before sending".into();
                continue;
            }
            7 => Key::CtrlG,
            10 => Key::CtrlJ,
            13 => Key::Enter,
            8 | 127 => Key::Backspace,
            32 => Key::Space,
            c if c < 32 => continue,
            c => Key::Char(terminal.character(c)?),
        };
        let result = engine.process_key(key, dict);
        text.push_str(&result.committed);
        match result.passthrough.as_deref() {
            Some("Backspace") => {
                text.pop();
            }
            Some("Enter") => text.push('\n'),
            _ => (),
        }
        message = result.error.unwrap_or_default();
    }
}
