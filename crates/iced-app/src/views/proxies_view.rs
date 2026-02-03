use crate::icons::{codes as fa, icon};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::app::{Message, ProxyVpnApp};
use crate::theme::colors::{dark, tokens};
use crate::theme::styles;

pub fn view(app: &ProxyVpnApp) -> Element<'_, Message> {
    let content = if app.show_proxy_form {
        view_proxy_form(app)
    } else {
        view_proxy_list(app)
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(dark::BACKGROUND)),
            ..Default::default()
        })
        .into()
}

fn view_proxy_list(app: &ProxyVpnApp) -> Element<'_, Message> {
    let header = container(
        row![
            text("Proxies").size(15).color(dark::TEXT),
            Space::new().width(Length::Fill),
            button(
                row![
                    icon(fa::PLUS, 12.0),
                    Space::new().width(6),
                    text("Add").size(13),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::ShowProxyForm)
            .padding(Padding::from([8, 14]))
            .style(styles::primary_button_style),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0, 4]));

    let proxy_items: Vec<Element<'_, Message>> = app
        .proxies
        .iter()
        .map(|proxy| {
            let is_selected = app.selected_proxy_id.as_ref() == Some(&proxy.id);

            // Selection indicator
            let indicator: Element<'_, Message> = if is_selected {
                container(
                    container(Space::new().width(8).height(8))
                        .style(|_| container::Style {
                            background: Some(Background::Color(dark::ACCENT)),
                            border: Border {
                                radius: tokens::RADIUS_FULL.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                )
                .width(24)
                .center_x(Length::Fill)
                .into()
            } else {
                Space::new().width(24).into()
            };

            // Proxy info
            let proxy_info = column![
                text(&proxy.name).size(14).color(dark::TEXT),
                text(format!("{}:{}", proxy.host, proxy.port))
                    .size(12)
                    .color(dark::TEXT_SECONDARY),
            ]
            .spacing(2);

            // Select button
            let select_btn = button(
                row![indicator, Space::new().width(8), proxy_info,].align_y(Alignment::Center),
            )
            .on_press(Message::SelectProxy(proxy.id.clone()))
            .padding(Padding::from([12, 12]))
            .width(Length::Fill)
            .style(styles::list_item_style(is_selected));

            // Action buttons
            let edit_btn = button(
                container(icon(fa::PENCIL, 14.0))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .on_press(Message::EditProxy(proxy.id.clone()))
            .padding(Padding::from([10, 12]))
            .style(styles::ghost_button_style);

            let delete_btn = button(
                container(icon(fa::TRASH, 14.0))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .on_press(Message::DeleteProxy(proxy.id.clone()))
            .padding(Padding::from([10, 12]))
            .style(styles::danger_button_style);

            container(
                row![select_btn, edit_btn, delete_btn,].align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .into()
        })
        .collect();

    let proxy_list: Element<'_, Message> = if proxy_items.is_empty() {
        container(
            column![
                icon(fa::SERVER, 48.0),
                Space::new().height(16),
                text("No proxies configured")
                    .size(14)
                    .color(dark::TEXT_SECONDARY),
                Space::new().height(8),
                text("Click + Add to create your first proxy")
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
        scrollable(column(proxy_items).spacing(4).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    column![header, Space::new().height(16), proxy_list]
        .spacing(0)
        .padding(Padding::from([20, 20]))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_proxy_form(app: &ProxyVpnApp) -> Element<'_, Message> {
    let form = &app.proxy_form;
    let is_edit = form.editing_id.is_some();
    let title = if is_edit { "Edit Proxy" } else { "Add Proxy" };

    // Name input with icon
    let name_section = form_field(
        fa::SERVER,
        "Name",
        text_input("My Proxy", &form.name)
            .on_input(Message::ProxyNameChanged)
            .padding(Padding::from([10, 12]))
            .size(14)
            .style(styles::text_input_style),
    );

    // Host & Port
    let host_input = text_input("proxy.example.com", &form.host)
        .on_input(Message::ProxyHostChanged)
        .padding(Padding::from([10, 12]))
        .size(14)
        .style(styles::text_input_style);

    let port_input = text_input("8080", &form.port)
        .on_input(Message::ProxyPortChanged)
        .padding(Padding::from([10, 12]))
        .size(14)
        .width(Length::Fixed(90.0))
        .style(styles::text_input_style);

    let host_section = column![
        row![
            icon(fa::NETWORK_WIRED, 12.0),
            Space::new().width(8),
            text("Host & Port").size(12).color(dark::TEXT_SECONDARY),
        ]
        .align_y(Alignment::Center),
        Space::new().height(8),
        row![host_input, Space::new().width(8), port_input].align_y(Alignment::Center),
    ]
    .spacing(0);

    // Authentication
    let username_input = text_input("Username", &form.username)
        .on_input(Message::UsernameChanged)
        .padding(Padding::from([10, 12]))
        .size(14)
        .style(styles::text_input_style);

    let password_input = text_input("Password", &form.password)
        .on_input(Message::PasswordChanged)
        .secure(true)
        .padding(Padding::from([10, 12]))
        .size(14)
        .style(styles::text_input_style);

    let auth_section = column![
        row![
            icon(fa::USER, 12.0),
            Space::new().width(8),
            text("Authentication (optional)")
                .size(12)
                .color(dark::TEXT_SECONDARY),
        ]
        .align_y(Alignment::Center),
        Space::new().height(8),
        username_input,
        Space::new().height(8),
        password_input,
    ]
    .spacing(0);

    // Advanced settings
    let tun_name_input = text_input("tun0", &form.tun_name)
        .on_input(Message::TunNameChanged)
        .padding(Padding::from([10, 12]))
        .size(13)
        .width(Length::Fixed(100.0))
        .style(styles::text_input_style);

    let tun_cidr_input = text_input("10.255.255.1/30", &form.tun_cidr)
        .on_input(Message::TunCidrChanged)
        .padding(Padding::from([10, 12]))
        .size(13)
        .style(styles::text_input_style);

    let advanced_section = container(
        column![
            row![
                icon(fa::LOCK, 12.0),
                Space::new().width(8),
                text("Advanced Settings")
                    .size(12)
                    .color(dark::TEXT_SECONDARY),
            ]
            .align_y(Alignment::Center),
            Space::new().height(12),
            row![
                column![
                    text("TUN Interface").size(11).color(dark::TEXT_SECONDARY),
                    Space::new().height(4),
                    tun_name_input,
                ],
                Space::new().width(12),
                column![
                    text("TUN CIDR").size(11).color(dark::TEXT_SECONDARY),
                    Space::new().height(4),
                    tun_cidr_input,
                ]
                .width(Length::Fill),
            ]
            .align_y(Alignment::End),
        ]
        .spacing(0),
    )
    .padding(Padding::from([12, 12]))
    .style(styles::card_style);

    // Validation
    let can_save = !form.name.is_empty() && !form.host.is_empty() && form.port.parse::<u16>().is_ok();

    // Buttons
    let cancel_btn = button(text("Cancel").size(13).color(dark::TEXT_SECONDARY))
        .on_press(Message::HideProxyForm)
        .padding(Padding::from([10, 20]))
        .style(styles::secondary_button_style);

    let save_btn = button(
        row![
            icon(fa::CHECK, 12.0),
            Space::new().width(6),
            text("Save").size(13),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10, 20]))
    .style(move |theme, status| {
        if can_save {
            styles::primary_button_style(theme, status)
        } else {
            styles::primary_button_style(theme, button::Status::Disabled)
        }
    });

    let save_btn = if can_save {
        save_btn.on_press(Message::SaveProxy)
    } else {
        save_btn
    };

    let content = column![
        text(title).size(18).color(dark::TEXT),
        Space::new().height(24),
        name_section,
        Space::new().height(16),
        host_section,
        Space::new().height(16),
        auth_section,
        Space::new().height(20),
        advanced_section,
        Space::new().height(Length::Fill),
        row![Space::new().width(Length::Fill), cancel_btn, Space::new().width(12), save_btn,]
            .align_y(Alignment::Center),
    ]
    .spacing(0)
    .padding(Padding::from([20, 20]))
    .width(Length::Fill)
    .height(Length::Fill);

    content.into()
}

/// Helper to create a form field with icon and label
fn form_field<'a>(
    icon_code: char,
    label: &'a str,
    input: iced::widget::TextInput<'a, Message>,
) -> Element<'a, Message> {
    column![
        row![
            icon(icon_code, 12.0),
            Space::new().width(8),
            text(label).size(12).color(dark::TEXT_SECONDARY),
        ]
        .align_y(Alignment::Center),
        Space::new().height(8),
        input,
    ]
    .spacing(0)
    .into()
}
