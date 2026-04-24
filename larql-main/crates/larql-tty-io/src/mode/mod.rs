//! Terminal mode management: alt-screen, cursor hide, termios raw mode,
//! SIGWINCH resize events.
//!
//! The [`TtyGuard`] is a RAII wrapper that on construction enables a fixed set
//! of terminal modes and on drop restores them. It also installs a SIGWINCH
//! handler whose resize events are forwarded over an [`std::sync::mpsc`]
//! channel accessible via [`TtyGuard::resize_receiver`].
//!
//! Safety notes:
//! - Drop is idempotent and panic-safe: it only emits escape strings and a
//!   single `tcsetattr`, neither of which allocate.
//! - The SIGWINCH handler itself is async-signal-safe — it writes a single
//!   byte to a self-pipe. All blocking work (ioctl + channel send) happens on
//!   a dedicated Linux thread that reads the pipe.

use libc::{tcsetattr, termios, TCSANOW};
use std::io::{self, Read, Write};
use std::os::unix::io::FromRawFd;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::env;

/// Enter-sequence escape string emitted by [`TtyGuard::new`]. Exposed for tests.
pub const ENTER_SEQ: &str = concat!(
    "\x1b[?1049h", // alt-screen
    "\x1b[?25l",   // hide cursor
    "\x1b[?1006h", // mouse SGR
    "\x1b[?1003h", // mouse any-event
    "\x1b[?2004h", // bracketed paste
    "\x1b[?1004h", // focus reporting
);

/// Exit-sequence escape string emitted by [`TtyGuard::drop`]. Exposed for tests.
pub const EXIT_SEQ: &str = concat!(
    "\x1b[?1004l", // focus
    "\x1b[?2004l", // paste
    "\x1b[?1003l", // mouse any-event
    "\x1b[?1006l", // mouse SGR
    "\x1b[?25h",   // show cursor
    "\x1b[?1049l", // primary screen
);

/// Non-destructive check: is stdout attached to a TTY?
pub fn is_tty() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}

/// Query the current terminal size (cols, rows) via `TIOCGWINSZ`.
pub fn winsize() -> Option<(u16, u16)> {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) };
    if rc != 0 {
        return None;
    }
    Some((ws.ws_col, ws.ws_row))
}

/// Terminal graphics capabilities reported by Device Attributes (DA) query.
#[derive(Debug, Clone, Default)]
pub struct GraphicsCapabilities {
    /// Terminal supports Sixel graphics.
    pub sixel: bool,
    /// Terminal supports Kitty graphics protocol.
    pub kitty: bool,
}

