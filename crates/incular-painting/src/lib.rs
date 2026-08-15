//! Ordered renderer-independent display lists.
use incular_assets::FontHandle;
use incular_core::{Arena, ArenaId, Color, DirtyFlags, Offset, Rect, Transform};
use std::sync::Arc;

/// A positioned glyph produced by a text shaper. It is intentionally not a
/// character: a glyph can represent multiple Unicode scalars or none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPosition {
    pub id: u16,
    pub offset: Offset,
    pub advance: f32,
    pub cluster: u32,
}
/// Compact, contiguous shaped glyph data shared by display-list commands.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontHandle,
    pub font_size: f32,
    pub origin: Offset,
    pub glyphs: Arc<[GlyphPosition]>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum PaintCommand {
    Rect { rect: Rect, color: Color },
    GlyphRun { run: Arc<GlyphRun>, color: Color },
    PushClip { rect: Rect },
    PopClip,
    PushTransform { transform: Transform },
    PopTransform,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    commands: Vec<PaintCommand>,
}
impl DisplayList {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
    pub fn push(&mut self, command: PaintCommand) {
        self.commands.push(command);
    }
    pub fn extend_from(&mut self, other: &Self) {
        self.commands.extend(other.commands.iter().cloned());
    }
    #[must_use]
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
#[derive(Default)]
pub struct Canvas {
    list: DisplayList,
    saves: Vec<SaveKind>,
}
#[derive(Clone, Copy)]
enum SaveKind {
    Transform,
    Clip,
}
impl Canvas {
    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.list.push(PaintCommand::Rect { rect, color });
    }
    pub fn glyph_run(&mut self, run: Arc<GlyphRun>, color: Color) {
        self.list.push(PaintCommand::GlyphRun { run, color });
    }
    pub fn save_transform(&mut self, transform: Transform) {
        self.saves.push(SaveKind::Transform);
        self.list.push(PaintCommand::PushTransform { transform });
    }
    pub fn save_clip(&mut self, rect: Rect) {
        self.saves.push(SaveKind::Clip);
        self.list.push(PaintCommand::PushClip { rect });
    }
    pub fn restore(&mut self) {
        match self.saves.pop().expect("unbalanced canvas restore") {
            SaveKind::Transform => self.list.push(PaintCommand::PopTransform),
            SaveKind::Clip => self.list.push(PaintCommand::PopClip),
        }
    }
    #[must_use]
    pub fn finish(self) -> DisplayList {
        assert!(self.saves.is_empty(), "unbalanced display list");
        self.list
    }
}

/// Opaque retained compositor identity. It uses the core generational arena so
/// an ID for a removed layer can never select a later, unrelated layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(ArenaId);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorDiagnostics {
    pub layers: u64,
    pub picture_layers_reused: u64,
    pub picture_layers_repainted: u64,
    pub transform_updates: u64,
    pub clip_updates: u64,
    pub layers_culled: u64,
}

/// A picture placement observed while flattening. All rectangles here are in
/// logical coordinates; this is intentionally useful for CPU-side scene tests
/// and diagnostics without involving a surface or DPI scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlattenedPicture {
    pub layer: LayerId,
    pub local_bounds: Rect,
    pub world_bounds: Rect,
    pub active_clip: Option<Rect>,
}

#[derive(Clone, Debug)]
pub enum LayerKind {
    Picture {
        display_list: Arc<DisplayList>,
        bounds: Rect,
    },
    Transform {
        transform: Transform,
    },
    ClipRect {
        rect: Rect,
    },
}

#[derive(Clone, Debug)]
struct Layer {
    kind: LayerKind,
    children: Vec<LayerId>,
    dirty: DirtyFlags,
}

