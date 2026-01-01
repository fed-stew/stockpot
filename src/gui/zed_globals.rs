use std::sync::Arc;

use gpui::Global;

pub struct GlobalLanguageRegistry(pub Arc<zed_language::LanguageRegistry>);

impl Global for GlobalLanguageRegistry {}
