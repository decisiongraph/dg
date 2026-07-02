//! Markdown parser using comrak with GFM extensions

use comrak::{parse_document, Arena, Options};

use crate::options::RenderOptions;
use crate::types::RenderedDocument;
use crate::walker::walk_document;

/// Parse markdown source into a rendered document
pub fn parse_markdown(source: &str, options: &RenderOptions) -> RenderedDocument {
    let arena = Arena::new();
    let opts = comrak_options();
    let root = parse_document(&arena, source, &opts);
    walk_document(root, options)
}

/// Build comrak options with all GFM extensions enabled
fn comrak_options() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.extension.footnotes = true;
    opts.extension.description_lists = true;
    opts.extension.superscript = true;
    opts.parse.smart = true;
    opts.render.r#unsafe = true; // allow raw HTML for <details>, <kbd> etc
    opts
}
