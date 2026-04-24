use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent};
use larql_lql::{parse as parse_lql, Session};
use larql_terminal_renderer::{GraphicsLayer, Image, ImageWidget, Renderer, Rgb};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::io;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(name = "larql-terminal-batch-dla")]
#[command(about = "Terminal-based Batch DLA Scan tool with Graphics")]
pub struct Cli {
    #[arg(long, default_value = "http://localhost:8080")]
    pub server: String,
}

// Re-export from larql-inference for type compatibility
pub use larql_inference::analysis::{
    AttentionLayer as AttentionData,
    LogitLensLayer as LogitLensData,
    HeadDlaLayer,
    HeadContribution,
    StepTopHeadSummary,
    TokenAnalysis,
    FirstFalseOrigin,
    AnalysisSummary,
    GeneratedStep,
    LayerRidge as RidgeByLayer,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct CircuitHighlight {
    pub layer: usize,
    pub head: usize,
    pub from_token: usize,
    pub to_token: usize,
    pub color_rgb: (u8, u8, u8),
}


// BatchDlaResult: TUI's result type, aligned with larql-inference's AnalysisResult
// for single-pass JSON deserialization. Adds TUI-specific 'circuits' field.
#[derive(Clone, Serialize, Deserialize)]
pub struct BatchDlaResult {
    pub attention: Vec<AttentionData>,
    #[serde(default)]
    pub logit_lens: Option<Vec<LogitLensData>>,
    #[serde(default)]
    pub head_dla: Vec<HeadDlaLayer>,
    pub num_layers: usize,
    pub predictions: Vec<(String, f64)>,
    pub seq_len: usize,
    pub tokens: Vec<u32>,
    #[serde(default)]
    pub strings: Option<Vec<String>>,
    #[serde(default)]
    pub circuits: Option<Vec<CircuitHighlight>>,
    #[serde(default)]
    pub generation_trace: Vec<GeneratedStep>,
    #[serde(default)]
    pub token_analysis: Vec<TokenAnalysis>,
    #[serde(default)]
    pub analysis_summary: Option<AnalysisSummary>,
    #[serde(default)]
    pub ridge_by_layer: Vec<RidgeByLayer>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecipeUiDefaults {
    pub layer: Option<usize>,
    pub head: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecipeAnalysis {
    pub mode: String,
    pub truth_spans: Vec<String>,
    pub materially_false_spans: Vec<String>,
    pub coherence_markers: Vec<String>,
    pub max_generated_tokens: Option<usize>,
    pub ridge_dead_zone: Option<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub prompt: String,
    pub description: Option<String>,
    pub ui_defaults: RecipeUiDefaults,
    pub analysis: RecipeAnalysis,
}

#[derive(Clone)]
pub struct ManualTargetDraft {
    pub prompt: String,
    pub mode: String,
    pub truth_spans: String,
    pub materially_false_spans: String,
    pub coherence_markers: String,
    pub max_generated_tokens: String,
    pub ridge_dead_zone: String,
}

impl Default for ManualTargetDraft {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            mode: "fact_probe".to_string(),
            truth_spans: String::new(),
            materially_false_spans: String::new(),
            coherence_markers: String::new(),
            max_generated_tokens: "1".to_string(),
            ridge_dead_zone: "0.05".to_string(),
        }
    }
}

impl ManualTargetDraft {
    fn from_recipe(recipe: &Recipe) -> Self {
        Self {
            prompt: recipe.prompt.clone(),
            mode: recipe.analysis.mode.clone(),
            truth_spans: recipe.analysis.truth_spans.join(", "),
            materially_false_spans: recipe.analysis.materially_false_spans.join(", "),
            coherence_markers: recipe.analysis.coherence_markers.join(", "),
            max_generated_tokens: recipe
                .analysis
                .max_generated_tokens
                .map(|v| v.to_string())
                .unwrap_or_default(),
            ridge_dead_zone: recipe
                .analysis
                .ridge_dead_zone
                .map(|v| v.to_string())
                .unwrap_or_else(|| "0.05".to_string()),
        }
    }

    fn split_csv(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn build_analysis(&self) -> RecipeAnalysis {
        let truth_spans = Self::split_csv(&self.truth_spans);
        let materially_false_spans = Self::split_csv(&self.materially_false_spans);
        let coherence_markers = Self::split_csv(&self.coherence_markers);
        let max_generated_tokens = self.max_generated_tokens.trim().parse::<usize>().ok();
        let ridge_dead_zone = self.ridge_dead_zone.trim().parse::<f32>().ok();

        RecipeAnalysis {
            mode: if self.mode.trim().is_empty() {
                "fact_probe".to_string()
            } else {
                self.mode.trim().to_string()
            },
            truth_spans,
            materially_false_spans,
            coherence_markers,
            max_generated_tokens,
            ridge_dead_zone,
        }
    }

    fn to_recipe(&self, name: String, selected_head: Option<usize>) -> Recipe {
        Recipe {
            name,
            prompt: self.prompt.clone(),
            description: None,
            ui_defaults: RecipeUiDefaults {
                layer: None,
                head: selected_head,
            },
            analysis: self.build_analysis(),
        }
    }
}


fn build_analyze_infer_ast(
    prompt: &str,
    analysis: &RecipeAnalysis,
    top: usize,
) -> larql_lql::Statement {
    let mode = match analysis.mode.as_str() {
        "workflow_probe" => larql_lql::ast::AnalysisMode::WorkflowProbe,
        _ => larql_lql::ast::AnalysisMode::FactProbe,
    };

    larql_lql::Statement::AnalyzeInfer {
        prompt: prompt.to_string(),
        mode,
        truth_spans: analysis.truth_spans.clone(),
        materially_false_spans: analysis.materially_false_spans.clone(),
        coherence_markers: analysis.coherence_markers.clone(),
        max_generated_tokens: analysis.max_generated_tokens.map(|v| v as u32),
        ridge_dead_zone: analysis.ridge_dead_zone,
        top: Some(top as u32),
        format: Some(larql_lql::ast::OutputFormat::Json),
    }
}

/// Execute a generic LQL statement remotely and return the output
pub async fn execute_lql_remote(
    server: &str,
    lql_statement: &str,
) -> Result<Vec<String>, String> {
    let mut session = Session::new();
    session
        .connect_remote(server)
        .map_err(|e| format!("Failed to connect remote LQL session: {e}"))?;

    let stmt = parse_lql(lql_statement)
        .map_err(|e| format!("Failed to parse LQL statement: {e}"))?;
    session
        .execute(&stmt)
        .map_err(|e| format!("Remote LQL execution failed: {e}"))
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualField {
    Prompt,
    Mode,
    TruthSpans,
    MateriallyFalseSpans,
    CoherenceMarkers,
    MaxGeneratedTokens,
    RidgeDeadZone,
}

impl ManualField {
    const ALL: [ManualField; 7] = [
        ManualField::Prompt,
        ManualField::Mode,
        ManualField::TruthSpans,
        ManualField::MateriallyFalseSpans,
        ManualField::CoherenceMarkers,
        ManualField::MaxGeneratedTokens,
        ManualField::RidgeDeadZone,
    ];

    fn title(self) -> &'static str {
        match self {
            ManualField::Prompt => "Prompt",
            ManualField::Mode => "Mode",
            ManualField::TruthSpans => "Truth Spans",
            ManualField::MateriallyFalseSpans => "False Spans",
            ManualField::CoherenceMarkers => "Coherence",
            ManualField::MaxGeneratedTokens => "Max Gen",
            ManualField::RidgeDeadZone => "Ridge DZ",
        }
    }
}

pub async fn execute_analyze_infer_lql(
    session: &mut Session,
    prompt: &str,
    analysis: &RecipeAnalysis,
) -> Result<BatchDlaResult, String> {
    let stmt = build_analyze_infer_ast(prompt, analysis, 5);

    let output = session
        .execute(&stmt)
        .map_err(|e| format!("Remote LQL execution failed: {e}"))?;

    // The executor returns formatted JSON as a single string
    let json_str = output
        .into_iter()
        .next()
        .ok_or_else(|| "Remote LQL execution returned no output".to_string())?;

    // Single-pass deserialization directly to BatchDlaResult
    serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse analysis JSON: {e}"))
}

pub fn attention_to_image(
    heads: &[Vec<f32>],
    _prev_heads: Option<&[Vec<f32>]>,
    seq_len: usize,
    #[allow(unused_variables)] selected_head: Option<usize>,
    #[allow(unused_variables)] diff_mode: bool,
    #[cfg(feature = "circuit-detect")] circuits: Option<&[CircuitHighlight]>,
    #[cfg(feature = "circuit-detect")] current_layer: usize,
) -> Image {
    if heads.is_empty() || seq_len == 0 {
        return Image::new(seq_len.max(1), 1);
    }

    #[cfg(feature = "head-isolation")]
    let row_heads: Vec<&[f32]> = if let Some(head) = selected_head {
        heads.get(head).map(|h| vec![h.as_slice()]).unwrap_or_default()
    } else {
        heads.iter().map(Vec::as_slice).collect()
    };
    #[cfg(not(feature = "head-isolation"))]
    let row_heads: Vec<&[f32]> = heads.iter().map(Vec::as_slice).collect();

    let row_count = row_heads.len().max(1);
    let mut img = Image::new(seq_len, row_count);

    for (row_idx, head) in row_heads.iter().enumerate() {
        #[cfg(feature = "layer-diff")]
        let prev = _prev_heads.and_then(|prev_all| {
            #[cfg(feature = "head-isolation")]
            {
                selected_head
                    .and_then(|selected| prev_all.get(selected))
                    .or_else(|| prev_all.get(row_idx))
            }
            #[cfg(not(feature = "head-isolation"))]
            {
                prev_all.get(row_idx)
            }
        });

        #[allow(unused_mut)]
        for col in 0..seq_len {
            let mut value = head.get(col).copied().unwrap_or(0.0);
            #[cfg(feature = "layer-diff")]
            if diff_mode {
                value -= prev.and_then(|layer| layer.get(col)).copied().unwrap_or(0.0);
            }

            let intensity = (value.abs() * 255.0 * 5.0).min(255.0) as u8;
            let color = if value < 0.0 {
                Rgb::new(0, (intensity as f32 * 0.45) as u8, intensity)
            } else {
                Rgb::new(intensity, (intensity as f32 * 0.45) as u8, 255u8.saturating_sub(intensity))
            };
            img.set(col, row_idx, color);
        }
    }

    #[cfg(feature = "circuit-detect")]
    if let Some(highlights) = circuits {
        for highlight in highlights {
            if highlight.layer != current_layer || highlight.from_token >= seq_len {
                continue;
            }
            #[cfg(feature = "head-isolation")]
            if let Some(selected) = selected_head {
                if highlight.head != selected {
                    continue;
                }
            }
            let row = if row_count == 1 {
                0
            } else {
                highlight.head.min(row_count - 1)
            };
            img.set(
                highlight.from_token,
                row,
                Rgb::new(
                    highlight.color_rgb.0,
                    highlight.color_rgb.1,
                    highlight.color_rgb.2,
                ),
            );
        }
    }

    img
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Focus {
    Prompt,
    Results,
    LayerList,
    Image,
    RecipeList,
}

pub struct RecipeStore {
    pub recipes: Vec<Recipe>,
    pub base_path: std::path::PathBuf,
}

impl RecipeStore {
    pub fn new(base_path: std::path::PathBuf) -> Self {
        if !base_path.exists() {
            let _ = std::fs::create_dir_all(&base_path);
        }
        let mut store = Self {
            recipes: Vec::new(),
            base_path,
        };
        let _ = store.load();
        store
    }

    pub fn load(&mut self) -> io::Result<()> {
        let path = self.base_path.join("recipes.json");
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            self.recipes = serde_json::from_str(&content).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid recipe schema: {err}"),
                )
            })?;
        }
        Ok(())
    }

    pub fn save(&self) -> io::Result<()> {
        let path = self.base_path.join("recipes.json");
        std::fs::write(path, serde_json::to_string_pretty(&self.recipes).unwrap())
    }
}

