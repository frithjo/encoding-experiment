/// LQL Abstract Syntax Tree
///
/// Every LQL statement parses into one `Statement` variant.
/// The executor dispatches each variant to the appropriate backend.

#[derive(Debug, Clone)]
pub enum Statement {
    // ── Lifecycle ──
    Extract {
        model: String,
        output: String,
        components: Option<Vec<Component>>,
        layers: Option<Range>,
        extract_level: ExtractLevel,
    },
    Compile {
        vindex: VindexRef,
        output: String,
        format: Option<OutputFormat>,
        target: CompileTarget,
        /// COMPILE INTO VINDEX only: how to resolve patches that touch the
        /// same (layer, feature) slot. None → default (LastWins).
        on_conflict: Option<CompileConflict>,
    },
    Diff {
        a: VindexRef,
        b: VindexRef,
        layer: Option<u32>,
        relation: Option<String>,
        limit: Option<u32>,
        into_patch: Option<String>,
        into_report: Option<String>,
    },
    Export {
        vindex: VindexRef,
        output: String,
        format: GraphExportFormat,
    },
    Use {
        target: UseTarget,
    },

    // ── Query ──
    Walk {
        prompt: String,
        top: Option<u32>,
        layers: Option<Range>,
        mode: Option<WalkMode>,
        compare: bool,
    },
    /// Full inference with attention — requires model weights.
    Infer {
        prompt: String,
        top: Option<u32>,
        compare: bool,
    },
    /// Scientific attribution analysis with truth/false span resolution
    AnalyzeInfer {
        prompt: String,
        mode: AnalysisMode,
        truth_spans: Vec<String>,
        materially_false_spans: Vec<String>,
        coherence_markers: Vec<String>,
        max_generated_tokens: Option<u32>,
        ridge_dead_zone: Option<f32>,
        top: Option<u32>,
        format: Option<OutputFormat>,
    },
    Select {
        source: SelectSource,
        fields: Vec<Field>,
        conditions: Vec<Condition>,
        nearest: Option<NearestClause>,
        order: Option<OrderBy>,
        limit: Option<u32>,
    },
    Describe {
        entity: String,
        band: Option<LayerBand>,
        layer: Option<u32>,
        relations_only: bool,
        mode: DescribeMode,
        stream: bool,
    },
    Explain {
        prompt: String,
        mode: ExplainMode,
        layers: Option<Range>,
        band: Option<LayerBand>,
        verbose: bool,
        top: Option<u32>,
        relations_only: bool,
        with_attention: bool,
    },

    // ── Mutation ──
    Insert {
        entity: String,
        relation: String,
        target: String,
        layer: Option<u32>,
        confidence: Option<f32>,
        /// Per-layer down-vector multiplier for the install
        /// (`d_ref * alpha_mul`). Default 0.1 — matches the validated
        /// Python `install_compiled_slot` pipeline (exp 14). Larger
        /// values push the inserted fact harder but dilute neighbours;
        /// smaller values reduce neighbour degradation at the cost of
        /// new-fact confidence. Validated range ~0.05–0.30.
        alpha: Option<f32>,
    },
    Delete {
        conditions: Vec<Condition>,
    },
    Update {
        set: Vec<Assignment>,
        conditions: Vec<Condition>,
    },
    Merge {
        source: String,
        target: Option<String>,
        conflict: Option<ConflictStrategy>,
    },

    // ── Introspection ──
    ShowRelations {
        layer: Option<u32>,
        with_examples: bool,
        mode: DescribeMode,
    },
    ShowLayers {
        range: Option<Range>,
    },
    ShowFeatures {
        layer: u32,
        conditions: Vec<Condition>,
        limit: Option<u32>,
    },
    ShowEntities {
        layer: Option<u32>,
        limit: Option<u32>,
    },
    ShowTokens {
        layer: Option<u32>,
        conditions: Vec<Condition>,
        verbose: bool,
        group_by: Option<TokenGroupBy>,
        order_by: Option<TokenSortBy>,
        limit: Option<u32>,
        export_format: Option<ExportFormat>,
    },
    ShowModels,
    Stats {
        vindex: Option<String>,
    },

    // ── Patch ──
    BeginPatch {
        path: String,
    },
    SavePatch,
    ApplyPatch {
        path: String,
    },
    ShowPatches,
    RemovePatch {
        path: String,
    },

    // ── Trace ──
    /// Residual stream trace — decomposed forward pass.
    Trace {
        prompt: String,
        /// Token to track through the trace (the `FOR <token>` clause).
        answer: Option<String>,
        decompose: bool,
        layers: Option<Range>,
        positions: Option<TracePositionMode>,
        save: Option<String>,
    },

