pub mod ansi;
mod cache;
pub mod iterm2;
pub mod kitty;
pub mod protocol;
pub mod sixel;

pub use ansi::AnsiRenderer;
pub use iterm2::ITerm2Renderer;
pub use kitty::KittyRenderer;
pub use protocol::RenderBackend;
pub use sixel::SixelRenderer;

use crate::image_buffer::Image;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Sixel,
    Kitty,
    ITerm2,
    Ansi,
}

pub struct Renderer {
    pub backend_type: BackendType,
}

impl Renderer {
    pub fn new_auto() -> Self {
        let backend_type = Self::detect_backend();
        Self { backend_type }
    }

    pub fn new_with_backend(backend_type: BackendType) -> Self {
        Self { backend_type }
    }

    fn detect_backend() -> BackendType {
        // 1. Kitty check
        if env::var("TERMINAL_EMULATOR")
            .map(|v| v == "kitty")
            .unwrap_or(false)
            || env::var("TERM")
                .map(|v| v.contains("kitty"))
                .unwrap_or(false)
        {
            return BackendType::Kitty;
        }

        // 2. ITerm2 check
        if env::var("ITERM_SESSION_ID").is_ok()
            || env::var("TERM_PROGRAM")
                .map(|v| v == "iTerm.app")
                .unwrap_or(false)
        {
            return BackendType::ITerm2;
        }

        // 3. Sixel check via explicit/known Sixel-capable TERM values.
        if let Ok(term) = env::var("TERM") {
            let term = term.to_lowercase();
            if term.contains("sixel")
                || term.contains("mlterm")
                || term == "foot"
                || term.starts_with("foot-")
            {
                return BackendType::Sixel;
            }
        }

        // 4. Fallback to ANSI
        BackendType::Ansi
    }

    pub fn render(&self, image: &Image) -> Result<(), String> {
        match self.backend_type {
            BackendType::Sixel => SixelRenderer.render(image),
            BackendType::Kitty => KittyRenderer.render(image),
            BackendType::ITerm2 => ITerm2Renderer.render(image),
            BackendType::Ansi => AnsiRenderer.render(image),
        }
    }

    pub fn render_at(&self, image: &Image, x: u16, y: u16, z_index: i32) -> Result<(), String> {
        match self.backend_type {
            BackendType::Sixel => SixelRenderer.render_at(image, x, y, z_index),
            BackendType::Kitty => KittyRenderer.render_at(image, x, y, z_index),
            BackendType::ITerm2 => ITerm2Renderer.render_at(image, x, y, z_index),
            BackendType::Ansi => AnsiRenderer.render_at(image, x, y, z_index),
        }
    }

    pub fn render_command_at(
        &self,
        image: &Image,
        x: u16,
        y: u16,
        z_index: i32,
    ) -> Result<Vec<u8>, String> {
        match self.backend_type {
            BackendType::Sixel => SixelRenderer.render_command_at(image, x, y, z_index),
            BackendType::Kitty => KittyRenderer.render_command_at(image, x, y, z_index),
            BackendType::ITerm2 => ITerm2Renderer.render_command_at(image, x, y, z_index),
            BackendType::Ansi => AnsiRenderer.render_command_at(image, x, y, z_index),
        }
    }

    pub fn render_file(&self, path: &std::path::Path) -> Result<(), String> {
        match self.backend_type {
            BackendType::Sixel => SixelRenderer.render_file(path),
            BackendType::Kitty => KittyRenderer.render_file(path),
            BackendType::ITerm2 => ITerm2Renderer.render_file(path),
            BackendType::Ansi => AnsiRenderer.render_file(path),
        }
    }

    pub fn clear_all_graphics(&self) -> Result<(), String> {
        match self.backend_type {
            BackendType::Kitty => KittyRenderer::clear_all_images(),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendType, Renderer};
    use std::env;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_terminal_env<T>(term: &str, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_term = env::var_os("TERM");
        let old_terminal_emulator = env::var_os("TERMINAL_EMULATOR");
        let old_iterm_session_id = env::var_os("ITERM_SESSION_ID");
        let old_term_program = env::var_os("TERM_PROGRAM");

        env::set_var("TERM", term);
        env::remove_var("TERMINAL_EMULATOR");
        env::remove_var("ITERM_SESSION_ID");
        env::remove_var("TERM_PROGRAM");

        let result = f();

        match old_term {
            Some(value) => env::set_var("TERM", value),
            None => env::remove_var("TERM"),
        }
        match old_terminal_emulator {
            Some(value) => env::set_var("TERMINAL_EMULATOR", value),
            None => env::remove_var("TERMINAL_EMULATOR"),
        }
        match old_iterm_session_id {
            Some(value) => env::set_var("ITERM_SESSION_ID", value),
            None => env::remove_var("ITERM_SESSION_ID"),
        }
        match old_term_program {
            Some(value) => env::set_var("TERM_PROGRAM", value),
            None => env::remove_var("TERM_PROGRAM"),
        }

        result
    }

    #[test]
    fn auto_detection_does_not_assume_generic_xterm_supports_sixel() {
        with_terminal_env("xterm-256color", || {
            assert_eq!(Renderer::new_auto().backend_type, BackendType::Ansi);
        });
    }
}
