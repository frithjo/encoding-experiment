use libc::{ioctl, winsize, STDOUT_FILENO, TIOCGWINSZ};
#[cfg(all(unix, feature = "crossterm"))]
use libc::{read, select, timeval, FD_SET, FD_ZERO, STDIN_FILENO};
#[cfg(all(unix, feature = "crossterm"))]
use std::io::{self, Write};
use std::mem;

#[cfg(feature = "crossterm")]
use crossterm::terminal;

#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl TerminalSize {
    /// Returns the pixel width/height of a single cell.
    pub fn cell_pixel_size(&self) -> (u16, u16) {
        if self.cols > 0 && self.rows > 0 && self.pixel_width > 0 && self.pixel_height > 0 {
            (self.pixel_width / self.cols, self.pixel_height / self.rows)
        } else {
            // Reasonably common default cell size
            (10, 20)
        }
    }
}

pub fn get_terminal_size() -> Option<TerminalSize> {
    terminal_size_from_ioctl_or_crossterm()
}

#[cfg(feature = "crossterm")]
pub fn get_terminal_size_with_pixel_query() -> Option<TerminalSize> {
    let mut size = terminal_size_from_ioctl_or_crossterm()?;
    if size.pixel_width > 0 && size.pixel_height > 0 {
        return Some(size);
    }

    if let Some((pixel_width, pixel_height)) = query_csi_14t_pixel_size() {
        size.pixel_width = pixel_width;
        size.pixel_height = pixel_height;
    }

    Some(size)
}

#[cfg(not(feature = "crossterm"))]
pub fn get_terminal_size_with_pixel_query() -> Option<TerminalSize> {
    get_terminal_size()
}

fn terminal_size_from_ioctl_or_crossterm() -> Option<TerminalSize> {
    // 1. Try ioctl (fastest)
    unsafe {
        let mut ws: winsize = mem::zeroed();
        if ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut ws) == 0 && ws.ws_xpixel > 0 && ws.ws_ypixel > 0 {
            return Some(TerminalSize {
                rows: ws.ws_row,
                cols: ws.ws_col,
                pixel_width: ws.ws_xpixel,
                pixel_height: ws.ws_ypixel,
            });
        }
    }

    // 2. Try crossterm (wraps ioctl usually, but might have other methods)
    #[cfg(feature = "crossterm")]
    if let Ok(ws) = terminal::window_size() {
        if ws.width > 0 && ws.height > 0 {
            return Some(TerminalSize {
                rows: ws.rows,
                cols: ws.columns,
                pixel_width: ws.width,
                pixel_height: ws.height,
            });
        }
    }

    #[cfg(feature = "crossterm")]
    if let Ok((cols, rows)) = terminal::size() {
        Some(TerminalSize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
    } else {
        None
    }

    #[cfg(not(feature = "crossterm"))]
    None
}

#[cfg(all(unix, feature = "crossterm"))]
fn query_csi_14t_pixel_size() -> Option<(u16, u16)> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(b"\x1b[14t").ok()?;
    stdout.flush().ok()?;

    let mut response = Vec::with_capacity(32);
    loop {
        let mut read_fds = unsafe { mem::zeroed() };
        unsafe {
            FD_ZERO(&mut read_fds);
            FD_SET(STDIN_FILENO, &mut read_fds);
        }
        let mut timeout = timeval {
            tv_sec: 0,
            tv_usec: 80_000,
        };
        let ready = unsafe {
            select(
                STDIN_FILENO + 1,
                &mut read_fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            )
        };
        if ready <= 0 {
            break;
        }

        let mut byte = [0u8; 1];
        let n = unsafe { read(STDIN_FILENO, byte.as_mut_ptr().cast(), 1) };
        if n <= 0 {
            break;
        }
        response.push(byte[0]);
        if byte[0] == b't' || response.len() >= 64 {
            break;
        }
    }

    parse_csi_14t_response(&response)
}

#[cfg(all(not(unix), feature = "crossterm"))]
fn query_csi_14t_pixel_size() -> Option<(u16, u16)> {
    None
}

#[cfg(any(test, feature = "crossterm"))]
fn parse_csi_14t_response(response: &[u8]) -> Option<(u16, u16)> {
    let text = std::str::from_utf8(response).ok()?;
    let start = text.find("\x1b[4;")?;
    let rest = &text[start + 4..];
    let end = rest.find('t')?;
    let mut parts = rest[..end].split(';');
    let height = parts.next()?.parse::<u16>().ok()?;
    let width = parts.next()?.parse::<u16>().ok()?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::parse_csi_14t_response;

    #[test]
    fn parses_csi_14t_pixel_response() {
        assert_eq!(
            parse_csi_14t_response(b"\x1b[4;900;1440t"),
            Some((1440, 900))
        );
    }

    #[test]
    fn rejects_malformed_csi_14t_response() {
        assert_eq!(parse_csi_14t_response(b"\x1b[4;nope;1440t"), None);
    }
}
