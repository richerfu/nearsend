//! Appearance settings and a draft color editor; cancel never applies the draft.

use super::*;
use crate::ui::theme::{
    accent_color_hex, accent_foreground, parse_accent_color, radius, ACCENT_PRESETS,
};
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_component::input::InputEvent;
use gpui_component::slider::Slider;

impl HomePage {
    pub(super) fn sync_system_bar_theme(&self, cx: &mut Context<Self>) {
        use crate::platform::openharmony::NearSendPlatformExt as _;
        let app = cx.global::<crate::GlobalOpenHarmonyApp>().0.clone();
        let dark = self
            .settings_state
            .theme_mode
            .resolve(self.system_appearance)
            .is_dark();
        cx.spawn(async move |_, _| {
            if let Err(error) = app.set_system_bar_theme(dark).await {
                log::warn!("failed to update system bar theme: {error}");
            }
        })
        .detach();
    }

    pub(super) fn open_theme_color_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = parse_accent_color(&self.settings_state.custom_theme_color)
            .unwrap_or_else(crate::ui::theme::brand_primary);
        let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(initial));
        picker.update(cx, |state, cx| state.set_value(initial, window, cx));
        let input = picker.read(cx).hex_input().clone();
        input.update(cx, |state, cx| {
            state.set_value(accent_color_hex(initial), window, cx);
        });
        // Re-render the preview and validation message as the draft changes.
        cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify())
            .detach();
        cx.subscribe(&picker, |_, _, _: &ColorPickerEvent, cx| cx.notify())
            .detach();
        let home = cx.entity();

        window.open_dialog(cx, move |dialog, _window, cx| {
            let input_for_ok = input.clone();
            let home_for_ok = home.clone();
            let draft = parse_accent_color(input.read(cx).value().as_str());
            let preview = draft.unwrap_or(initial);
            let sliders = picker.read(cx).sliders().clone();

            dialog
                .title(dialog_title("自定义主题色"))
                .overlay(true)
                .w(px(340.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(16.))
                        .child(
                            div().grid().grid_cols(3).gap(px(8.)).children(
                                ACCENT_PRESETS
                                    .iter()
                                    .enumerate()
                                    .map(|(index, (label, value))| {
                                        let color: gpui::Hsla = gpui::rgb(*value).into();
                                        let selected = draft.map(accent_color_hex).as_deref()
                                            == Some(accent_color_hex(color).as_str());
                                        let picker = picker.clone();
                                        v_flex()
                                            .id(("theme-color-preset", index))
                                            .items_center()
                                            .justify_center()
                                            .min_h(px(64.))
                                            .gap(px(4.))
                                            .rounded(radius::MD)
                                            .cursor_pointer()
                                            .on_click(move |_, window, cx| {
                                                picker.update(cx, |state, cx| {
                                                    state.select_color(color, window, cx);
                                                });
                                            })
                                            .child(
                                                div()
                                                    .size(px(36.))
                                                    .rounded_full()
                                                    .bg(color)
                                                    .border_2()
                                                    .border_color(if selected {
                                                        cx.theme().foreground
                                                    } else {
                                                        cx.theme().border
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when(selected, |this| {
                                                        this.child(app_icon(
                                                            paths::CHECK,
                                                            Size::Small,
                                                            accent_foreground(color),
                                                        ))
                                                    }),
                                            )
                                            .child(div().text_xs().child(*label))
                                    }),
                            ),
                        )
                        .child(
                            v_flex().w_full().gap(px(4.)).children(
                                [
                                    ("色相", sliders.hue()),
                                    ("饱和度", sliders.saturation()),
                                    ("明度", sliders.lightness()),
                                ]
                                .into_iter()
                                .map(|(label, state)| {
                                    h_flex()
                                        .w_full()
                                        .min_h(px(36.))
                                        .items_center()
                                        .gap(px(12.))
                                        .child(div().w(px(48.)).text_xs().child(label))
                                        .child(Slider::new(state).flex_1())
                                }),
                            ),
                        )
                        .child(Input::new(&input).large().w_full())
                        .child(
                            div()
                                .text_xs()
                                .text_color(if draft.is_some() {
                                    cx.theme().muted_foreground
                                } else {
                                    cx.theme().danger
                                })
                                .child(if draft.is_some() {
                                    "选择颜色，或输入十六进制色值，如 #2563EB"
                                } else {
                                    "请输入有效的颜色：#RGB 或 #RRGGBB"
                                }),
                        )
                        .child(
                            div()
                                .w_full()
                                .py(px(12.))
                                .rounded(radius::MD)
                                .text_center()
                                .bg(preview)
                                .text_color(accent_foreground(preview))
                                .child("颜色预览"),
                        ),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "theme-color",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_, _, cx| {
                    let Some(color) = parse_accent_color(input_for_ok.read(cx).value().as_str())
                    else {
                        return false;
                    };
                    home_for_ok.update(cx, |this, cx| {
                        this.settings_state.custom_theme_color = accent_color_hex(color);
                        this.settings_state.color_mode = ColorMode::Custom;
                        this.apply_theme(cx);
                        this.persist_settings();
                    });
                    true
                })
        });
    }
}
