//! Reusable style functions for consistent UI across the app

#![allow(dead_code)]

use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color};

use super::colors::{dark, tokens};

/// Create a color with alpha transparency
pub fn color_with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

/// Card container style - surface background with rounded corners
pub fn card_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(dark::SURFACE)),
        border: Border {
            color: dark::BORDER,
            width: tokens::BORDER_WIDTH,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Page background style
pub fn page_background(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(dark::BACKGROUND)),
        ..Default::default()
    }
}

/// Primary button style (accent color)
pub fn primary_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let (bg, text_color) = match status {
        button::Status::Active => (dark::ACCENT, dark::BACKGROUND),
        button::Status::Hovered => (dark::ACCENT_HOVER, dark::BACKGROUND),
        button::Status::Pressed => (dark::ACCENT_HOVER, dark::BACKGROUND),
        button::Status::Disabled => (color_with_alpha(dark::ACCENT, 0.5), dark::BACKGROUND),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Secondary button style (surface background, text color)
pub fn secondary_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let (bg, border_color) = match status {
        button::Status::Active => (dark::SURFACE, dark::BORDER),
        button::Status::Hovered => (dark::SURFACE_HOVER, dark::MUTED),
        button::Status::Pressed => (dark::SURFACE_HOVER, dark::MUTED),
        button::Status::Disabled => (color_with_alpha(dark::SURFACE, 0.5), dark::BORDER),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: dark::TEXT,
        border: Border {
            color: border_color,
            width: tokens::BORDER_WIDTH,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Ghost button style (transparent background)
pub fn ghost_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Active => Color::TRANSPARENT,
        button::Status::Hovered => color_with_alpha(dark::SURFACE, 0.5),
        button::Status::Pressed => color_with_alpha(dark::SURFACE, 0.7),
        button::Status::Disabled => Color::TRANSPARENT,
    };

    let text_color = match status {
        button::Status::Disabled => color_with_alpha(dark::TEXT_SECONDARY, 0.5),
        _ => dark::TEXT_SECONDARY,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Danger button style (error color)
pub fn danger_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let (bg, text_color) = match status {
        button::Status::Active => (color_with_alpha(dark::ERROR, 0.15), dark::ERROR),
        button::Status::Hovered => (color_with_alpha(dark::ERROR, 0.25), dark::ERROR),
        button::Status::Pressed => (color_with_alpha(dark::ERROR, 0.3), dark::ERROR),
        button::Status::Disabled => (color_with_alpha(dark::ERROR, 0.1), color_with_alpha(dark::ERROR, 0.5)),
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Nav button style (header navigation)
pub fn nav_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Active => Color::TRANSPARENT,
        button::Status::Hovered => color_with_alpha(dark::SURFACE, 0.6),
        button::Status::Pressed => color_with_alpha(dark::SURFACE, 0.8),
        button::Status::Disabled => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: dark::TEXT_SECONDARY,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Proxy selector button style (transparent background)
pub fn proxy_selector_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Active => Color::TRANSPARENT,
        button::Status::Hovered => color_with_alpha(dark::SURFACE, 0.4),
        button::Status::Pressed => color_with_alpha(dark::SURFACE, 0.6),
        button::Status::Disabled => Color::TRANSPARENT,
    };

    let border_color = match status {
        button::Status::Hovered | button::Status::Pressed => dark::BORDER,
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(bg)),
        text_color: dark::TEXT,
        border: Border {
            color: border_color,
            width: tokens::BORDER_WIDTH,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

/// Text input style
pub fn text_input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Active => dark::BORDER,
        text_input::Status::Hovered => dark::MUTED,
        text_input::Status::Focused { .. } => dark::ACCENT,
        text_input::Status::Disabled => color_with_alpha(dark::BORDER, 0.5),
    };

    text_input::Style {
        background: Background::Color(dark::SURFACE),
        border: Border {
            color: border_color,
            width: tokens::BORDER_WIDTH,
            radius: tokens::RADIUS_SM.into(),
        },
        icon: dark::TEXT_SECONDARY,
        placeholder: dark::MUTED,
        value: dark::TEXT,
        selection: color_with_alpha(dark::ACCENT, 0.3),
    }
}

/// List item button style
pub fn list_item_style(is_selected: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme: &iced::Theme, status: button::Status| {
        let bg = match (is_selected, status) {
            (true, _) => color_with_alpha(dark::ACCENT, 0.1),
            (false, button::Status::Hovered) => color_with_alpha(dark::SURFACE, 0.5),
            (false, button::Status::Pressed) => color_with_alpha(dark::SURFACE, 0.7),
            _ => Color::TRANSPARENT,
        };

        let border_color = if is_selected {
            color_with_alpha(dark::ACCENT, 0.3)
        } else {
            Color::TRANSPARENT
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color: dark::TEXT,
            border: Border {
                color: border_color,
                width: if is_selected { tokens::BORDER_WIDTH } else { 0.0 },
                radius: tokens::RADIUS_SM.into(),
            },
            ..Default::default()
        }
    }
}

/// Section header container style
pub fn section_header_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(dark::SURFACE)),
        border: Border {
            color: dark::BORDER,
            width: tokens::BORDER_WIDTH,
            radius: tokens::RADIUS_SM.into(),
        },
        ..Default::default()
    }
}