pub struct App {
    pub result: Option<BatchDlaResult>,
    pub error: Option<String>,
    pub last_export: Option<String>,
    pub renderer: Renderer,
    pub heatmaps: Vec<Image>,
    pub focus: Focus,
    pub list_state: ListState,
    pub recipe_list_state: ListState,
    pub recipe_store: RecipeStore,
    pub manual: ManualTargetDraft,
    pub manual_field: ManualField,
    pub zoom: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub z_index: i32,
    pub is_scanning: bool,
    pub session: Option<larql_lql::Session>,
    pub server: String,
    #[cfg(feature = "head-isolation")]
    pub selected_head: Option<usize>,
    #[cfg(feature = "layer-diff")]
    pub diff_mode: bool,
}

impl App {
    pub fn new(server: String) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let mut recipe_list_state = ListState::default();
        recipe_list_state.select(Some(0));
        Self {
            result: None,
            error: None,
            last_export: None,
            renderer: Renderer::new_auto(),
            heatmaps: Vec::new(),
            focus: Focus::Prompt,
            list_state,
            recipe_list_state,
            recipe_store: RecipeStore::new(std::path::PathBuf::from("recipes")),
            manual: ManualTargetDraft::default(),
            manual_field: ManualField::Prompt,
            zoom: 1.0,
            offset_x: 0.5,
            offset_y: 0.5,
            cursor_x: 0,
            cursor_y: 0,
            z_index: 0,
            is_scanning: false,
            session: None,
            server,
            #[cfg(feature = "head-isolation")]
            selected_head: None,
            #[cfg(feature = "layer-diff")]
            diff_mode: false,
        }
    }

    pub fn prompt(&self) -> &str {
        &self.manual.prompt
    }

    fn ensure_session(&mut self) -> Result<(), String> {
        if self.session.is_none() {
            let mut session = Session::new();
            session
                .connect_remote(&self.server)
                .map_err(|e| format!("Failed to connect remote LQL session: {e}"))?;
            self.session = Some(session);
        }
        Ok(())
    }

    fn selected_recipe(&self) -> Option<&Recipe> {
        self.recipe_list_state
            .selected()
            .and_then(|idx| self.recipe_store.recipes.get(idx))
    }

    fn active_field_mut(&mut self) -> &mut String {
        match self.manual_field {
            ManualField::Prompt => &mut self.manual.prompt,
            ManualField::Mode => &mut self.manual.mode,
            ManualField::TruthSpans => &mut self.manual.truth_spans,
            ManualField::MateriallyFalseSpans => &mut self.manual.materially_false_spans,
            ManualField::CoherenceMarkers => &mut self.manual.coherence_markers,
            ManualField::MaxGeneratedTokens => &mut self.manual.max_generated_tokens,
            ManualField::RidgeDeadZone => &mut self.manual.ridge_dead_zone,
        }
    }

    fn cycle_manual_field(&mut self, delta: isize) {
        let current = ManualField::ALL
            .iter()
            .position(|field| *field == self.manual_field)
            .unwrap_or(0) as isize;
        let len = ManualField::ALL.len() as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        self.manual_field = ManualField::ALL[next];
    }

    fn apply_recipe(&mut self, recipe: &Recipe) {
        self.manual = ManualTargetDraft::from_recipe(recipe);
        #[cfg(feature = "head-isolation")]
        {
            self.selected_head = recipe.ui_defaults.head;
        }
        if let Some(layer) = recipe.ui_defaults.layer {
            self.list_state.select(Some(layer));
        }
    }

    pub fn regenerate_heatmaps(&mut self) {
        if let Some(res) = &self.result {
            let seq_len = res.seq_len;
            let heatmaps: Vec<Image> = res
                .attention
                .iter()
                .enumerate()
                .map(|(idx, layer)| {
                    let prev = idx.checked_sub(1).and_then(|prev_idx| res.attention.get(prev_idx));
                    #[cfg(feature = "head-isolation")]
                    let selected_head = self.selected_head;
                    #[cfg(not(feature = "head-isolation"))]
                    let selected_head = None;
                    #[cfg(feature = "layer-diff")]
                    let diff_mode = self.diff_mode;
                    #[cfg(not(feature = "layer-diff"))]
                    let diff_mode = false;
                    attention_to_image(
                        &layer.heads,
                        prev.map(|entry| entry.heads.as_slice()),
                        res.seq_len,
                        selected_head,
                        diff_mode,
                        #[cfg(feature = "circuit-detect")]
                        res.circuits.as_deref(),
                        #[cfg(feature = "circuit-detect")]
                        layer.layer,
                    )
                })
                .collect();
            self.heatmaps = heatmaps;
            self.clamp_layer_selection();
            self.cursor_x = self
                .cursor_x
                .min(seq_len.saturating_sub(1));
            self.cursor_y = self
                .cursor_y
                .min(self.heatmaps.first().map(|img| img.height).unwrap_or(1).saturating_sub(1));
        }
    }

    fn clamp_layer_selection(&mut self) {
        let selection = match self.heatmaps.len() {
            0 => None,
            len => Some(
                self.list_state
                    .selected()
                    .map(|selected| selected.min(len - 1))
                    .unwrap_or(0),
            ),
        };
        self.list_state.select(selection);
    }

    pub fn handle_key(&mut self, key: KeyEvent, rt: &Runtime) -> bool {
        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Prompt => Focus::RecipeList,
                    Focus::RecipeList => Focus::Results,
                    Focus::Results => Focus::LayerList,
                    Focus::LayerList => Focus::Image,
                    Focus::Image => Focus::Prompt,
                };
            }
            KeyCode::Char('q') | KeyCode::Esc => return true,
            _ => match self.focus {
                Focus::Prompt => match key.code {
                    KeyCode::Char('[') => self.cycle_manual_field(-1),
                    KeyCode::Char(']') => self.cycle_manual_field(1),
                    KeyCode::Backspace => {
                        self.active_field_mut().pop();
                    }
                    KeyCode::Char('R') => {
                        if !self.manual.prompt.is_empty() {
                            #[cfg(feature = "head-isolation")]
                            let selected_head = self.selected_head;
                            #[cfg(not(feature = "head-isolation"))]
                            let selected_head = None;
                            let name = format!("Recipe {}", self.recipe_store.recipes.len() + 1);
                            let recipe = self.manual.to_recipe(name, selected_head);
                            self.recipe_store.recipes.push(recipe);
                            let _ = self.recipe_store.save();
                        }
                    }
                    KeyCode::Enter => {
                        if self.manual.prompt.is_empty() {
                            return false;
                        }
                        self.error = None;
                        self.is_scanning = true;
                        if let Err(err) = self.ensure_session() {
                            self.error = Some(err);
                            self.is_scanning = false;
                            return false;
                        }
                        let analysis = self.manual.build_analysis();
                        let session = self.session.as_mut().unwrap();
                        match rt.block_on(execute_analyze_infer_lql(
                            session,
                            &self.manual.prompt,
                            &analysis,
                        ))
                        {
                            Ok(result) => {
                                self.result = Some(result);
                                self.regenerate_heatmaps();
                                self.focus = Focus::LayerList;
                            }
                            Err(err) => {
                                self.heatmaps.clear();
                                self.result = None;
                                if let Err(clear_err) = self.renderer.clear_all_graphics() {
                                    self.error = Some(format!(
                                        "{err}; additionally failed to clear stale graphics: {clear_err}"
                                    ));
                                } else {
                                    self.error = Some(err);
                                }
                            }
                        }
                        self.is_scanning = false;
                    }
                    KeyCode::Char(ch) => self.active_field_mut().push(ch),
                    _ => {}
                },
                Focus::RecipeList => match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        let idx = match self.recipe_list_state.selected() {
                            Some(idx) if idx + 1 < self.recipe_store.recipes.len() => idx + 1,
                            _ => 0,
                        };
                        self.recipe_list_state.select(Some(idx));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let idx = match self.recipe_list_state.selected() {
                            Some(0) | None => self.recipe_store.recipes.len().saturating_sub(1),
                            Some(idx) => idx - 1,
                        };
                        self.recipe_list_state.select(Some(idx));
                    }
                    KeyCode::Enter => {
                        if let Some(recipe) = self.selected_recipe().cloned() {
                            self.apply_recipe(&recipe);
                            self.focus = Focus::Prompt;
                        }
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        if let Some(recipe) = self.selected_recipe() {
                            let stmt = build_analyze_infer_ast(&recipe.prompt, &recipe.analysis, 5);
                            self.last_export = Some(format!(
                                "Exported LQL: {}",
                                stmt.to_string()
                            ));
                        }
                    }
                    KeyCode::Char('x') => {
                        if let Some(idx) = self.recipe_list_state.selected() {
                            if idx < self.recipe_store.recipes.len() {
                                self.recipe_store.recipes.remove(idx);
                                let _ = self.recipe_store.save();
                            }
                        }
                    }
                    _ => {}
                },
                Focus::Results => {}
                Focus::LayerList => match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        let idx = match self.list_state.selected() {
                            Some(idx) if idx + 1 < self.heatmaps.len() => idx + 1,
                            _ => 0,
                        };
                        self.list_state.select(Some(idx));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let idx = match self.list_state.selected() {
                            Some(0) | None => self.heatmaps.len().saturating_sub(1),
                            Some(idx) => idx - 1,
                        };
                        self.list_state.select(Some(idx));
                    }
                    _ => {}
                },
                Focus::Image => match key.code {
                    KeyCode::Char('w') => self.offset_y = (self.offset_y - 0.1 / self.zoom).max(0.0),
                    KeyCode::Char('s') => self.offset_y = (self.offset_y + 0.1 / self.zoom).min(1.0),
                    KeyCode::Char('a') => self.offset_x = (self.offset_x - 0.1 / self.zoom).max(0.0),
                    KeyCode::Char('d') => self.offset_x = (self.offset_x + 0.1 / self.zoom).min(1.0),
                    KeyCode::Up => self.cursor_y = self.cursor_y.saturating_sub(1),
                    KeyCode::Down => {
                        if let Some(selected) = self.list_state.selected() {
                            if let Some(img) = self.heatmaps.get(selected) {
                                self.cursor_y = (self.cursor_y + 1).min(img.height.saturating_sub(1));
                            }
                        }
                    }
                    KeyCode::Left => self.cursor_x = self.cursor_x.saturating_sub(1),
                    KeyCode::Right => {
                        if let Some(selected) = self.list_state.selected() {
                            if let Some(img) = self.heatmaps.get(selected) {
                                self.cursor_x = (self.cursor_x + 1).min(img.width.saturating_sub(1));
                            }
                        }
                    }
                    KeyCode::Char('z') => self.zoom *= 1.2,
                    KeyCode::Char('x') => self.zoom = (self.zoom / 1.2).max(1.0),
                    KeyCode::Char('b') => {
                        use larql_terminal_renderer::BackendType;
                        self.renderer.backend_type = match self.renderer.backend_type {
                            BackendType::Sixel => BackendType::Kitty,
                            BackendType::Kitty => BackendType::ITerm2,
                            BackendType::ITerm2 => BackendType::Ansi,
                            BackendType::Ansi => BackendType::Sixel,
                        };
                    }
                    KeyCode::Char('l') => self.z_index = if self.z_index == 1 { -1 } else { 1 },
                    #[cfg(feature = "head-isolation")]
                    KeyCode::Char('h') => {
                        if let Some(layer) = self.result.as_ref().and_then(|res| res.attention.first()) {
                            let count = layer.heads.len();
                            self.selected_head = match self.selected_head {
                                None if count > 0 => Some(0),
                                Some(head) if head + 1 < count => Some(head + 1),
                                _ => None,
                            };
                            self.regenerate_heatmaps();
                        }
                    }
                    #[cfg(feature = "layer-diff")]
                    KeyCode::Char('f') => {
                        self.diff_mode = !self.diff_mode;
                        self.regenerate_heatmaps();
                    }
                    _ => {}
                },
            },
        }
        false
    }
}

