use crate::ui::components::{device_badge::DeviceBadge, progress_bar::ProgressBar};
use crate::ui::icons::{app_icon, paths};
use crate::ui::theme::{radius, sizing, spacing};
use gpui::{div, prelude::*, px, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _,
};
use localsend::http::state::ClientInfo;
use localsend::model::discovery::DeviceType;

/// Device card matching LocalSend's DeviceListTile, styled as a shadcn card.
#[derive(IntoElement)]
pub struct DeviceCard {
    device: ClientInfo,
    is_favorite: bool,
    protocol_badge: Option<String>,
    ip_suffix_badge: Option<String>,
    name_override: Option<String>,
    info: Option<String>,
    progress: Option<f64>,
    on_select: Option<std::rc::Rc<dyn Fn(&ClientInfo, &mut Window, &mut gpui::App) + 'static>>,
    on_favorite_tap:
        Option<std::rc::Rc<dyn Fn(&ClientInfo, &mut Window, &mut gpui::App) + 'static>>,
}

impl DeviceCard {
    pub fn new(device: ClientInfo) -> Self {
        Self {
            device,
            is_favorite: false,
            protocol_badge: None,
            ip_suffix_badge: None,
            name_override: None,
            info: None,
            progress: None,
            on_select: None,
            on_favorite_tap: None,
        }
    }

    pub fn is_favorite(mut self, is_favorite: bool) -> Self {
        self.is_favorite = is_favorite;
        self
    }

    pub fn protocol_badge(mut self, label: impl Into<String>) -> Self {
        self.protocol_badge = Some(label.into());
        self
    }

    pub fn ip_suffix_badge(mut self, label: impl Into<String>) -> Self {
        self.ip_suffix_badge = Some(label.into());
        self
    }

    pub fn name_override(mut self, name: impl Into<String>) -> Self {
        self.name_override = Some(name.into());
        self
    }

    #[allow(dead_code)]
    pub fn info(mut self, info: impl Into<String>) -> Self {
        self.info = Some(info.into());
        self
    }

    #[allow(dead_code)]
    pub fn progress(mut self, progress: Option<f64>) -> Self {
        self.progress = progress;
        self
    }

    #[allow(dead_code)]
    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClientInfo, &mut Window, &mut gpui::App) + 'static,
    {
        self.on_select = Some(std::rc::Rc::new(handler));
        self
    }

    pub fn on_favorite_tap<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClientInfo, &mut Window, &mut gpui::App) + 'static,
    {
        self.on_favorite_tap = Some(std::rc::Rc::new(handler));
        self
    }
}

fn device_type_icon_path(device_type: &Option<DeviceType>) -> &'static str {
    match device_type {
        Some(DeviceType::Mobile) => paths::SMARTPHONE,
        Some(DeviceType::Desktop) => paths::MONITOR,
        Some(DeviceType::Web) => paths::GLOBE,
        Some(DeviceType::Server) | Some(DeviceType::Headless) => paths::SERVER,
        None => paths::SMARTPHONE,
    }
}

impl gpui::RenderOnce for DeviceCard {
    fn render(self, _window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let device = self.device.clone();
        let on_select = self.on_select.clone();
        let on_favorite_tap = self.on_favorite_tap.clone();
        let device_name = self.name_override.unwrap_or_else(|| device.alias.clone());
        let is_favorite = self.is_favorite;
        let protocol_badge = self
            .protocol_badge
            .clone()
            .unwrap_or_else(|| "LAN • HTTP".to_string());
        let ip_suffix_badge = self.ip_suffix_badge.clone();
        let info = self.info.clone();
        let progress = self.progress;
        let icon_path = device_type_icon_path(&device.device_type);

        let subtitle = if let Some(ref info_text) = info {
            div()
                .w_full()
                .overflow_hidden()
                .truncate()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(info_text.clone())
                .into_any_element()
        } else if let Some(progress_val) = progress {
            ProgressBar::new(Some(progress_val)).into_any_element()
        } else {
            h_flex()
                .gap(px(6.))
                .flex_wrap()
                .child(
                    DeviceBadge::new(protocol_badge)
                        .background_color(cx.theme().primary.opacity(0.14).into())
                        .foreground_color(cx.theme().primary.into())
                        .border_color(cx.theme().primary.opacity(0.28).into()),
                )
                .when_some(ip_suffix_badge, |this, tag| {
                    this.child(
                        DeviceBadge::new(tag)
                            .background_color(cx.theme().muted.into())
                            .foreground_color(cx.theme().foreground.into())
                            .border_color(cx.theme().border.into()),
                    )
                })
                .when(device.device_model.is_some(), |this| {
                    this.child(
                        DeviceBadge::new(device.device_model.clone().unwrap_or_default())
                            .background_color(cx.theme().muted.into())
                            .foreground_color(cx.theme().muted_foreground.into())
                            .border_color(cx.theme().border.opacity(0.6).into()),
                    )
                })
                .into_any_element()
        };

        div()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border.opacity(0.8))
            .rounded(radius::LG)
            .p(sizing::CARD_PADDING)
            .child(
                h_flex()
                    .items_center()
                    .gap(spacing::MD)
                    .w_full()
                    .child(
                        div()
                            .w(px(42.))
                            .h(px(42.))
                            .rounded(radius::MD)
                            .bg(cx.theme().muted)
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_none()
                            .child(app_icon(icon_path, Size::Small, cx.theme().foreground)),
                    )
                    .child(
                        v_flex()
                            .gap(px(4.))
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .w_full()
                                    .overflow_hidden()
                                    .truncate()
                                    .text_base()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(device_name),
                            )
                            .child(subtitle),
                    )
                    .child(if on_favorite_tap.is_some() || on_select.is_some() {
                        Button::new(format!("favorite-{}", device.token))
                            .ghost()
                            .h(sizing::TOUCH)
                            .w(sizing::TOUCH)
                            .p(px(0.))
                            .rounded_full()
                            .on_click(move |_event, window, cx| {
                                if let Some(ref handler) = on_favorite_tap {
                                    handler(&device, window, cx);
                                } else if let Some(ref handler) = on_select {
                                    handler(&device, window, cx);
                                }
                            })
                            .child(app_icon(
                                paths::HEART,
                                Size::Small,
                                if is_favorite {
                                    cx.theme().danger
                                } else {
                                    cx.theme().muted_foreground
                                },
                            ))
                            .into_any_element()
                    } else {
                        Button::new(format!("send-{}", device.token))
                            .primary()
                            .on_click(move |_event, window, cx| {
                                if let Some(ref handler) = on_select {
                                    handler(&device, window, cx);
                                }
                            })
                            .child("发送")
                            .into_any_element()
                    }),
            )
    }
}
