use sumi_skk::Key;
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Key(Key),
    Submit,
    Cancel,
    Abort,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Delete,
    KillToStart,
    DeleteWord,
    Redraw,
    Idle,
    Suspend,
    Register,
    Paste(String),
    Unknown,
}
pub fn control(byte: u8) -> Event {
    match byte {
        1 => Event::Home,
        2 => Event::Left,
        3 => Event::Abort,
        4 => Event::Submit,
        5 => Event::End,
        6 => Event::Right,
        7 => Event::Key(Key::CtrlG),
        8 | 127 => Event::Key(Key::Backspace),
        9 => Event::Key(Key::Space),
        10 => Event::Key(Key::CtrlJ),
        12 => Event::Redraw,
        13 => Event::Key(Key::Enter),
        14 => Event::Down,
        16 => Event::Up,
        18 => Event::Register,
        21 => Event::KillToStart,
        23 => Event::DeleteWord,
        26 => Event::Suspend,
        _ => Event::Unknown,
    }
}
pub fn csi(sequence: &[u8]) -> Event {
    match sequence {
        b"A" => Event::Up,
        b"B" => Event::Down,
        b"C" => Event::Right,
        b"D" => Event::Left,
        b"H" | b"1~" | b"7~" => Event::Home,
        b"F" | b"4~" | b"8~" => Event::End,
        b"3~" => Event::Delete,
        b"5~" => Event::Up,
        b"6~" => Event::Down,
        _ => Event::Unknown,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_is_not_cancellation() {
        assert_eq!(csi(b"D"), Event::Left);
        assert_eq!(csi(b"3~"), Event::Delete);
        assert_eq!(csi(b"999~"), Event::Unknown);
        assert_eq!(control(3), Event::Abort);
    }
}