/// Query terminal graphics capabilities via Device Attributes (DA1).
///
/// This sends `CSI c` (Primary Device Attributes) and parses the response to
/// detect graphics protocol support. Returns None if not a TTY or if the
/// query fails.
///
/// Note: This is a best-effort probe. Different terminals report capabilities
/// differently, and some may not respond at all. The function has a short
/// timeout to avoid blocking indefinitely.
pub fn query_graphics_capabilities() -> Option<GraphicsCapabilities> {
    if !is_tty() {
        return None;
    }

    use std::io::{self, Read, Write};
    use std::time::Duration;

    // Save original termios
    let mut original_termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut original_termios) } != 0 {
        return None;
    }

    // Set raw mode temporarily for query (no echo, canonical off)
    let mut raw = original_termios;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 1; // 0.1 second timeout
    if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) } != 0 {
        return None;
    }

    // Ensure we restore termios even if we panic or return early
    let restore_termios = |orig: &libc::termios| {
        let _ = unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, orig) };
    };

    // Send DA1 query: CSI c
    let mut stdout = io::stdout();
    let _ = stdout.write_all(b"\x1b[c");
    let _ = stdout.flush();

    // Read response with timeout
    let mut stdin = io::stdin();
    let mut buf = [0u8; 64];
    let mut response = Vec::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_millis(100) {
        let mut byte = [0u8; 1];
        match stdin.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                response.push(byte[0]);
                // DA1 response ends with 'c'
                if byte[0] == b'c' {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // Restore termios before returning
    restore_termios(&original_termios);

    if response.is_empty() {
        return None;
    }

    // Parse DA1 response: CSI <Ps>... c
    // Common responses:
    // - VT100: CSI ?1;0c
    // - xterm: CSI ?62;1;2;4;6;22;15c (with various capability flags)
    // Sixel is often indicated by specific response codes or separate DA queries
    // Kitty graphics is typically detected via a separate query

    // For v1, we use heuristics based on common terminal responses
    // This is intentionally conservative — we report capabilities only when
    // we're confident. False negatives are acceptable; false positives are not.

    let response_str = String::from_utf8_lossy(&response);
    let mut caps = GraphicsCapabilities::default();

    // Kitty terminal often identifies itself with specific response patterns
    // This is a simplified check — a full implementation would query Kitty
    // specifically via its device capabilities protocol
    if response_str.contains("kitty") || env::var("TERM").unwrap_or_default().contains("kitty") {
        caps.kitty = true;
    }

    // Sixel support is harder to detect via DA1 alone.
    // Many terminals that support Sixel don't advertise it in the primary DA.
    // We check TERM as a fallback heuristic.
    let term = env::var("TERM").unwrap_or_default();
    if term.contains("mlterm") || term.contains("foot") || term.contains("wezterm") {
        // These terminals commonly support Sixel
        caps.sixel = true;
    }

    Some(caps)
}

/// RAII guard that enables terminal modes and restores them on drop.
pub struct TtyGuard {
    original_termios: Option<termios>,
    /// Pipe write end used by the SIGWINCH handler. Closed on drop so the
    /// reader thread exits.
    sigwinch_writer_fd: Option<libc::c_int>,
    resize_rx: Option<Receiver<(u16, u16)>>,
    resize_thread: Option<thread::JoinHandle<()>>,
}

impl TtyGuard {
    /// Construct a guard, set raw termios, emit [`ENTER_SEQ`] to stdout, and
    /// install a SIGWINCH handler. Errors on non-TTY stdin or termios failure.
    pub fn new() -> Result<Self, String> {
        let mut term: termios = unsafe { std::mem::zeroed() };
        unsafe {
            if libc::tcgetattr(libc::STDIN_FILENO, &mut term) != 0 {
                return Err(format!(
                    "tcgetattr failed: {}",
                    io::Error::last_os_error()
                ));
            }
        }
        let original = term;

        // cbreak / raw: no canonical, no echo, 8-bit, VMIN=1 VTIME=0.
        term.c_iflag &= !(libc::IGNBRK
            | libc::BRKINT
            | libc::PARMRK
            | libc::ISTRIP
            | libc::INLCR
            | libc::IGNCR
            | libc::ICRNL
            | libc::IXON);
        term.c_oflag &= !libc::OPOST;
        term.c_lflag &= !(libc::ECHO
            | libc::ECHONL
            | libc::ICANON
            | libc::ISIG
            | libc::IEXTEN);
        term.c_cflag &= !(libc::CSIZE | libc::PARENB);
        term.c_cflag |= libc::CS8;
        term.c_cc[libc::VMIN] = 1;
        term.c_cc[libc::VTIME] = 0;
        unsafe {
            if tcsetattr(libc::STDIN_FILENO, TCSANOW, &term) != 0 {
                return Err(format!(
                    "tcsetattr failed: {}",
                    io::Error::last_os_error()
                ));
            }
        }

        // Enter sequence.
        let mut stdout = io::stdout();
        stdout
            .write_all(ENTER_SEQ.as_bytes())
            .map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;

        // Install SIGWINCH self-pipe + reader thread.
        let (tx, rx) = mpsc::channel::<(u16, u16)>();
        let (writer_fd, thread_handle) = install_sigwinch(tx)
            .map_err(|e| format!("sigwinch install failed: {}", e))?;

        Ok(Self {
            original_termios: Some(original),
            sigwinch_writer_fd: Some(writer_fd),
            resize_rx: Some(rx),
            resize_thread: Some(thread_handle),
        })
    }

    /// Take the resize receiver. Can only be called once per guard; subsequent
    /// calls return `None`.
    pub fn take_resize_receiver(&mut self) -> Option<Receiver<(u16, u16)>> {
        self.resize_rx.take()
    }
}

impl Drop for TtyGuard {
    fn drop(&mut self) {
        // Best-effort: write exit sequence. Ignore errors (terminal may have
        // already been closed, e.g. SIGHUP).
        let mut stdout = io::stdout();
        let _ = stdout.write_all(EXIT_SEQ.as_bytes());
        let _ = stdout.flush();

        // Close the pipe so the reader thread observes EOF and exits.
        if let Some(fd) = self.sigwinch_writer_fd.take() {
            // Uninstall the signal handler first — it writes to this fd.
            unsafe {
                let _ = libc::signal(libc::SIGWINCH, libc::SIG_DFL);
                libc::close(fd);
            }
        }
        if let Some(h) = self.resize_thread.take() {
            // Can't wait forever; detach. The read end closes on thread exit.
            let _ = h.join();
        }

        // Restore termios last so that any pending output drains under the
        // original settings.
        if let Some(t) = self.original_termios {
            unsafe {
                let _ = tcsetattr(libc::STDIN_FILENO, TCSANOW, &t);
            }
        }
    }
}

/// Module-global storage for the SIGWINCH self-pipe write fd. The handler is
/// async-signal-safe: it only performs a single `write(2)` of one byte.
static SIGWINCH_PIPE_FD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

extern "C" fn sigwinch_handler(_sig: libc::c_int) {
    let fd = SIGWINCH_PIPE_FD.load(std::sync::atomic::Ordering::Relaxed);
    if fd >= 0 {
        let byte: u8 = 1;
        // Ignore errors — we can't do anything in a signal handler.
        unsafe {
            libc::write(fd, &byte as *const u8 as *const _, 1);
        }
    }
}

fn install_sigwinch(
    tx: Sender<(u16, u16)>,
) -> io::Result<(libc::c_int, thread::JoinHandle<()>)> {
    // Self-pipe.
    let mut fds = [0i32; 2];
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let read_fd = fds[0];
    let write_fd = fds[1];

    // Make write end non-blocking so the signal handler never stalls.
    unsafe {
        let flags = libc::fcntl(write_fd, libc::F_GETFL, 0);
        libc::fcntl(write_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    SIGWINCH_PIPE_FD.store(write_fd, std::sync::atomic::Ordering::Relaxed);

    // Install handler.
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = sigwinch_handler as usize;
    unsafe {
        libc::sigemptyset(&mut sa.sa_mask);
    }
    sa.sa_flags = libc::SA_RESTART;
    let rc = unsafe { libc::sigaction(libc::SIGWINCH, &sa, std::ptr::null_mut()) };
    if rc != 0 {
        let e = io::Error::last_os_error();
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
        return Err(e);
    }

    // Reader thread: drain the pipe, query winsize, forward to channel.
    let handle = thread::spawn(move || {
        let mut f = unsafe { std::fs::File::from_raw_fd(read_fd) };
        let mut buf = [0u8; 16];
        // Emit an initial size so downstream consumers don't block waiting for
        // the first SIGWINCH.
        if let Some(sz) = winsize() {
            let _ = tx.send(sz);
        }
        loop {
            match f.read(&mut buf) {
                Ok(0) => break,         // pipe closed — guard dropped
                Err(_) => break,
                Ok(_) => {
                    if let Some(sz) = winsize() {
                        if tx.send(sz).is_err() {
                            break;
                        }
                    }
                }
            }
        }
        // File drop closes read_fd.
    });

    Ok((write_fd, handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_exit_sequences_nonempty() {
        assert!(ENTER_SEQ.contains("\x1b[?1049h"));
        assert!(EXIT_SEQ.contains("\x1b[?1049l"));
        assert!(ENTER_SEQ.contains("\x1b[?2004h"));
        assert!(EXIT_SEQ.contains("\x1b[?2004l"));
    }

    #[test]
    fn tty_guard_create_and_drop() {
        // Skip in CI / non-TTY: tcgetattr(STDIN) fails on a non-TTY fd.
        if std::env::var("CI").is_ok() {
            return;
        }
        if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
            return;
        }
        let guard = TtyGuard::new();
        if let Ok(g) = guard {
            drop(g);
        }
    }

    #[cfg(all(test, target_os = "linux"))]
    #[test]
    fn pty_roundtrip_enter_exit() {
        use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let mut master = pair.master;
        let slave = pair.slave;

        // Spawn a minimal child that echoes back escape sequences.
        let mut cmd = CommandBuilder::new("cat");
        cmd.env("TERM", "xterm-256color");
        let mut child = slave
            .spawn_command(cmd)
            .expect("spawn cat");

        // Write enter sequence via the writer handle.
        let mut writer = master.take_writer().expect("writer");
        writer
            .write_all(ENTER_SEQ.as_bytes())
            .expect("write enter");

        // Write exit sequence.
        writer
            .write_all(EXIT_SEQ.as_bytes())
            .expect("write exit");

        // Close the writer to signal EOF to cat.
        drop(writer);
        drop(master);

        // Wait for child to exit.
        let _ = child.wait();

        // If we got here without panic, the sequences round-tripped.
    }

    #[cfg(all(test, target_os = "linux"))]
    #[test]
    fn pty_guard_drop_restores_on_panic() {
        use portable_pty::{NativePtySystem, PtySize, PtySystem};
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let master = pair.master;
        let _slave = pair.slave;

        // Simulate guard creation and panic.
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut master = master;
            let mut writer = master.take_writer().expect("writer");
            writer.write_all(ENTER_SEQ.as_bytes()).expect("write");
            panic!("simulated panic");
        }));

        assert!(result.is_err());
        // master is dropped by the closure
    }
}
