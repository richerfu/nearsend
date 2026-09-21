//! Bottom navigation rendering for the home page.

use super::*;
use crate::ui::components::logo::Logo;
use crate::ui::icons::{app_icon, paths};
use crate::ui::theme::sizing;

impl HomePage {
    pub(super) fn navigate_to(&self, pathname: &str, cx: &mut Context<Self>) {
        let current = RouterState::global(cx).location.pathname.clone();
        if current.as_ref() == pathname {
            return;
        }

        let pathname_owned = pathname.to_string();
        crate::ui::router_history::RouterHistoryState::global_mut(cx)
            .history
            .push(crate::ui::router_history::HistoryEntry::new(
                pathname_owned.clone(),
            ));
        RouterState::global_mut(cx).location.pathname = pathname_owned.into();
        cx.notify();
    }

    pub(super) fn render_bottom_nav(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let items: [(TabType, &'static str, &'static str); 3] = [
            (TabType::Receive, "接收", paths::WIFI),
            (TabType::Send, "发送", paths::SEND),
            (TabType::Settings, "设置", paths::SETTINGS),
        ];

        h_flex()
            .w_full()
            .flex_none()
            .items_center()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.7))
            .bg(cx.theme().background)
            .children(items.iter().map(|(tab, label, icon_path)| {
                div()
                    .flex_1()
                    .child(self.render_bottom_nav_item(*tab, label, *icon_path, cx))
            }))
            .into_any_element()
    }

    pub(super) fn render_side_nav(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let items: [(TabType, &'static str, &'static str); 3] = [
            (TabType::Receive, "接收", paths::WIFI),
            (TabType::Send, "发送", paths::SEND),
            (TabType::Settings, "设置", paths::SETTINGS),
        ];

        v_flex()
            .w(sizing::SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(cx.theme().border.opacity(0.75))
            .bg(cx.theme().background)
            .p(px(12.))
            .child(
                h_flex()
                    .h(px(56.))
                    .px(px(10.))
                    .items_center()
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(cx.theme().border.opacity(0.65))
                    .child(Logo::new().size(30.))
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child("NearSend"),
                    ),
            )
            .child(
                div()
                    .px(px(10.))
                    .pt(px(16.))
                    .pb(px(6.))
                    .text_xs()
                    .font_medium()
                    .text_color(cx.theme().muted_foreground)
                    .child("功能"),
            )
            .child(v_flex().w_full().gap(px(4.)).children(items.iter().map(
                |(tab, label, icon_path)| self.render_side_nav_item(*tab, label, *icon_path, cx),
            )))
            .child(div().flex_1())
            .child(
                div()
                    .px(px(10.))
                    .pb(px(8.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("NearSend {}", crate::APP_VERSION)),
            )
            .into_any_element()
    }

    fn render_side_nav_item(
        &mut self,
        tab: TabType,
        label: &'static str,
        icon_path: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.current_tab == tab;
        let foreground = if selected {
            cx.theme().foreground
        } else {
            cx.theme().muted_foreground
        };

        h_flex()
            .id(format!("sidebar-tab-{tab:?}"))
            .w_full()
            .h(px(42.))
            .px(px(12.))
            .gap(px(10.))
            .items_center()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .text_color(foreground)
            .when(selected, |this| {
                this.bg(cx.theme().secondary).font_semibold()
            })
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.current_tab = tab;
                cx.notify();
            }))
            .child(app_icon(icon_path, gpui_component::Size::Small, foreground))
            .child(div().text_sm().child(label))
            .into_any_element()
    }

    fn render_bottom_nav_item(
        &mut self,
        tab: TabType,
        label: &'static str,
        icon_path: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.current_tab == tab;
        let tab_id = format!("tab-{:?}", tab);
        let text_color = if selected {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        };
        let icon_el = app_icon(icon_path, gpui_component::Size::Small, text_color);

        div()
            .id(tab_id)
            .w_full()
            .h(sizing::TAB_BAR_HEIGHT)
            .py(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                this.current_tab = tab;
            }))
            .child(
                v_flex()
                    .items_center()
                    .gap(px(3.))
                    .text_color(text_color)
                    .child(
                        div()
                            .w(px(48.))
                            .h(px(28.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(selected, |this| this.bg(cx.theme().primary.opacity(0.12)))
                            .child(icon_el),
                    )
                    .child(
                        div()
                            .text_xs()
                            .when(selected, |this| this.font_semibold())
                            .child(label),
                    ),
            )
            .into_any_element()
    }
}
