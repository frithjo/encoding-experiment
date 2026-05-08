// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Ratatui-based TUI layer for layout, navigation, and keyboard handling.

pub mod layout;
pub mod navigation;
pub mod status;
pub mod theme;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Application state for the Ratatui TUI.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Current active task/page.
    pub active_task: Task,
    /// Workspace information.
    pub workspace_info: Option<WorkspaceInfo>,
    /// Capabilities available in current workspace.
    pub capabilities: Vec<Capability>,
    /// Performance metrics.
    pub performance: PerformanceMetrics,
    /// Help panel visibility.
    pub show_help: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    Workspace,
    Studio,
    Explorer,
    LQL,
    Trace,
    Recipes,
    Runs,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceInfo {
    pub name: String,
    pub extract_level: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Browse,
    Infer,
    Trace,
    Mlx,
    WalkFfn,
    Labels,
}

/// Performance metrics for the TUI.
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub fps: Option<f64>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_task: Task::Workspace,
            workspace_info: None,
            capabilities: vec![],
            performance: PerformanceMetrics { fps: None },
            show_help: false,
        }
    }
}

/// Keyboard event sent from Ratatui to bridge layer.
#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    /// Task switch (g + key).
    SwitchTask(Task),
    /// Toggle help panel.
    ToggleHelp,
    /// Enter content mode (switch to Carbonyl).
    EnterContent,
    /// Exit content mode (return to navigation).
    ExitContent,
    /// Quit application.
    Quit,
}

/// Control message sent from bridge layer to the Ratatui loop.
#[derive(Debug, Clone, Copy)]
pub enum TuiControl {
    /// Stop reading and rendering so another terminal owner can inherit stdio.
    Shutdown,
}

struct TerminalModeGuard;

impl TerminalModeGuard {
    fn enter() -> io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}

/// Run the Ratatui TUI event loop and return keyboard events via channel.
pub fn run_tui_loop(
    tx: mpsc::Sender<KeyboardEvent>,
    app_state: Arc<Mutex<AppState>>,
    control_rx: mpsc::Receiver<TuiControl>,
) -> io::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let _terminal_guard = TerminalModeGuard::enter()?;
    let mut last_frame_time = Instant::now();

    loop {
        if matches!(control_rx.try_recv(), Ok(TuiControl::Shutdown)) {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                let state = app_state.lock().unwrap().clone();
                let _ = handle_key_event(key, &state, &tx);
            }
        }

        let mut render_state = {
            let mut state = app_state.lock().unwrap();
            let now = Instant::now();
            let elapsed = now.duration_since(last_frame_time).as_secs_f64();
            if elapsed > 0.0 {
                state.performance.fps = Some(1.0 / elapsed);
            }
            last_frame_time = now;
            state.clone()
        };

        // Render frame
        terminal.draw(|f| {
            layout::render(f, &mut render_state);
        })?;
    }

    terminal.show_cursor()?;
    Ok(())
}

/// Handle keyboard events and send to bridge layer if needed.
fn handle_key_event(key: KeyEvent, app_state: &AppState, tx: &mpsc::Sender<KeyboardEvent>) -> bool {
    match key.code {
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // g + key for task switching
            // Need to wait for next key press
            false
        }
        KeyCode::Char(c) => {
            // Task switching: g + w/s/e/l/t/r/u
            // Ctrl+C for quit
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                tx.send(KeyboardEvent::Quit).unwrap();
                return true;
            }
            match c {
                'w' => tx.send(KeyboardEvent::SwitchTask(Task::Workspace)).unwrap(),
                's' => tx.send(KeyboardEvent::SwitchTask(Task::Studio)).unwrap(),
                'e' => tx.send(KeyboardEvent::SwitchTask(Task::Explorer)).unwrap(),
                'l' => tx.send(KeyboardEvent::SwitchTask(Task::LQL)).unwrap(),
                't' => tx.send(KeyboardEvent::SwitchTask(Task::Trace)).unwrap(),
                'r' => tx.send(KeyboardEvent::SwitchTask(Task::Recipes)).unwrap(),
                'u' => tx.send(KeyboardEvent::SwitchTask(Task::Runs)).unwrap(),
                '?' => tx.send(KeyboardEvent::ToggleHelp).unwrap(),
                _ => return false,
            }
            true
        }
        KeyCode::Enter => {
            tx.send(KeyboardEvent::EnterContent).unwrap();
            true
        }
        KeyCode::Esc => {
            if app_state.show_help {
                tx.send(KeyboardEvent::ToggleHelp).unwrap();
            } else {
                tx.send(KeyboardEvent::ExitContent).unwrap();
            }
            true
        }
        _ => false,
    }
}
