use crate::icons::{codes as fa, icon};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::app::{LogLevel, Message, ProxyVpnApp};
use crate::theme::colors::{dark, tokens};
use crate::theme::styles;

pub fn view(app: &ProxyVpnApp) -> Element<'_, Message> {
    let header = view_header(app);
    let log_list = view_log_list(app);

    let content = column![header, Space::new().height(16), log_list]
        .spacing(0)
        .padding(Padding::from([20, 20]))
        .width(Length::Fill)
        .height(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(dark::BACKGROUND)),
            ..Default::default()
        })
        .into()
}

fn view_header(app: &ProxyVpnApp) -> Element<'_, Message> {
    let log_count = container(
        text(format!("{}", app.logs.len()))
            .size(11)
            .color(dark::TEXT_SECONDARY),
    )
    .padding(Padding::from([2, 8]))
    .style(|_| container::Style {
        background: Some(Background::Color(dark::SURFACE)),
        border: Border {
            radius: tokens::RADIUS_FULL.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let clear_btn = button(
        row![
            icon(fa::TRASH, 12.0),
            Space::new().width(6),
            text("Clear").size(12),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(Message::ClearLogs)
    .padding(Padding::from([8, 12]))
    .style(styles::ghost_button_style);

    row![
        text("Logs").size(15).color(dark::TEXT),
        Space::new().width(10),
        log_count,
        Space::new().width(Length::Fill),
        clear_btn,
    ]
    .align_y(Alignment::Center)
    .into()
}

fn view_log_list(app: &ProxyVpnApp) -> Element<'_, Message> {
    let log_entries: Vec<Element<Message>> = app
        .logs
        .iter()
        .rev()
        .map(|entry| {
            let (icon_code, icon_color) = match entry.level {
                LogLevel::Info => (fa::INFO_CIRCLE, dark::TEXT_SECONDARY),
                LogLevel::Error => (fa::CIRCLE_EXCLAMATION, dark::ERROR),
            };

            let level_color = match entry.level {
                LogLevel::Info => dark::TEXT_SECONDARY,
                LogLevel::Error => dark::ERROR,
            };

            container(
                row![
                    // Timestamp
                    text(&entry.timestamp)
                        .size(11)
                        .color(dark::MUTED)
                        .width(Length::Fixed(65.0)),
                    Space::new().width(12),
                    // Level icon
                    container(icon(icon_code, 12.0))
                        .style(move |_| container::Style {
                            text_color: Some(icon_color),
                            ..Default::default()
                        }),
                    Space::new().width(8),
                    // Level text
                    text(entry.level.as_str())
                        .size(11)
                        .color(level_color)
                        .width(Length::Fixed(45.0)),
                    Space::new().width(12),
                    // Message
                    text(&entry.message).size(12).color(dark::TEXT),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([10, 12]))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(dark::SURFACE)),
                border: Border {
                    color: dark::BORDER,
                    width: tokens::BORDER_WIDTH,
                    radius: tokens::RADIUS_SM.into(),
                },
                ..Default::default()
            })
            .into()
        })
        .collect();

    let log_column: Element<Message> = if log_entries.is_empty() {
        container(
            column![
                icon(fa::FILE_LINES, 48.0),
                Space::new().height(16),
                text("No log entries").size(14).color(dark::TEXT_SECONDARY),
                Space::new().height(8),
                text("Logs will appear here when events occur")
                    .size(12)
                    .color(dark::MUTED),
            ]
            .align_x(Alignment::Center)
            .spacing(0),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
    } else {
        scrollable(column(log_entries).spacing(6).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    container(log_column)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
