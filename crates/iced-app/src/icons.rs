//! Font Awesome icons support for iced 0.14
//!
//! Provides Font Awesome icon rendering using text widgets.

use iced::widget::text;
use iced::{Color, Element, Font};

/// Font Awesome 6 Free Solid font
pub const FA_SOLID: Font = Font::with_name("Font Awesome 6 Free");

// Icon unicode codepoints (Font Awesome 6)
#[allow(dead_code)]
pub mod codes {
    pub const POWER_OFF: char = '\u{f011}';
    pub const GEAR: char = '\u{f013}';
    pub const CLOCK: char = '\u{f017}';
    pub const CHEVRON_LEFT: char = '\u{f053}';
    pub const CHEVRON_DOWN: char = '\u{f078}';
    pub const GLOBE: char = '\u{f0ac}';
    pub const PLUS: char = '\u{f067}';
    pub const SCROLL: char = '\u{f70e}';
    pub const TRASH: char = '\u{f1f8}';
    pub const PEN: char = '\u{f304}';
    pub const PENCIL: char = '\u{f303}';
    pub const CHECK: char = '\u{f00c}';
    pub const XMARK: char = '\u{f00d}';
    pub const INFO_CIRCLE: char = '\u{f05a}';
    pub const EXCLAMATION_TRIANGLE: char = '\u{f071}';
    pub const CIRCLE_EXCLAMATION: char = '\u{f06a}';
    pub const SERVER: char = '\u{f233}';
    pub const NETWORK_WIRED: char = '\u{f6ff}';
    pub const SHIELD: char = '\u{f132}';
    pub const PALETTE: char = '\u{f53f}';
    pub const MOON: char = '\u{f186}';
    pub const SUN: char = '\u{f185}';
    pub const DESKTOP: char = '\u{f390}';
    pub const FILE_LINES: char = '\u{f15c}';
    pub const CIRCLE_HALF_STROKE: char = '\u{f042}';
    pub const WINDOW_MINIMIZE: char = '\u{f2d1}';
    pub const EYE_SLASH: char = '\u{f070}';
    pub const USER: char = '\u{f007}';
    pub const LOCK: char = '\u{f023}';
}

/// Create a Font Awesome icon element (solid style)
pub fn icon<'a, Message: 'a>(code: char, size: f32) -> Element<'a, Message> {
    text(code.to_string())
        .font(FA_SOLID)
        .size(size)
        .into()
}

/// Create a Font Awesome icon element with custom color
#[allow(dead_code)]
pub fn icon_with_color<'a, Message: 'a>(code: char, size: f32, color: Color) -> Element<'a, Message> {
    text(code.to_string())
        .font(FA_SOLID)
        .size(size)
        .color(color)
        .into()
}
