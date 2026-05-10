// Copyright 2025 LARQL Project
// SPDX-License-Identifier: Apache-2.0

//! Layout management for the hybrid TUI.

use super::{navigation, status, AppState};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::Frame;

/// Main layout areas.
pub struct AppLayout {
    /// Navigation bar (top).
    pub nav: Rect,
    /// Main content area (middle).
    pub content: Rect,
    /// Status bar (bottom).
    pub status: Rect,
    /// Help panel (overlay).
    pub help: Option<Rect>,
}

/// Compute layout based on viewport size and app state.
pub fn compute(area: Rect, app_state: &AppState) -> AppLayout {
    let show_help = app_state.show_help;

    if area.height < 10 {
        // Ultra-compact: minimal layout
        let [nav, content, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
        ])
        .areas(area);

        AppLayout {
            nav,
            content,
            status,
            help: None,
        }
    } else if show_help {
        // Help panel overlay
        let [nav, content, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .areas(area);

        // Help panel takes 60% of content area
        let help_height = (content.height as f32 * 0.6) as u16;
        let help_width = (content.width as f32 * 0.8) as u16;
        let help_x = content.x + (content.width - help_width) / 2;
        let help_y = content.y + (content.height - help_height) / 2;

        AppLayout {
            nav,
            content,
            status,
            help: Some(Rect {
                x: help_x,
                y: help_y,
                width: help_width,
                height: help_height,
            }),
        }
    } else {
        // Standard layout
        let [nav, content, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .areas(area);

        AppLayout {
            nav,
            content,
            status,
            help: None,
        }
    }
}

/// Render the complete layout.
pub fn render(frame: &mut Frame, app_state: &mut AppState) {
    let area = frame.area();
    let layout = compute(area, app_state);

    // Render navigation bar
    navigation::render(frame, layout.nav, app_state);

    // Render status bar
    status::render(frame, layout.status, app_state);

    // Render help panel if visible
    if let Some(help_area) = layout.help {
        render_help_panel(frame, help_area, app_state);
    }

    // Render content area placeholder
    render_content_placeholder(frame, layout.content, app_state);
}

/// Render help panel overlay.
fn render_help_panel(frame: &mut Frame, area: Rect, _app_state: &AppState) {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(super::theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  g + w  Workspace"),
        Line::from("  g + s  Studio"),
        Line::from("  g + e  Explorer"),
        Line::from("  g + l  LQL"),
        Line::from("  g + t  Trace"),
        Line::from("  g + r  Recipes"),
        Line::from("  g + u  Runs"),
        Line::from(""),
        Line::from("General:"),
        Line::from("  Enter  Enter content mode (Carbonyl)"),
        Line::from("  Esc    Return to navigation / close help"),
        Line::from("  ?      Toggle help panel"),
        Line::from("  Ctrl+C Quit"),
        Line::from(""),
        Line::from("Content Mode (Carbonyl):"),
        Line::from("  Ctrl+Enter  Submit form"),
        Line::from("  Alt+E       Export"),
        Line::from("  Alt+I       Import"),
        Line::from("  Esc         Return to navigation"),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(super::theme::DIM))
                .title("Help"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Render content area placeholder (will be replaced by Carbonyl).
fn render_content_placeholder(frame: &mut Frame, area: Rect, app_state: &AppState) {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

    let task_name = format!("{:?}", app_state.active_task);
    let placeholder = vec![
        Line::from(Span::styled(
            format!("{} - Carbonyl Content Area", task_name),
            Style::default()
                .fg(super::theme::DIM)
                .add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to switch to Carbonyl rendering",
            Style::default().fg(super::theme::ACCENT),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Carbonyl will render HTML/CSS/JS content here",
            Style::default().fg(super::theme::DIM),
        )),
    ];

    let paragraph = Paragraph::new(placeholder)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(super::theme::DIM)),
        )
        .wrap(Wrap { trim: false })
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(paragraph, area);
}
