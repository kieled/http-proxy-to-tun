use crate::icons::{codes as fa, icon};
use iced::widget::{button, checkbox, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};

use crate::app::{Message, ProxyVpnApp, ThemeMode};
use crate::theme::colors::{dark, tokens};
use crate::theme::styles;

/// Checkbox style function that creates a styled checkbox based on checked state
fn checkbox_style(is_checked: bool) -> impl Fn(&Theme, checkbox::Status) -> checkbox::Style {
    move |_theme, _status| checkbox::Style {
        background: Background::Color(if is_checked {
            dark::ACCENT
        } else {
            dark::SURFACE
        }),
        icon_color: dark::BACKGROUND,
        border: Border {
            color: if is_checked {
                dark::ACCENT
            } else {
                dark::BORDER
            },
            width: tokens::BORDER_WIDTH,
            radius: tokens::RADIUS_SM.into(),
        },
        text_color: Some(dark::TEXT),
    }
}

pub fn view(app: &ProxyVpnApp) -> Element<'_, Message> {
    let content = column![
        view_connection_settings(app),
        Space::new().height(16),
        view_appearance_settings(app),
        Space::new().height(16),
        view_about_section(),
    ]
    .spacing(0)
    .padding(Padding::from([20, 20]))
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(dark::BACKGROUND)),
            ..Default::default()
        })
        .into()
}

fn view_connection_settings(app: &ProxyVpnApp) -> Element<'_, Message> {
    let is_killswitch_enabled = app.settings.killswitch_enabled;
    let killswitch_checkbox = checkbox(is_killswitch_enabled)
        .label("Enable Killswitch")
        .on_toggle(Message::ToggleKillswitch)
        .text_size(14)
        .spacing(10)
        .style(checkbox_style(is_killswitch_enabled));

    let killswitch_desc = text("Block all network traffic if VPN disconnects unexpectedly")
        .size(12)
        .color(dark::TEXT_SECONDARY);

    container(
        column![
            row![
                icon(fa::SHIELD, 14.0),
                Space::new().width(10),
                text("Connection").size(14).color(dark::TEXT),
            ]
            .align_y(Alignment::Center),
            Space::new().height(16),
            killswitch_checkbox,
            Space::new().height(6),
            row![Space::new().width(26), killswitch_desc],
        ]
        .spacing(0),
    )
    .padding(Padding::from([16, 16]))
    .width(Length::Fill)
    .style(styles::card_style)
    .into()
}

fn view_appearance_settings(app: &ProxyVpnApp) -> Element<'_, Message> {
    // Theme selector buttons
    let theme_buttons = row![
        theme_button(ThemeMode::Light, app.settings.theme_mode, fa::SUN, "Light"),
        theme_button(ThemeMode::Dark, app.settings.theme_mode, fa::MOON, "Dark"),
        theme_button(
            ThemeMode::System,
            app.settings.theme_mode,
            fa::CIRCLE_HALF_STROKE,
            "System"
        ),
    ]
    .spacing(8);

    let minimize_to_tray = app.settings.minimize_to_tray;
    let minimize_checkbox = checkbox(minimize_to_tray)
        .label("Close to system tray")
        .on_toggle(Message::ToggleMinimizeToTray)
        .text_size(14)
        .spacing(10)
        .style(checkbox_style(minimize_to_tray));

    let start_minimized = app.settings.start_minimized;
    let start_minimized_checkbox = checkbox(start_minimized)
        .label("Start minimized")
        .on_toggle(Message::ToggleStartMinimized)
        .text_size(14)
        .spacing(10)
        .style(checkbox_style(start_minimized));

    container(
        column![
            row![
                icon(fa::PALETTE, 14.0),
                Space::new().width(10),
                text("Appearance").size(14).color(dark::TEXT),
            ]
            .align_y(Alignment::Center),
            Space::new().height(16),
            text("Theme").size(13).color(dark::TEXT_SECONDARY),
            Space::new().height(8),
            theme_buttons,
            Space::new().height(20),
            row![
                icon(fa::WINDOW_MINIMIZE, 12.0),
                Space::new().width(10),
                minimize_checkbox,
            ]
            .align_y(Alignment::Center),
            Space::new().height(12),
            row![
                icon(fa::EYE_SLASH, 12.0),
                Space::new().width(10),
                start_minimized_checkbox,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(0),
    )
    .padding(Padding::from([16, 16]))
    .width(Length::Fill)
    .style(styles::card_style)
    .into()
}

fn theme_button(mode: ThemeMode, current: ThemeMode, icon_code: char, label: &str) -> Element<'_, Message> {
    let is_selected = mode == current;

    button(
        row![
            icon(icon_code, 14.0),
            Space::new().width(6),
            text(label).size(13),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(Message::ThemeModeChanged(mode))
    .padding(Padding::from([10, 16]))
    .style(move |theme, status| {
        if is_selected {
            let base = styles::primary_button_style(theme, status);
            button::Style {
                border: Border {
                    color: dark::ACCENT,
                    width: tokens::BORDER_WIDTH,
                    radius: tokens::RADIUS_SM.into(),
                },
                ..base
            }
        } else {
            styles::secondary_button_style(theme, status)
        }
    })
    .into()
}

fn view_about_section() -> Element<'static, Message> {
    let version = env!("CARGO_PKG_VERSION");

    container(
        column![
            row![
                icon(fa::INFO_CIRCLE, 14.0),
                Space::new().width(10),
                text("About").size(14).color(dark::TEXT),
            ]
            .align_y(Alignment::Center),
            Space::new().height(16),
            row![
                text("ProxyVPN").size(14).color(dark::TEXT),
                Space::new().width(Length::Fill),
                container(
                    text(format!("v{}", version))
                        .size(12)
                        .color(dark::ACCENT)
                )
                .padding(Padding::from([4, 8]))
                .style(|_| container::Style {
                    background: Some(Background::Color(styles::color_with_alpha(
                        dark::ACCENT,
                        0.1
                    ))),
                    border: Border {
                        radius: tokens::RADIUS_SM.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ]
            .align_y(Alignment::Center),
            Space::new().height(12),
            text("Route system traffic through HTTP proxy via TUN interface")
                .size(12)
                .color(dark::TEXT_SECONDARY),
            Space::new().height(8),
            text("Requires root privileges or CAP_NET_ADMIN capability")
                .size(11)
                .color(dark::MUTED),
        ]
        .spacing(0),
    )
    .padding(Padding::from([16, 16]))
    .width(Length::Fill)
    .style(styles::card_style)
    .into()
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeMode::Dark => write!(f, "Dark"),
            ThemeMode::Light => write!(f, "Light"),
            ThemeMode::System => write!(f, "System"),
        }
    }
}
