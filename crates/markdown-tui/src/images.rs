//! Image rendering via ratatui-image (feature-gated)

/// Placeholder for image rendering support
/// TODO: implement actual image rendering with ratatui-image
pub fn is_available() -> bool {
    #[cfg(feature = "images")]
    {
        true
    }
    #[cfg(not(feature = "images"))]
    {
        false
    }
}
