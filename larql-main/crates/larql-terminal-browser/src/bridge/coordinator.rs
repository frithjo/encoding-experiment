// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Render coordinator managing alternation between Ratatui and Carbonyl.

use super::{BridgeState, CarbonylProcess, RenderMode};
use crossterm::terminal;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Coordinate rendering between Ratatui and Carbonyl.
pub struct RenderCoordinator {
    bridge_state: BridgeState,
    keyboard_rx: mpsc::Receiver<crate::ratatui::KeyboardEvent>,
    ratatui_state: Arc<Mutex<crate::ratatui::AppState>>,
    tui_control_tx: mpsc::Sender<crate::ratatui::TuiControl>,
    tui_handle: Option<JoinHandle<()>>,
}

impl RenderCoordinator {
    /// Create a new render coordinator.
    pub fn new(
        bridge_state: BridgeState,
        keyboard_rx: mpsc::Receiver<crate::ratatui::KeyboardEvent>,
        ratatui_state: Arc<Mutex<crate::ratatui::AppState>>,
        tui_control_tx: mpsc::Sender<crate::ratatui::TuiControl>,
        tui_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            bridge_state,
            keyboard_rx,
            ratatui_state,
            tui_control_tx,
            tui_handle: Some(tui_handle),
        }
    }

    /// Run the render coordination loop.
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Spawn background task to fetch workspace info periodically
        let ratatui_state_clone: Arc<Mutex<crate::ratatui::AppState>> =
            Arc::clone(&self.ratatui_state);
        let bridge_state_clone = self.bridge_state.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                loop {
                    if let Ok(Some(workspace_info)) =
                        bridge_state_clone.fetch_workspace_info().await
                    {
                        let mut state = ratatui_state_clone.lock().unwrap();
                        state.workspace_info = Some(workspace_info);

                        // Fetch capabilities
                        if let Ok(capabilities) = bridge_state_clone
                            .fetch_capabilities(&state.workspace_info.as_ref().unwrap())
                            .await
                        {
                            state.capabilities = capabilities;
                        }
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
            });
        });

        loop {
            // Handle keyboard events
            if let Ok(event) = self.keyboard_rx.try_recv() {
                self.handle_keyboard_event(event)?;
            }

            // Render based on current mode
            match self.bridge_state.mode {
                RenderMode::Navigation => {
                    // Ratatui handles rendering in its own loop
                    // We just need to ensure Carbonyl is suspended
                    self.suspend_carbonyl_if_needed()?;
                }
                RenderMode::Content => {
                    // Carbonyl handles rendering
                    // We need to exit Ratatui alternate screen
                    self.enter_carbonyl_mode()?;
                }
            }

            // Small delay to prevent busy-waiting
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Handle keyboard events from Ratatui.
    fn handle_keyboard_event(
        &mut self,
        event: crate::ratatui::KeyboardEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            crate::ratatui::KeyboardEvent::SwitchTask(task) => {
                let old_task = self.bridge_state.active_task;
                self.bridge_state.switch_task(task);
                self.with_ratatui_state(|state| {
                    state.active_task = task;
                });
                eprintln!(
                    "[TELEMETRY] Task switch: {:?} -> {:?} ({}ms)",
                    old_task, task, self.bridge_state.performance.mode_switch_time_ms
                );
                // If in content mode, need to reload Carbonyl with new URL
                if self.bridge_state.mode == RenderMode::Content {
                    self.reload_carbonyl()?;
                }
            }
            crate::ratatui::KeyboardEvent::EnterContent => {
                let old_mode = self.bridge_state.mode;
                self.bridge_state.switch_mode(RenderMode::Content);
                eprintln!(
                    "[TELEMETRY] Mode switch: {:?} -> {:?} ({}ms)",
                    old_mode,
                    RenderMode::Content,
                    self.bridge_state.performance.mode_switch_time_ms
                );
            }
            crate::ratatui::KeyboardEvent::ExitContent => {
                let old_mode = self.bridge_state.mode;
                self.bridge_state.switch_mode(RenderMode::Navigation);
                eprintln!(
                    "[TELEMETRY] Mode switch: {:?} -> {:?} ({}ms)",
                    old_mode,
                    RenderMode::Navigation,
                    self.bridge_state.performance.mode_switch_time_ms
                );
            }
            crate::ratatui::KeyboardEvent::Quit => {
                eprintln!("[TELEMETRY] Quit requested");
                // Clean up and exit
                self.cleanup()?;
                std::process::exit(0);
            }
            crate::ratatui::KeyboardEvent::ToggleHelp => {
                self.with_ratatui_state(|state| {
                    state.show_help = !state.show_help;
                });
            }
        }
        Ok(())
    }

    fn with_ratatui_state(&self, update: impl FnOnce(&mut crate::ratatui::AppState)) {
        if let Ok(mut state) = self.ratatui_state.lock() {
            update(&mut state);
        }
    }

    /// Suspend Carbonyl if it's running and not already suspended.
    fn suspend_carbonyl_if_needed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.bridge_state.is_carbonyl_running() && !self.bridge_state.is_carbonyl_suspended() {
            // Send SIGSTOP to Carbonyl process
            if let Some(proc) = self.bridge_state.carbonyl_process {
                let pid = proc.pid;
                let start = std::time::Instant::now();
                signal::kill(Pid::from_raw(pid as i32), Signal::SIGSTOP)?;
                self.bridge_state.carbonyl_process = Some(CarbonylProcess {
                    pid,
                    suspended: true,
                });
                self.bridge_state.performance.carbonyl_suspend_time_ms =
                    start.elapsed().as_millis() as u64;
                eprintln!(
                    "[TELEMETRY] Carbonyl suspended: PID={}, suspend_time={}ms",
                    pid, self.bridge_state.performance.carbonyl_suspend_time_ms
                );
            }
        }
        Ok(())
    }

    /// Resume Carbonyl if it's suspended.
    fn resume_carbonyl_if_needed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.bridge_state.is_carbonyl_running() && self.bridge_state.is_carbonyl_suspended() {
            // Send SIGCONT to Carbonyl process
            if let Some(proc) = self.bridge_state.carbonyl_process {
                let pid = proc.pid;
                let start = std::time::Instant::now();
                signal::kill(Pid::from_raw(pid as i32), Signal::SIGCONT)?;
                self.bridge_state.carbonyl_process = Some(CarbonylProcess {
                    pid,
                    suspended: false,
                });
                self.bridge_state.performance.carbonyl_resume_time_ms =
                    start.elapsed().as_millis() as u64;
                eprintln!(
                    "[TELEMETRY] Carbonyl resumed: PID={}, resume_time={}ms",
                    pid, self.bridge_state.performance.carbonyl_resume_time_ms
                );
            }
        }
        Ok(())
    }

    /// Enter Carbonyl rendering mode.
    fn enter_carbonyl_mode(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop_ratatui_for_carbonyl()?;

        // Resume Carbonyl if suspended
        self.resume_carbonyl_if_needed()?;

        // If Carbonyl not running, start it
        if !self.bridge_state.is_carbonyl_running() {
            self.start_carbonyl()?;
        }

        Ok(())
    }

    fn stop_ratatui_for_carbonyl(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop_ratatui_thread()?;
        terminal::disable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        )?;
        Ok(())
    }

    fn stop_ratatui_thread(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(handle) = self.tui_handle.take() {
            let _ = self
                .tui_control_tx
                .send(crate::ratatui::TuiControl::Shutdown);
            handle.join().map_err(|_| "Ratatui TUI thread panicked")?;
        }

        Ok(())
    }

    /// Start Carbonyl process.
    fn start_carbonyl(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        let url = self.bridge_state.task_url();

        // Get Carbonyl binary path
        let carbonyl_path = self
            .bridge_state
            .carbonyl_path
            .as_ref()
            .ok_or("Carbonyl path not set")?;

        eprintln!(
            "[TELEMETRY] Starting Carbonyl: {} with URL: {}",
            carbonyl_path.display(),
            url
        );

        // Spawn Carbonyl process
        let child = Command::new(carbonyl_path)
            .arg(&url)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let pid = child.id();

        // Store process info
        self.bridge_state.carbonyl_process = Some(CarbonylProcess {
            pid,
            suspended: false,
        });

        self.bridge_state.performance.carbonyl_startup_time_ms = start.elapsed().as_millis() as u64;

        eprintln!(
            "[TELEMETRY] Carbonyl started: PID={}, startup_time={}ms",
            pid, self.bridge_state.performance.carbonyl_startup_time_ms
        );

        Ok(())
    }

    /// Reload Carbonyl with new URL (when switching tasks).
    fn reload_carbonyl(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // For now, kill existing Carbonyl and start new one with updated URL
        if self.bridge_state.is_carbonyl_running() {
            if let Some(ref proc) = self.bridge_state.carbonyl_process {
                signal::kill(Pid::from_raw(proc.pid as i32), Signal::SIGKILL)?;
            }
            self.bridge_state.carbonyl_process = None;
        }

        self.start_carbonyl()?;

        Ok(())
    }

    /// Clean up resources before exit.
    fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Kill Carbonyl process if running
        if self.bridge_state.is_carbonyl_running() {
            if let Some(ref proc) = self.bridge_state.carbonyl_process {
                signal::kill(Pid::from_raw(proc.pid as i32), Signal::SIGKILL)?;
            }
        }

        // Restore terminal state
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(std::io::stdout(), terminal::LeaveAlternateScreen)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ratatui::{AppState, KeyboardEvent, Task, TuiControl};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    fn coordinator_with_dummy_tui() -> (RenderCoordinator, Arc<Mutex<AppState>>) {
        let (_keyboard_tx, keyboard_rx) = mpsc::channel();
        let (control_tx, control_rx) = mpsc::channel::<TuiControl>();
        let state = Arc::new(Mutex::new(AppState::default()));
        let handle = std::thread::spawn(move || {
            let _ = control_rx.recv();
        });

        let coordinator = RenderCoordinator::new(
            BridgeState::default(),
            keyboard_rx,
            Arc::clone(&state),
            control_tx,
            handle,
        );

        (coordinator, state)
    }

    #[test]
    fn task_switch_updates_shared_ratatui_state() {
        let (mut coordinator, state) = coordinator_with_dummy_tui();

        coordinator
            .handle_keyboard_event(KeyboardEvent::SwitchTask(Task::Trace))
            .unwrap();

        assert_eq!(coordinator.bridge_state.active_task, Task::Trace);
        assert_eq!(state.lock().unwrap().active_task, Task::Trace);
    }

    #[test]
    fn help_toggle_updates_shared_ratatui_state() {
        let (mut coordinator, state) = coordinator_with_dummy_tui();

        coordinator
            .handle_keyboard_event(KeyboardEvent::ToggleHelp)
            .unwrap();

        assert!(state.lock().unwrap().show_help);
    }

    #[test]
    fn carbonyl_handoff_stops_tui_thread() {
        let (mut coordinator, _state) = coordinator_with_dummy_tui();

        coordinator.stop_ratatui_thread().unwrap();

        assert!(coordinator.tui_handle.is_none());
    }
}
