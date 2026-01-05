pub mod diagram;
pub mod parse;
pub mod validate;

// Re-export key types
pub use diagram::{DiagramFormat, DiagramStyle};
pub use parse::GherkinError;
pub use validate::{Severity, ValidationResult, ValidationWarning};

/// Result of processing gherkin blocks.
pub struct ProcessResult {
    /// Parsed features
    pub features: Vec<gherkin::Feature>,
    /// Semantic validation warnings
    pub validation: ValidationResult,
}

/// Parse and validate gherkin content blocks.
/// Takes raw gherkin strings (e.g. from `ast_util::collect_code_blocks` filtered by lang).
pub fn process_blocks(blocks: &[String], filename: &str) -> Result<ProcessResult, GherkinError> {
    if blocks.is_empty() {
        return Ok(ProcessResult {
            features: Vec::new(),
            validation: ValidationResult::default(),
        });
    }

    let features = parse::parse_gherkin_blocks(blocks, filename)?;
    let validation = validate::validate_features(&features);

    Ok(ProcessResult {
        features,
        validation,
    })
}

/// Generate a diagram from parsed features.
pub fn generate_diagram(
    features: &[gherkin::Feature],
    format: DiagramFormat,
    style: DiagramStyle,
) -> String {
    let generator: Box<dyn diagram::DiagramGenerator> = match format {
        DiagramFormat::Mermaid => Box::new(diagram::mermaid::MermaidGenerator),
        DiagramFormat::D2 => Box::new(diagram::d2::D2Generator),
    };
    generator.generate(features, style)
}