    // ── Pipe ──
    Pipe {
        left: Box<Statement>,
        right: Box<Statement>,
    },
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Statement::Extract {
                model,
                output,
                components,
                layers,
                extract_level,
            } => {
                write!(f, "EXTRACT MODEL {} INTO {}", quote_string(model), quote_string(output))?;
                if let Some(comps) = components {
                    if !comps.is_empty() {
                        let comp_strs: Vec<String> = comps.iter().map(|c| format!("{:?}", c)).collect();
                        write!(f, " COMPONENTS ({})", comp_strs.join(", "))?;
                    }
                }
                if let Some(range) = layers {
                    write!(f, " LAYERS {}..{}", range.start, range.end)?;
                }
                write!(f, " LEVEL {:?}", extract_level)?;
                write!(f, ";")
            }
            Statement::Compile {
                vindex,
                output,
                format,
                target,
                on_conflict,
            } => {
                write!(f, "COMPILE {:?} INTO {}", vindex, quote_string(output))?;
                if let Some(fmt) = format {
                    write!(f, " FORMAT {}", fmt)?;
                }
                write!(f, " TARGET {:?}", target)?;
                if let Some(conflict) = on_conflict {
                    write!(f, " ON_CONFLICT {:?}", conflict)?;
                }
                write!(f, ";")
            }
            Statement::Use { target } => {
                match target {
                    UseTarget::Vindex(path) => write!(f, "USE {}", quote_string(path))?,
                    UseTarget::Model { id, auto_extract } => {
                        write!(f, "USE MODEL {}", quote_string(id))?;
                        if *auto_extract {
                            write!(f, " AUTO_EXTRACT")?;
                        }
                    }
                    UseTarget::Remote(url) => write!(f, "USE REMOTE {}", quote_string(url))?,
                }
                write!(f, ";")
            }
            Statement::Walk {
                prompt,
                top,
                layers,
                mode,
                compare,
            } => {
                write!(f, "WALK {}", quote_string(prompt))?;
                if let Some(t) = top {
                    write!(f, " TOP {}", t)?;
                }
                if let Some(range) = layers {
                    write!(f, " LAYERS {}..{}", range.start, range.end)?;
                }
                if let Some(m) = mode {
                    write!(f, " MODE {:?}", m)?;
                }
                if *compare {
                    write!(f, " COMPARE")?;
                }
                write!(f, ";")
            }
            Statement::Infer { prompt, top, compare } => {
                write!(f, "INFER {}", quote_string(prompt))?;
                if let Some(t) = top {
                    write!(f, " TOP {}", t)?;
                }
                if *compare {
                    write!(f, " COMPARE")?;
                }
                write!(f, ";")
            }
            Statement::AnalyzeInfer {
                prompt,
                mode,
                truth_spans,
                materially_false_spans,
                coherence_markers,
                max_generated_tokens,
                ridge_dead_zone,
                top,
                format,
            } => {
                write!(f, "ANALYZE INFER {}", quote_string(prompt))?;
                write!(f, " MODE {}", mode)?;
                if !truth_spans.is_empty() {
                    write!(f, " TRUTH_SPANS ({})", quote_list(truth_spans))?;
                }
                if !materially_false_spans.is_empty() {
                    write!(f, " FALSE_SPANS ({})", quote_list(materially_false_spans))?;
                }
                if !coherence_markers.is_empty() {
                    write!(f, " COHERENCE_MARKERS ({})", quote_list(coherence_markers))?;
                }
                if let Some(max_gen) = max_generated_tokens {
                    write!(f, " MAX_GENERATED_TOKENS {}", max_gen)?;
                }
                if let Some(rdz) = ridge_dead_zone {
                    write!(f, " RIDGE_DEAD_ZONE {}", rdz)?;
                }
                if let Some(t) = top {
                    write!(f, " TOP {}", t)?;
                }
                if let Some(fmt) = format {
                    write!(f, " FORMAT {}", fmt)?;
                }
                write!(f, ";")
            }
            Statement::Select {
                source,
                fields,
                conditions,
                nearest,
                order,
                limit,
            } => {
                write!(f, "SELECT ")?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    match field {
                        Field::Star => write!(f, "*")?,
                        Field::Named(name) => write!(f, "{}", name)?,
                    }
                }
                write!(f, " FROM {:?}", source)?;
                if !conditions.is_empty() {
                    write!(f, " WHERE ")?;
                    for (i, cond) in conditions.iter().enumerate() {
                        if i > 0 {
                            write!(f, " AND ")?;
                        }
                        write!(f, "{} {:?} {}", cond.field, cond.op, format_value(&cond.value))?;
                    }
                }
                if let Some(n) = nearest {
                    write!(f, " NEAREST {} AT LAYER {}", quote_string(&n.entity), n.layer)?;
                }
                if let Some(o) = order {
                    write!(f, " ORDER BY {} {}", o.field, if o.descending { "DESC" } else { "ASC" })?;
                }
                if let Some(l) = limit {
                    write!(f, " LIMIT {}", l)?;
                }
                write!(f, ";")
            }
            Statement::Describe {
                entity,
                band,
                layer,
                relations_only,
                mode,
                stream,
            } => {
                write!(f, "DESCRIBE {}", quote_string(entity))?;
                if let Some(b) = band {
                    write!(f, " BAND {:?}", b)?;
                }
                if let Some(l) = layer {
                    write!(f, " LAYER {}", l)?;
                }
                if *relations_only {
                    write!(f, " RELATIONS_ONLY")?;
                }
                write!(f, " MODE {:?}", mode)?;
                if *stream {
                    write!(f, " STREAM")?;
                }
                write!(f, ";")
            }
            Statement::Explain {
                prompt,
                mode,
                layers,
                band,
                verbose,
                top,
                relations_only,
                with_attention,
            } => {
                write!(f, "EXPLAIN {} MODE {:?}", quote_string(prompt), mode)?;
                if let Some(range) = layers {
                    write!(f, " LAYERS {}..{}", range.start, range.end)?;
                }
                if let Some(b) = band {
                    write!(f, " BAND {:?}", b)?;
                }
                if *verbose {
                    write!(f, " VERBOSE")?;
                }
                if let Some(t) = top {
                    write!(f, " TOP {}", t)?;
                }
                if *relations_only {
                    write!(f, " RELATIONS_ONLY")?;
                }
                if *with_attention {
                    write!(f, " WITH_ATTENTION")?;
                }
                write!(f, ";")
            }
            Statement::Insert {
                entity,
                relation,
                target,
                layer,
                confidence,
                alpha,
            } => {
                write!(f, "INSERT ({}, {}, {})", quote_string(entity), quote_string(relation), quote_string(target))?;
                if let Some(l) = layer {
                    write!(f, " AT LAYER {}", l)?;
                }
                if let Some(c) = confidence {
                    write!(f, " CONFIDENCE {}", c)?;
                }
                if let Some(a) = alpha {
                    write!(f, " ALPHA {}", a)?;
                }
                write!(f, ";")
            }
            Statement::Delete { conditions } => {
                write!(f, "DELETE")?;
                if !conditions.is_empty() {
                    write!(f, " WHERE ")?;
                    for (i, cond) in conditions.iter().enumerate() {
                        if i > 0 {
                            write!(f, " AND ")?;
                        }
                        write!(f, "{} {:?} {}", cond.field, cond.op, format_value(&cond.value))?;
                    }
                }
                write!(f, ";")
            }
            Statement::Update { set, conditions } => {
                write!(f, "UPDATE SET ")?;
                for (i, assignment) in set.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} = {}", assignment.field, format_value(&assignment.value))?;
                }
                if !conditions.is_empty() {
                    write!(f, " WHERE ")?;
                    for (i, cond) in conditions.iter().enumerate() {
                        if i > 0 {
                            write!(f, " AND ")?;
                        }
                        write!(f, "{} {:?} {}", cond.field, cond.op, format_value(&cond.value))?;
                    }
                }
                write!(f, ";")
            }
            Statement::Trace {
                prompt,
                answer,
                decompose,
                layers,
                positions,
                save,
            } => {
                write!(f, "TRACE {}", quote_string(prompt))?;
                if let Some(a) = answer {
                    write!(f, " FOR {}", quote_string(a))?;
                }
                if *decompose {
                    write!(f, " DECOMPOSE")?;
                }
                if let Some(range) = layers {
                    write!(f, " LAYERS {}..{}", range.start, range.end)?;
                }
                if let Some(pos) = positions {
                    write!(f, " POSITIONS {:?}", pos)?;
                }
                if let Some(s) = save {
                    write!(f, " SAVE {}", quote_string(s))?;
                }
                write!(f, ";")
            }
            Statement::Diff {
                a,
                b,
                layer,
                relation,
                limit,
                into_patch,
                into_report,
            } => {
                write!(f, "DIFF {:?} {:?}", a, b)?;
                if let Some(l) = layer {
                    write!(f, " LAYER {}", l)?;
                }
                if let Some(rel) = relation {
                    write!(f, " RELATION {}", quote_string(rel))?;
                }
                if let Some(lim) = limit {
                    write!(f, " LIMIT {}", lim)?;
                }
                if let Some(patch) = into_patch {
                    write!(f, " INTO PATCH {}", quote_string(patch))?;
                }
                if let Some(report) = into_report {
                    write!(f, " INTO REPORT {}", quote_string(report))?;
                }
                write!(f, ";")
            }
            Statement::Export {
                vindex,
                output,
                format,
            } => {
                write!(f, "EXPORT {:?} INTO {}", vindex, quote_string(output))?;
                write!(f, " FORMAT {:?}", format)?;
                write!(f, ";")
            }
            _ => write!(f, "{:?}", self),
        }
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => quote_string(s),
        Value::Number(n) => n.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::List(vals) => {
            let formatted: Vec<String> = vals.iter().map(format_value).collect();
            format!("({})", formatted.join(", "))
        }
    }
}

