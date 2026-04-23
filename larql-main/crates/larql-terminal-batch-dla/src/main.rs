use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use larql_terminal_renderer::{
    AtomicGraphicsBackend, GraphicsLayer, Image, ImageWidget, Renderer, Rgb, draw_frame,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::io;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(name = "larql-terminal-batch-dla")]
#[command(about = "Terminal-based Batch DLA Scan tool with Graphics")]
struct Cli {
    /// Backend server URL
    #[arg(long, default_value = "http://localhost:8080")]
    server: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct ToolCallRequest {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
struct ToolCallResponse {
    result: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
struct AttentionData {
    layer: usize,
    heads: Vec<Vec<f32>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct BatchDlaResult {
    attention: Vec<AttentionData>,
    num_layers: usize,
    predictions: Vec<(String, f32)>,
    seq_len: usize,
    tokens: Vec<u32>,
}

async fn call_batch_dla_scan(server: &str, prompt: &str) -> Result<BatchDlaResult, String> {
    let client = reqwest::Client::new();
    let request = ToolCallRequest {
        name: "batch_dla_scan".to_string(),
        arguments: serde_json::json!({ "prompt": prompt }),
    };

    let response = client
        .post(&format!("{}/tools/call", server))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to call API: {}", e))?;

    if response.status().is_success() {
        let tool_response: ToolCallResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        serde_json::from_value(tool_response.result)
            .map_err(|e| format!("Failed to parse result: {}", e))
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        Err(format!("API error ({}): {}", status, error_text))
    }
}

fn attention_to_image(heads: &[Vec<f32>], seq_len: usize) -> Image {
    let mut img = Image::new(seq_len, seq_len);

    if heads.is_empty() || seq_len == 0 {
        return img;
    }

    let averaged_attention: Vec<f32> = (0..seq_len)
        .map(|token_idx| {
            let sum: f32 = heads
                .iter()
                .map(|head| head.get(token_idx).copied().unwrap_or(0.0))
                .sum();
            sum / heads.len() as f32
        })
        .collect();

    for (c, val) in averaged_attention.into_iter().enumerate() {
        let intensity = (val * 255.0 * 5.0).min(255.0) as u8;
        let color = Rgb::new(intensity, (intensity as f32 * 0.5) as u8, 255 - intensity);
        for r in 0..seq_len {
            img.set(c, r, color);
        }
    }

    img
}

#[cfg(test)]
mod tests {
    use super::attention_to_image;

    #[test]
    fn attention_to_image_repeats_last_token_attention_vector_across_rows() {
        let heads = vec![vec![0.02, 0.08, 0.16], vec![0.04, 0.10, 0.18]];

        let img = attention_to_image(&heads, 3);

        for row in 1..3 {
            for col in 0..3 {
                assert_eq!(
                    img.get(col, 0),
                    img.get(col, row),
                    "column {} should carry the same averaged attention value in every row",
                    col
                );
            }
        }

        assert_ne!(
            img.get(0, 0),
            img.get(1, 0),
            "distinct token attention values should remain distinguishable across columns"
        );
    }
}

struct App {
    prompt: String,
    result: Option<BatchDlaResult>,
    error: Option<String>,
    renderer: Renderer,
    heatmaps: Vec<Image>,
}

impl App {
    fn new() -> Self {
        Self {
            prompt: String::new(),
            result: None,
            error: None,
            renderer: Renderer::new_auto(),
            heatmaps: Vec::new(),
        }
    }
}

fn ui(frame: &mut Frame, app: &App, graphics_layer: &mut GraphicsLayer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status
        ])
        .split(frame.size());

    let input = Paragraph::new(format!("Prompt: {}", app.prompt))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    frame.render_widget(input, chunks[0]);

    if let Some(err) = &app.error {
        let error_widget = Paragraph::new(err.as_str())
            .block(Block::default().borders(Borders::ALL).title("Error"));
        frame.render_widget(error_widget, chunks[1]);
    } else if let Some(result) = &app.result {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Stats
                Constraint::Percentage(70), // Heatmaps
            ])
            .split(chunks[1]);

        let stats_text = format!(
            "Layers: {}\nSeq Len: {}\n\nPredictions:\n{}",
            result.num_layers,
            result.seq_len,
            result
                .predictions
                .iter()
                .map(|(t, p)| format!("  {}: {:.4}", t, p))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let stats =
            Paragraph::new(stats_text).block(Block::default().borders(Borders::ALL).title("Stats"));
        frame.render_widget(stats, content_chunks[0]);

        // Heatmap display
        let heatmap_count = app.heatmaps.len();
        if heatmap_count > 0 {
            let heatmap_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    (0..heatmap_count)
                        .map(|_| Constraint::Ratio(1, heatmap_count as u32))
                        .collect::<Vec<_>>(),
                )
                .split(content_chunks[1]);

            for (i, heatmap) in app.heatmaps.iter().enumerate() {
                let area = heatmap_chunks[i];
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Layer {}", i));
                let inner = block.inner(area);
                frame.render_widget(block, area);

                // Ratatui Widget Pass: marks area as skipped and handles ANSI fallback
                let widget = ImageWidget::new(heatmap, &app.renderer);
                frame.render_widget(widget, inner);

                // Graphics Layer Pass: queues high-res graphics for out-of-band flush
                graphics_layer.add(heatmap, inner);
            }
        }
    }

    let status = Paragraph::new("Enter: Scan | Q: Quit | Backspace: Delete")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(status, chunks[2]);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let rt = Runtime::new()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = AtomicGraphicsBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.renderer.clear_all_graphics()?;

    loop {
        draw_frame(&mut terminal, &app.renderer, |f, graphics_layer| {
            ui(f, &app, graphics_layer);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char(c) => app.prompt.push(c),
                    KeyCode::Backspace => {
                        app.prompt.pop();
                    }
                    KeyCode::Enter => {
                        let prompt = app.prompt.clone();
                        app.error = None;
                        match rt.block_on(call_batch_dla_scan(&cli.server, &prompt)) {
                            Ok(res) => {
                                app.heatmaps = res
                                    .attention
                                    .iter()
                                    .map(|a| attention_to_image(&a.heads, res.seq_len))
                                    .collect();
                                app.result = Some(res);
                            }
                            Err(e) => {
                                app.heatmaps.clear();
                                app.result = None;
                                match app.renderer.clear_all_graphics() {
                                    Ok(()) => app.error = Some(e),
                                    Err(clear_err) => {
                                        app.error = Some(format!(
                                            "{e}; additionally failed to clear stale graphics: {clear_err}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
