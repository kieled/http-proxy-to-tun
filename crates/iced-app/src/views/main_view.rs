use crate::icons::{codes as fa, icon};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::app::{ConnectionStatus, Message, ProxyVpnApp};
use crate::theme::colors::{dark, tokens};
use crate::theme::styles;

pub fn view(app: &ProxyVpnApp) -> Element<'_, Message> {
    let content = column![
        // Top spacer
        Space::new().height(40),
        // Power button section
        view_power_button(app),
        // Status text
        Space::new().height(24),
        view_status_text(app),
        // Spacer to push status bar down
        Space::new().height(Length::Fill),
        // Bottom status bar
        view_status_bar(app),
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::new(24.0));

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_power_button(app: &ProxyVpnApp) -> Element<'_, Message> {
    let status = app.connection_status;
    let can_interact = matches!(
        status,
        ConnectionStatus::Connected | ConnectionStatus::Disconnected | ConnectionStatus::Error
    );

    // Determine colors based on status
    let (border_color, text_color, bg_color) = match status {
        ConnectionStatus::Connected => (
            dark::ACCENT,
            dark::ACCENT,
            styles::color_with_alpha(dark::ACCENT, 0.12),
        ),
        ConnectionStatus::Connecting => (
            dark::WARNING,
            dark::WARNING,
            styles::color_with_alpha(dark::WARNING, 0.12),
        ),
        ConnectionStatus::Disconnecting => (
            dark::WARNING,
            dark::WARNING,
            styles::color_with_alpha(dark::WARNING, 0.12),
        ),
        ConnectionStatus::Error => (
            dark::ERROR,
            dark::ERROR,
            styles::color_with_alpha(dark::ERROR, 0.12),
        ),
        ConnectionStatus::Disconnected => (dark::BORDER, dark::TEXT_SECONDARY, dark::SURFACE),
    };

    // Power icon from Font Awesome
    let power_icon: Element<'_, Message> = icon(fa::POWER_OFF, 48.0);

    let btn = button(
        container(power_icon)
            .width(128)
            .height(128)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding(0)
    .style(move |_theme, btn_status| {
        let current_border_color = if matches!(btn_status, button::Status::Hovered) && can_interact
        {
            match status {
                ConnectionStatus::Disconnected => dark::ACCENT,
                _ => border_color,
            }
        } else {
            border_color
        };

        button::Style {
            background: Some(Background::Color(bg_color)),
            border: Border {
                color: current_border_color,
                width: tokens::BORDER_WIDTH_THICK,
                radius: 64.0.into(), // Full circle (half of 128)
            },
            text_color,
            ..Default::default()
        }
    });

    let btn = if can_interact {
        if app.connection_status == ConnectionStatus::Connected {
            btn.on_press(Message::Disconnect)
        } else {
            btn.on_press(Message::Connect)
        }
    } else {
        btn
    };

    container(btn)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn view_status_text(app: &ProxyVpnApp) -> Element<'_, Message> {
    let (status_text, status_color) = match app.connection_status {
        ConnectionStatus::Connected => ("Connected", dark::ACCENT),
        ConnectionStatus::Connecting => ("Connecting...", dark::WARNING),
        ConnectionStatus::Disconnecting => ("Disconnecting...", dark::WARNING),
        ConnectionStatus::Error => ("Connection Error", dark::ERROR),
        ConnectionStatus::Disconnected => ("Disconnected", dark::TEXT_SECONDARY),
    };

    let status_label = text(status_text).size(18).color(status_color);

    // Show proxy name or error message below
    let subtitle = if app.connection_status == ConnectionStatus::Error {
        app.connection_error
            .as_ref()
            .map(|e| text(e).size(12).color(dark::ERROR))
    } else if matches!(
        app.connection_status,
        ConnectionStatus::Connected | ConnectionStatus::Connecting
    ) {
        app.get_selected_proxy()
            .map(|p| text(&p.name).size(12).color(dark::TEXT_SECONDARY))
    } else {
        None
    };

    let mut col = column![status_label]
        .spacing(6)
        .align_x(Alignment::Center);

    if let Some(sub) = subtitle {
        col = col.push(sub);
    }

    container(col)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn view_status_bar(app: &ProxyVpnApp) -> Element<'_, Message> {
    // IP address
    let ip_display = app.public_ip.as_deref().unwrap_or("---.---.---.---");

    // Connection duration
    let duration = app
        .uptime_string()
        .unwrap_or_else(|| "00:00:00".to_string());

    // Globe icon for IP
    let globe_icon: Element<'_, Message> = icon(fa::GLOBE, 14.0);

    // Clock icon for duration
    let clock_icon: Element<'_, Message> = icon(fa::CLOCK, 14.0);

    let bar = row![
        globe_icon,
        Space::new().width(8),
        text(ip_display).size(13).color(dark::TEXT_SECONDARY),
        Space::new().width(Length::Fill),
        clock_icon,
        Space::new().width(8),
        text(duration).size(13).color(dark::TEXT_SECONDARY),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([16, 20]));

    container(bar)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(dark::SURFACE)),
            border: Border {
                color: dark::BORDER,
                width: tokens::BORDER_WIDTH,
                radius: tokens::RADIUS_LG.into(),
            },
            ..Default::default()
        })
        .into()
}
