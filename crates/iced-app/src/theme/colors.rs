//! Color palette matching the React app design
//!
//! Based on oklch color space converted to sRGB

use iced::Color;

/// Dark theme colors (default)
pub mod dark {
    use super::*;

    /// Background: oklch(0.18 0 0)
    pub const BACKGROUND: Color = Color::from_rgb(0.145, 0.145, 0.145);

    /// Surface: oklch(0.22 0 0) - cards, panels
    pub const SURFACE: Color = Color::from_rgb(0.188, 0.188, 0.188);

    /// Surface elevated: slightly lighter for hover states
    pub const SURFACE_HOVER: Color = Color::from_rgb(0.22, 0.22, 0.22);

    /// Border: oklch(0.32 0 0)
    pub const BORDER: Color = Color::from_rgb(0.282, 0.282, 0.282);

    /// Muted: oklch(0.5 0 0)
    pub const MUTED: Color = Color::from_rgb(0.40, 0.40, 0.40);

    /// Primary text: oklch(0.95 0 0)
    pub const TEXT: Color = Color::from_rgb(0.941, 0.941, 0.941);

    /// Secondary text: oklch(0.7 0 0)
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.659, 0.659, 0.659);

    /// Accent/Success green: oklch(0.7 0.12 155) - pastel green
    pub const ACCENT: Color = Color::from_rgb(0.290, 0.725, 0.502);

    /// Accent hover: oklch(0.65 0.14 155) - slightly darker
    pub const ACCENT_HOVER: Color = Color::from_rgb(0.220, 0.670, 0.450);

    /// Warning: oklch(0.78 0.12 75) - pastel yellow/orange
    pub const WARNING: Color = Color::from_rgb(0.870, 0.720, 0.350);

    /// Error: oklch(0.65 0.14 25) - pastel red
    pub const ERROR: Color = Color::from_rgb(0.820, 0.420, 0.420);

    /// Transparent (for transparent backgrounds)
    #[allow(dead_code)]
    pub const TRANSPARENT: Color = Color::TRANSPARENT;
}

/// Light theme colors (reserved for future use)
#[allow(dead_code)]
pub mod light {
    use super::*;

    /// Background: oklch(1 0 0) - pure white
    pub const BACKGROUND: Color = Color::from_rgb(1.0, 1.0, 1.0);

    /// Surface: oklch(0.985 0 0) - off-white
    pub const SURFACE: Color = Color::from_rgb(0.97, 0.97, 0.97);

    /// Surface hover
    pub const SURFACE_HOVER: Color = Color::from_rgb(0.94, 0.94, 0.94);

    /// Border: oklch(0.9 0 0)
    pub const BORDER: Color = Color::from_rgb(0.88, 0.88, 0.88);

    /// Muted: oklch(0.7 0 0)
    pub const MUTED: Color = Color::from_rgb(0.659, 0.659, 0.659);

    /// Primary text: oklch(0.15 0 0)
    pub const TEXT: Color = Color::from_rgb(0.145, 0.145, 0.145);

    /// Secondary text: oklch(0.45 0 0)
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.40, 0.40, 0.40);

    /// Accent/Success green (same as dark)
    pub const ACCENT: Color = Color::from_rgb(0.290, 0.725, 0.502);

    /// Accent hover
    pub const ACCENT_HOVER: Color = Color::from_rgb(0.220, 0.670, 0.450);

    /// Warning (same as dark)
    pub const WARNING: Color = Color::from_rgb(0.870, 0.720, 0.350);

    /// Error (same as dark)
    pub const ERROR: Color = Color::from_rgb(0.820, 0.420, 0.420);

    /// Transparent
    pub const TRANSPARENT: Color = Color::TRANSPARENT;
}

/// Common design tokens
#[allow(dead_code)]
pub mod tokens {
    /// Border radius for cards/containers (4px equivalent)
    pub const RADIUS_SM: f32 = 4.0;

    /// Border radius for buttons (8px equivalent)
    pub const RADIUS_MD: f32 = 8.0;

    /// Border radius for larger elements (12px equivalent)
    pub const RADIUS_LG: f32 = 12.0;

    /// Full radius for circular elements
    pub const RADIUS_FULL: f32 = 9999.0;

    /// Border width for standard borders
    pub const BORDER_WIDTH: f32 = 1.0;

    /// Border width for emphasis (connection button)
    pub const BORDER_WIDTH_THICK: f32 = 4.0;
}
