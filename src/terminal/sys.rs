use super::input::{self, Event};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::fd::AsRawFd,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    time::{Duration, Instant},
};
use sumi_skk::Key;
static SIGNAL: AtomicI32 = AtomicI32::new(0);
static SUSPEND: AtomicBool = AtomicBool::new(false);
static REDRAW: AtomicBool = AtomicBool::new(false);
extern "C" fn signal_handler(signal: libc::c_int) {
    if signal == libc::SIGWINCH {
        REDRAW.store(true, Ordering::Relaxed);
        return;
    }
    if signal == libc::SIGTSTP {
        SUSPEND.store(true, Ordering::Relaxed);
        return;
    }
    let _ = SIGNAL.compare_exchange(0, signal, Ordering::Relaxed, Ordering::Relaxed);
}
pub fn interrupted_signal() -> Option<i32> {
    match SIGNAL.load(Ordering::Relaxed) {
        0 => None,
        signal => Some(signal),
    }
}
struct Signals(Vec<(i32, libc::sigaction)>);
impl Signals {
    fn install() -> io::Result<Self> {
        SIGNAL.store(0, Ordering::Relaxed);
        SUSPEND.store(false, Ordering::Relaxed);
        let mut guard = Self(Vec::new());
        for signal in [
            libc::SIGINT,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGQUIT,
            libc::SIGWINCH,
            libc::SIGTSTP,
        ] {
            // SAFETY: the initialized action uses a signal-safe atomic-only handler;
            // sigaction fills old before it is read. Guard restores earlier actions.
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = signal_handler as *const () as usize;
                libc::sigemptyset(&mut action.sa_mask);
                let mut old = std::mem::MaybeUninit::zeroed();
                if libc::sigaction(signal, &action, old.as_mut_ptr()) < 0 {
                    return Err(io::Error::last_os_error());
                }
                guard.0.push((signal, old.assume_init()));
            }
        }
        Ok(guard)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for (signal, action) in self.0.iter().rev() {
            // SAFETY: action is a previously returned valid sigaction.
            unsafe {
                libc::sigaction(*signal, action, std::ptr::null_mut());
            }
        }
    }
}
pub struct Terminal {
    pub tty: File,
    saved: libc::termios,
    _signals: Signals,
    active: bool,
}
impl Terminal {
    pub fn open() -> io::Result<Self> {
        let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        // Some libc implementations leave unused c_cc elements untouched.
        // Zero the full struct before tcgetattr, rather than assuming it fills
        // every Rust field (including unused control-character slots).
        let mut saved = std::mem::MaybeUninit::zeroed();
        // SAFETY: tty is open, and tcgetattr initializes saved on success.
        if unsafe { libc::tcgetattr(tty.as_raw_fd(), saved.as_mut_ptr()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut terminal = Self {
            tty,
            saved: unsafe { saved.assume_init() },
            _signals: Signals::install()?,
            active: false,
        };
        terminal.activate()?;
        Ok(terminal)
    }
    fn activate(&mut self) -> io::Result<()> {
        let mut raw = self.saved;
        // SAFETY: termios came from tcgetattr; fd is an open terminal.
        unsafe {
            libc::cfmakeraw(&mut raw);
            if libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &raw) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        self.active = true;
        write!(self.tty, "\x1b[?1049h\x1b[?2004h\x1b[2J\x1b[H")?;
        self.tty.flush()
    }
    fn restore(&mut self) {
        if !self.active {
            return;
        }
        let _ = write!(self.tty, "\x1b[?2004l\x1b[?25h\x1b[?1049l");
        let _ = self.tty.flush();
        // SAFETY: saved termios and terminal fd remain valid throughout Drop.
        unsafe {
            libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &self.saved);
        }
        self.active = false;
    }
    pub fn suspend(&mut self) -> io::Result<()> {
        self.restore();
        // SAFETY: restore first, then stop the foreground job's process group,
        // so shell pipelines suspend together. SIGCONT resumes with the draft.
        if unsafe { libc::kill(0, libc::SIGSTOP) } != 0 {
            return Err(io::Error::last_os_error());
        }
        self.activate()?;
        REDRAW.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub fn size(&self) -> (usize, usize) {
        let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
        // SAFETY: ioctl writes a winsize when successful.
        if unsafe { libc::ioctl(self.tty.as_raw_fd(), libc::TIOCGWINSZ, size.as_mut_ptr()) } == 0 {
            let size = unsafe { size.assume_init() };
            if size.ws_col > 0 && size.ws_row > 0 {
                return (
                    (size.ws_col as usize).clamp(2, 1024),
                    (size.ws_row as usize).clamp(2, 1024),
                );
            }
        }
        (80, 24)
    }
    fn byte(&mut self, timeout: Duration) -> io::Result<Option<u8>> {
        let deadline = Instant::now() + timeout;
        loop {
            if interrupted_signal().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "input interrupted",
                ));
            }
            if SUSPEND.swap(false, Ordering::Relaxed) {
                self.suspend()?;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let mut fd = libc::pollfd {
                fd: self.tty.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll borrows one live descriptor for the call only.
            let result = unsafe {
                libc::poll(
                    &mut fd,
                    1,
                    remaining.as_millis().min(i32::MAX as u128) as i32,
                )
            };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if result == 0 {
                return Ok(None);
            }
            if fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "terminal disconnected",
                ));
            }
            let mut byte = [0];
            match self.tty.read(&mut byte) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "terminal closed",
                    ));
                }
                Ok(_) => return Ok(Some(byte[0])),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }
    fn next_byte(&mut self) -> io::Result<Option<u8>> {
        self.byte(Duration::from_millis(100))
    }
    fn escape(&mut self) -> io::Result<Event> {
        let Some(first) = self.byte(Duration::from_millis(35))? else {
            return Ok(Event::Cancel);
        };
        if first != b'[' && first != b'O' {
            return Ok(Event::Unknown);
        }
        let mut sequence = Vec::new();
        for _ in 0..64 {
            let Some(byte) = self.next_byte()? else {
                return Ok(Event::Unknown);
            };
            sequence.push(byte);
            if (0x40..=0x7e).contains(&byte) {
                if sequence == b"200~" {
                    return self.paste();
                }
                return Ok(input::csi(&sequence));
            }
        }
        Ok(Event::Unknown)
    }
    fn paste(&mut self) -> io::Result<Event> {
        const END: &[u8] = b"\x1b[201~";
        let mut bytes = Vec::new();
        loop {
            let Some(byte) = self.byte(Duration::from_secs(1))? else {
                return Ok(Event::Unknown);
            };
            bytes.push(byte);
            if bytes.ends_with(END) {
                bytes.truncate(bytes.len() - END.len());
                return Ok(String::from_utf8(bytes).map_or(Event::Unknown, |s| {
                    Event::Paste(s.replace("\r\n", "\n").replace('\r', "\n"))
                }));
            }
            if bytes.len() > 1_048_576 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "paste exceeds 1 MiB",
                ));
            }
        }
    }
    pub fn event(&mut self) -> io::Result<Event> {
        if REDRAW.swap(false, Ordering::Relaxed) {
            return Ok(Event::Redraw);
        }
        let Some(first) = self.byte(Duration::from_millis(100))? else {
            return Ok(if REDRAW.swap(false, Ordering::Relaxed) {
                Event::Redraw
            } else {
                Event::Idle
            });
        };
        match first {
            27 => self.escape(),
            0..=31 | 127 => Ok(input::control(first)),
            32..=126 => Ok(Event::Key(if first == b' ' {
                Key::Space
            } else {
                Key::Char(first as char)
            })),
            _ => {
                let width = match first {
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => return Ok(Event::Unknown),
                };
                let mut bytes = vec![first];
                for _ in 1..width {
                    let Some(byte) = self.next_byte()? else {
                        return Ok(Event::Unknown);
                    };
                    bytes.push(byte);
                }
                Ok(std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| s.chars().next())
                    .map_or(Event::Unknown, |c| Event::Key(Key::Char(c))))
            }
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.restore();
    }
}
