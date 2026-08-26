//! Shared shadcn-inspired mobile chrome: headers, icon buttons, cards.

use crate::ui::icons::{app_icon, paths};
use crate::ui::theme::{radius, sizing, spacing};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, Context, ElementId, IntoElement, SharedString,
    Window,
};
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _,
};

/// Dialog title that reserves space for the absolute close icon.
///
/// gpui-component places the close button at the top-right of the dialog;
/// without extra padding a long title paints underneath it.
pub fn dialog_title(title: impl Into<SharedString>) -> impl IntoElement {
    div()
        .w_full()
        .min_w(px(0.))
        .pr(px(32.))
        .line_height(px(22.))
        .child(
            div()
                .w_full()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_normal()
                .child(title.into()),
        )
}

pub fn page_header(
    title: impl Into<SharedString>,
    back: impl IntoElement,
    trailing: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .h(sizing::HEADER_HEIGHT)
        .px(spacing::PAGE)
        .items_center()
        .gap(px(4.))
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.65))
        .child(back)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .truncate()
                .text_lg()
                .font_semibold()
                .text_color(cx.theme().foreground)
                .child(title.into()),
        )
        .child(trailing)
}

pub fn back_icon_button<V: 'static>(
    id: impl Into<ElementId>,
    cx: &mut Context<V>,
    on_back: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let hover = cx.theme().muted;
    Button::new(id)
        .ghost()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(cx.theme().transparent)
                .foreground(cx.theme().foreground)
                .hover(hover)
                .active(hover),
        )
        .h(sizing::ICON_BUTTON)
        .w(sizing::ICON_BUTTON)
        .p(px(0.))
        .rounded_full()
        .child(app_icon(paths::ARROW_LEFT, Size::Small, cx.theme().foreground))
        .on_click(cx.listener(move |this, _event, window, cx| on_back(this, window, cx)))
}

pub fn header_icon_button<V: 'static>(
    id: impl Into<ElementId>,
    icon_path: &'static str,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let hover = cx.theme().muted;
    Button::new(id)
        .ghost()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(cx.theme().transparent)
                .foreground(cx.theme().foreground)
                .hover(hover)
                .active(hover),
        )
        .h(sizing::ICON_BUTTON)
        .w(sizing::ICON_BUTTON)
        .p(px(0.))
        .rounded_full()
        .child(app_icon(icon_path, Size::Small, cx.theme().foreground))
        .on_click(cx.listener(move |this, _event, window, cx| on_click(this, window, cx)))
}

/// Circular muted icon button used on Receive / Send toolbars.
pub fn circle_icon_button<V: 'static>(
    id: impl Into<ElementId>,
    icon_path: &'static str,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let size = sizing::TOUCH;
    div()
        .id(id)
        .w(size)
        .h(size)
        .rounded_full()
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .cursor_pointer()
        .child(app_icon(icon_path, Size::Small, cx.theme().foreground))
        .on_click(cx.listener(on_click))
}

pub fn circle_icon_slot(icon_el: impl IntoElement, cx: &App) -> impl IntoElement {
    div()
        .w(sizing::ICON_BUTTON)
        .h(sizing::ICON_BUTTON)
        .rounded_full()
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .child(icon_el)
}

pub fn surface_card(cx: &App) -> gpui::Div {
    div()
        .w_full()
        .rounded(radius::LG)
        .border_1()
        .border_color(cx.theme().border.opacity(0.8))
        .bg(cx.theme().background)
}

pub fn muted_card(cx: &App) -> gpui::Div {
    div()
        .w_full()
        .rounded(radius::LG)
        .border_1()
        .border_color(cx.theme().border.opacity(0.55))
        .bg(cx.theme().secondary)
}

pub fn section_title(title: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .px(px(2.))
        .text_sm()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(title.into())
}

pub fn empty_state(icon_path: &'static str, title: &str, detail: &str, cx: &App) -> AnyElement {
    v_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap(spacing::SM)
        .py(px(48.))
        .px(spacing::PAGE)
        .child(
            div()
                .w(px(56.))
                .h(px(56.))
                .rounded_full()
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .child(app_icon(icon_path, Size::Large, cx.theme().muted_foreground)),
        )
        .child(
            div()
                .text_base()
                .font_semibold()
                .text_color(cx.theme().foreground)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child(detail.to_string()),
        )
        .into_any_element()
}

