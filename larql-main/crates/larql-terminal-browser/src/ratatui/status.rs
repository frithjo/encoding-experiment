// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Status bar implementation.

use super::{theme, AppState, Capability};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the status bar at the bottom of the screen.
pub fn render(frame: &mut Frame, area: Rect, app_state: &AppState) {
    let mut spans = Vec::new();

    // Workspace info
    if let Some(ref ws) = app_state.workspace_info {
        spans.push(Span::styled(
            format!("{}: {}", ws.name, ws.extract_level),
            Style::default().fg(theme::ACCENT),
        ));
        spans.push(Span::raw(" | "));
    }

    // Capabilities
    let caps: Vec<&str> = app_state
        .capabilities
        .iter()
        .map(|c| match c {
            Capability::Browse => "browse",
            Capability::Infer => "infer",
            Capability::Trace => "trace",
            Capability::Mlx => "mlx",
            Capability::WalkFfn => "walk_ffn",
            Capability::Labels => "labels",
        })
        .collect();

    if !caps.is_empty() {
        spans.push(Span::styled(
            caps.join(", "),
            Style::default().fg(theme::CAP_READY),
        ));
        spans.push(Span::raw(" | "));
    }

    // Performance metrics
    if let Some(fps) = app_state.performance.fps {
        spans.push(Span::styled(
            format!("{:.1} FPS", fps),
            Style::default().fg(theme::DIM),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme::DIM)),
        )
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
