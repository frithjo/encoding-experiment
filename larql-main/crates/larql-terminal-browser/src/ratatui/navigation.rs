// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Navigation bar implementation.

use super::{theme, AppState, Task};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the navigation bar at the top of the screen.
pub fn render(frame: &mut Frame, area: Rect, app_state: &AppState) {
    let tasks = [
        Task::Workspace,
        Task::Studio,
        Task::Explorer,
        Task::LQL,
        Task::Trace,
        Task::Recipes,
        Task::Runs,
    ];

    let mut spans = Vec::new();

    for (i, task) in tasks.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }

        let is_active = *task == app_state.active_task;
        let icon = theme::task_icon(task);
        let name = format!("{:?}", task);

        if is_active {
            spans.push(Span::styled(
                format!("{} {}", icon, name),
                Style::default()
                    .fg(theme::TASK_ACTIVE)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("{} {}", icon, name),
                Style::default().fg(theme::TASK_INACTIVE),
            ));
        }
    }

    // Add help hint
    spans.push(Span::raw(" "));
    spans.push(Span::styled("?", Style::default().fg(theme::DIM)));

    let paragraph = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme::DIM)),
        )
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}