fn focused_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

fn manual_lines(app: &App) -> Vec<Line<'static>> {
    ManualField::ALL
        .iter()
        .map(|field| {
            let value = match field {
                ManualField::Prompt => app.manual.prompt.as_str(),
                ManualField::Mode => app.manual.mode.as_str(),
                ManualField::TruthSpans => app.manual.truth_spans.as_str(),
                ManualField::MateriallyFalseSpans => app.manual.materially_false_spans.as_str(),
                ManualField::CoherenceMarkers => app.manual.coherence_markers.as_str(),
                ManualField::MaxGeneratedTokens => app.manual.max_generated_tokens.as_str(),
                ManualField::RidgeDeadZone => app.manual.ridge_dead_zone.as_str(),
            };
            let style = if *field == app.manual_field && app.focus == Focus::Prompt {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(format!("{:<12}", field.title()), style.add_modifier(Modifier::BOLD)),
                Span::raw(": "),
                Span::raw(value.to_string()),
            ])
        })
        .collect()
}

fn analysis_lines(res: &BatchDlaResult) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from("Token analysis")];
    if let Some(summary) = &res.analysis_summary {
        lines.push(Line::from(format!(
            "False detected: {}",
            summary.materially_false_detected
        )));
        if let Some(origin) = &summary.first_false_origin {
            lines.push(Line::from(format!(
                "First false: pos {} '{}' L{} H{} src {} {:.4}",
                origin.position,
                origin.token,
                origin.layer,
                origin.head,
                origin.source_token,
                origin.contribution
            )));
        } else {
            lines.push(Line::from("First false: none"));
        }
        if !summary.top_coherence_heads.is_empty() {
            lines.push(Line::from("Top coherence heads:"));
            for head in summary.top_coherence_heads.iter().take(3) {
                lines.push(Line::from(format!(
                    "  L{} H{} src {} {:.4}",
                    head.layer, head.head, head.source_token, head.contribution
                )));
            }
        }
        if !summary.top_false_content_heads.is_empty() {
            lines.push(Line::from("Top false heads:"));
            for head in summary.top_false_content_heads.iter().take(3) {
                lines.push(Line::from(format!(
                    "  L{} H{} src {} {:.4}",
                    head.layer, head.head, head.source_token, head.contribution
                )));
            }
        }
    } else {
        lines.push(Line::from("No analysis block provided"));
    }

    if !res.ridge_by_layer.is_empty() {
        lines.push(Line::from("Ridge by layer:"));
        for ridge in res.ridge_by_layer.iter().take(5) {
            lines.push(Line::from(format!("  L{} {:.4}", ridge.layer, ridge.ridge)));
        }
    }

    if let Some(step) = res.token_analysis.first() {
        lines.push(Line::from("Step 0:"));
        lines.push(Line::from(format!(
            "  token '{}' p={:.4} label={}",
            step.token, step.probability, step.label
        )));
        lines.push(Line::from(format!(
            "  truth={:.4} false={:.4} coherence={:.4} ridge={:.4}",
            step.truth_mass, step.false_mass, step.coherence_mass, step.ridge
        )));
    }
    lines
}

