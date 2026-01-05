//! Math rendering module - LaTeX/MathML to Unicode terminal output
//!
//! Moved from the original tui-math crate.

pub mod canvas_widget;
pub mod mathbox;
pub mod renderer;
pub mod unicode_maps;
pub mod widget;

pub use canvas_widget::CanvasMathWidget;
pub use mathbox::MathBox;
pub use renderer::{MathRenderer, RenderError};
pub use widget::{MathWidget, MathWidgetState, StatefulMathWidget as StatefulMathWidgetLegacy};
