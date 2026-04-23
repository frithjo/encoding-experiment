use libc::{ioctl, winsize, STDOUT_FILENO, TIOCGWINSZ};
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

    // 3. Fallback to basic row/col only if pixel detection failed
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