fn current_dla_value(app: &App, res: &BatchDlaResult) -> Option<f32> {
    let selected_layer = app.list_state.selected()?;
    let layer = res.head_dla.iter().find(|layer| layer.layer == selected_layer)?;
    #[cfg(feature = "head-isolation")]
    let row = app.selected_head.unwrap_or(app.cursor_y);
    #[cfg(not(feature = "head-isolation"))]
    let row = app.cursor_y;
    layer.heads.get(row)?.get(app.cursor_x).copied()
}

pub fn ui(frame: &mut Frame, app: &App, graphics_layer: &mut GraphicsLayer) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.size());

    let stats = Paragraph::new(if let Some(res) = &app.result {
        stats_summary_text(res)
    } else {
        "No scan results yet.".to_string()
    })
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Stats")
            .border_style(focused_style(app.focus == Focus::Results)),
    );
    frame.render_widget(stats, chunks[0]);

    let input = Paragraph::new(manual_lines(app))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Manual Target ([ / ] field, Enter scan, R save) ")
                .border_style(focused_style(app.focus == Focus::Prompt)),
        );
    frame.render_widget(input, chunks[1]);

    if let Some(err) = &app.error {
        frame.render_widget(
            Paragraph::new(err.as_str()).block(Block::default().borders(Borders::ALL).title("Error")),
            chunks[2],
        );
    } else if let Some(res) = &app.result {
        let content = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(53),
                Constraint::Percentage(25),
            ])
            .split(chunks[2]);

        let layer_items: Vec<ListItem> = app
            .heatmaps
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let lens = res
                    .logit_lens
                    .as_ref()
                    .and_then(|rows| rows.iter().find(|row| row.layer == idx))
                    .and_then(|row| row.predictions.first())
                    .map(|(token, prob)| format!(" L{idx}: {token} ({prob:.2})"))
                    .unwrap_or_else(|| format!(" L{idx}"));
                ListItem::new(lens)
            })
            .collect();
        let mut layer_state = app.list_state.clone();
        frame.render_stateful_widget(
            List::new(layer_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Layers")
                        .border_style(focused_style(app.focus == Focus::LayerList)),
                )
                .highlight_style(Style::default().bg(Color::DarkGray)),
            content[0],
            &mut layer_state,
        );

        let view_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(content[1]);

        let viewport_block = Block::default()
            .borders(Borders::ALL)
            .title("Last-Token Attention Strips")
            .border_style(focused_style(app.focus == Focus::Image));
        let inner = viewport_block.inner(view_chunks[0]);
        frame.render_widget(viewport_block, view_chunks[0]);

        if let Some(selected) = app.list_state.selected() {
            if let Some(img) = app.heatmaps.get(selected) {
                use larql_terminal_renderer::BackendType;
                if app.renderer.backend_type == BackendType::Ansi {
                    #[allow(unused_mut)]
                    let mut widget = ImageWidget::new(img, &app.renderer)
                        .zoom(app.zoom)
                        .offset(app.offset_x, app.offset_y)
                        .z_index(app.z_index);
                    #[cfg(feature = "token-decode")]
                    if let Some(strings) = &res.strings {
                        widget = widget.labels(strings);
                    }
                    frame.render_widget(widget, inner);
                } else {
                    frame.render_widget(Block::default(), inner);
                }
                graphics_layer.add_with_view(
                    img,
                    inner,
                    app.offset_x,
                    app.offset_y,
                    app.zoom,
                    app.z_index,
                );
            }
        }

        let mut cursor_text = format!("Cursor x={} y={}", app.cursor_x, app.cursor_y);
        if let Some(value) = current_dla_value(app, res) {
            cursor_text.push_str(&format!(" | head DLA {:.4}", value));
        }
        #[cfg(feature = "token-decode")]
        if let Some(strings) = &res.strings {
            let token = strings
                .get(app.cursor_x)
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            cursor_text.push_str(&format!(" | token '{}'", token));
        }
        frame.render_widget(Paragraph::new(cursor_text), view_chunks[1]);

        frame.render_widget(
            Paragraph::new(analysis_lines(res))
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Analysis")),
            content[2],
        );
    } else {
        let items: Vec<ListItem> = app
            .recipe_store
            .recipes
            .iter()
            .map(|recipe| {
                ListItem::new(format!(
                    "{} | {}",
                    recipe.name,
                    recipe.prompt.chars().take(48).collect::<String>()
                ))
            })
            .collect();
        let mut recipe_state = app.recipe_list_state.clone();
        frame.render_stateful_widget(
            List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Recipes")
                        .border_style(focused_style(app.focus == Focus::RecipeList)),
                )
                .highlight_style(Style::default().bg(Color::DarkGray)),
            chunks[2],
            &mut recipe_state,
        );
    }

    let footer = match app.focus {
        Focus::Prompt => "Tab next | [ ] field | Enter scan | R save recipe | Q quit".to_string(),
        Focus::RecipeList => "Tab next | j/k nav | Enter load | C export LQL | x delete | Q quit".to_string(),
        Focus::Results => "Tab next | Q quit".to_string(),
        Focus::LayerList => "Tab next | j/k select layer | Q quit".to_string(),
        Focus::Image => {
            #[allow(unused_mut)]
            let mut text = format!(
                "Tab next | wasd pan ({:.1},{:.1}) | z/x zoom {:.1} | arrows cursor | b backend | l z-index {} | Q quit",
                app.offset_x, app.offset_y, app.zoom, app.z_index
            );
            #[cfg(feature = "head-isolation")]
            {
                if let Some(head) = app.selected_head {
                    text.push_str(&format!(" | h head {}", head));
                } else {
                    text.push_str(" | h head all");
                }
            }
            #[cfg(feature = "layer-diff")]
            {
                text.push_str(if app.diff_mode { " | f diff on" } else { " | f diff off" });
            }
            text
        }
    };
    let status = format!(
        "{}{}{}{}{}",
        footer,
        if app.is_scanning { " | scanning" } else { "" },
        app.list_state
            .selected()
            .map(|idx| format!(" | layer {}", idx))
            .unwrap_or_default(),
        app.error
            .as_ref()
            .map(|err| format!(" | ERR: {}", err))
            .unwrap_or_default(),
        app.last_export
            .as_ref()
            .map(|msg| format!(" | {}", msg))
            .unwrap_or_default(),
    );
    frame.render_widget(
        Paragraph::new(status).block(Block::default().borders(Borders::ALL)),
        chunks[3],
    );
}