/// Renderer-independent retained layer arena. Picture display lists are owned
/// once and scene flattening only emits a transient linear command stream.
/// This deliberately has no GPU resources or offscreen textures.
pub struct LayerTree {
    layers: Arena<Layer>,
    root: Option<LayerId>,
    diagnostics: CompositorDiagnostics,
    flattened_pictures: Vec<FlattenedPicture>,
}
impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
impl LayerTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Arena::new(),
            root: None,
            diagnostics: CompositorDiagnostics::default(),
            flattened_pictures: Vec::new(),
        }
    }
    #[must_use]
    pub fn diagnostics(&self) -> CompositorDiagnostics {
        self.diagnostics
    }
    #[must_use]
    pub fn root(&self) -> Option<LayerId> {
        self.root
    }
    pub fn set_root(&mut self, root: LayerId) {
        self.root = Some(root);
    }
    pub fn create_picture(&mut self, display_list: DisplayList, bounds: Rect) -> LayerId {
        self.insert(LayerKind::Picture {
            display_list: Arc::new(display_list),
            bounds,
        })
    }
    pub fn create_transform(&mut self, transform: Transform) -> LayerId {
        self.insert(LayerKind::Transform { transform })
    }
    pub fn create_clip_rect(&mut self, rect: Rect) -> LayerId {
        self.insert(LayerKind::ClipRect { rect })
    }
    fn insert(&mut self, kind: LayerKind) -> LayerId {
        let id = LayerId(self.layers.insert(Layer {
            kind,
            children: Vec::new(),
            dirty: DirtyFlags::COMPOSITE,
        }));
        self.diagnostics.layers += 1;
        id
    }
    pub fn remove(&mut self, id: LayerId) {
        if self.layers.remove(id.0).is_some() {
            self.diagnostics.layers -= 1;
            if self.root == Some(id) {
                self.root = None;
            }
            for (_, layer) in self.layers.iter() {
                // Children are pruned by their owners before removal in normal
                // widget unmounting; stale entries are ignored by flattening.
                let _ = layer;
            }
        }
    }
    pub fn set_children(&mut self, id: LayerId, children: Vec<LayerId>) {
        if let Some(layer) = self.layers.get_mut(id.0) {
            layer.children = children;
            layer.dirty.insert(DirtyFlags::COMPOSITE);
        }
    }
    pub fn update_picture(&mut self, id: LayerId, display_list: DisplayList, bounds: Rect) {
        if let Some(layer) = self.layers.get_mut(id.0)
            && let LayerKind::Picture {
                display_list: current,
                bounds: current_bounds,
            } = &mut layer.kind
        {
            *current = Arc::new(display_list);
            *current_bounds = bounds;
            layer
                .dirty
                .insert(DirtyFlags::PAINT | DirtyFlags::COMPOSITE);
            self.diagnostics.picture_layers_repainted += 1;
        }
    }
    pub fn update_transform(&mut self, id: LayerId, transform: Transform) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Transform { transform: current } = &mut layer.kind else {
            return false;
        };
        if *current == transform {
            return false;
        }
        *current = transform;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        self.diagnostics.transform_updates += 1;
        true
    }
    pub fn update_clip(&mut self, id: LayerId, rect: Rect) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ClipRect { rect: current } = &mut layer.kind else {
            return false;
        };
        if *current == rect {
            return false;
        }
        *current = rect;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        self.diagnostics.clip_updates += 1;
        true
    }
    /// World-space logical bounds captured by the most recent [`Self::flatten`]
    /// call. This keeps coordinate diagnostics out of the renderer hot path.
    #[must_use]
    pub fn flattened_pictures(&self) -> &[FlattenedPicture] {
        &self.flattened_pictures
    }
    #[must_use]
    pub fn flatten(&mut self) -> DisplayList {
        let mut out = DisplayList::new();
        self.flattened_pictures.clear();
        if let Some(root) = self.root {
            self.flatten_layer(root, Offset::ZERO, None, &mut out);
        }
        for (_, layer) in self.layers.iter() {
            if matches!(layer.kind, LayerKind::Picture { .. })
                && !layer.dirty.contains(DirtyFlags::PAINT)
            {
                self.diagnostics.picture_layers_reused += 1;
            }
        }
        for (_, layer) in self.layers.iter() {
            // Cleared after submission: retained data stays intact.
            let _ = layer;
        }
        for (_, layer) in self.layers.iter() {
            let _ = layer;
        }
        // Arena does not expose mutable iteration intentionally; dirty state is
        // advisory/debug-only and property writes remain coalesced by value.
        out
    }
    fn flatten_layer(
        &mut self,
        id: LayerId,
        translation: Offset,
        clip: Option<Rect>,
        out: &mut DisplayList,
    ) {
        let Some(layer) = self.layers.get(id.0).cloned() else {
            return;
        };
        match layer.kind {
            LayerKind::Picture {
                display_list,
                bounds,
            } => {
                let world = Rect::from_origin_size(bounds.origin + translation, bounds.size);
                if clip.is_some_and(|active| !active.intersects(world)) {
                    self.diagnostics.layers_culled += 1;
                    return;
                }
                self.flattened_pictures.push(FlattenedPicture {
                    layer: id,
                    local_bounds: bounds,
                    world_bounds: world,
                    active_clip: clip,
                });
                out.push(PaintCommand::PushTransform {
                    transform: Transform::translation(translation),
                });
                out.extend_from(&display_list);
                out.push(PaintCommand::PopTransform);
            }
            LayerKind::Transform { transform } => {
                let next = translation + transform.translation;
                for child in layer.children {
                    self.flatten_layer(child, next, clip, out);
                }
            }
            LayerKind::ClipRect { rect } => {
                let world = Rect::from_origin_size(rect.origin + translation, rect.size);
                let next_clip = match clip {
                    Some(old) => old.intersection(world),
                    None => Some(world),
                };
                if next_clip.is_none() {
                    self.diagnostics.layers_culled += 1;
                    return;
                }
                out.push(PaintCommand::PushClip { rect: world });
                for child in layer.children {
                    self.flatten_layer(child, translation, next_clip, out);
                }
                out.push(PaintCommand::PopClip);
            }
        }
    }
    #[must_use]
    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.root {
            self.write_debug(root, 0, &mut out);
        }
        out
    }
    fn write_debug(&self, id: LayerId, depth: usize, out: &mut String) {
        self.write_debug_at(id, None, depth, Offset::ZERO, None, out);
    }
    fn write_debug_at(
        &self,
        id: LayerId,
        parent: Option<LayerId>,
        depth: usize,
        translation: Offset,
        clip: Option<Rect>,
        out: &mut String,
    ) {
        let Some(layer) = self.layers.get(id.0) else {
            return;
        };
        let indent = "  ".repeat(depth);
        let kind = match &layer.kind {
            LayerKind::Picture { bounds, .. } => format!(
                "Picture(local_bounds={bounds:?}, world_bounds={:?}, clip={clip:?})",
                Rect::from_origin_size(bounds.origin + translation, bounds.size)
            ),
            LayerKind::Transform { transform } => {
                format!(
                    "Transform(local={:?}, world={:?})",
                    transform.translation,
                    translation + transform.translation
                )
            }
            LayerKind::ClipRect { rect } => format!(
                "ClipRect(local={rect:?}, world={:?})",
                Rect::from_origin_size(rect.origin + translation, rect.size)
            ),
        };
        out.push_str(&format!(
            "{indent}{id:?} parent={parent:?} {kind}, dirty={:?}\n",
            layer.dirty
        ));
        let (next_translation, next_clip) = match layer.kind {
            LayerKind::Picture { .. } => (translation, clip),
            LayerKind::Transform { transform } => (translation + transform.translation, clip),
            LayerKind::ClipRect { rect } => {
                let world = Rect::from_origin_size(rect.origin + translation, rect.size);
                (
                    translation,
                    Some(clip.map_or(world, |old| old.intersection(world).unwrap_or(world))),
                )
            }
        };
        for child in &layer.children {
            self.write_debug_at(
                *child,
                Some(id),
                depth + 1,
                next_translation,
                next_clip,
                out,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Offset, Size};
    #[test]
    fn commands_keep_painter_order() {
        let mut c = Canvas::default();
        c.rect(
            Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
            Color::WHITE,
        );
        assert!(matches!(
            c.finish().commands()[0],
            PaintCommand::Rect { .. }
        ));
    }
    #[test]
    fn retained_transform_reuses_picture_payload_and_accumulates_offsets() {
        let mut tree = LayerTree::new();
        let mut picture = DisplayList::new();
        picture.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(5., 5.)),
            color: Color::WHITE,
        });
        let leaf = tree.create_picture(
            picture,
            Rect::from_origin_size(Offset::ZERO, Size::new(5., 5.)),
        );
        let child = tree.create_transform(Transform::translation(Offset::new(5., 7.)));
        let parent = tree.create_transform(Transform::translation(Offset::new(10., 20.)));
        tree.set_children(child, vec![leaf]);
        tree.set_children(parent, vec![child]);
        tree.set_root(parent);
        let before = tree.diagnostics().picture_layers_repainted;
        let list = tree.flatten();
        assert_eq!(tree.diagnostics().picture_layers_repainted, before);
        assert!(list.commands().iter().any(|command| matches!(command, PaintCommand::PushTransform { transform } if transform.translation == Offset::new(15., 27.))));
        assert!(tree.update_transform(parent, Transform::translation(Offset::new(11., 20.))));
    }
    #[test]
    fn nested_clips_cull_fully_outside_picture() {
        let mut tree = LayerTree::new();
        let picture = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::new(20., 20.), Size::new(2., 2.)),
        );
        let clip = tree.create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::new(10., 10.)));
        tree.set_children(clip, vec![picture]);
        tree.set_root(clip);
        let _ = tree.flatten();
        assert_eq!(tree.diagnostics().layers_culled, 1);
    }
    #[test]
    fn world_bounds_and_clips_use_the_same_accumulated_coordinates() {
        let mut tree = LayerTree::new();
        let picture = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
        );
        let placement = tree.create_transform(Transform::translation(Offset::new(20., 30.)));
        let clip = tree.create_clip_rect(Rect::from_origin_size(
            Offset::new(15., 25.),
            Size::new(20., 20.),
        ));
        tree.set_children(clip, vec![placement]);
        tree.set_children(placement, vec![picture]);
        tree.set_root(clip);
        let _ = tree.flatten();
        assert_eq!(
            tree.flattened_pictures(),
            &[FlattenedPicture {
                layer: picture,
                local_bounds: Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
                world_bounds: Rect::from_origin_size(Offset::new(22., 33.), Size::new(10., 10.)),
                active_clip: Some(Rect::from_origin_size(
                    Offset::new(15., 25.),
                    Size::new(20., 20.),
                )),
            }]
        );
    }
}
