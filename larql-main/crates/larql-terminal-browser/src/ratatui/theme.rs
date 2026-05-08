// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Theme colors and styling constants for the Ratatui TUI.

use ratatui::style::Color;

/// Accent color for the TUI.
pub const ACCENT: Color = Color::Rgb(255, 127, 80); // Rust orange

/// Dimmed foreground color.
pub const DIM: Color = Color::Rgb(150, 150, 150);

/// Capability color when ready.
pub const CAP_READY: Color = Color::Green;

/// Task color when active.
pub const TASK_ACTIVE: Color = ACCENT;

/// Task color when inactive.
pub const TASK_INACTIVE: Color = DIM;

/// Get the icon for a task.
pub fn task_icon(task: &super::Task) -> &str {
    match task {
        super::Task::Workspace => "📁",
        super::Task::Studio => "🎨",
        super::Task::Explorer => "🔍",
        super::Task::LQL => "💬",
        super::Task::Trace => "📊",
        super::Task::Recipes => "📝",
        super::Task::Runs => "▶️",
    }
}