fn stats_summary_text(res: &BatchDlaResult) -> String {
    let mut text = format!("Layers: {} | Seq Len: {}", res.num_layers, res.seq_len);
    if !res.predictions.is_empty() {
        text.push_str(" | Top Predictions: ");
        for (idx, (token, prob)) in res.predictions.iter().take(3).enumerate() {
            if idx > 0 {
                text.push_str(", ");
            }
            text.push_str(&format!("{}({:.2})", token, prob));
        }
    }
    if let Some(summary) = &res.analysis_summary {
        if let Some(origin) = &summary.first_false_origin {
            text.push_str(&format!(
                " | First False: {} @ L{}H{}",
                origin.token, origin.layer, origin.head
            ));
        } else if !res.token_analysis.is_empty() {
            text.push_str(" | First False: none");
        }
    }
    if let Some(max_ridge) = res
        .ridge_by_layer
        .iter()
        .max_by(|a, b| a.ridge.partial_cmp(&b.ridge).unwrap())
    {
        text.push_str(&format!(" | Ridge Max: L{} {:.4}", max_ridge.layer, max_ridge.ridge));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{
        attention_to_image, build_analyze_infer_ast, stats_summary_text,
        AnalysisSummary, App, AttentionData, BatchDlaResult, HeadContribution, HeadDlaLayer,
        ManualField, Recipe, RecipeAnalysis, RecipeStore, RecipeUiDefaults, RidgeByLayer,
        StepTopHeadSummary, TokenAnalysis,
    };

    fn mock_result(num_layers: usize) -> BatchDlaResult {
        BatchDlaResult {
            attention: (0..num_layers)
                .map(|layer| AttentionData {
                    layer,
                    heads: vec![vec![0.1, 0.2], vec![0.3, 0.4]],
                })
                .collect(),
            logit_lens: None,
            head_dla: vec![HeadDlaLayer {
                layer: 0,
                heads: vec![vec![0.1, 0.2], vec![0.3, 0.4]],
            }],
            num_layers,
            predictions: vec![("tok_a".to_string(), 0.9), ("tok_b".to_string(), 0.05)],
            seq_len: 2,
            tokens: vec![1, 2],
            strings: Some(vec!["a".to_string(), "b".to_string()]),
            circuits: None,
            generation_trace: vec![],
            token_analysis: vec![TokenAnalysis {
                position: 0,
                token_id: 9,
                token: "tok_a".to_string(),
                probability: 0.9,
                label: "unlabeled".to_string(),
                truth_mass: 0.1,
                false_mass: 0.2,
                coherence_mass: 0.3,
                ridge: 0.06,
                top_heads: StepTopHeadSummary {
                    false_content: vec![],
                    material_coherence: vec![],
                },
            }],
            analysis_summary: Some(AnalysisSummary {
                first_false_position: None,
                first_false_token: None,
                first_false_origin: None,
                top_coherence_heads: vec![HeadContribution {
                    layer: 1,
                    head: 2,
                    source_token: 0,
                    contribution: 0.8,
                }],
                top_false_content_heads: vec![],
                materially_false_detected: false,
            }),
            ridge_by_layer: vec![RidgeByLayer {
                layer: 1,
                ridge: 0.06,
            }],
        }
    }

    #[test]
    fn stats_summary_keeps_predictions_visible() {
        let text = stats_summary_text(&mock_result(2));
        assert!(text.contains("Layers: 2 | Seq Len: 2"));
        assert!(text.contains("Top Predictions: tok_a(0.90), tok_b(0.05)"));
        assert!(text.contains("Ridge Max: L1 0.0600"));
    }

    #[test]
    fn regenerate_heatmaps_clamps_selected_layer_to_new_result() {
        let mut app = App::new("http://localhost:8080".to_string());
        app.result = Some(mock_result(3));
        app.regenerate_heatmaps();
        app.list_state.select(Some(2));

        app.result = Some(mock_result(1));
        app.regenerate_heatmaps();

        assert_eq!(app.heatmaps.len(), 1);
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn attention_image_uses_head_rows_not_square_duplication() {
        let image = attention_to_image(
            &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
            None,
            3,
            None,
            false,
            #[cfg(feature = "circuit-detect")]
            None,
            #[cfg(feature = "circuit-detect")]
            0,
        );
        assert_eq!(image.width, 3);
        #[cfg(feature = "head-isolation")]
        assert_eq!(image.height, 2);
        #[cfg(not(feature = "head-isolation"))]
        assert_eq!(image.height, 2);
    }

    #[test]
    fn strict_recipe_schema_loads_typed_entries() {
        let temp_dir = std::env::temp_dir().join(format!(
            "larql-batch-dla-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(
            temp_dir.join("recipes.json"),
            serde_json::to_string(&vec![Recipe {
                name: "Freedonia".to_string(),
                prompt: "The capital of Freedonia is".to_string(),
                description: None,
                ui_defaults: RecipeUiDefaults {
                    layer: Some(14),
                    head: Some(2),
                },
                analysis: RecipeAnalysis {
                    mode: "fact_probe".to_string(),
                    truth_spans: vec!["Markov".to_string()],
                    materially_false_spans: vec!["Paris".to_string()],
                    coherence_markers: vec!["capital".to_string()],
                    max_generated_tokens: Some(1),
                    ridge_dead_zone: Some(0.05),
                },
            }])
            .unwrap(),
        )
        .unwrap();

        let store = RecipeStore::new(temp_dir.clone());
        assert_eq!(store.recipes.len(), 1);
        assert_eq!(store.recipes[0].analysis.truth_spans, vec!["Markov"]);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn manual_field_cycle_covers_all_fields() {
        let mut app = App::new("http://localhost:8080".to_string());
        for _ in 0..ManualField::ALL.len() {
            app.cycle_manual_field(1);
        }
        assert_eq!(app.manual_field, ManualField::Prompt);
    }

    #[test]
    fn exported_lql_preserves_full_analysis_spec() {
        let stmt = build_analyze_infer_ast(
            "The capital of Freedonia is",
            &RecipeAnalysis {
                mode: "fact_probe".to_string(),
                truth_spans: vec!["Markov".to_string()],
                materially_false_spans: vec!["Paris".to_string(), "London".to_string()],
                coherence_markers: vec!["capital".to_string(), "is".to_string()],
                max_generated_tokens: Some(1),
                ridge_dead_zone: Some(0.05),
            },
            5,
        );
        let lql = stmt.to_string();

        assert_eq!(
            lql,
            "ANALYZE INFER \"The capital of Freedonia is\" MODE FACT_PROBE TRUTH_SPANS (\"Markov\") FALSE_SPANS (\"Paris\", \"London\") COHERENCE_MARKERS (\"capital\", \"is\") MAX_GENERATED_TOKENS 1 RIDGE_DEAD_ZONE 0.05 TOP 5 FORMAT JSON;"
        );

        // Assert all clauses are present
        assert!(lql.contains("MODE FACT_PROBE"));
        assert!(lql.contains("TRUTH_SPANS"));
        assert!(lql.contains("FALSE_SPANS"));
        assert!(lql.contains("COHERENCE_MARKERS"));
        assert!(lql.contains("MAX_GENERATED_TOKENS"));
        assert!(lql.contains("RIDGE_DEAD_ZONE"));
        assert!(lql.contains("TOP"));
        assert!(lql.contains("FORMAT JSON"));
    }

    #[test]
    fn exported_lql_fact_probe_truth_only() {
        let stmt = build_analyze_infer_ast(
            "Test",
            &RecipeAnalysis {
                mode: "fact_probe".to_string(),
                truth_spans: vec!["Paris".to_string()],
                materially_false_spans: vec![],
                coherence_markers: vec![],
                max_generated_tokens: None,
                ridge_dead_zone: None,
            },
            5,
        );
        let lql = stmt.to_string();
        assert!(lql.contains("MODE FACT_PROBE"));
        assert!(lql.contains("TRUTH_SPANS"));
    }

    #[test]
    fn exported_lql_fact_probe_false_only() {
        let stmt = build_analyze_infer_ast(
            "Test",
            &RecipeAnalysis {
                mode: "fact_probe".to_string(),
                truth_spans: vec![],
                materially_false_spans: vec!["London".to_string()],
                coherence_markers: vec![],
                max_generated_tokens: None,
                ridge_dead_zone: None,
            },
            5,
        );
        let lql = stmt.to_string();
        assert!(lql.contains("MODE FACT_PROBE"));
        assert!(lql.contains("FALSE_SPANS"));
    }

    #[test]
    fn exported_lql_workflow_probe_requires_false_spans() {
        let stmt = build_analyze_infer_ast(
            "Test",
            &RecipeAnalysis {
                mode: "workflow_probe".to_string(),
                truth_spans: vec![],
                materially_false_spans: vec!["hallucination".to_string()],
                coherence_markers: vec![],
                max_generated_tokens: None,
                ridge_dead_zone: None,
            },
            5,
        );
        let lql = stmt.to_string();
        assert!(lql.contains("MODE WORKFLOW_PROBE"));
        assert!(lql.contains("FALSE_SPANS"));
    }
}
