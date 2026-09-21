//! Home page render shell.

use super::*;
use crate::ui::responsive::ResponsiveLayout;

impl gpui::Render for HomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_selected_files_from_shared(cx);
        if !self.services_started {
            self.services_started = true;
            // Initialize select states for settings dropdowns
            self.init_select_states(window, cx);
            // Start server and discovery services
            self.start_services(cx);
        }

        let layout = ResponsiveLayout::current(window, cx);
        let content = match self.current_tab {
            TabType::Receive => receive_tab::render_receive_content(self, window, cx),
            TabType::Send => send_tab::render_send_content(self, window, cx),
            TabType::Settings => settings_tab::render_settings_content(self, window, cx),
        };

        if layout.is_desktop() {
            h_flex()
                .size_full()
                .bg(cx.theme().muted.opacity(0.35))
                .child(self.render_side_nav(cx))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .h_full()
                        .overflow_hidden()
                        .child(content),
                )
                .into_any_element()
        } else {
            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.))
                        .w_full()
                        .overflow_hidden()
                        .child(content),
                )
                .child(self.render_bottom_nav(cx))
                .into_any_element()
        }
    }
}
