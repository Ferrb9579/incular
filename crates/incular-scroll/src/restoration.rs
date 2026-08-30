use incular_core::{RestorationKey, RestorationScope};

#[derive(Clone)]
pub(crate) struct ScrollRestoration {
    pub(crate) scope: RestorationScope,
    pub(crate) key: RestorationKey,
}

pub(crate) fn restored_scroll_offset(value: serde_json::Value) -> Option<f32> {
    let offset = value.get("offset")?.as_f64()? as f32;
    (offset.is_finite() && offset >= 0.).then_some(offset)
}

pub(crate) fn persist_scroll_offset(restoration: Option<ScrollRestoration>, offset: f32) {
    if let Some(restoration) = restoration
        && offset.is_finite()
        && offset >= 0.
    {
        restoration
            .scope
            .set_json(&restoration.key, serde_json::json!({ "offset": offset }));
    }
}
