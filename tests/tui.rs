#![cfg(all(unix, feature = "terminal"))]
use std::{
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

// An actual controlling PTY, with final output captured on a separate pipe.
// No terminal emulator, tmux, Kakoune, shell script or user configuration needed.
struct Tui {
    child: Child,
    master: File,
    initial: libc::termios,
    screen: Vec<u8>,
}
impl Tui {
    fn start() -> Self {
        Self::start_with(&[])
    }
    fn start_with(args: &[&str]) -> Self {
        let mut master = -1;
        let mut slave = -1;
        let size = libc::winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: openpty fills both fds and reads a valid winsize.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &size,
                )
            },
            0
        );
        let master = unsafe { File::from_raw_fd(master) };
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
        let slave = unsafe { File::from_raw_fd(slave) };
        let mut initial = std::mem::MaybeUninit::zeroed();
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), initial.as_mut_ptr()) },
            0
        );
        let initial = unsafe { initial.assume_init() };
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_skk"));
        cmd.args(["tui"])
            .args(args)
            .env_remove("SKK_DICTIONARY")
            .stdin(Stdio::from(slave))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: after fork, only async-signal-safe syscalls are used.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().unwrap();
        let mut tui = Self {
            child,
            master,
            initial,
            screen: Vec::new(),
        };
        tui.wait_for("SKK");
        tui
    }
    fn send(&mut self, input: &[u8]) {
        self.master.write_all(input).unwrap();
    }
    fn wait_for(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !String::from_utf8_lossy(&self.screen).contains(needle) {
            assert!(
                Instant::now() < deadline,
                "missing {needle:?}: {}",
                String::from_utf8_lossy(&self.screen)
            );
            let mut fd = libc::pollfd {
                fd: self.master.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            if unsafe { libc::poll(&mut fd, 1, 50) } > 0 {
                let mut bytes = [0; 16384];
                let n = self.master.read(&mut bytes).unwrap();
                assert!(n > 0);
                self.screen.extend_from_slice(&bytes[..n]);
            }
        }
    }
    fn finish(&mut self, expected_code: i32) -> String {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(expected_code));
                break;
            }
            assert!(Instant::now() < deadline, "TUI did not terminate");
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut stdout = String::new();
        self.child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        let mut termios = std::mem::MaybeUninit::zeroed();
        assert_eq!(
            unsafe { libc::tcgetattr(self.master.as_raw_fd(), termios.as_mut_ptr()) },
            0
        );
        let restored = unsafe { termios.assume_init() };
        assert_eq!(restored.c_iflag, self.initial.c_iflag);
        assert_eq!(restored.c_oflag, self.initial.c_oflag);
        assert_eq!(restored.c_lflag, self.initial.c_lflag);
        assert_eq!(restored.c_cc, self.initial.c_cc);
        stdout
    }
}
impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn conversion_candidates_and_utf8_cursor_editing() {
    let mut tui = Tui::start();
    tui.send(b"Kanji ");
    tui.wait_for("漢字");
    tui.wait_for("感覚");
    tui.send(b"\r\x1b[Dka\x04");
    assert_eq!(tui.finish(0), "漢か字");
}
#[test]
fn bracketed_paste_is_literal_and_can_be_edited() {
    let mut tui = Tui::start();
    tui.send("\x1b[200~Kanji 日本\r\nabc\x1b[201~\x1b[H\x1b[3~\x04".as_bytes());
    assert_eq!(tui.finish(0), "Kanji 日本\nbc");
}
#[test]
fn escape_cancels_but_navigation_does_not() {
    let mut tui = Tui::start();
    tui.send(b"ka\x1b[D\x1b");
    assert_eq!(tui.finish(0), "");
}
#[test]
fn termination_signals_restore_the_terminal_and_send_no_text() {
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        let mut tui = Tui::start();
        assert_eq!(unsafe { libc::kill(tui.child.id() as i32, signal) }, 0);
        assert_eq!(tui.finish(128 + signal), "");
    }
}
#[test]
fn small_resized_terminal_stays_usable() {
    let mut tui = Tui::start();
    let size = libc::winsize {
        ws_row: 2,
        ws_col: 8,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    assert_eq!(
        unsafe { libc::ioctl(tui.master.as_raw_fd(), libc::TIOCSWINSZ, &size) },
        0
    );
    tui.send(b"OkuRu\r\x04");
    assert_eq!(tui.finish(0), "送る");
}

#[test]
fn suspension_restores_the_terminal_and_retains_the_draft() {
    let mut tui = Tui::start();
    tui.send(b"ka\x1a");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let mut status = 0;
        let result = unsafe {
            libc::waitpid(
                tui.child.id() as i32,
                &mut status,
                libc::WUNTRACED | libc::WNOHANG,
            )
        };
        if result > 0 && libc::WIFSTOPPED(status) {
            break;
        }
        assert!(Instant::now() < deadline, "TUI did not suspend");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut termios = std::mem::MaybeUninit::zeroed();
    assert_eq!(
        unsafe { libc::tcgetattr(tui.master.as_raw_fd(), termios.as_mut_ptr()) },
        0
    );
    assert_eq!(
        unsafe { termios.assume_init() }.c_lflag,
        tui.initial.c_lflag
    );
    tui.screen.clear();
    assert_eq!(
        unsafe { libc::kill(tui.child.id() as i32, libc::SIGCONT) },
        0
    );
    tui.wait_for("\x1b[?1049h");
    tui.send(b"na\x04");
    assert_eq!(tui.finish(0), "かな");
}

#[test]
fn backspace_removes_a_complete_combined_character() {
    let mut tui = Tui::start();
    tui.send("\x1b[200~か\u{3099}👨‍👩‍👧‍👦\x1b[201~\x7f\x7fka\x04".as_bytes());
    assert_eq!(tui.finish(0), "か");
}

#[test]
fn word_registration_works_without_a_user_dictionary() {
    let mut tui = Tui::start();
    tui.send(b"Michi \x12");
    tui.wait_for("Register");
    tui.send("\x1b[200~未知\x1b[201~\rMichi \r\x04".as_bytes());
    assert_eq!(tui.finish(0), "未知未知");
}
#[test]
fn cancelling_registration_returns_to_the_original_reading() {
    let mut tui = Tui::start();
    tui.send(b"Michi \x12");
    tui.wait_for("Register");
    tui.send(b"abc\x1b");
    tui.wait_for("Registration cancelled");
    tui.send(b"\r\x04");
    assert_eq!(tui.finish(0), "みち");
}
#[test]
fn registration_can_use_existing_conversions_and_persist_okuri() {
    let directory = std::env::temp_dir().join(format!("sumi-tui-register-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("user.skk");
    let mut tui = Tui::start_with(&["--user-dict", path.to_str().unwrap()]);
    tui.send(b"OkuRu\x07\x12");
    tui.wait_for("Register");
    tui.send("\x1b[200~贈\x1b[201~\r\x04".as_bytes());
    assert_eq!(tui.finish(0), "贈る");
    let d = sumi_skk::dictionary::Dictionary::load(&path).unwrap();
    assert_eq!(d.lookup_okuri("おくr", "る")[0].text, "贈");
    std::fs::remove_dir_all(directory).unwrap();
    let mut tui = Tui::start();
    tui.send(b"Michi \x12");
    tui.wait_for("Register");
    tui.send(b"Kanji \r\x04");
    assert_eq!(tui.finish(0), "漢字");
}