pub fn quote_string(s: &str) -> String {
    let mut quoted = String::with_capacity(s.len() + 2);
    quoted.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn quote_list(values: &[String]) -> String {
    let quoted: Vec<String> = values.iter().map(|value| quote_string(value)).collect();
    quoted.join(", ")
}

#[derive(Debug, Clone)]
pub enum VindexRef {
    Path(String),
    Current,
}

#[derive(Debug, Clone)]
pub enum UseTarget {
    Vindex(String),
    Model { id: String, auto_extract: bool },
    Remote(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainMode {
    /// EXPLAIN WALK — pure vindex gate KNN, no attention
    Walk,
    /// EXPLAIN INFER — full inference with feature trace
    Infer,
}

/// Analysis mode for ANALYZE INFER statement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisMode {
    /// Fact probe: exact span resolution, truth/false attribution
    FactProbe,
    /// Workflow probe: mass-based attribution, coherence tracking
    WorkflowProbe,
}

impl std::fmt::Display for AnalysisMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisMode::FactProbe => write!(f, "FACT_PROBE"),
            AnalysisMode::WorkflowProbe => write!(f, "WORKFLOW_PROBE"),
        }
    }
}

/// Display mode for DESCRIBE and SHOW RELATIONS output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DescribeMode {
    /// Full detail: relation labels, also-tokens, layer ranges, multi-layer hits.
    Verbose,
    /// Default. Compact: top edges only, primary layer, no also-tokens.
    #[default]
    Brief,
    /// No probe labels — pure model signal.
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerBand {
    /// L0-13: morphological, syntactic, code structure
    Syntax,
    /// L14-27: semantic/factual features (default for DESCRIBE)
    Knowledge,
    /// L28-33: formatting/output features
    Output,
    /// All layers: syntax + knowledge + output
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractLevel {
    /// Default: gate + embed + down_meta (~3 GB f16)
    Browse,
    /// + attention weights (~6 GB f16), enables INFER
    Inference,
    /// + up, norms, lm_head (~10 GB f16), enables COMPILE
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileTarget {
    /// COMPILE ... INTO MODEL — produce safetensors/gguf
    Model,
    /// COMPILE ... INTO VINDEX — bake patches into clean vindex
    Vindex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkMode {
    Hybrid,
    Pure,
    Dense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Safetensors,
    Gguf,
    Json,
    Csv,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Safetensors => write!(f, "SAFETENSORS"),
            OutputFormat::Gguf => write!(f, "GGUF"),
            OutputFormat::Json => write!(f, "JSON"),
            OutputFormat::Csv => write!(f, "CSV"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    KeepSource,
    KeepTarget,
    HighestConfidence,
}

/// Conflict resolution for COMPILE INTO VINDEX when multiple patches touch
/// the same (layer, feature) slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileConflict {
    /// Last patch in the apply order wins (default).
    LastWins,
    /// The patch with the highest confidence on that slot wins.
    HighestConfidence,
    /// Abort the compile if any slot has conflicting writes.
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    FfnGate,
    FfnDown,
    FfnUp,
    Embeddings,
    AttnOv,
    AttnQk,
}

#[derive(Debug, Clone)]
pub struct Range {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectSource {
    Edges,
    Features,
    Entities,
    Tokens,
}

#[derive(Debug, Clone)]
pub enum Field {
    Star,
    Named(String),
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub field: String,
    pub op: CompareOp,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub enum CompareOp {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Like,
    In,
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Integer(i64),
    List(Vec<Value>),
}

#[derive(Debug, Clone)]
pub struct NearestClause {
    pub entity: String,
    pub layer: u32,
}

#[derive(Debug, Clone)]
pub struct OrderBy {
    pub field: String,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenGroupBy {
    Layer,
    Band,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSortBy {
    MaxScore,
    Distinct,
    EntityLike,
    Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

/// Graph export formats for EXPORT statement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphExportFormat {
    Turtle,
    Neo4j,
    JsonLd,
    Graphml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracePositionMode {
    Last,
    All,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub field: String,
    pub value: Value,
}
