//! Retained render-node storage.
//!
//! `WidgetTree` owns render-node identity and orchestration. This module owns
//! the representation of an individual retained render node. Common topology,
//! geometry and dirtiness stay on [`RenderNode`]; feature-local mutable state
//! is stored only in the corresponding closed [`RenderFeatureState`] variant.

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use incular_config::Constraints;
use incular_core::{DirtyFlags, Offset, Size};
use incular_rendering::DisplayList;
use incular_text::TextLayout;

use crate::{
    advanced_scrolling::{
        DraggableScrollableState, RawScrollbar, TwoDimensionalViewportLayout, WheelLayout,
    },
    tree::{ButtonState, RenderKind, RenderObjectId, Widget},
};

mod layers;
mod update;
pub(crate) use layers::RenderLayers;
pub(crate) use update::RenderInvalidation;

/// Geometry that is meaningful for every retained render node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RenderGeometry {
    pub(crate) size: Size,
    pub(crate) offset: Offset,
    pub(crate) constraints: Option<Constraints>,
    pub(crate) baseline: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderButtonState {
    pub(crate) visual: ButtonState,
    pub(crate) hovered: bool,
    pub(crate) pressed: bool,
    pub(crate) focused: bool,
}

impl Default for RenderButtonState {
    fn default() -> Self {
        Self {
            visual: ButtonState::Normal,
            hovered: false,
            pressed: false,
            focused: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderTextState {
    pub(crate) layout: Option<Arc<TextLayout>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderSelectableTextState {
    pub(crate) text: RenderTextState,
    pub(crate) focused: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderTextFieldState {
    pub(crate) text: RenderTextState,
    pub(crate) content_revision: u64,
    pub(crate) visual_revision: u64,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
    pub(crate) focused: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderRawScrollbarState {
    pub(crate) scrollbar: Option<RawScrollbar>,
}

/// Overlay-scrollbar interaction state owned by scroll viewports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RenderScrollState {
    pub(crate) hovered: bool,
    pub(crate) dragging: bool,
}

#[derive(Default)]
pub(crate) struct RenderWheelState {
    pub(crate) layout: Option<WheelLayout<Widget>>,
}

#[derive(Default)]
pub(crate) struct RenderDraggableSheetState {
    pub(crate) state: Option<DraggableScrollableState>,
}

#[derive(Default)]
pub(crate) struct RenderTwoDimensionalState {
    pub(crate) layout: Option<TwoDimensionalViewportLayout<Widget>>,
}

/// Closed set of feature-local mutable state. This is deliberately not an
/// `Any` property bag; the payload constructor derives the state from kind.
pub(crate) enum RenderFeatureState {
    None,
    Text(RenderTextState),
    SelectableText(RenderSelectableTextState),
    TextField(RenderTextFieldState),
    Button(RenderButtonState),
    RawScrollbar(RenderRawScrollbarState),
    Scroll(RenderScrollState),
    Wheel(RenderWheelState),
    DraggableSheet(RenderDraggableSheetState),
    TwoDimensional(RenderTwoDimensionalState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FeatureClass {
    None,
    Text,
    SelectableText,
    TextField,
    Button,
    RawScrollbar,
    Scroll,
    Wheel,
    DraggableSheet,
    TwoDimensional,
}

impl RenderFeatureState {
    fn class_for(kind: &RenderKind) -> FeatureClass {
        match kind {
            RenderKind::Text { .. } | RenderKind::Banner { .. } => FeatureClass::Text,
            RenderKind::SelectableText { .. } => FeatureClass::SelectableText,
            RenderKind::TextField { .. } => FeatureClass::TextField,
            RenderKind::Button { .. } => FeatureClass::Button,
            RenderKind::RawScrollbar { .. } => FeatureClass::RawScrollbar,
            RenderKind::Scroll { .. } | RenderKind::SliverViewport { .. } => FeatureClass::Scroll,
            RenderKind::ListWheelScrollView { .. } | RenderKind::ListWheelViewport { .. } => {
                FeatureClass::Wheel
            }
            RenderKind::DraggableScrollableSheet { .. } => FeatureClass::DraggableSheet,
            RenderKind::TwoDimensionalScrollView { .. }
            | RenderKind::TwoDimensionalViewport { .. } => FeatureClass::TwoDimensional,
            _ => FeatureClass::None,
        }
    }

    fn class(&self) -> FeatureClass {
        match self {
            Self::None => FeatureClass::None,
            Self::Text(_) => FeatureClass::Text,
            Self::SelectableText(_) => FeatureClass::SelectableText,
            Self::TextField(_) => FeatureClass::TextField,
            Self::Button(_) => FeatureClass::Button,
            Self::RawScrollbar(_) => FeatureClass::RawScrollbar,
            Self::Scroll(_) => FeatureClass::Scroll,
            Self::Wheel(_) => FeatureClass::Wheel,
            Self::DraggableSheet(_) => FeatureClass::DraggableSheet,
            Self::TwoDimensional(_) => FeatureClass::TwoDimensional,
        }
    }

    fn for_kind(kind: &RenderKind) -> Self {
        match Self::class_for(kind) {
            FeatureClass::None => Self::None,
            FeatureClass::Text => Self::Text(RenderTextState::default()),
            FeatureClass::SelectableText => {
                Self::SelectableText(RenderSelectableTextState::default())
            }
            FeatureClass::TextField => Self::TextField(RenderTextFieldState::default()),
            FeatureClass::Button => Self::Button(RenderButtonState::default()),
            FeatureClass::RawScrollbar => Self::RawScrollbar(RenderRawScrollbarState::default()),
            FeatureClass::Scroll => Self::Scroll(RenderScrollState::default()),
            FeatureClass::Wheel => Self::Wheel(RenderWheelState::default()),
            FeatureClass::DraggableSheet => {
                Self::DraggableSheet(RenderDraggableSheetState::default())
            }
            FeatureClass::TwoDimensional => {
                Self::TwoDimensional(RenderTwoDimensionalState::default())
            }
        }
    }

    fn reconcile_for_kind(&mut self, kind: &RenderKind) {
        let expected = Self::class_for(kind);
        if self.class() != expected {
            *self = Self::for_kind(kind);
        }
    }
}

/// One typed payload owned by a retained render node.
pub(crate) struct RenderObjectPayload {
    pub(crate) kind: RenderKind,
    pub(crate) feature: RenderFeatureState,
    pub(crate) cache: DisplayList,
    pub(crate) layers: RenderLayers,
}

impl RenderObjectPayload {
    pub(crate) fn new(kind: RenderKind, layers: RenderLayers) -> Self {
        let feature = RenderFeatureState::for_kind(&kind);
        Self {
            kind,
            feature,
            cache: DisplayList::new(),
            layers,
        }
    }

    /// Replace declarative configuration while preserving local retained state
    /// only when the render kind owns the same feature-state family.
    pub(crate) fn replace_kind(&mut self, kind: RenderKind) {
        self.feature.reconcile_for_kind(&kind);
        self.kind = kind;
    }
}

/// Persistent render-tree node. The shell intentionally contains no feature-
/// specific fields.
pub(crate) struct RenderNode {
    pub(crate) parent: Option<RenderObjectId>,
    pub(crate) children: Vec<RenderObjectId>,
    pub(crate) geometry: RenderGeometry,
    pub(crate) dirty: DirtyFlags,
    pub(crate) object: RenderObjectPayload,
}

// Field-style geometry access keeps layout code readable without re-exposing
// feature state through the common shell.
impl Deref for RenderNode {
    type Target = RenderGeometry;

    fn deref(&self) -> &Self::Target {
        &self.geometry
    }
}

impl DerefMut for RenderNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.geometry
    }
}

impl RenderNode {
    pub(crate) fn text_layout(&self) -> Option<&Arc<TextLayout>> {
        match &self.object.feature {
            RenderFeatureState::Text(state) => state.layout.as_ref(),
            RenderFeatureState::SelectableText(state) => state.text.layout.as_ref(),
            RenderFeatureState::TextField(state) => state.text.layout.as_ref(),
            _ => None,
        }
    }

    pub(crate) fn text_layout_cloned(&self) -> Option<Arc<TextLayout>> {
        self.text_layout().cloned()
    }

    pub(crate) fn set_text_layout(&mut self, layout: Arc<TextLayout>) {
        match &mut self.object.feature {
            RenderFeatureState::Text(state) => state.layout = Some(layout),
            RenderFeatureState::SelectableText(state) => state.text.layout = Some(layout),
            RenderFeatureState::TextField(state) => state.text.layout = Some(layout),
            _ => debug_assert!(false, "text layout assigned to non-text render object"),
        }
    }

    pub(crate) fn button_state(&self) -> Option<&RenderButtonState> {
        match &self.object.feature {
            RenderFeatureState::Button(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn button_state_mut(&mut self) -> Option<&mut RenderButtonState> {
        match &mut self.object.feature {
            RenderFeatureState::Button(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn selectable_text_state(&self) -> Option<&RenderSelectableTextState> {
        match &self.object.feature {
            RenderFeatureState::SelectableText(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn selectable_text_state_mut(&mut self) -> Option<&mut RenderSelectableTextState> {
        match &mut self.object.feature {
            RenderFeatureState::SelectableText(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn text_field_state(&self) -> Option<&RenderTextFieldState> {
        match &self.object.feature {
            RenderFeatureState::TextField(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn text_field_state_mut(&mut self) -> Option<&mut RenderTextFieldState> {
        match &mut self.object.feature {
            RenderFeatureState::TextField(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn raw_scrollbar_state_mut(&mut self) -> Option<&mut RenderRawScrollbarState> {
        match &mut self.object.feature {
            RenderFeatureState::RawScrollbar(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn scroll_state(&self) -> Option<&RenderScrollState> {
        match &self.object.feature {
            RenderFeatureState::Scroll(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn scroll_state_mut(&mut self) -> Option<&mut RenderScrollState> {
        match &mut self.object.feature {
            RenderFeatureState::Scroll(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn wheel_state(&self) -> Option<&RenderWheelState> {
        match &self.object.feature {
            RenderFeatureState::Wheel(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn wheel_state_mut(&mut self) -> Option<&mut RenderWheelState> {
        match &mut self.object.feature {
            RenderFeatureState::Wheel(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn draggable_sheet_state(&self) -> Option<&RenderDraggableSheetState> {
        match &self.object.feature {
            RenderFeatureState::DraggableSheet(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn draggable_sheet_state_mut(&mut self) -> Option<&mut RenderDraggableSheetState> {
        match &mut self.object.feature {
            RenderFeatureState::DraggableSheet(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn two_dimensional_state(&self) -> Option<&RenderTwoDimensionalState> {
        match &self.object.feature {
            RenderFeatureState::TwoDimensional(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn two_dimensional_state_mut(&mut self) -> Option<&mut RenderTwoDimensionalState> {
        match &mut self.object.feature {
            RenderFeatureState::TwoDimensional(state) => Some(state),
            _ => None,
        }
    }
}

// Architecture regression guard. The legacy `RenderObject` at e7590b1 was
// measured at 968 bytes on x86_64 MSVC. The typed representation is 768 bytes
// on that same toolchain. Keep enough headroom for harmless alignment changes
// while preventing feature work from rebuilding the old one-field-per-feature
// state bag. This is a compile-time invariant rather than test-only code so it
// is enforced by every normal build (and complies with Incular's test-placement
// policy).
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<RenderNode>() <= 800);
    assert!(std::mem::size_of::<RenderFeatureState>() <= 192);
};
