//! Template residual caching for fast inference.
//!
//! Defines default templates for caching. Template residual computation
//! is done via the CLI command, not during extraction (due to dependency
//! chain constraints).

/// Default templates for caching.
/// These are common prompt patterns that benefit from cached residuals.
pub const DEFAULT_TEMPLATES: &[TemplateDef] = &[
    TemplateDef {
        name: "capital_of",
        pattern: "The capital of {} is",
        layer_start: 0,
        layer_end: 12,
    },
    TemplateDef {
        name: "lives_in",
        pattern: "{} lives in",
        layer_start: 0,
        layer_end: 12,
    },
    TemplateDef {
        name: "born_in",
        pattern: "{} was born in",
        layer_start: 0,
        layer_end: 12,
    },
];

/// Template definition.
#[derive(Debug, Clone, Copy)]
pub struct TemplateDef {
    /// Human-readable name for the template.
    pub name: &'static str,
    /// Pattern with {} placeholder for entity slot.
    pub pattern: &'static str,
    /// First layer to cache (typically 0).
    pub layer_start: usize,
    /// Last layer to cache (typically 12 for template-fixed regime).
    pub layer_end: usize,
}

impl TemplateDef {
    /// Get the layer range for this template.
    pub fn layer_range(&self) -> std::ops::RangeInclusive<usize> {
        self.layer_start..=self.layer_end
    }

    /// Get the number of layers in the cache range.
    pub fn layer_count(&self) -> usize {
        if self.layer_end >= self.layer_start {
            self.layer_end - self.layer_start + 1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_def() {
        let tmpl = DEFAULT_TEMPLATES[0];
        assert_eq!(tmpl.name, "capital_of");
        assert_eq!(tmpl.pattern, "The capital of {} is");
        assert_eq!(tmpl.layer_start, 0);
        assert_eq!(tmpl.layer_end, 12);
        assert_eq!(tmpl.layer_range(), 0..=12);
        assert_eq!(tmpl.layer_count(), 13);
    }
}
