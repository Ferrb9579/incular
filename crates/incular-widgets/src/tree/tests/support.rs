//! Shared fixtures and inspection helpers for retained-tree tests.

pub(super) use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub(super) use serde_json::{Value, json};

use super::*;
pub(super) use crate::drag_drop::DragDropContext;
pub(super) use crate::internal::ActionSurface;
pub(super) use crate::scrolling::{
    CustomScrollView, SliverFixedExtentList, SliverList, SliverVariedExtentList,
};
pub(super) use crate::{
    AbsorbPointer, DismissDirection, Dismissible, DragTarget, Draggable, Expanded, Flexible,
    GestureDetector, IgnorePointer, IndexedStack, Positioned, SizedBox, Spacer,
};

#[derive(Default)]
pub(super) struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

pub(super) fn fixed_sliver_list<W>(
    item_count: usize,
    item_extent: f32,
    controller: ScrollController,
    builder: impl Fn(usize) -> W + 'static,
) -> Widget
where
    W: Into<Widget> + 'static,
{
    CustomScrollView::new(vec![Box::new(SliverFixedExtentList::new(
        item_count,
        item_extent,
        builder,
    )) as Box<dyn crate::scrolling::Sliver>])
    .controller(controller)
    .into()
}

impl incular_core::RestorationBackend for MemoryRestorationBackend {
    fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
        self.0.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[RestorationKey], value: Value) {
        self.0.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        self.0.borrow_mut().remove(path);
    }
}

pub(super) fn restoration_key(value: &str) -> RestorationKey {
    RestorationKey::new(value).unwrap()
}

pub(super) fn restoration_scope() -> RestorationScope {
    RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
        .child_unchecked(restoration_key("window"))
        .child_unchecked(restoration_key("main"))
}

pub(super) fn rect_origins(list: &DisplayList) -> Vec<Offset> {
    let mut transforms = vec![Offset::ZERO];
    let mut origins = Vec::new();
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(*transforms.last().unwrap() + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::Rect { rect, .. } => {
                origins.push(rect.origin + *transforms.last().unwrap());
            }
            PaintCommand::GlyphRun { .. }
            | PaintCommand::Image { .. }
            | PaintCommand::RRect { .. }
            | PaintCommand::Border { .. }
            | PaintCommand::FillPath { .. }
            | PaintCommand::StrokePath { .. }
            | PaintCommand::PushClip { .. }
            | PaintCommand::PushClipRRect { .. }
            | PaintCommand::PushClipPath { .. }
            | PaintCommand::PushClipOval { .. }
            | PaintCommand::PopClip
            | PaintCommand::PushOpacity { .. }
            | PaintCommand::PopOpacity
            | PaintCommand::PushBlur { .. }
            | PaintCommand::PushDropShadow { .. }
            | PaintCommand::PushColorFilter { .. }
            | PaintCommand::PushBlend { .. }
            | PaintCommand::PopEffect => {}
        }
    }
    origins
}
pub(super) fn glyph_origins(list: &DisplayList) -> Vec<Offset> {
    let mut transforms = vec![Offset::ZERO];
    let mut origins = Vec::new();
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(*transforms.last().unwrap() + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::GlyphRun { run, .. } => {
                origins.push(run.origin + *transforms.last().unwrap());
            }
            PaintCommand::Rect { .. }
            | PaintCommand::Image { .. }
            | PaintCommand::RRect { .. }
            | PaintCommand::Border { .. }
            | PaintCommand::FillPath { .. }
            | PaintCommand::StrokePath { .. }
            | PaintCommand::PushClip { .. }
            | PaintCommand::PushClipRRect { .. }
            | PaintCommand::PushClipPath { .. }
            | PaintCommand::PushClipOval { .. }
            | PaintCommand::PopClip
            | PaintCommand::PushOpacity { .. }
            | PaintCommand::PopOpacity
            | PaintCommand::PushBlur { .. }
            | PaintCommand::PushDropShadow { .. }
            | PaintCommand::PushColorFilter { .. }
            | PaintCommand::PushBlend { .. }
            | PaintCommand::PopEffect => {}
        }
    }
    origins
}
pub(super) fn rrect_origins(list: &DisplayList) -> Vec<Offset> {
    let mut transforms = vec![Offset::ZERO];
    let mut origins = Vec::new();
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(*transforms.last().unwrap() + transform.translation_offset());
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::RRect { rrect, .. } => {
                origins.push(rrect.rect.origin + *transforms.last().unwrap());
            }
            _ => {}
        }
    }
    origins
}
pub(super) fn box_(key: u64) -> Widget {
    Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(key)
}
