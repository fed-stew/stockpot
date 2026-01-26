use gpui::{div, prelude::*, px, rgb, Styled};

use super::ChatApp;

impl ChatApp {
    pub(super) fn render_error(&self) -> impl IntoElement {
        let theme = self.theme.clone();
        let error = self.error_message.clone();

        div().when_some(error, |d, msg| {
            d.px(px(16.))
                .py(px(8.))
                .bg(theme.error)
                .text_color(rgb(0xffffff))
                .text_size(px(13.))
                .child(format!("⚠️ {}", msg))
        })
    }
}
