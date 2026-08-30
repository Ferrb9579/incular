//! Declarative widgets backed by persistent element and render-object arenas.
//!
//! A [`Widget`] is a cheap value. [`WidgetTree`] owns mounted identity and all
//! mutable layout/paint state. Reconciliation only examines direct children of
//! the element being updated.

use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use icu_segmenter::GraphemeClusterSegmenter;
use incular_animation::AnimationController;
use incular_config::{
    Alignment, Axis, Clip, Constraints, CrossAxisAlignment, EdgeInsets, FlexFit, MainAxisAlignment,
    MainAxisSize, RuntimeEnvironment, StackFit, TextDirection, VerticalDirection, WrapAlignment,
    WrapCrossAlignment,
};
use incular_core::{
    Arena, ArenaId, Color, DirtyFlags, KeyboardEvent, KeyboardKey, NamedKey, Offset, Rect, Size,
    Transform as CoreTransform,
};
#[cfg(test)]
use incular_core::{RestorationKey, RestorationScope};
use incular_image::ImageHandle;
use incular_rendering as incular_painting;
use incular_rendering::{
    BlendMode, Border, Brush, ColorFilter, CornerRadii, DisplayList, DropShadowEffect, FillRule,
    FilterQuality, GaussianBlur, ImageSampling, LayerId, LayerTree, PaintCommand, Path, RRect,
    Stroke, normalize_opacity, normalize_sigma,
};
use incular_scroll::{
    ScrollController, ScrollNotification, ScrollNotificationSubscription, ScrollPhysics,
    ScrollbarGeometry, SliverConstraints, scrollbar_geometry,
};
use incular_semantics::{
    Role as SemanticRole, SemanticActionKind, SemanticNode, SemanticNodeId, SemanticState,
    SemanticsDiagnostics, SemanticsTree, TextSelection as SemanticTextSelection,
};
use incular_text::{
    RichText, TextAlign, TextDiagnostics, TextEditingController, TextEngine, TextLayout,
    TextLayoutOptions, TextOverflow, TextRange, TextSelection, TextStyle,
};
use std::sync::Arc;

#[cfg(feature = "devtools")]
use incular_devtools_protocol::{DebugValue, DevWidgetId, TraceEvent, TracePhase};

use crate::drag_drop::{RetainedDragSource, RetainedDragTarget};
use crate::focus_keyboard::FocusTraversalPolicyKind;
use crate::gestures::{
    GestureAction, GestureArena, GestureArenaEntry, GestureArenaKey, GestureArenaMember,
    GestureCallbacks, GestureDecision, GestureDisposition, PointerEvent, PointerGestureRecognizer,
    ScaleGestureDetector,
};
use crate::recursion::{DiagnosticNode, DiagnosticNodeId, RecursionDiagnostics};
pub use crate::recursion::{FramePhase, RecursionReport};
use crate::scrolling::{
    SliverChildId, SliverViewportConfig, SliverViewportDelegate, SliverViewportLayout,
};
use crate::selection::SelectionAreaController;

thread_local! {
    /// Type-erased values made available while a retained layout builder is
    /// materialized. Control libraries use this hook for ambient, typed
    /// scopes without coupling the raw widget crate to a design-system crate.
    static BUILD_ENVIRONMENT: RefCell<Vec<Option<Rc<dyn Any>>>> = const { RefCell::new(Vec::new()) };
}

fn text_call_label(method: &str, text: &str) -> String {
    let mut preview = text.chars().take(80).collect::<String>();
    if text.chars().count() > 80 {
        preview.push('…');
    }
    format!("{method}({preview:?})")
}

/// A persistent chain of typed values inherited by a retained subtree.
///
/// `Widget::environment_scope` is represented by a layout-builder node so the
/// core widget crate does not need to know the concrete environment types. A
/// single erased value is not enough when independent scopes are nested (for
/// example, Material's theme, input-decoration theme, and control theme), so
/// effective environments retain the complete nearest-first chain here.
struct InheritedEnvironment {
    value: Rc<dyn Any>,
    parent: Option<Rc<InheritedEnvironment>>,
}

fn environment_chain(value: Rc<dyn Any>) -> Rc<InheritedEnvironment> {
    match value.downcast::<InheritedEnvironment>() {
        Ok(chain) => chain,
        Err(value) => Rc::new(InheritedEnvironment {
            value,
            parent: None,
        }),
    }
}

fn compose_environment(
    local: Option<Rc<dyn Any>>,
    inherited: Option<Rc<dyn Any>>,
) -> Option<Rc<dyn Any>> {
    let Some(local) = local else {
        return inherited;
    };
    let Some(inherited) = inherited else {
        return Some(local);
    };
    Some(Rc::new(InheritedEnvironment {
        value: local,
        parent: Some(environment_chain(inherited)),
    }) as Rc<dyn Any>)
}

fn environment_value<T: Any + Clone>(environment: &Rc<dyn Any>) -> Option<T> {
    if let Some(value) = environment.downcast_ref::<T>() {
        return Some(value.clone());
    }
    let mut chain = environment.downcast_ref::<InheritedEnvironment>();
    while let Some(scope) = chain {
        if let Some(value) = scope.value.downcast_ref::<T>() {
            return Some(value.clone());
        }
        chain = scope.parent.as_deref();
    }
    None
}

/// Runs a retained builder with one inherited, type-erased environment value.
/// This is intentionally small and renderer-neutral; higher-level crates
/// provide typed accessors around it.
pub fn with_build_environment<R>(
    environment: Option<Rc<dyn Any>>,
    callback: impl FnOnce() -> R,
) -> R {
    BUILD_ENVIRONMENT.with(|stack| stack.borrow_mut().push(environment));
    struct EnvironmentGuard;
    impl Drop for EnvironmentGuard {
        fn drop(&mut self) {
            BUILD_ENVIRONMENT.with(|stack| {
                let _ = stack.borrow_mut().pop();
            });
        }
    }
    let _guard = EnvironmentGuard;
    callback()
}

/// Reads the nearest typed value from the active retained builder scope.
#[must_use]
pub fn current_build_environment<T: Any + Clone>() -> Option<T> {
    BUILD_ENVIRONMENT.with(|stack| {
        let stack = stack.borrow();
        for value in stack.iter().rev() {
            let Some(environment) = value.as_ref() else {
                continue;
            };
            if let Some(value) = environment_value::<T>(environment) {
                return Some(value);
            }
        }
        None
    })
}

/// Reads a typed retained environment without placing the value-sized result
/// on the caller's stack. This matters for large theme descriptors on native
/// entry threads, whose stack is smaller than a test harness thread's stack.
#[must_use]
pub fn current_build_environment_boxed<T: Any + Clone>() -> Option<Box<T>> {
    BUILD_ENVIRONMENT.with(|stack| {
        let stack = stack.borrow();
        for value in stack.iter().rev() {
            let Some(environment) = value.as_ref() else {
                continue;
            };
            if let Some(value) = environment_value::<T>(environment) {
                return Some(Box::new(value));
            }
        }
        None
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId(pub(crate) ArenaId);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderObjectId(pub(crate) ArenaId);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub u64);

/// Retained text-input information exposed to the runtime/native boundary.
/// The controller remains the source of truth; this value is a frame-local
/// snapshot and contains no native handles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputTypeHint {
    #[default]
    Text,
    Multiline,
    Number,
    Phone,
    Email,
    Url,
    Password,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputActionHint {
    #[default]
    Unspecified,
    None,
    Done,
    Go,
    Search,
    Send,
    Next,
    Previous,
    Newline,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextFieldInputSnapshot {
    /// Stable for the lifetime of the mounted text field and opaque to the
    /// application. Native input bridges use it to reject stale updates.
    pub client_id: u64,
    pub text: String,
    pub selection: TextSelection,
    pub composing: Option<TextRange>,
    pub multiline: bool,
    pub enabled: bool,
    pub read_only: bool,
    pub obscure_text: bool,
    pub input_type: TextInputTypeHint,
    pub input_action: TextInputActionHint,
    pub bounds: Rect,
    pub caret_rect: Rect,
}
/// A window-local retained pointer-capture token.
///
/// Capture keeps a mounted gesture sequence routed to its original retained
/// target even when the contact leaves its bounds. Native adapters may mirror
/// this to OS pointer capture where available; the token itself never crosses
/// window boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PointerCapture {
    key: GestureArenaKey,
}
impl PointerCapture {
    #[must_use]
    pub const fn window(self) -> u64 {
        self.key.window
    }
    #[must_use]
    pub const fn pointer(self) -> u64 {
        self.key.pointer
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonState {
    #[default]
    Normal,
    Hovered,
    Focused,
    Pressed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Value(u64),
    String(String),
}
impl std::hash::Hash for Key {
    // Discriminant is folded into the hashed payload so cross-variant
    // collisions stay impossible while hashing becomes one `write_u64`
    // (or one `write_str`) instead of the derived multi-write form. This
    // was measured at ~86 ns/key via derived Hash on hot reconciliation
    // paths (Task 15).
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Key::Value(value) => state.write_u64(*value),
            Key::String(text) => {
                // Strings cannot collide with values because their hashed
                // length participates; hash str bytes plus a tagged length.
                state.write_u64(u64::try_from(text.len()).unwrap_or(u64::MAX) | 1 << 63);
                state.write(text.as_bytes());
            }
        }
    }
}
impl From<u64> for Key {
    fn from(value: u64) -> Self {
        Self::Value(value)
    }
}
impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

#[derive(Clone)]
pub struct TranslationController {
    offset: Rc<Cell<Offset>>,
    revision: Rc<Cell<u64>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<Offset>>,
    to: Rc<Cell<Offset>>,
}

/// Retained scalar scale state. Updates and animations change only an inner
/// compositor affine layer; the child keeps its warm layout and picture.
#[derive(Clone)]
pub struct ScaleController {
    scale: Rc<Cell<f32>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<f32>>,
    to: Rc<Cell<f32>>,
}
impl std::fmt::Debug for ScaleController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScaleController")
            .field("scale", &self.scale())
            .finish()
    }
}
impl PartialEq for ScaleController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.scale, &other.scale)
    }
}
impl Default for ScaleController {
    fn default() -> Self {
        Self {
            scale: Rc::new(Cell::new(1.)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(1.)),
            to: Rc::new(Cell::new(1.)),
        }
    }
}
impl ScaleController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale.get()
    }
    pub fn set_scale(&self, scale: f32) -> bool {
        let scale = if scale.is_finite() { scale } else { 1. };
        if self.scale.get() == scale {
            return false;
        }
        self.scale.set(scale);
        true
    }
    pub fn animate_to(&self, target: f32, duration: Duration, now: Instant) {
        self.from.set(self.scale());
        self.to.set(if target.is_finite() { target } else { 1. });
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        self.set_scale(self.from.get() + (self.to.get() - self.from.get()) * animation.value())
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}

/// Retained clockwise rotation state in logical radians.
#[derive(Clone)]
pub struct RotationController {
    radians: Rc<Cell<f32>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<f32>>,
    to: Rc<Cell<f32>>,
}
impl std::fmt::Debug for RotationController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RotationController")
            .field("radians", &self.radians())
            .finish()
    }
}
impl PartialEq for RotationController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.radians, &other.radians)
    }
}
impl Default for RotationController {
    fn default() -> Self {
        Self {
            radians: Rc::new(Cell::new(0.)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(0.)),
            to: Rc::new(Cell::new(0.)),
        }
    }
}
impl RotationController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn angle_degrees(&self) -> f32 {
        self.radians().to_degrees()
    }
    pub fn radians(&self) -> f32 {
        self.radians.get()
    }
    /// Flutter-style turns (one turn is a full revolution).  Radians remain
    /// the retained primitive internally so callers that need mathematical
    /// angles can avoid a conversion round-trip.
    #[must_use]
    pub fn turns(&self) -> f32 {
        self.radians() / std::f32::consts::TAU
    }
    pub fn set_turns(&self, turns: f32) -> bool {
        self.set_radians(turns * std::f32::consts::TAU)
    }
    pub fn set_radians(&self, radians: f32) -> bool {
        let radians = if radians.is_finite() { radians } else { 0. };
        if self.radians.get() == radians {
            return false;
        }
        self.radians.set(radians);
        true
    }
    pub fn animate_to(&self, target: f32, duration: Duration, now: Instant) {
        self.from.set(self.radians());
        self.to.set(if target.is_finite() { target } else { 0. });
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    pub fn animate_to_turns(&self, target: f32, duration: Duration, now: Instant) {
        self.animate_to(target * std::f32::consts::TAU, duration, now);
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        self.set_radians(self.from.get() + (self.to.get() - self.from.get()) * animation.value())
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}
impl std::fmt::Debug for TranslationController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranslationController")
            .field("offset", &self.offset())
            .finish()
    }
}
impl PartialEq for TranslationController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.offset, &other.offset)
    }
}
impl Default for TranslationController {
    fn default() -> Self {
        Self {
            offset: Rc::new(Cell::new(Offset::ZERO)),
            revision: Rc::new(Cell::new(0)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(Offset::ZERO)),
            to: Rc::new(Cell::new(Offset::ZERO)),
        }
    }
}
impl TranslationController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn offset(&self) -> Offset {
        self.offset.get()
    }
    pub fn set_offset(&self, offset: Offset) -> bool {
        if self.offset.get() == offset {
            return false;
        }
        self.offset.set(offset);
        self.revision.set(self.revision.get() + 1);
        true
    }
    pub fn animate_to(&self, target: Offset, duration: Duration, now: Instant) {
        self.from.set(self.offset());
        self.to.set(target);
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        self.set_offset(Offset::new(
            self.from.get().x + (self.to.get().x - self.from.get().x) * t,
            self.from.get().y + (self.to.get().y - self.from.get().y) * t,
        ))
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}

/// Retained opacity state. Updating this controller changes only the
/// compositor layer; the child display list and layout remain untouched.
#[derive(Clone)]
pub struct OpacityController {
    opacity: Rc<Cell<f32>>,
    revision: Rc<Cell<u64>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<f32>>,
    to: Rc<Cell<f32>>,
}
impl std::fmt::Debug for OpacityController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpacityController")
            .field("opacity", &self.opacity())
            .finish()
    }
}
impl PartialEq for OpacityController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.opacity, &other.opacity)
    }
}
impl Default for OpacityController {
    fn default() -> Self {
        Self {
            opacity: Rc::new(Cell::new(1.)),
            revision: Rc::new(Cell::new(0)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(1.)),
            to: Rc::new(Cell::new(1.)),
        }
    }
}
impl OpacityController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn opacity(&self) -> f32 {
        self.opacity.get()
    }
    pub fn set_opacity(&self, opacity: f32) -> bool {
        let opacity = normalize_opacity(opacity);
        if self.opacity.get() == opacity {
            return false;
        }
        self.opacity.set(opacity);
        self.revision.set(self.revision.get().wrapping_add(1));
        true
    }
    pub fn animate_to(&self, target: f32, duration: Duration, now: Instant) {
        self.from.set(self.opacity());
        self.to.set(normalize_opacity(target));
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        self.set_opacity(self.from.get() + (self.to.get() - self.from.get()) * t)
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}

/// Retained Gaussian sigma controller. Ticking changes only compositor
/// parameters; the child render object is never marked for paint.
#[derive(Clone)]
pub struct BlurController {
    sigma: Rc<Cell<f32>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<f32>>,
    to: Rc<Cell<f32>>,
}
impl std::fmt::Debug for BlurController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlurController")
            .field("sigma", &self.sigma())
            .finish()
    }
}
impl PartialEq for BlurController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.sigma, &other.sigma)
    }
}
impl BlurController {
    #[must_use]
    pub fn new(sigma: f32) -> Self {
        let sigma = normalize_sigma(sigma);
        Self {
            sigma: Rc::new(Cell::new(sigma)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(sigma)),
            to: Rc::new(Cell::new(sigma)),
        }
    }
    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma.get()
    }
    pub fn set_sigma(&self, sigma: f32) -> bool {
        let sigma = normalize_sigma(sigma);
        if self.sigma() == sigma {
            return false;
        }
        self.sigma.set(sigma);
        true
    }
    pub fn animate_to(&self, target: f32, duration: Duration, now: Instant) {
        self.from.set(self.sigma());
        self.to.set(normalize_sigma(target));
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        self.set_sigma(self.from.get() + (self.to.get() - self.from.get()) * t)
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}
impl Default for BlurController {
    fn default() -> Self {
        Self::new(0.)
    }
}

/// Retained color-matrix controller. Matrix animation is a filter/compositor
/// update; it never marks the child picture dirty.
#[derive(Clone)]
pub struct ColorFilterController {
    matrix: Rc<Cell<[f32; 20]>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<[f32; 20]>>,
    to: Rc<Cell<[f32; 20]>>,
}
impl std::fmt::Debug for ColorFilterController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorFilterController")
            .field("matrix", &self.matrix())
            .finish()
    }
}
impl PartialEq for ColorFilterController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.matrix, &other.matrix)
    }
}
impl ColorFilterController {
    #[must_use]
    pub fn new(filter: ColorFilter) -> Self {
        let matrix = filter.to_matrix();
        Self {
            matrix: Rc::new(Cell::new(matrix)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(matrix)),
            to: Rc::new(Cell::new(matrix)),
        }
    }
    #[must_use]
    pub fn matrix(&self) -> [f32; 20] {
        self.matrix.get()
    }
    #[must_use]
    pub fn filter(&self) -> ColorFilter {
        ColorFilter::matrix(self.matrix())
    }
    pub fn set_matrix(&self, matrix: [f32; 20]) -> bool {
        let matrix = ColorFilter::matrix(matrix).to_matrix();
        if self.matrix() == matrix {
            return false;
        }
        self.matrix.set(matrix);
        true
    }
    pub fn set_filter(&self, filter: ColorFilter) -> bool {
        self.set_matrix(filter.to_matrix())
    }
    pub fn animate_to(&self, target: ColorFilter, duration: Duration, now: Instant) {
        self.from.set(self.matrix());
        self.to.set(target.to_matrix());
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        let from = self.from.get();
        let to = self.to.get();
        let mut matrix = [0.; 20];
        for index in 0..20 {
            matrix[index] = from[index] + (to[index] - from[index]) * t;
        }
        self.set_matrix(matrix)
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}
impl Default for ColorFilterController {
    fn default() -> Self {
        Self::new(ColorFilter::identity())
    }
}

/// Compatibility spelling for applications that call a 4×5 filter a color
/// matrix. It is the same retained controller and has identical invalidation
/// semantics.
pub type ColorMatrixController = ColorFilterController;

/// Retained drop-shadow presentation controller. Offset and color are pure
/// composite properties; sigma changes invalidate only the blurred mask.
#[derive(Clone)]
pub struct DropShadowController {
    offset: Rc<Cell<Offset>>,
    sigma: Rc<Cell<f32>>,
    color: Rc<Cell<Color>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<Offset>>,
    to: Rc<Cell<Offset>>,
}
impl std::fmt::Debug for DropShadowController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropShadowController")
            .field("offset", &self.offset())
            .field("sigma", &self.sigma())
            .field("color", &self.color())
            .finish()
    }
}
impl PartialEq for DropShadowController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.offset, &other.offset)
    }
}
impl DropShadowController {
    #[must_use]
    pub fn new(offset: Offset, sigma: f32, color: Color) -> Self {
        let offset = finite_offset(offset);
        let sigma = normalize_sigma(sigma);
        Self {
            offset: Rc::new(Cell::new(offset)),
            sigma: Rc::new(Cell::new(sigma)),
            color: Rc::new(Cell::new(color)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(offset)),
            to: Rc::new(Cell::new(offset)),
        }
    }
    #[must_use]
    pub fn offset(&self) -> Offset {
        self.offset.get()
    }
    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma.get()
    }
    #[must_use]
    pub fn color(&self) -> Color {
        self.color.get()
    }
    pub fn set_offset(&self, offset: Offset) -> bool {
        let offset = finite_offset(offset);
        if self.offset() == offset {
            return false;
        }
        self.offset.set(offset);
        true
    }
    pub fn set_sigma(&self, sigma: f32) -> bool {
        let sigma = normalize_sigma(sigma);
        if self.sigma() == sigma {
            return false;
        }
        self.sigma.set(sigma);
        true
    }
    pub fn set_color(&self, color: Color) -> bool {
        if self.color() == color {
            return false;
        }
        self.color.set(color);
        true
    }
    pub fn animate_offset_to(&self, target: Offset, duration: Duration, now: Instant) {
        self.from.set(self.offset());
        self.to.set(finite_offset(target));
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        let from = self.from.get();
        let to = self.to.get();
        self.set_offset(Offset::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
        ))
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}
impl Default for DropShadowController {
    fn default() -> Self {
        Self::new(Offset::new(0., 4.), 8., Color::rgba(0, 0, 0, 96))
    }
}

fn finite_offset(offset: Offset) -> Offset {
    Offset::new(
        if offset.x.is_finite() { offset.x } else { 0. },
        if offset.y.is_finite() { offset.y } else { 0. },
    )
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() { value.max(0.) } else { 0. }
}

/// The first built-in widgets. Their values contain no mutable runtime state.
#[derive(Clone)]
pub struct Widget {
    pub key: Option<Key>,
    pub kind: WidgetKind,
    semantics: SemanticProperties,
}

/// Application-authored semantic metadata for a visual widget that does not
/// have a more specific built-in semantic role. Incular keeps this data in its
/// retained `SemanticsTree`; native adapters only project that tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ExplicitSemantics {
    pub role: SemanticRole,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub state: SemanticState,
    pub actions: Vec<SemanticActionKind>,
}

impl ExplicitSemantics {
    #[must_use]
    pub fn new(role: SemanticRole) -> Self {
        Self {
            role,
            label: None,
            value: None,
            description: None,
            state: SemanticState::default(),
            actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn state(mut self, state: SemanticState) -> Self {
        self.state = state;
        self
    }

    #[must_use]
    pub fn actions(mut self, actions: impl IntoIterator<Item = SemanticActionKind>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct SemanticProperties {
    label: Option<String>,
    description: Option<String>,
    hidden: bool,
    explicit: Option<ExplicitSemantics>,
    merge_descendants: bool,
    block_previous_siblings: bool,
    focus_traversal_policy: Option<FocusTraversalPolicyKind>,
    focus_traversal_order: Option<f64>,
    exclude_focus: bool,
    exclude_focus_traversal: bool,
    undo_history_max_entries: Option<usize>,
    focus_scope_autofocus: bool,
    text_input_type: Option<TextInputTypeHint>,
    text_input_action: Option<TextInputActionHint>,
    callbacks: SemanticCallbacks,
}

#[derive(Clone, Default)]
struct SemanticCallbacks {
    activate: Option<Rc<dyn Fn() + 'static>>,
    increment: Option<Rc<dyn Fn() + 'static>>,
    decrement: Option<Rc<dyn Fn() + 'static>>,
    scroll_forward: Option<Rc<dyn Fn() + 'static>>,
    scroll_backward: Option<Rc<dyn Fn() + 'static>>,
}

impl std::fmt::Debug for SemanticCallbacks {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SemanticCallbacks")
            .field("activate", &self.activate.is_some())
            .field("increment", &self.increment.is_some())
            .field("decrement", &self.decrement.is_some())
            .field("scroll_forward", &self.scroll_forward.is_some())
            .field("scroll_backward", &self.scroll_backward.is_some())
            .finish()
    }
}

impl PartialEq for SemanticCallbacks {
    fn eq(&self, other: &Self) -> bool {
        fn same<T: ?Sized>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
            match (left, right) {
                (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
        }

        same(&self.activate, &other.activate)
            && same(&self.increment, &other.increment)
            && same(&self.decrement, &other.decrement)
            && same(&self.scroll_forward, &other.scroll_forward)
            && same(&self.scroll_backward, &other.scroll_backward)
    }
}

impl SemanticCallbacks {
    fn callback(&self, action: SemanticActionKind) -> Option<Rc<dyn Fn() + 'static>> {
        match action {
            SemanticActionKind::Activate => self.activate.clone(),
            SemanticActionKind::Increment => self.increment.clone(),
            SemanticActionKind::Decrement => self.decrement.clone(),
            SemanticActionKind::ScrollForward => self.scroll_forward.clone(),
            SemanticActionKind::ScrollBackward => self.scroll_backward.clone(),
            SemanticActionKind::Focus
            | SemanticActionKind::SetText
            | SemanticActionKind::SetSelection => None,
        }
    }

    fn supports(&self, action: SemanticActionKind) -> bool {
        self.callback(action).is_some()
    }
}
#[derive(Clone)]
pub enum WidgetKind {
    Box {
        size: Size,
        color: Color,
    },
    Shape {
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        size: Option<Size>,
    },
    CustomPaint {
        size: Size,
        display_list: DisplayList,
    },
    Decorated {
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Box<Widget>,
    },
    Button {
        size: Size,
        color: Color,
        hover_color: Option<Color>,
        pressed_color: Option<Color>,
        focused_color: Option<Color>,
        disabled_color: Option<Color>,
        enabled: bool,
        focusable_when_disabled: bool,
        action: ActionId,
        callback: Option<Rc<dyn Fn()>>,
        hover_action: ActionId,
        hover_callback: Option<Rc<dyn Fn()>>,
        exit_action: ActionId,
        exit_callback: Option<Rc<dyn Fn()>>,
        has_callback: bool,
        child: Option<Box<Widget>>,
    },
    Text {
        text: String,
        style: TextStyle,
        align: TextAlign,
        soft_wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    },
    SelectableText {
        text: String,
        style: TextStyle,
        align: TextAlign,
    },
    SelectionArea {
        controller: SelectionAreaController,
        child: Box<Widget>,
    },
    Image {
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        repeat: ImageRepeat,
        alignment: Alignment,
        sampling: ImageSampling,
    },
    TextField {
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
        cursor_width: f32,
        cursor_height: Option<f32>,
        cursor_radius: f32,
        show_cursor: bool,
        cursor_color: Color,
        selection_color: Color,
    },
    Padding {
        padding: EdgeInsets,
        child: Box<Widget>,
    },
    Constrained {
        constraints: Constraints,
        child: Box<Widget>,
    },
    Limited {
        max_width: f32,
        max_height: f32,
        child: Box<Widget>,
    },
    Overflow {
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        child: Box<Widget>,
    },
    Unconstrained {
        constrained_axis: Option<Axis>,
        child: Box<Widget>,
    },
    Fractional {
        width_factor: Option<f32>,
        height_factor: Option<f32>,
        child: Box<Widget>,
    },
    Baseline {
        baseline: f32,
        child: Box<Widget>,
    },
    RepaintBoundary {
        child: Box<Widget>,
    },
    Gesture {
        behavior: crate::gestures::HitTestBehavior,
        callbacks: Box<GestureCallbacks>,
        child: Box<Widget>,
    },
    Draggable {
        source: Rc<dyn RetainedDragSource>,
        child: Box<Widget>,
    },
    DragTarget {
        target: Rc<dyn RetainedDragTarget>,
        child: Box<Widget>,
    },
    IgnorePointer {
        ignoring: bool,
        child: Box<Widget>,
    },
    AbsorbPointer {
        absorbing: bool,
        child: Box<Widget>,
    },
    Align {
        alignment: Alignment,
        width_factor: Option<f32>,
        height_factor: Option<f32>,
        child: Box<Widget>,
    },
    Flex {
        axis: Axis,
        main_axis_alignment: MainAxisAlignment,
        main_axis_size: MainAxisSize,
        cross_axis_alignment: CrossAxisAlignment,
        text_direction: TextDirection,
        vertical_direction: VerticalDirection,
        spacing: f32,
        children: Vec<Widget>,
    },
    Flexible {
        flex: u32,
        fit: incular_config::FlexFit,
        child: Box<Widget>,
    },
    Wrap {
        axis: Axis,
        alignment: WrapAlignment,
        spacing: f32,
        run_alignment: WrapAlignment,
        run_spacing: f32,
        cross_axis_alignment: WrapCrossAlignment,
        text_direction: TextDirection,
        vertical_direction: VerticalDirection,
        children: Vec<Widget>,
    },
    Table {
        columns: usize,
        column_spacing: f32,
        row_spacing: f32,
        children: Vec<Widget>,
    },
    Stack {
        alignment: Alignment,
        text_direction: TextDirection,
        fit: StackFit,
        clip_behavior: Clip,
        children: Vec<Widget>,
    },
    Positioned {
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
        child: Box<Widget>,
    },
    IndexedStack {
        alignment: Alignment,
        index: usize,
        children: Vec<Widget>,
    },
    SafeArea {
        minimum: EdgeInsets,
        left: bool,
        top: bool,
        right: bool,
        bottom: bool,
        maintain_bottom_view_padding: bool,
        child: Box<Widget>,
    },
    ClipRect {
        clip_behavior: Clip,
        child: Box<Widget>,
    },
    ClipRRect {
        radius: CornerRadii,
        clip_behavior: Clip,
        child: Box<Widget>,
    },
    ClipOval {
        clip_behavior: Clip,
        child: Box<Widget>,
    },
    ClipPath {
        path: Arc<Path>,
        clip_behavior: Clip,
        child: Box<Widget>,
    },
    LayoutBuilder {
        builder: Rc<dyn Fn(Constraints) -> Widget>,
        environment: Option<Rc<dyn Any>>,
        /// Optional mutable revision for builders whose callback updates
        /// retained local state without replacing the parent widget.  The
        /// runtime samples this value during layout and rematerializes the
        /// builder child when it changes.
        revision: Option<Rc<Cell<u64>>>,
    },
    Visibility {
        visible: bool,
        child: Box<Widget>,
    },
    AspectRatio {
        ratio: f32,
        child: Box<Widget>,
    },
    Scroll {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
        child: Box<Widget>,
    },
    /// A flow child which remains in the scrolling layout while an inner
    /// compositor transform pins it at the viewport's leading edge.
    PersistentHeader {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        pinned: bool,
        child: Box<Widget>,
    },
    NotificationListener {
        callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
        child: Box<Widget>,
    },
    SliverViewport {
        config: Rc<SliverViewportConfig>,
    },
    Translate {
        controller: TranslationController,
        child: Box<Widget>,
    },
    Transform {
        transform: CoreTransform,
        origin: Option<Offset>,
        child: Box<Widget>,
    },
    Scale {
        controller: ScaleController,
        origin: Option<Offset>,
        child: Box<Widget>,
    },
    Rotation {
        controller: RotationController,
        origin: Option<Offset>,
        alignment: Option<Alignment>,
        child: Box<Widget>,
    },
    FittedBox {
        fit: ImageFit,
        alignment: Alignment,
        child: Box<Widget>,
    },
    Opacity {
        alpha: f32,
        controller: Option<OpacityController>,
        child: Box<Widget>,
    },
    Blur {
        sigma_x: f32,
        sigma_y: f32,
        controller: Option<BlurController>,
        child: Box<Widget>,
    },
    DropShadow {
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        controller: Option<DropShadowController>,
        child: Box<Widget>,
    },
    ColorFiltered {
        filter: ColorFilter,
        controller: Option<ColorFilterController>,
        child: Box<Widget>,
    },
    Blend {
        mode: BlendMode,
        child: Box<Widget>,
    },
}

/// Converts the controller's logical offset into the physical content offset
/// used by a viewport transform.  A reversed viewport keeps the controller's
/// public range in the same `0..max` coordinate system, while its leading edge
/// is the content's physical trailing edge.
#[inline]
fn physical_scroll_offset(controller: &ScrollController, reverse: bool) -> f32 {
    if reverse {
        (controller.max_offset() - controller.offset()).max(0.)
    } else {
        controller.offset()
    }
}

#[inline]
fn scroll_translation(controller: &ScrollController, axis: Axis, reverse: bool) -> Offset {
    let offset = physical_scroll_offset(controller, reverse);
    axis.offset(-offset, 0.)
}

#[inline]
fn scroll_viewport_extent(axis: Axis, size: Size) -> f32 {
    axis.main_extent(size)
}

#[inline]
fn scroll_delta_for_axis(axis: Axis, delta: Offset) -> f32 {
    match axis {
        Axis::Horizontal => delta.x,
        Axis::Vertical => delta.y,
    }
}

#[inline]
fn scroll_constraints(axis: Axis, constraints: Constraints) -> Constraints {
    match axis {
        Axis::Vertical => Constraints::new(0., constraints.max_width, 0., f32::INFINITY),
        Axis::Horizontal => Constraints::new(0., f32::INFINITY, 0., constraints.max_height),
    }
}

#[inline]
fn scroll_size(axis: Axis, constraints: Constraints, content: Size) -> Size {
    let main = if axis.main_extent(content).is_finite()
        && ((axis.is_horizontal() && constraints.is_width_bounded())
            || (axis.is_vertical() && constraints.is_height_bounded()))
    {
        axis.main_extent(constraints.biggest())
    } else {
        axis.main_extent(content)
    };
    let cross = if (axis.is_horizontal() && constraints.is_height_bounded())
        || (axis.is_vertical() && constraints.is_width_bounded())
    {
        axis.cross_extent(constraints.biggest())
    } else {
        axis.cross_extent(content)
    };
    constraints.constrain(axis.size(main, cross))
}

#[inline]
fn sliver_viewport_size(axis: Axis, constraints: Constraints, shrink_wrap: bool) -> Size {
    let biggest = constraints.biggest();
    let main = if shrink_wrap
        && ((axis.is_vertical() && !constraints.is_height_bounded())
            || (axis.is_horizontal() && !constraints.is_width_bounded()))
    {
        0.
    } else {
        axis.main_extent(biggest)
    };
    let cross = axis.cross_extent(biggest);
    constraints.constrain(axis.size(main, cross))
}

/// Selects the first retained sliver child at or after the physical viewport
/// origin. Keeping its logical identity and offset lets a variable-extent
/// sliver compensate the scroll position when rows above that child are
/// measured more accurately.
fn sliver_anchor(
    layout: &SliverViewportLayout,
    physical_offset: f32,
) -> Option<(SliverChildId, f32)> {
    layout
        .children
        .iter()
        .find(|child| {
            child.offset + child.extent > physical_offset
                || (child.extent <= 0. && child.offset >= physical_offset)
        })
        .map(|child| (child.id, child.offset))
}

/// Applies a child's additional constraints without ever allowing it to
/// escape the bounds imposed by its parent.
fn enforced_constraints(parent: Constraints, additional: Constraints) -> Constraints {
    Constraints::new(
        additional
            .min_width
            .clamp(parent.min_width, parent.max_width),
        additional
            .max_width
            .clamp(parent.min_width, parent.max_width),
        additional
            .min_height
            .clamp(parent.min_height, parent.max_height),
        additional
            .max_height
            .clamp(parent.min_height, parent.max_height),
    )
}

fn unconstrained_constraints(parent: Constraints, constrained_axis: Option<Axis>) -> Constraints {
    match constrained_axis {
        Some(Axis::Horizontal) => {
            Constraints::new(parent.min_width, parent.max_width, 0., f32::INFINITY)
        }
        Some(Axis::Vertical) => {
            Constraints::new(0., f32::INFINITY, parent.min_height, parent.max_height)
        }
        None => Constraints::unbounded(),
    }
}

fn fractional_constraints(
    parent: Constraints,
    width_factor: Option<f32>,
    height_factor: Option<f32>,
) -> Constraints {
    let width = width_factor
        .filter(|_| parent.max_width.is_finite())
        .map(|factor| parent.max_width * factor);
    let height = height_factor
        .filter(|_| parent.max_height.is_finite())
        .map(|factor| parent.max_height * factor);
    Constraints::new(
        width.unwrap_or(0.),
        width.unwrap_or(parent.max_width),
        height.unwrap_or(0.),
        height.unwrap_or(parent.max_height),
    )
}
impl std::fmt::Debug for Widget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Widget")
            .field("key", &self.key)
            .field("kind", &self.kind)
            .finish()
    }
}
impl PartialEq for Widget {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.kind == other.kind && self.semantics == other.semantics
    }
}
impl std::fmt::Debug for WidgetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Box { size, color } => f
                .debug_struct("Box")
                .field("size", size)
                .field("color", color)
                .finish(),
            Self::Shape {
                path,
                fill,
                stroke,
                size,
            } => f
                .debug_struct("PathView")
                .field("path", &path.id())
                .field("fill", fill)
                .field("stroke", stroke)
                .field("size", size)
                .finish(),
            Self::CustomPaint { size, display_list } => f
                .debug_struct("CustomPaint")
                .field("size", size)
                .field("command_count", &display_list.len())
                .finish(),
            Self::Decorated {
                size,
                background,
                border,
                radius,
                ..
            } => f
                .debug_struct("DecoratedBox")
                .field("size", size)
                .field("background", background)
                .field("border", border)
                .field("radius", radius)
                .finish(),
            Self::Button {
                size,
                color,
                action,
                ..
            } => f
                .debug_struct("Button")
                .field("size", size)
                .field("color", color)
                .field("action", action)
                .finish(),
            Self::Text {
                text,
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            } => f
                .debug_struct("Text")
                .field("text", text)
                .field("style", style)
                .field("align", align)
                .field("soft_wrap", soft_wrap)
                .field("max_lines", max_lines)
                .field("overflow", overflow)
                .finish(),
            Self::Image {
                image,
                width,
                height,
                fit,
                repeat,
                alignment,
                sampling: _,
            } => f
                .debug_struct("Image")
                .field("id", &image.id())
                .field("width", width)
                .field("height", height)
                .field("fit", fit)
                .field("repeat", repeat)
                .field("alignment", alignment)
                .finish(),
            Self::TextField { placeholder, .. } => f
                .debug_struct("TextField")
                .field("placeholder", placeholder)
                .finish(),
            Self::Padding { padding, child } => f
                .debug_struct("Padding")
                .field("padding", padding)
                .field("child", child)
                .finish(),
            Self::Constrained { constraints, child } => f
                .debug_struct("ConstrainedBox")
                .field("constraints", constraints)
                .field("child", child)
                .finish(),
            Self::Limited {
                max_width,
                max_height,
                child,
            } => f
                .debug_struct("LimitedBox")
                .field("max_width", max_width)
                .field("max_height", max_height)
                .field("child", child)
                .finish(),
            Self::Overflow {
                min_width,
                max_width,
                min_height,
                max_height,
                child,
            } => f
                .debug_struct("OverflowBox")
                .field("min_width", min_width)
                .field("max_width", max_width)
                .field("min_height", min_height)
                .field("max_height", max_height)
                .field("child", child)
                .finish(),
            Self::Unconstrained {
                constrained_axis,
                child,
            } => f
                .debug_struct("UnconstrainedBox")
                .field("constrained_axis", constrained_axis)
                .field("child", child)
                .finish(),
            Self::Fractional {
                width_factor,
                height_factor,
                child,
            } => f
                .debug_struct("FractionallySizedBox")
                .field("width_factor", width_factor)
                .field("height_factor", height_factor)
                .field("child", child)
                .finish(),
            Self::Baseline { baseline, child } => f
                .debug_struct("Baseline")
                .field("baseline", baseline)
                .field("child", child)
                .finish(),
            Self::RepaintBoundary { child } => f
                .debug_struct("RepaintBoundary")
                .field("child", child)
                .finish(),
            Self::Gesture { child, .. } => f
                .debug_struct("GestureDetector")
                .field("child", child)
                .finish(),
            Self::Draggable { child, .. } => {
                f.debug_struct("Draggable").field("child", child).finish()
            }
            Self::DragTarget { child, .. } => {
                f.debug_struct("DragTarget").field("child", child).finish()
            }
            Self::IgnorePointer { ignoring, child } => f
                .debug_struct("IgnorePointer")
                .field("ignoring", ignoring)
                .field("child", child)
                .finish(),
            Self::AbsorbPointer { absorbing, child } => f
                .debug_struct("AbsorbPointer")
                .field("absorbing", absorbing)
                .field("child", child)
                .finish(),
            Self::Align {
                alignment,
                width_factor,
                height_factor,
                child,
            } => f
                .debug_struct("Align")
                .field("alignment", alignment)
                .field("width_factor", width_factor)
                .field("height_factor", height_factor)
                .field("child", child)
                .finish(),
            Self::Flex {
                axis,
                main_axis_alignment,
                main_axis_size,
                cross_axis_alignment,
                text_direction,
                vertical_direction,
                spacing,
                children,
            } => f
                .debug_struct("Flex")
                .field("axis", axis)
                .field("main_axis_alignment", main_axis_alignment)
                .field("main_axis_size", main_axis_size)
                .field("cross_axis_alignment", cross_axis_alignment)
                .field("text_direction", text_direction)
                .field("vertical_direction", vertical_direction)
                .field("spacing", spacing)
                .field("children", children)
                .finish(),
            Self::Flexible { flex, fit, child } => f
                .debug_struct("Flexible")
                .field("flex", flex)
                .field("fit", fit)
                .field("child", child)
                .finish(),
            Self::Wrap {
                axis,
                alignment,
                spacing,
                run_alignment,
                run_spacing,
                cross_axis_alignment,
                text_direction,
                vertical_direction,
                children,
            } => f
                .debug_struct("Wrap")
                .field("axis", axis)
                .field("alignment", alignment)
                .field("spacing", spacing)
                .field("run_alignment", run_alignment)
                .field("run_spacing", run_spacing)
                .field("cross_axis_alignment", cross_axis_alignment)
                .field("text_direction", text_direction)
                .field("vertical_direction", vertical_direction)
                .field("children", children)
                .finish(),
            Self::Table {
                columns,
                column_spacing,
                row_spacing,
                children,
            } => f
                .debug_struct("Table")
                .field("columns", columns)
                .field("column_spacing", column_spacing)
                .field("row_spacing", row_spacing)
                .field("children", children)
                .finish(),
            Self::Stack {
                alignment,
                text_direction,
                fit,
                clip_behavior,
                children,
            } => f
                .debug_struct("Stack")
                .field("alignment", alignment)
                .field("text_direction", text_direction)
                .field("fit", fit)
                .field("clip_behavior", clip_behavior)
                .field("children", children)
                .finish(),
            Self::SafeArea {
                minimum,
                left,
                top,
                right,
                bottom,
                maintain_bottom_view_padding,
                child,
            } => f
                .debug_struct("SafeArea")
                .field("minimum", minimum)
                .field("left", left)
                .field("top", top)
                .field("right", right)
                .field("bottom", bottom)
                .field("maintain_bottom_view_padding", maintain_bottom_view_padding)
                .field("child", child)
                .finish(),
            Self::ClipRect {
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipRect")
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::ClipRRect {
                radius,
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipRRect")
                .field("radius", radius)
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::ClipOval {
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipOval")
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::ClipPath {
                path,
                clip_behavior,
                child,
            } => f
                .debug_struct("ClipPath")
                .field("path", path)
                .field("clip_behavior", clip_behavior)
                .field("child", child)
                .finish(),
            Self::Positioned {
                left,
                top,
                right,
                bottom,
                width,
                height,
                child,
            } => f
                .debug_struct("Positioned")
                .field("left", left)
                .field("top", top)
                .field("right", right)
                .field("bottom", bottom)
                .field("width", width)
                .field("height", height)
                .field("child", child)
                .finish(),
            Self::IndexedStack {
                alignment,
                index,
                children,
            } => f
                .debug_struct("IndexedStack")
                .field("alignment", alignment)
                .field("index", index)
                .field("children", children)
                .finish(),
            Self::LayoutBuilder { .. } => f.debug_struct("LayoutBuilder").finish(),
            Self::Visibility { visible, child } => f
                .debug_struct("Visibility")
                .field("visible", visible)
                .field("child", child)
                .finish(),
            Self::AspectRatio { ratio, child } => f
                .debug_struct("AspectRatio")
                .field("ratio", ratio)
                .field("child", child)
                .finish(),
            Self::Scroll { .. } => f.debug_struct("ScrollView").finish(),
            Self::PersistentHeader { .. } => f.debug_struct("PersistentHeader").finish(),
            Self::NotificationListener { child, .. } => f
                .debug_struct("NotificationListener")
                .field("child", child)
                .finish(),
            Self::SliverViewport { config } => f
                .debug_struct("SliverViewport")
                .field("config", config)
                .finish(),
            Self::Translate { .. } => f.debug_struct("Translate").finish(),
            Self::Transform {
                transform, origin, ..
            } => f
                .debug_struct("Transform")
                .field("transform", transform)
                .field("origin", origin)
                .finish(),
            Self::Scale {
                controller, origin, ..
            } => f
                .debug_struct("ScaleTransition")
                .field("controller", controller)
                .field("origin", origin)
                .finish(),
            Self::Rotation {
                controller, origin, ..
            } => f
                .debug_struct("RotationTransition")
                .field("controller", controller)
                .field("origin", origin)
                .finish(),
            Self::FittedBox { fit, alignment, .. } => f
                .debug_struct("FittedBox")
                .field("fit", fit)
                .field("alignment", alignment)
                .finish(),
            Self::Opacity {
                alpha, controller, ..
            } => f
                .debug_struct("Opacity")
                .field("alpha", alpha)
                .field("controller", controller)
                .finish(),
            Self::Blur {
                sigma_x,
                sigma_y,
                controller,
                ..
            } => f
                .debug_struct("Blur")
                .field("sigma_x", sigma_x)
                .field("sigma_y", sigma_y)
                .field("controller", controller)
                .finish(),
            Self::DropShadow {
                offset,
                sigma_x,
                sigma_y,
                color,
                controller,
                ..
            } => f
                .debug_struct("DropShadow")
                .field("offset", offset)
                .field("sigma_x", sigma_x)
                .field("sigma_y", sigma_y)
                .field("color", color)
                .field("controller", controller)
                .finish(),
            Self::ColorFiltered {
                filter, controller, ..
            } => f
                .debug_struct("ColorFiltered")
                .field("filter", filter)
                .field("controller", controller)
                .finish(),
            Self::Blend { mode, .. } => f.debug_struct("Blend").field("mode", mode).finish(),
            Self::SelectableText { text, style, align } => f
                .debug_struct("SelectableText")
                .field("text", text)
                .field("style", style)
                .field("align", align)
                .finish(),
            Self::SelectionArea { controller, .. } => f
                .debug_struct("SelectionArea")
                .field("controller", controller)
                .finish(),
        }
    }
}
impl PartialEq for WidgetKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Box { size: a, color: b }, Self::Box { size: c, color: d }) => a == c && b == d,
            (
                Self::Shape {
                    path: a,
                    fill: b,
                    stroke: c,
                    size: d,
                },
                Self::Shape {
                    path: e,
                    fill: f,
                    stroke: g,
                    size: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::CustomPaint {
                    size: a,
                    display_list: b,
                },
                Self::CustomPaint {
                    size: c,
                    display_list: d,
                },
            ) => a == c && b == d,
            (
                Self::Limited {
                    max_width: a,
                    max_height: b,
                    child: c,
                },
                Self::Limited {
                    max_width: d,
                    max_height: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::SelectableText {
                    text: a,
                    style: b,
                    align: c,
                },
                Self::SelectableText {
                    text: d,
                    style: e,
                    align: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::SelectionArea {
                    controller: a,
                    child: b,
                },
                Self::SelectionArea {
                    controller: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Overflow {
                    min_width: a,
                    max_width: b,
                    min_height: c,
                    max_height: d,
                    child: e,
                },
                Self::Overflow {
                    min_width: f,
                    max_width: g,
                    min_height: h,
                    max_height: i,
                    child: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
            (Self::RepaintBoundary { child: a }, Self::RepaintBoundary { child: b }) => a == b,
            (
                Self::Gesture {
                    behavior: a_behavior,
                    callbacks: a,
                    child: b,
                },
                Self::Gesture {
                    behavior: b_behavior,
                    callbacks: c,
                    child: d,
                },
            ) => a_behavior == b_behavior && gesture_callbacks_eq(a, c) && b == d,
            (
                Self::Draggable {
                    source: a,
                    child: b,
                },
                Self::Draggable {
                    source: c,
                    child: d,
                },
            ) => Rc::ptr_eq(a, c) && b == d,
            (
                Self::DragTarget {
                    target: a,
                    child: b,
                },
                Self::DragTarget {
                    target: c,
                    child: d,
                },
            ) => Rc::ptr_eq(a, c) && b == d,
            (
                Self::IgnorePointer {
                    ignoring: a,
                    child: b,
                },
                Self::IgnorePointer {
                    ignoring: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::AbsorbPointer {
                    absorbing: a,
                    child: b,
                },
                Self::AbsorbPointer {
                    absorbing: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Table {
                    columns: a,
                    column_spacing: b,
                    row_spacing: c,
                    children: d,
                },
                Self::Table {
                    columns: e,
                    column_spacing: f,
                    row_spacing: g,
                    children: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::Decorated {
                    size: a,
                    background: b,
                    border: c,
                    radius: d,
                    child: e,
                },
                Self::Decorated {
                    size: f,
                    background: g,
                    border: h,
                    radius: i,
                    child: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
            (
                Self::Button {
                    size: a,
                    color: b,
                    hover_color: c0,
                    pressed_color: d0,
                    focused_color: e0,
                    disabled_color: f0,
                    enabled: g0,
                    focusable_when_disabled: h0,
                    action: c,
                    callback: d,
                    hover_action: i,
                    hover_callback: j,
                    exit_action: k,
                    exit_callback: l,
                    has_callback: m,
                    child: n,
                },
                Self::Button {
                    size: e,
                    color: f,
                    hover_color: c1,
                    pressed_color: d1,
                    focused_color: e1,
                    disabled_color: f1,
                    enabled: g1,
                    focusable_when_disabled: h1,
                    action: g,
                    callback: h,
                    hover_action: o,
                    hover_callback: p,
                    exit_action: q,
                    exit_callback: r,
                    has_callback: s,
                    child: t,
                },
            ) => {
                a == e
                    && b == f
                    && c0 == c1
                    && d0 == d1
                    && e0 == e1
                    && f0 == f1
                    && g0 == g1
                    && h0 == h1
                    && c == g
                    && i == o
                    && k == q
                    && m == s
                    && n == t
                    && match (d, h) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (j, p) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (l, r) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Text {
                    text: a,
                    style: b,
                    align: c,
                    soft_wrap: d,
                    max_lines: e,
                    overflow: f,
                },
                Self::Text {
                    text: g,
                    style: h,
                    align: i,
                    soft_wrap: j,
                    max_lines: k,
                    overflow: l,
                },
            ) => a == g && b == h && c == i && d == j && e == k && f == l,
            (
                Self::Baseline {
                    baseline: a,
                    child: b,
                },
                Self::Baseline {
                    baseline: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Image {
                    image: a,
                    width: b,
                    height: c,
                    fit: d,
                    repeat: e,
                    alignment: k,
                    sampling: m,
                },
                Self::Image {
                    image: f,
                    width: g,
                    height: h,
                    fit: i,
                    repeat: j,
                    alignment: l,
                    sampling: n,
                },
            ) => a == f && b == g && c == h && d == i && e == j && k == l && m == n,
            (
                Self::TextField {
                    controller: a,
                    size: b,
                    style: c,
                    placeholder: d,
                    on_submit: e,
                    multiline: k,
                    min_lines: l,
                    max_lines: m,
                    expands: n,
                    text_align: o,
                    enabled: p,
                    read_only: q,
                    obscure_text: r,
                    cursor_width: s,
                    cursor_height: t,
                    cursor_radius: u,
                    show_cursor: v,
                    cursor_color: w,
                    selection_color: x,
                },
                Self::TextField {
                    controller: f,
                    size: g,
                    style: h,
                    placeholder: i,
                    on_submit: j,
                    multiline: l2,
                    min_lines: l3,
                    max_lines: m2,
                    expands: n2,
                    text_align: o2,
                    enabled: y,
                    read_only: z,
                    obscure_text: aa,
                    cursor_width: ab,
                    cursor_height: ac,
                    cursor_radius: ad,
                    show_cursor: ae,
                    cursor_color: af,
                    selection_color: ag,
                },
            ) => {
                a == f
                    && b == g
                    && c == h
                    && d == i
                    && k == l2
                    && l == l3
                    && m == m2
                    && n == n2
                    && o == o2
                    && p == y
                    && q == z
                    && r == aa
                    && s == ab
                    && t == ac
                    && u == ad
                    && v == ae
                    && w == af
                    && x == ag
                    && match (e, j) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Padding {
                    padding: a,
                    child: b,
                },
                Self::Padding {
                    padding: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Fractional {
                    width_factor: a,
                    height_factor: b,
                    child: c,
                },
                Self::Fractional {
                    width_factor: d,
                    height_factor: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Wrap {
                    axis: a,
                    alignment: b,
                    spacing: c,
                    run_alignment: d,
                    run_spacing: e,
                    cross_axis_alignment: f,
                    text_direction: g,
                    vertical_direction: h,
                    children: i,
                },
                Self::Wrap {
                    axis: j,
                    alignment: k,
                    spacing: l,
                    run_alignment: m,
                    run_spacing: n,
                    cross_axis_alignment: o,
                    text_direction: p,
                    vertical_direction: q,
                    children: r,
                },
            ) => {
                a == j
                    && b == k
                    && c == l
                    && d == m
                    && e == n
                    && f == o
                    && g == p
                    && h == q
                    && i == r
            }
            (
                Self::Constrained {
                    constraints: a,
                    child: b,
                },
                Self::Constrained {
                    constraints: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Unconstrained {
                    constrained_axis: a,
                    child: b,
                },
                Self::Unconstrained {
                    constrained_axis: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Scroll {
                    controller: a,
                    axis: e,
                    reverse: f,
                    physics: g,
                    child: b,
                },
                Self::Scroll {
                    controller: c,
                    axis: h,
                    reverse: i,
                    physics: j,
                    child: d,
                },
            ) => a == c && e == h && f == i && g == j && b == d,
            (
                Self::PersistentHeader {
                    controller: a,
                    axis: e,
                    reverse: f,
                    pinned: g,
                    child: b,
                },
                Self::PersistentHeader {
                    controller: c,
                    axis: h,
                    reverse: i,
                    pinned: j,
                    child: d,
                },
            ) => a == c && e == h && f == i && g == j && b == d,
            (Self::SliverViewport { config: a }, Self::SliverViewport { config: b }) => a == b,
            (
                Self::Translate {
                    controller: a,
                    child: b,
                },
                Self::Translate {
                    controller: c,
                    child: d,
                },
            ) => Rc::ptr_eq(&a.offset, &c.offset) && b == d,
            (
                Self::Transform {
                    transform: a,
                    origin: b,
                    child: c,
                },
                Self::Transform {
                    transform: d,
                    origin: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Scale {
                    controller: a,
                    origin: b,
                    child: c,
                },
                Self::Scale {
                    controller: d,
                    origin: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Rotation {
                    controller: a,
                    origin: b,
                    alignment: c,
                    child: d,
                    ..
                },
                Self::Rotation {
                    controller: e,
                    origin: f,
                    alignment: g,
                    child: h,
                    ..
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::FittedBox {
                    fit: a,
                    alignment: b,
                    child: c,
                },
                Self::FittedBox {
                    fit: d,
                    alignment: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Opacity {
                    alpha: a,
                    controller: b,
                    child: c,
                },
                Self::Opacity {
                    alpha: d,
                    controller: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Blur {
                    sigma_x: a,
                    sigma_y: b,
                    controller: c,
                    child: d,
                },
                Self::Blur {
                    sigma_x: e,
                    sigma_y: f,
                    controller: g,
                    child: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::DropShadow {
                    offset: a,
                    sigma_x: b,
                    sigma_y: c,
                    color: d,
                    controller: e,
                    child: f,
                },
                Self::DropShadow {
                    offset: g,
                    sigma_x: h,
                    sigma_y: i,
                    color: j,
                    controller: k,
                    child: l,
                },
            ) => a == g && b == h && c == i && d == j && e == k && f == l,
            (
                Self::ColorFiltered {
                    filter: a,
                    controller: b,
                    child: c,
                },
                Self::ColorFiltered {
                    filter: d,
                    controller: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (Self::Blend { mode: a, child: b }, Self::Blend { mode: c, child: d }) => {
                a == c && b == d
            }
            (
                Self::Align {
                    alignment: a,
                    width_factor: b,
                    height_factor: c,
                    child: d,
                },
                Self::Align {
                    alignment: e,
                    width_factor: f,
                    height_factor: g,
                    child: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::Flex {
                    axis: a,
                    main_axis_alignment: b,
                    main_axis_size: c,
                    cross_axis_alignment: d,
                    text_direction: e,
                    vertical_direction: f,
                    spacing: g,
                    children: h,
                },
                Self::Flex {
                    axis: i,
                    main_axis_alignment: j,
                    main_axis_size: k,
                    cross_axis_alignment: l,
                    text_direction: m,
                    vertical_direction: n,
                    spacing: o,
                    children: p,
                },
            ) => a == i && b == j && c == k && d == l && e == m && f == n && g == o && h == p,
            (
                Self::Flexible {
                    flex: a,
                    fit: b,
                    child: c,
                },
                Self::Flexible {
                    flex: d,
                    fit: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,

            (
                Self::Stack {
                    alignment: a,
                    text_direction: b,
                    fit: c,
                    clip_behavior: d,
                    children: e,
                },
                Self::Stack {
                    alignment: f,
                    text_direction: g,
                    fit: h,
                    clip_behavior: i,
                    children: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
            (
                Self::Positioned {
                    left: a,
                    top: b,
                    right: c,
                    bottom: d,
                    width: e,
                    height: f,
                    child: g,
                },
                Self::Positioned {
                    left: h,
                    top: i,
                    right: j,
                    bottom: k,
                    width: l,
                    height: m,
                    child: n,
                },
            ) => a == h && b == i && c == j && d == k && e == l && f == m && g == n,
            (
                Self::IndexedStack {
                    alignment: a,
                    index: b,
                    children: c,
                },
                Self::IndexedStack {
                    alignment: d,
                    index: e,
                    children: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::SafeArea {
                    minimum: a,
                    left: b,
                    top: c,
                    right: d,
                    bottom: e,
                    maintain_bottom_view_padding: f,
                    child: g,
                },
                Self::SafeArea {
                    minimum: h,
                    left: i,
                    top: j,
                    right: k,
                    bottom: l,
                    maintain_bottom_view_padding: m,
                    child: n,
                },
            ) => a == h && b == i && c == j && d == k && e == l && f == m && g == n,
            (
                Self::ClipRect {
                    clip_behavior: a,
                    child: b,
                },
                Self::ClipRect {
                    clip_behavior: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::ClipRRect {
                    radius: a,
                    clip_behavior: b,
                    child: c,
                },
                Self::ClipRRect {
                    radius: d,
                    clip_behavior: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::ClipOval {
                    clip_behavior: a,
                    child: b,
                },
                Self::ClipOval {
                    clip_behavior: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::ClipPath {
                    path: a,
                    clip_behavior: b,
                    child: c,
                },
                Self::ClipPath {
                    path: d,
                    clip_behavior: e,
                    child: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::LayoutBuilder {
                    builder: a,
                    environment: c,
                    revision: e,
                },
                Self::LayoutBuilder {
                    builder: b,
                    environment: d,
                    revision: f,
                },
            ) => {
                Rc::ptr_eq(a, b)
                    && match (c, d) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (e, f) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Visibility {
                    visible: a,
                    child: b,
                },
                Self::Visibility {
                    visible: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::AspectRatio { ratio: a, child: b },
                Self::AspectRatio { ratio: c, child: d },
            ) => a == c && b == d,
            (
                Self::NotificationListener {
                    callback: a,
                    child: b,
                },
                Self::NotificationListener {
                    callback: c,
                    child: d,
                },
            ) => {
                let callback_equal = match (a, c) {
                    (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                    (None, None) => true,
                    _ => false,
                };
                callback_equal && b.as_ref() == d.as_ref()
            }
            _ => false,
        }
    }
}

fn gesture_callbacks_eq(left: &GestureCallbacks, right: &GestureCallbacks) -> bool {
    fn same_callback<T: ?Sized>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    same_callback(&left.on_tap, &right.on_tap)
        && same_callback(&left.on_tap_down, &right.on_tap_down)
        && same_callback(&left.on_tap_up, &right.on_tap_up)
        && same_callback(&left.on_tap_cancel, &right.on_tap_cancel)
        && same_callback(&left.on_double_tap, &right.on_double_tap)
        && same_callback(&left.on_double_tap_down, &right.on_double_tap_down)
        && same_callback(&left.on_double_tap_cancel, &right.on_double_tap_cancel)
        && same_callback(&left.on_long_press, &right.on_long_press)
        && same_callback(&left.on_long_press_start, &right.on_long_press_start)
        && same_callback(
            &left.on_long_press_move_update,
            &right.on_long_press_move_update,
        )
        && same_callback(&left.on_long_press_up, &right.on_long_press_up)
        && same_callback(&left.on_long_press_end, &right.on_long_press_end)
        && same_callback(&left.on_pan_down, &right.on_pan_down)
        && same_callback(&left.on_pan_start, &right.on_pan_start)
        && same_callback(&left.on_pan_update, &right.on_pan_update)
        && same_callback(&left.on_pan_end, &right.on_pan_end)
        && same_callback(&left.on_pan_cancel, &right.on_pan_cancel)
        && same_callback(
            &left.on_horizontal_drag_down,
            &right.on_horizontal_drag_down,
        )
        && same_callback(
            &left.on_horizontal_drag_start,
            &right.on_horizontal_drag_start,
        )
        && same_callback(
            &left.on_horizontal_drag_update,
            &right.on_horizontal_drag_update,
        )
        && same_callback(&left.on_horizontal_drag_end, &right.on_horizontal_drag_end)
        && same_callback(
            &left.on_horizontal_drag_cancel,
            &right.on_horizontal_drag_cancel,
        )
        && same_callback(&left.on_vertical_drag_down, &right.on_vertical_drag_down)
        && same_callback(&left.on_vertical_drag_start, &right.on_vertical_drag_start)
        && same_callback(
            &left.on_vertical_drag_update,
            &right.on_vertical_drag_update,
        )
        && same_callback(&left.on_vertical_drag_end, &right.on_vertical_drag_end)
        && same_callback(
            &left.on_vertical_drag_cancel,
            &right.on_vertical_drag_cancel,
        )
        && same_callback(&left.on_scale_start, &right.on_scale_start)
        && same_callback(&left.on_scale_update, &right.on_scale_update)
        && same_callback(&left.on_scale_end, &right.on_scale_end)
        && same_callback(&left.on_cancel, &right.on_cancel)
        && same_callback(&left.on_key, &right.on_key)
        && same_callback(&left.on_key_down, &right.on_key_down)
        && same_callback(&left.on_key_repeat, &right.on_key_repeat)
        && same_callback(&left.on_key_up, &right.on_key_up)
        && same_callback(&left.on_shortcut, &right.on_shortcut)
        && left.focus_node == right.focus_node
        && left.autofocus == right.autofocus
        && left.include_semantics == right.include_semantics
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidgetType {
    Box,
    Shape,
    CustomPaint,
    Decorated,
    Image,
    Button,
    Text,
    SelectableText,
    SelectionArea,
    TextField,
    Padding,
    Constrained,
    Limited,
    Overflow,
    Unconstrained,
    Fractional,
    Baseline,
    RepaintBoundary,
    Gesture,
    Draggable,
    DragTarget,
    IgnorePointer,
    AbsorbPointer,
    Align,
    Flex,
    Flexible,
    Wrap,
    Table,
    Stack,
    Positioned,
    IndexedStack,
    SafeArea,
    ClipRect,
    ClipRRect,
    ClipOval,
    ClipPath,
    LayoutBuilder,
    Visibility,
    AspectRatio,
    Scroll,
    PersistentHeader,
    NotificationListener,
    SliverViewport,
    Translate,
    Transform,
    Scale,
    Rotation,
    FittedBox,
    Opacity,
    Blur,
    DropShadow,
    ColorFiltered,
    Blend,
}
impl WidgetType {
    const fn name(self) -> &'static str {
        match self {
            Self::Box => "Box",
            Self::Shape => "Shape",
            Self::CustomPaint => "CustomPaint",
            Self::Decorated => "DecoratedBox",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::Text => "Text",
            Self::SelectableText => "SelectableText",
            Self::SelectionArea => "SelectionArea",
            Self::TextField => "TextField",
            Self::Padding => "Padding",
            Self::Constrained => "ConstrainedBox",
            Self::Limited => "LimitedBox",
            Self::Overflow => "OverflowBox",
            Self::Unconstrained => "UnconstrainedBox",
            Self::Fractional => "FractionallySizedBox",
            Self::Baseline => "Baseline",
            Self::RepaintBoundary => "RepaintBoundary",
            Self::Gesture => "GestureDetector",
            Self::Draggable => "Draggable",
            Self::DragTarget => "DragTarget",
            Self::IgnorePointer => "IgnorePointer",
            Self::AbsorbPointer => "AbsorbPointer",
            Self::Align => "Align",
            Self::Flex => "Flex",
            Self::Flexible => "Flexible",
            Self::Wrap => "Wrap",
            Self::Table => "Table",
            Self::Stack => "Stack",
            Self::Positioned => "Positioned",
            Self::IndexedStack => "IndexedStack",
            Self::SafeArea => "SafeArea",
            Self::ClipRect => "ClipRect",
            Self::ClipRRect => "ClipRRect",
            Self::ClipOval => "ClipOval",
            Self::ClipPath => "ClipPath",
            Self::LayoutBuilder => "LayoutBuilder",
            Self::Visibility => "Visibility",
            Self::AspectRatio => "AspectRatio",
            Self::Scroll => "ScrollView",
            Self::PersistentHeader => "PersistentHeader",
            Self::NotificationListener => "NotificationListener",
            Self::SliverViewport => "SliverViewport",
            Self::Translate => "Translate",
            Self::Transform => "Transform",
            Self::Scale => "ScaleTransition",
            Self::Rotation => "RotationTransition",
            Self::FittedBox => "FittedBox",
            Self::Opacity => "Opacity",
            Self::Blur => "ImageFiltered",
            Self::DropShadow => "DropShadow",
            Self::ColorFiltered => "ColorFiltered",
            Self::Blend => "Blend",
        }
    }
}
impl Widget {
    /// Creates a widget from a internal kind descriptor.
    #[must_use]
    pub fn from_kind(kind: WidgetKind) -> Self {
        Self {
            key: None,
            kind,
            semantics: SemanticProperties::default(),
        }
    }

    /// Attaches retained focus traversal metadata to a transparent wrapper.
    /// This is crate-internal because public callers use the Flutter-shaped
    /// `FocusTraversal*` widgets.
    #[doc(hidden)]
    pub(crate) fn with_focus_traversal_policy(mut self, policy: FocusTraversalPolicyKind) -> Self {
        self.semantics.focus_traversal_policy = Some(policy);
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_focus_traversal_order(mut self, order: Option<f64>) -> Self {
        self.semantics.focus_traversal_order = order.filter(|value| value.is_finite());
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_excluded_focus(mut self, excluding: bool) -> Self {
        self.semantics.exclude_focus = excluding;
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_excluded_focus_traversal(mut self, excluding: bool) -> Self {
        self.semantics.exclude_focus_traversal = excluding;
        self
    }

    /// Attaches the retained undo-history capacity to a text-field subtree.
    /// The runtime consumes this metadata without making Widgets depend on
    /// the runtime crate.
    #[doc(hidden)]
    pub(crate) fn with_undo_history_max_entries(mut self, max_entries: usize) -> Self {
        self.semantics.undo_history_max_entries = Some(max_entries);
        self
    }

    /// Marks this retained subtree as the initial focus scope.
    #[doc(hidden)]
    pub(crate) fn with_focus_scope_autofocus(mut self, autofocus: bool) -> Self {
        self.semantics.focus_scope_autofocus = autofocus;
        self
    }

    /// Attaches native text-input hints to the retained editor without making
    /// the renderer-neutral widget depend on a platform window handle.
    #[doc(hidden)]
    pub(crate) fn with_text_input_hints(
        mut self,
        input_type: TextInputTypeHint,
        input_action: TextInputActionHint,
    ) -> Self {
        self.semantics.text_input_type = Some(input_type);
        self.semantics.text_input_action = Some(input_action);
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_semantic_callback(
        mut self,
        action: SemanticActionKind,
        callback: Rc<dyn Fn() + 'static>,
    ) -> Self {
        match action {
            SemanticActionKind::Activate => self.semantics.callbacks.activate = Some(callback),
            SemanticActionKind::Increment => self.semantics.callbacks.increment = Some(callback),
            SemanticActionKind::Decrement => self.semantics.callbacks.decrement = Some(callback),
            SemanticActionKind::ScrollForward => {
                self.semantics.callbacks.scroll_forward = Some(callback)
            }
            SemanticActionKind::ScrollBackward => {
                self.semantics.callbacks.scroll_backward = Some(callback)
            }
            SemanticActionKind::Focus
            | SemanticActionKind::SetText
            | SemanticActionKind::SetSelection => {}
        }
        self
    }

    /// Text content when this widget is text-like (DevTools labels only).
    pub fn text_if_any(&self) -> Option<String> {
        match &self.kind {
            WidgetKind::Text { text, .. } | WidgetKind::SelectableText { text, .. } => {
                Some(text.clone())
            }
            _ => None,
        }
    }

    /// Returns the best text label exposed by this widget or a transparent
    /// semantic/layout wrapper around it.
    ///
    /// Control crates use this when a Flutter-shaped control accepts a custom
    /// child instead of a dedicated `label` argument. The visual child and
    /// the accessible name are separate concepts, but text children provide a
    /// safe default for ordinary custom-content controls.
    #[doc(hidden)]
    pub fn semantic_text(&self) -> Option<String> {
        self.semantics
            .label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| {
                self.semantics
                    .explicit
                    .as_ref()
                    .and_then(|semantics| semantics.label.clone())
                    .filter(|label| !label.trim().is_empty())
            })
            .or_else(|| widget_text(self))
    }

    #[must_use]
    pub fn box_(size: Size, color: Color) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Box { size, color },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn fixed_box(size: Size, color: Color) -> Self {
        Self::box_(size, color)
    }
    #[must_use]
    fn shape(
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        size: Option<Size>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Shape {
                path,
                fill,
                stroke,
                size,
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Paints a caller-provided renderer-neutral display list at a fixed
    /// logical size.
    #[must_use]
    pub fn custom_paint(size: Size, display_list: DisplayList) -> Self {
        Self {
            key: None,
            kind: WidgetKind::CustomPaint { size, display_list },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    fn decorated(
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Widget,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Decorated {
                size,
                background,
                border,
                radius,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub(crate) fn button(size: Size, color: Color, action: ActionId) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Button {
                size,
                color,
                hover_color: None,
                pressed_color: None,
                focused_color: None,
                disabled_color: None,
                enabled: true,
                focusable_when_disabled: false,
                action,
                callback: None,
                hover_action: ActionId(0),
                hover_callback: None,
                exit_action: ActionId(0),
                exit_callback: None,
                has_callback: false,
                child: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Builds the retained action surface shared by sibling control crates.
    ///
    /// This is crate-internal plumbing rather than a Flutter-facing widget;
    /// applications should use `GestureDetector` or a Material button.
    #[doc(hidden)]
    pub(crate) fn action_surface(surface: crate::internal::ActionSurface) -> Self {
        let semantic_label = if surface.label.is_empty() {
            surface.content.as_ref().and_then(Widget::semantic_text)
        } else {
            Some(surface.label.clone())
        };
        let label = surface.content.unwrap_or_else(|| {
            Widget::padding(
                surface.padding,
                Widget::text_styled(surface.label, surface.label_style, TextAlign::Start),
            )
        });
        Self {
            key: None,
            kind: WidgetKind::Button {
                size: surface.size,
                color: surface.color,
                hover_color: surface.hover_color,
                pressed_color: surface.pressed_color,
                focused_color: surface.focused_color,
                disabled_color: surface.disabled_color,
                enabled: surface.enabled,
                focusable_when_disabled: surface.focusable_when_disabled,
                action: ActionId(0),
                callback: surface.callback,
                hover_action: ActionId(0),
                hover_callback: surface.hover_callback,
                exit_action: ActionId(0),
                exit_callback: surface.exit_callback,
                has_callback: false,
                child: Some(Box::new(label)),
            },
            semantics: SemanticProperties {
                // Custom content is inspected for a text or explicit semantic
                // label so low-level controls retain a useful accessible name.
                label: semantic_label,
                ..SemanticProperties::default()
            },
        }
    }
    pub fn bind_callbacks(&mut self, allocate: &mut impl FnMut(Rc<dyn Fn()>) -> ActionId) {
        match &mut self.kind {
            WidgetKind::Button {
                action,
                callback,
                hover_action,
                hover_callback,
                exit_action,
                exit_callback,
                has_callback,
                child,
                ..
            } => {
                if let Some(callback) = callback.take() {
                    *action = allocate(callback);
                    *has_callback = true;
                }
                if let Some(callback) = hover_callback.take() {
                    *hover_action = allocate(callback);
                }
                if let Some(callback) = exit_callback.take() {
                    *exit_action = allocate(callback);
                }
                if let Some(child) = child {
                    child.bind_callbacks(allocate);
                }
            }
            WidgetKind::Padding { child, .. }
            | WidgetKind::Decorated { child, .. }
            | WidgetKind::Constrained { child, .. }
            | WidgetKind::Limited { child, .. }
            | WidgetKind::Overflow { child, .. }
            | WidgetKind::Unconstrained { child, .. }
            | WidgetKind::Fractional { child, .. }
            | WidgetKind::Baseline { child, .. }
            | WidgetKind::RepaintBoundary { child, .. }
            | WidgetKind::Gesture { child, .. }
            | WidgetKind::Draggable { child, .. }
            | WidgetKind::DragTarget { child, .. }
            | WidgetKind::IgnorePointer { child, .. }
            | WidgetKind::AbsorbPointer { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Flexible { child, .. }
            | WidgetKind::Positioned { child, .. }
            | WidgetKind::SafeArea { child, .. }
            | WidgetKind::ClipRect { child, .. }
            | WidgetKind::ClipRRect { child, .. }
            | WidgetKind::ClipOval { child, .. }
            | WidgetKind::ClipPath { child, .. }
            | WidgetKind::Visibility { child, .. }
            | WidgetKind::AspectRatio { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::PersistentHeader { child, .. }
            | WidgetKind::NotificationListener { child, .. }
            | WidgetKind::Translate { child, .. }
            | WidgetKind::Transform { child, .. }
            | WidgetKind::Scale { child, .. }
            | WidgetKind::Rotation { child, .. }
            | WidgetKind::FittedBox { child, .. }
            | WidgetKind::Opacity { child, .. }
            | WidgetKind::Blur { child, .. }
            | WidgetKind::DropShadow { child, .. }
            | WidgetKind::ColorFiltered { child, .. }
            | WidgetKind::Blend { child, .. } => child.bind_callbacks(allocate),
            WidgetKind::SelectionArea { child, .. } => child.bind_callbacks(allocate),
            WidgetKind::SliverViewport { .. } | WidgetKind::LayoutBuilder { .. } => {}
            WidgetKind::Flex { children, .. }
            | WidgetKind::Wrap { children, .. }
            | WidgetKind::Table { children, .. }
            | WidgetKind::Stack { children, .. }
            | WidgetKind::IndexedStack { children, .. } => {
                for child in children {
                    child.bind_callbacks(allocate);
                }
            }
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::CustomPaint { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::SelectableText { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. } => {}
        }
    }
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style: TextStyle::default(),
                align: TextAlign::Start,
                soft_wrap: true,
                max_lines: None,
                overflow: TextOverflow::Clip,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn text_styled(text: impl Into<String>, style: TextStyle, align: TextAlign) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
                soft_wrap: true,
                max_lines: None,
                overflow: TextOverflow::Clip,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn text_configured(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
        soft_wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn selectable_text_styled(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SelectableText {
                text: text.into(),
                style,
                align,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn selection_area(controller: SelectionAreaController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SelectionArea {
                controller,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    fn image(
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        repeat: ImageRepeat,
        alignment: Alignment,
        sampling: ImageSampling,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Image {
                image,
                width,
                height,
                fit,
                repeat,
                alignment,
                sampling,
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Creates the retained core editing primitive used by
    /// [`EditableText`]. Higher-level Material controls may add chrome and
    /// validation around this descriptor.
    #[must_use]
    pub fn editable_text(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
    ) -> Self {
        Self::editable_text_configured(
            controller,
            size,
            style,
            placeholder,
            on_submit,
            multiline,
            None,
            None,
            false,
            TextAlign::Start,
            true,
            false,
            false,
        )
    }

    /// Creates an editable text primitive with the common editing policies
    /// used by Material text fields.  The original [`Widget::editable_text`]
    /// constructor remains as the renderer-neutral, enabled single-line
    /// default; Material uses this richer boundary so read-only, obscured,
    /// alignment, and multiline sizing are real retained properties rather
    /// than decoration-only metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn editable_text_configured(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
    ) -> Self {
        Self::editable_text_configured_with_cursor(
            controller,
            size,
            style,
            placeholder,
            on_submit,
            multiline,
            min_lines,
            max_lines,
            expands,
            text_align,
            enabled,
            read_only,
            obscure_text,
            1.0,
            None,
            0.0,
            true,
            Color::WHITE,
            Color::rgba(72, 120, 220, 150),
        )
    }

    /// Creates the editable primitive with renderer-neutral caret and
    /// selection paint controls. Material uses this richer boundary for
    /// `TextField` cursor/selection configuration while the legacy constructor
    /// above keeps its source-compatible defaults.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn editable_text_configured_with_cursor(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
        cursor_width: f32,
        cursor_height: Option<f32>,
        cursor_radius: f32,
        show_cursor: bool,
        cursor_color: Color,
        selection_color: Color,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::TextField {
                controller,
                size,
                style,
                placeholder,
                on_submit,
                multiline,
                min_lines: min_lines.map(|value| value.max(1)),
                max_lines: max_lines.map(|value| value.max(1)),
                expands,
                text_align,
                enabled,
                read_only,
                obscure_text,
                cursor_width: cursor_width.max(0.0),
                cursor_height: cursor_height.map(|value| value.max(0.0)),
                cursor_radius: cursor_radius.max(0.0),
                show_cursor,
                cursor_color,
                selection_color,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn padding(padding: EdgeInsets, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Padding {
                padding,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Tightens the incoming layout bounds before passing them to `child`.
    #[must_use]
    pub fn constrained(constraints: Constraints, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Constrained {
                constraints,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn limited_box(max_width: f32, max_height: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Limited {
                max_width: finite_non_negative(max_width),
                max_height: finite_non_negative(max_height),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn overflow_box(
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Overflow {
                min_width: min_width.map(finite_non_negative),
                max_width: max_width.map(finite_non_negative),
                min_height: min_height.map(finite_non_negative),
                max_height: max_height.map(finite_non_negative),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Lets a child take its natural size, optionally retaining the parent's
    /// limits on one axis while this wrapper itself still fits its parent.
    #[must_use]
    pub fn unconstrained(constrained_axis: Option<Axis>, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Unconstrained {
                constrained_axis,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Sizes a child to factors of the finite parent bounds on the specified
    /// axes. Missing factors preserve the child's natural size on that axis.
    #[must_use]
    pub fn fractionally_sized(
        width_factor: Option<f32>,
        height_factor: Option<f32>,
        child: Self,
    ) -> Self {
        for factor in [width_factor, height_factor].into_iter().flatten() {
            assert!(
                factor.is_finite() && factor >= 0.,
                "fractional factors must be finite and non-negative"
            );
        }
        Self {
            key: None,
            kind: WidgetKind::Fractional {
                width_factor,
                height_factor,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Positions a child so that its reported baseline is at `baseline`.
    #[must_use]
    pub fn baseline(baseline: f32, child: Self) -> Self {
        assert!(
            baseline.is_finite() && baseline >= 0.,
            "baseline must be finite and non-negative"
        );
        Self {
            key: None,
            kind: WidgetKind::Baseline {
                baseline,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Creates an explicit retained picture boundary around `child`.
    ///
    /// Descendant paint changes update their own cached picture without
    /// repainting this boundary's otherwise empty retained picture.
    #[must_use]
    pub fn repaint_boundary(child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::RepaintBoundary {
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Attaches tap, double-tap, long-press, and pan recognition to a retained
    /// subtree. A hit-tested down event captures the sequence for this region.
    #[must_use]
    pub fn gesture(callbacks: GestureCallbacks, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Gesture {
                behavior: crate::gestures::HitTestBehavior::DeferToChild,
                callbacks: Box::new(callbacks),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    pub(crate) fn draggable(source: Rc<dyn RetainedDragSource>, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Draggable {
                source,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    pub(crate) fn drag_target(target: Rc<dyn RetainedDragTarget>, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DragTarget {
                target,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Removes this subtree from pointer hit testing while leaving painting and
    /// semantics intact. Siblings behind it remain eligible for the event.
    #[must_use]
    pub fn ignore_pointer(ignoring: bool, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::IgnorePointer {
                ignoring,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Intercepts pointer hit testing at this boundary. Descendants do not
    /// receive ordinary retained interaction while painting and semantics are
    /// preserved.
    #[must_use]
    pub fn absorb_pointer(absorbing: bool, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::AbsorbPointer {
                absorbing,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn align(alignment: Alignment, child: Self) -> Self {
        crate::layout::Align::new(alignment, child).into()
    }
    #[must_use]
    pub fn row(children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Row::new(children.into())
            .main_axis_size(incular_config::MainAxisSize::Min)
            .cross_axis_alignment(incular_config::CrossAxisAlignment::Start)
            .into()
    }
    #[must_use]
    pub fn column(children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Column::new(children.into())
            .main_axis_size(incular_config::MainAxisSize::Min)
            .cross_axis_alignment(incular_config::CrossAxisAlignment::Start)
            .into()
    }
    #[must_use]
    pub fn flex(direction: Axis, children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Flex::new(direction, children.into()).into()
    }
    #[must_use]
    pub fn flexible(flex: u32, fit: incular_config::FlexFit, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Flexible {
                flex: flex.max(1),
                fit,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Packs children into successive runs when the main-axis bound is
    /// exhausted. The axis chooses whether runs flow horizontally or vertically.
    #[must_use]
    pub fn wrap(
        axis: Axis,
        spacing: f32,
        run_spacing: f32,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        crate::layout::Wrap::new(children.into())
            .direction(axis)
            .spacing(spacing)
            .run_spacing(run_spacing)
            .into()
    }
    /// Places row-major children in max-content columns.
    #[must_use]
    pub fn table(
        columns: usize,
        column_spacing: f32,
        row_spacing: f32,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        crate::layout::Table::new(columns, children.into())
            .column_spacing(column_spacing)
            .row_spacing(row_spacing)
            .into()
    }
    /// Paints children in order at the same origin. The last child is the
    /// front-most hit-test target, matching Flutter's stack semantics.
    #[must_use]
    pub fn stack(alignment: Alignment, children: impl Into<Vec<Self>>) -> Self {
        crate::layout::Stack::new(children.into())
            .alignment(alignment)
            .into()
    }
    #[must_use]
    pub fn positioned(
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Positioned {
                left: left.map(finite_non_negative),
                top: top.map(finite_non_negative),
                right: right.map(finite_non_negative),
                bottom: bottom.map(finite_non_negative),
                width: width.map(finite_non_negative),
                height: height.map(finite_non_negative),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn indexed_stack(
        alignment: Alignment,
        index: usize,
        children: impl Into<Vec<Self>>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::IndexedStack {
                alignment,
                index,
                children: children.into(),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn layout_builder(builder: impl Fn(Constraints) -> Self + 'static) -> Self {
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Wraps a child in an ambient retained builder environment. The wrapper
    /// is transparent to layout and paint; descendants read the value when
    /// their own deferred builders are materialized.
    #[must_use]
    pub fn environment_scope<T: Any>(value: T, child: Self) -> Self {
        let value: Rc<dyn Any> = Rc::new(value);
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_| child.clone()),
                environment: Some(value),
                revision: None,
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained layout builder backed by an explicit local state
    /// revision.  A callback can increment `revision` and the next frame will
    /// rebuild only this builder's child, preserving the rest of the tree.
    /// This is the primitive used by uncontrolled controls such as checkbox,
    /// switch, toggle, and slider.
    #[must_use]
    pub fn stateful_layout_builder(
        revision: Rc<Cell<u64>>,
        builder: impl Fn(Constraints) -> Self + 'static,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                revision: Some(revision),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn visibility(visible: bool, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Visibility {
                visible,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn aspect_ratio(ratio: f32, child: Self) -> Self {
        assert!(
            ratio.is_finite() && ratio > 0.,
            "aspect ratio must be finite and positive"
        );
        Self {
            key: None,
            kind: WidgetKind::AspectRatio {
                ratio,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn scroll_view(controller: ScrollController, child: Self) -> Self {
        Self::scroll_view_with_config(
            controller,
            child,
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
        )
    }

    /// Creates a retained viewport with its complete scroll policy.  The
    /// public `scroll_view` helper intentionally keeps its historical
    /// vertical/clamping defaults; scroll descriptors use this crate-local
    /// constructor so axis, reverse, and physics survive lowering.
    #[must_use]
    pub(crate) fn scroll_view_with_config(
        controller: ScrollController,
        child: Self,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Scroll {
                controller,
                axis,
                reverse,
                physics,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Keeps this flow child at the leading edge of `controller`'s viewport
    /// once it reaches that edge. Consecutive persistent headers push their
    /// predecessors away instead of visually overlapping them.
    #[must_use]
    pub fn persistent_header(controller: ScrollController, child: Self) -> Self {
        Self::persistent_header_with_config(controller, child, Axis::Vertical, false, true)
    }
    /// Creates a persistent header with the axis and direction of its sliver
    /// viewport.  The plain helper retains the historical vertical defaults;
    /// sliver descriptors use this configured form during lowering.
    #[must_use]
    pub(crate) fn persistent_header_with_config(
        controller: ScrollController,
        child: Self,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained notification-listener wrapper. The wrapper has no
    /// visual effect; the tree installs the callback on descendant scroll
    /// positions after those positions are mounted.
    pub(crate) fn notification_listener(
        callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::NotificationListener {
                callback,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }

    /// Creates a retained sliver viewport. Sliver children are materialized by
    /// the viewport delegate during layout, so they are not represented as a
    /// declarative `Column` child list.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sliver_viewport_with_delegate_options(
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
        cache_extent: f32,
        shrink_wrap: bool,
        clip_behavior: Clip,
        delegate: Rc<dyn SliverViewportDelegate>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::SliverViewport {
                config: Rc::new(SliverViewportConfig {
                    controller,
                    axis,
                    reverse,
                    physics,
                    cache_extent: cache_extent.max(0.),
                    shrink_wrap,
                    clip_behavior,
                    delegate,
                }),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn translate(controller: TranslationController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Translate {
                controller,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    /// Applies an arbitrary Kurbo-backed affine transform after layout.
    /// The transform is compositor-only and defaults to the child's center.
    #[must_use]
    pub fn transform(transform: CoreTransform, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn transform_around(transform: CoreTransform, origin: Offset, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: Some(finite_offset(origin)),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn scale(scale: f32, child: Self) -> Self {
        Self::transform(CoreTransform::scale(scale), child)
    }
    #[must_use]
    pub fn rotate(radians: f32, child: Self) -> Self {
        Self::transform(CoreTransform::rotation(radians), child)
    }
    #[must_use]
    pub fn fitted_box(fit: ImageFit, alignment: Alignment, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::FittedBox {
                fit,
                alignment,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_scale(controller: ScaleController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Scale {
                controller,
                origin: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_rotation(controller: RotationController, child: Self) -> Self {
        Self::controlled_rotation_with_alignment(controller, None, child)
    }
    #[must_use]
    pub(crate) fn controlled_rotation_with_alignment(
        controller: RotationController,
        alignment: Option<Alignment>,
        child: Self,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Rotation {
                controller,
                origin: None,
                alignment,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn opacity(alpha: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: normalize_opacity(alpha),
                controller: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_opacity(controller: OpacityController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: controller.opacity(),
                controller: Some(controller),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn blur(sigma: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                controller: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn asymmetric_blur(sigma_x: f32, sigma_y: f32, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma_x),
                sigma_y: normalize_sigma(sigma_y),
                controller: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_blur(controller: BlurController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                controller: Some(controller),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn drop_shadow(offset: Offset, sigma: f32, color: Color, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: finite_offset(offset),
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                color,
                controller: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn controlled_drop_shadow(controller: DropShadowController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: controller.offset(),
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                color: controller.color(),
                controller: Some(controller),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn color_filtered(filter: ColorFilter, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter,
                controller: None,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn color_matrix(filter: ColorFilter, child: Self) -> Self {
        Self::color_filtered(filter, child)
    }
    #[must_use]
    pub fn controlled_color_filtered(controller: ColorFilterController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter: controller.filter(),
                controller: Some(controller),
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn blend(mode: BlendMode, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Blend {
                mode,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn with_key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Overrides the accessible label contributed by this meaningful widget.
    #[must_use]
    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.semantics.label = Some(label.into());
        self
    }
    /// Adds a screen-reader description without changing visible text.
    #[must_use]
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.semantics.description = Some(description.into());
        self
    }
    /// Supplies explicit Incular semantic metadata for this visual widget.
    /// This is useful for icon-only controls, meaningful images, headings,
    /// dialogs, and custom-painted controls; it never exposes native adapter
    /// types to application code.
    #[must_use]
    pub fn semantics(mut self, semantics: ExplicitSemantics) -> Self {
        self.semantics.explicit = Some(semantics);
        self
    }
    /// Excludes this widget and its implementation-detail subtree from semantics.
    #[must_use]
    pub fn exclude_semantics(mut self) -> Self {
        self.semantics.hidden = true;
        self
    }
    /// Merges meaningful descendants into one logical accessible node. The
    /// widget itself must have explicit semantics or a meaningful built-in
    /// role; descendants are not exposed separately.
    #[must_use]
    pub fn merge_semantics(mut self) -> Self {
        self.semantics.merge_descendants = true;
        self
    }
    /// Suppresses preceding semantic siblings at this stacking level. Use it
    /// for a modal/dialog region so screen-reader traversal cannot fall through
    /// to visual content behind the active modal.
    #[must_use]
    pub fn block_semantics(mut self) -> Self {
        self.semantics.block_previous_siblings = true;
        self
    }
    #[must_use]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }
    fn type_(&self) -> WidgetType {
        match self.kind {
            WidgetKind::Box { .. } => WidgetType::Box,
            WidgetKind::Shape { .. } => WidgetType::Shape,
            WidgetKind::CustomPaint { .. } => WidgetType::CustomPaint,
            WidgetKind::Decorated { .. } => WidgetType::Decorated,
            WidgetKind::Image { .. } => WidgetType::Image,
            WidgetKind::Button { .. } => WidgetType::Button,
            WidgetKind::Text { .. } => WidgetType::Text,
            WidgetKind::SelectableText { .. } => WidgetType::SelectableText,
            WidgetKind::SelectionArea { .. } => WidgetType::SelectionArea,
            WidgetKind::TextField { .. } => WidgetType::TextField,
            WidgetKind::Padding { .. } => WidgetType::Padding,
            WidgetKind::Constrained { .. } => WidgetType::Constrained,
            WidgetKind::Limited { .. } => WidgetType::Limited,
            WidgetKind::Overflow { .. } => WidgetType::Overflow,
            WidgetKind::Unconstrained { .. } => WidgetType::Unconstrained,
            WidgetKind::Fractional { .. } => WidgetType::Fractional,
            WidgetKind::Baseline { .. } => WidgetType::Baseline,
            WidgetKind::RepaintBoundary { .. } => WidgetType::RepaintBoundary,
            WidgetKind::Gesture { .. } => WidgetType::Gesture,
            WidgetKind::Draggable { .. } => WidgetType::Draggable,
            WidgetKind::DragTarget { .. } => WidgetType::DragTarget,
            WidgetKind::IgnorePointer { .. } => WidgetType::IgnorePointer,
            WidgetKind::AbsorbPointer { .. } => WidgetType::AbsorbPointer,
            WidgetKind::Align { .. } => WidgetType::Align,
            WidgetKind::Flex { .. } => WidgetType::Flex,
            WidgetKind::Flexible { .. } => WidgetType::Flexible,
            WidgetKind::Wrap { .. } => WidgetType::Wrap,
            WidgetKind::Table { .. } => WidgetType::Table,
            WidgetKind::Stack { .. } => WidgetType::Stack,
            WidgetKind::Positioned { .. } => WidgetType::Positioned,
            WidgetKind::IndexedStack { .. } => WidgetType::IndexedStack,
            WidgetKind::SafeArea { .. } => WidgetType::SafeArea,
            WidgetKind::ClipRect { .. } => WidgetType::ClipRect,
            WidgetKind::ClipRRect { .. } => WidgetType::ClipRRect,
            WidgetKind::ClipOval { .. } => WidgetType::ClipOval,
            WidgetKind::ClipPath { .. } => WidgetType::ClipPath,
            WidgetKind::LayoutBuilder { .. } => WidgetType::LayoutBuilder,
            WidgetKind::Visibility { .. } => WidgetType::Visibility,
            WidgetKind::AspectRatio { .. } => WidgetType::AspectRatio,
            WidgetKind::Scroll { .. } => WidgetType::Scroll,
            WidgetKind::PersistentHeader { .. } => WidgetType::PersistentHeader,
            WidgetKind::NotificationListener { .. } => WidgetType::NotificationListener,
            WidgetKind::SliverViewport { .. } => WidgetType::SliverViewport,
            WidgetKind::Translate { .. } => WidgetType::Translate,
            WidgetKind::Transform { .. } => WidgetType::Transform,
            WidgetKind::Scale { .. } => WidgetType::Scale,
            WidgetKind::Rotation { .. } => WidgetType::Rotation,
            WidgetKind::FittedBox { .. } => WidgetType::FittedBox,
            WidgetKind::Opacity { .. } => WidgetType::Opacity,
            WidgetKind::Blur { .. } => WidgetType::Blur,
            WidgetKind::DropShadow { .. } => WidgetType::DropShadow,
            WidgetKind::ColorFiltered { .. } => WidgetType::ColorFiltered,
            WidgetKind::Blend { .. } => WidgetType::Blend,
        }
    }
    /// Shallow child view for reconciliation. Deep per-child clones were the
    /// measured allocation fire on wide trees (Task 15); reconciliation only
    /// needs references because cloning happens once per *created* element.
    fn children_refs(&self) -> Vec<&Widget> {
        match &self.kind {
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::CustomPaint { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::SelectableText { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. } => Vec::new(),
            WidgetKind::Button { child, .. } => child.iter().map(|c| c.as_ref()).collect(),
            WidgetKind::Padding { child, .. }
            | WidgetKind::Constrained { child, .. }
            | WidgetKind::Limited { child, .. }
            | WidgetKind::Overflow { child, .. }
            | WidgetKind::Unconstrained { child, .. }
            | WidgetKind::Fractional { child, .. }
            | WidgetKind::Baseline { child, .. }
            | WidgetKind::RepaintBoundary { child, .. }
            | WidgetKind::Gesture { child, .. }
            | WidgetKind::Draggable { child, .. }
            | WidgetKind::DragTarget { child, .. }
            | WidgetKind::IgnorePointer { child, .. }
            | WidgetKind::AbsorbPointer { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Flexible { child, .. }
            | WidgetKind::Positioned { child, .. }
            | WidgetKind::SafeArea { child, .. }
            | WidgetKind::ClipRect { child, .. }
            | WidgetKind::ClipRRect { child, .. }
            | WidgetKind::ClipOval { child, .. }
            | WidgetKind::ClipPath { child, .. }
            | WidgetKind::Visibility { child, .. }
            | WidgetKind::AspectRatio { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::PersistentHeader { child, .. }
            | WidgetKind::NotificationListener { child, .. }
            | WidgetKind::Translate { child, .. }
            | WidgetKind::Transform { child, .. }
            | WidgetKind::Scale { child, .. }
            | WidgetKind::Rotation { child, .. }
            | WidgetKind::FittedBox { child, .. }
            | WidgetKind::Decorated { child, .. }
            | WidgetKind::Opacity { child, .. }
            | WidgetKind::Blur { child, .. }
            | WidgetKind::DropShadow { child, .. }
            | WidgetKind::ColorFiltered { child, .. }
            | WidgetKind::Blend { child, .. } => vec![child.as_ref()],
            WidgetKind::SelectionArea { child, .. } => vec![child.as_ref()],
            WidgetKind::Flex { children, .. }
            | WidgetKind::Wrap { children, .. }
            | WidgetKind::Table { children, .. }
            | WidgetKind::Stack { children, .. }
            | WidgetKind::IndexedStack { children, .. } => children.iter().collect(),
            WidgetKind::SliverViewport { .. } | WidgetKind::LayoutBuilder { .. } => Vec::new(),
        }
    }
}

/// Renderer-neutral image sizing policy (maps to Flutter's `BoxFit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    Fill,
    #[default]
    Contain,
    Cover,
    FitWidth,
    FitHeight,
    None,
    ScaleDown,
}

/// Alias for [`ImageFit`], matching Flutter naming.
pub type BoxFit = ImageFit;

/// Repetition policy for raster image painting inside its allocated bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageRepeat {
    #[default]
    NoRepeat,
    RepeatX,
    RepeatY,
    Repeat,
}
/// A declarative raster image. It retains only a shared asset handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    image: ImageHandle,
    width: Option<f32>,
    height: Option<f32>,
    fit: ImageFit,
    repeat: ImageRepeat,
    alignment: Alignment,
    sampling: ImageSampling,
}
impl Image {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            width: None,
            height: None,
            fit: ImageFit::Contain,
            repeat: ImageRepeat::NoRepeat,
            alignment: Alignment::CENTER,
            sampling: ImageSampling::Linear,
        }
    }
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    #[must_use]
    pub fn repeat(mut self, repeat: ImageRepeat) -> Self {
        self.repeat = repeat;
        self
    }
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
    #[must_use]
    pub fn sampling(mut self, sampling: ImageSampling) -> Self {
        self.sampling = sampling;
        self
    }

    #[must_use]
    pub fn filter_quality(self, quality: FilterQuality) -> Self {
        self.sampling(quality.into())
    }
}
impl From<Image> for Widget {
    fn from(value: Image) -> Self {
        Widget::image(
            value.image,
            value.width,
            value.height,
            value.fit,
            value.repeat,
            value.alignment,
            value.sampling,
        )
    }
}

/// Immutable declarative vector shape. Its coordinates remain local; normal
/// layout and compositor translation position it without changing PathId.
#[derive(Clone, Debug, PartialEq)]
pub struct PathView {
    path: Arc<Path>,
    fill: Option<Brush>,
    stroke: Option<(Brush, Stroke)>,
    size: Option<Size>,
}
impl PathView {
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>) -> Self {
        Self {
            path: path.into(),
            fill: None,
            stroke: None,
            size: None,
        }
    }
    #[must_use]
    pub fn fill(mut self, brush: impl Into<Brush>) -> Self {
        self.fill = Some(brush.into());
        self
    }
    #[must_use]
    pub fn stroke(mut self, brush: impl Into<Brush>, stroke: Stroke) -> Self {
        self.stroke = Some((brush.into(), stroke));
        self
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}
impl From<PathView> for Widget {
    fn from(value: PathView) -> Self {
        Widget::shape(value.path, value.fill, value.stroke, value.size)
    }
}

/// Reusable vector icon backed by the same retained Path renderer as PathView.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    path: Arc<Path>,
    size: f32,
    brush: Brush,
}
impl Icon {
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>) -> Self {
        Self {
            path: path.into(),
            size: 24.,
            brush: Color::WHITE.into(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(0.);
        self
    }
    #[must_use]
    pub fn brush(mut self, brush: impl Into<Brush>) -> Self {
        self.brush = brush.into();
        self
    }
}
impl From<Icon> for Widget {
    fn from(value: Icon) -> Self {
        // Icon paths use a canonical coordinate system (the built-in paths
        // are authored around a 24px viewport).  A PathView's `size` controls
        // layout only, so fit the geometry itself as well; otherwise a 12px
        // check would still paint at coordinates 3..21 and appear offset or
        // clipped inside its indicator.
        let path = value
            .path
            .bounds()
            .filter(|bounds| bounds.size.width > 0. && bounds.size.height > 0.)
            .map_or_else(
                || value.path.clone(),
                |bounds| {
                    let scale =
                        (value.size / bounds.size.width).min(value.size / bounds.size.height);
                    let fitted = Size::new(bounds.size.width * scale, bounds.size.height * scale);
                    let offset = Offset::new(
                        (value.size - fitted.width) * 0.5 - bounds.origin.x * scale,
                        (value.size - fitted.height) * 0.5 - bounds.origin.y * scale,
                    );
                    Arc::new(
                        value.path.transformed(
                            CoreTransform::translation(offset)
                                .then(CoreTransform::scale_non_uniform(scale, scale)),
                        ),
                    )
                },
            );
        PathView::new(path)
            .fill(value.brush)
            .size(Size::new(value.size, value.size))
            .into()
    }
}

/// Small shared demo icons. Each function returns the same immutable `Path`,
/// so repeated icons share PathId and retained tessellation/GPU meshes.
pub mod icons {
    use super::*;
    use std::sync::OnceLock;
    fn path(build: impl FnOnce(&mut incular_painting::PathBuilder)) -> Arc<Path> {
        let mut builder = Path::builder();
        build(&mut builder);
        Arc::new(builder.build())
    }
    #[must_use]
    pub fn check() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 12.))
                    .line_to(Offset::new(9., 18.))
                    .line_to(Offset::new(21., 4.))
                    .line_to(Offset::new(18., 2.))
                    .line_to(Offset::new(9., 14.))
                    .line_to(Offset::new(5., 10.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn close() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 5.))
                    .line_to(Offset::new(5., 3.))
                    .line_to(Offset::new(12., 10.))
                    .line_to(Offset::new(19., 3.))
                    .line_to(Offset::new(21., 5.))
                    .line_to(Offset::new(14., 12.))
                    .line_to(Offset::new(21., 19.))
                    .line_to(Offset::new(19., 21.))
                    .line_to(Offset::new(12., 14.))
                    .line_to(Offset::new(5., 21.))
                    .line_to(Offset::new(3., 19.))
                    .line_to(Offset::new(10., 12.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn plus() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(10., 3.))
                    .line_to(Offset::new(14., 3.))
                    .line_to(Offset::new(14., 10.))
                    .line_to(Offset::new(21., 10.))
                    .line_to(Offset::new(21., 14.))
                    .line_to(Offset::new(14., 14.))
                    .line_to(Offset::new(14., 21.))
                    .line_to(Offset::new(10., 21.))
                    .line_to(Offset::new(10., 14.))
                    .line_to(Offset::new(3., 14.))
                    .line_to(Offset::new(3., 10.))
                    .line_to(Offset::new(10., 10.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn minus() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 10.))
                    .line_to(Offset::new(21., 10.))
                    .line_to(Offset::new(21., 14.))
                    .line_to(Offset::new(3., 14.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn chevron_right() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(7., 3.))
                    .line_to(Offset::new(10., 0.))
                    .line_to(Offset::new(22., 12.))
                    .line_to(Offset::new(10., 24.))
                    .line_to(Offset::new(7., 21.))
                    .line_to(Offset::new(16., 12.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact downward chevron used by select, disclosure, and menu
    /// controls.  Keeping each direction as a shared immutable path avoids
    /// per-control path construction and makes the icon independent of font
    /// fallback or glyph metrics.
    #[must_use]
    pub fn chevron_down() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 7.))
                    .line_to(Offset::new(6., 4.))
                    .line_to(Offset::new(12., 10.))
                    .line_to(Offset::new(18., 4.))
                    .line_to(Offset::new(21., 7.))
                    .line_to(Offset::new(12., 16.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact upward chevron.
    #[must_use]
    pub fn chevron_up() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 17.))
                    .line_to(Offset::new(6., 20.))
                    .line_to(Offset::new(12., 14.))
                    .line_to(Offset::new(18., 20.))
                    .line_to(Offset::new(21., 17.))
                    .line_to(Offset::new(12., 8.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact left-pointing chevron.
    #[must_use]
    pub fn chevron_left() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(17., 3.))
                    .line_to(Offset::new(20., 6.))
                    .line_to(Offset::new(14., 12.))
                    .line_to(Offset::new(20., 18.))
                    .line_to(Offset::new(17., 21.))
                    .line_to(Offset::new(8., 12.))
                    .close();
            })
        })
        .clone()
    }
}

/// A small background/border wrapper. It intentionally does not clip children;
/// true rounded clipping belongs to Phase 9.1C.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoratedBox {
    child: Widget,
    size: Option<Size>,
    background: Option<Brush>,
    border: Option<Border>,
    radius: CornerRadii,
}
impl DecoratedBox {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            size: None,
            background: None,
            border: None,
            radius: CornerRadii::default(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
    #[must_use]
    pub fn background(mut self, brush: impl Into<Brush>) -> Self {
        self.background = Some(brush.into());
        self
    }
    #[must_use]
    pub fn border(mut self, border: impl Into<Border>) -> Self {
        self.border = Some(border.into());
        self
    }
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = CornerRadii::uniform(radius);
        self
    }
    #[must_use]
    pub fn decoration(mut self, decoration: crate::BoxDecoration) -> Self {
        if let Some(color) = decoration.color {
            self.background = Some(Brush::Solid(color));
        }
        if let Some(border) = decoration.border {
            self.border = Some(incular_rendering::Border::new(
                border.top.width,
                border.top.color,
            ));
        }
        if let Some(radius) = decoration.border_radius {
            self.radius = radius.to_corner_radii();
        }
        self
    }
}
impl From<DecoratedBox> for Widget {
    fn from(value: DecoratedBox) -> Self {
        Widget::decorated(
            value.size,
            value.background,
            value.border,
            value.radius,
            value.child,
        )
    }
}

/// Public text description. Its font metrics are resolved during layout, not
/// paint.  A plain [`Text::new`] uses [`TextStyle::default`] (16 logical px,
/// system UI family, normal weight) and reports its natural shaped size; it
/// does not expand to the parent width unless a parent layout allocates that
/// width explicitly.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    text: String,
    style: TextStyle,
    align: TextAlign,
    soft_wrap: bool,
    max_lines: Option<usize>,
    overflow: TextOverflow,
}
impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            align: TextAlign::Start,
            soft_wrap: true,
            max_lines: None,
            overflow: TextOverflow::Clip,
        }
    }
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }
    #[must_use]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
    /// Enables or disables Parley's soft line breaking at the allocated width.
    #[must_use]
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }
    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }
    #[must_use]
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}
impl From<Text> for Widget {
    fn from(value: Text) -> Self {
        Widget::text_configured(
            value.text,
            value.style,
            value.align,
            value.soft_wrap,
            value.max_lines,
            value.overflow,
        )
    }
}

/// Makes the renderer-independent rich paragraph description available in a
/// retained widget tree. Paragraph policy is preserved; inline style flattening
/// follows the existing `incular-text::RichText` contract.
impl From<RichText> for Widget {
    fn from(value: RichText) -> Self {
        let style = value
            .flatten()
            .first()
            .map_or_else(TextStyle::default, |run| run.style.clone());
        Widget::text_configured(
            value.plain_text(),
            style,
            value.text_align,
            value.soft_wrap,
            value.max_lines,
            value.overflow,
        )
    }
}

/// Core editable text widget.
///
/// This is Incular's renderer-neutral counterpart to Flutter's
/// `widgets/EditableText`.  Material `TextField` and multiline field chrome
/// belong to a higher-level component library. Keep the controller outside a
/// rebuilt widget description so text, selection, and active composition
/// survive rebuilds.
pub struct EditableText {
    controller: TextEditingController,
    size: Size,
    style: TextStyle,
    placeholder: String,
    on_submit: Option<Rc<dyn Fn(String)>>,
    multiline: bool,
    min_lines: Option<usize>,
    max_lines: Option<usize>,
    expands: bool,
    text_align: TextAlign,
    enabled: bool,
    read_only: bool,
    obscure_text: bool,
    cursor_width: f32,
    cursor_height: Option<f32>,
    cursor_radius: f32,
    show_cursor: bool,
    cursor_color: Color,
    selection_color: Color,
    input_type: TextInputTypeHint,
    input_action: TextInputActionHint,
}
impl EditableText {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::ZERO,
            style: TextStyle::default(),
            placeholder: String::new(),
            on_submit: None,
            multiline: false,
            min_lines: None,
            max_lines: Some(1),
            expands: false,
            text_align: TextAlign::Start,
            enabled: true,
            read_only: false,
            obscure_text: false,
            cursor_width: 1.0,
            cursor_height: None,
            cursor_radius: 0.0,
            show_cursor: true,
            cursor_color: Color::WHITE,
            selection_color: Color::rgba(72, 120, 220, 150),
            input_type: TextInputTypeHint::Text,
            input_action: TextInputActionHint::Unspecified,
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    #[must_use]
    pub fn on_submit(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(callback));
        self
    }
    /// Enables multiline editing. Material text-field wrappers should use
    /// this core primitive rather than introducing a separate `TextArea`
    /// widget name.
    #[must_use]
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        if multiline && self.max_lines == Some(1) {
            self.max_lines = None;
        } else if !multiline {
            self.max_lines = Some(1);
        }
        self
    }
    /// Sets an explicit multiline height while preserving the current width.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.size = Size::new(self.size.width, height);
        self
    }

    #[must_use]
    pub fn min_lines(mut self, lines: Option<usize>) -> Self {
        self.min_lines = lines.map(|value| value.max(1));
        self
    }

    #[must_use]
    pub fn max_lines(mut self, lines: Option<usize>) -> Self {
        self.max_lines = lines.map(|value| value.max(1));
        self.multiline = !matches!(self.max_lines, Some(1));
        self
    }

    #[must_use]
    pub fn expands(mut self, expands: bool) -> Self {
        self.expands = expands;
        self
    }

    #[must_use]
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.text_align = align;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn obscure_text(mut self, obscure_text: bool) -> Self {
        self.obscure_text = obscure_text;
        self
    }

    /// Sets the caret width in logical pixels.
    #[must_use]
    pub fn cursor_width(mut self, width: f32) -> Self {
        self.cursor_width = width.max(0.0);
        self
    }

    /// Sets an optional caret height. `None` uses the shaped line height.
    #[must_use]
    pub fn cursor_height(mut self, height: Option<f32>) -> Self {
        self.cursor_height = height.map(|value| value.max(0.0));
        self
    }

    /// Sets the caret corner radius. A zero radius keeps the caret rectangular.
    #[must_use]
    pub fn cursor_radius(mut self, radius: f32) -> Self {
        self.cursor_radius = radius.max(0.0);
        self
    }

    /// Controls whether the caret is painted while this editor is focused.
    #[must_use]
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    /// Sets the caret paint color.
    #[must_use]
    pub fn cursor_color(mut self, color: Color) -> Self {
        self.cursor_color = color;
        self
    }

    /// Sets the selection highlight paint color.
    #[must_use]
    pub fn selection_color(mut self, color: Color) -> Self {
        self.selection_color = color;
        self
    }

    /// Selects the native keyboard/input method hint for this editor. The
    /// runtime still validates and owns the committed text value.
    #[must_use]
    pub fn input_type(mut self, input_type: TextInputTypeHint) -> Self {
        self.input_type = input_type;
        self
    }

    /// Selects the action shown by a native software keyboard.
    #[must_use]
    pub fn input_action(mut self, input_action: TextInputActionHint) -> Self {
        self.input_action = input_action;
        self
    }
}
impl From<EditableText> for Widget {
    fn from(value: EditableText) -> Self {
        Widget::editable_text_configured_with_cursor(
            value.controller,
            value.size,
            value.style,
            value.placeholder,
            value.on_submit,
            value.multiline,
            value.min_lines,
            value.max_lines,
            value.expands,
            value.text_align,
            value.enabled,
            value.read_only,
            value.obscure_text,
            value.cursor_width,
            value.cursor_height,
            value.cursor_radius,
            value.show_cursor,
            value.cursor_color,
            value.selection_color,
        )
        .with_text_input_hints(value.input_type, value.input_action)
    }
}

/// Declarative retained group opacity. The child is painted into an isolated
/// compositor target when alpha is between zero and one, so overlapping
/// descendants are attenuated exactly once.
#[derive(Clone, Debug, PartialEq)]
pub struct Opacity {
    alpha: f32,
    controller: Option<OpacityController>,
    child: Widget,
}
impl Opacity {
    #[must_use]
    pub fn new(alpha: f32, child: impl Into<Widget>) -> Self {
        Self {
            alpha: normalize_opacity(alpha),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: OpacityController, child: impl Into<Widget>) -> Self {
        Self {
            alpha: controller.opacity(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controller(mut self, controller: OpacityController) -> Self {
        self.alpha = controller.opacity();
        self.controller = Some(controller);
        self
    }
    #[must_use]
    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = normalize_opacity(alpha);
        self
    }
}
impl From<Opacity> for Widget {
    fn from(value: Opacity) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_opacity(controller, value.child),
            None => Widget::opacity(value.alpha, value.child),
        }
    }
}

/// A retained opacity transition driven directly by an [`OpacityController`].
#[derive(Clone, Debug, PartialEq)]
pub struct FadeTransition {
    controller: OpacityController,
    child: Widget,
}
impl FadeTransition {
    #[must_use]
    pub fn new(controller: OpacityController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<FadeTransition> for Widget {
    fn from(value: FadeTransition) -> Self {
        Widget::controlled_opacity(value.controller, value.child)
    }
}

/// A retained translation transition driven directly by a
/// [`TranslationController`].
#[derive(Clone, Debug, PartialEq)]
pub struct SlideTransition {
    controller: TranslationController,
    child: Widget,
}
impl SlideTransition {
    #[must_use]
    pub fn new(controller: TranslationController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<SlideTransition> for Widget {
    fn from(value: SlideTransition) -> Self {
        Widget::translate(value.controller, value.child)
    }
}

/// An arbitrary retained affine transform. Layout remains the child's normal
/// layout; only the compositor, pointer coordinate conversion, and semantic
/// bounds observe the affine transform.
#[derive(Clone, Debug, PartialEq)]
pub struct Transform {
    transform: CoreTransform,
    origin: Option<Offset>,
    child: Widget,
}
impl Transform {
    #[must_use]
    pub fn new(transform: CoreTransform, child: impl Into<Widget>) -> Self {
        Self {
            transform,
            origin: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn translation(offset: Offset, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::translation(offset), child)
    }
    #[must_use]
    pub fn scale(scale: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::scale(scale), child)
    }
    #[must_use]
    pub fn rotation(radians: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::rotation(radians), child)
    }
    #[must_use]
    pub fn skew(x: f32, y: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::skew(x, y), child)
    }
    /// Selects the local pivot. The default is the child's center.
    #[must_use]
    pub fn origin(mut self, origin: Offset) -> Self {
        self.origin = Some(finite_offset(origin));
        self
    }
}
impl From<Transform> for Widget {
    fn from(value: Transform) -> Self {
        match value.origin {
            Some(origin) => Widget::transform_around(value.transform, origin, value.child),
            None => Widget::transform(value.transform, value.child),
        }
    }
}

/// A compositor-only scale transition driven by [`ScaleController`].
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleTransition {
    controller: ScaleController,
    child: Widget,
}
impl ScaleTransition {
    #[must_use]
    pub fn new(controller: ScaleController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<ScaleTransition> for Widget {
    fn from(value: ScaleTransition) -> Self {
        Widget::controlled_scale(value.controller, value.child)
    }
}

/// A compositor-only clockwise rotation transition driven by
/// [`RotationController`].
#[derive(Clone, Debug, PartialEq)]
pub struct RotationTransition {
    controller: RotationController,
    alignment: Alignment,
    child: Widget,
}
impl RotationTransition {
    #[must_use]
    pub fn new(controller: RotationController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    /// Creates a rotation from a normalized turns value without exposing the
    /// retained controller implementation. One turn is a full revolution.
    #[must_use]
    pub fn from_turns(turns: f32, child: impl Into<Widget>) -> Self {
        let controller = RotationController::new();
        controller.set_radians(if turns.is_finite() {
            turns * std::f32::consts::TAU
        } else {
            0.
        });
        Self::new(controller, child)
    }

    /// Selects the normalized pivot for the compositor transform. The
    /// default is the child's center, matching Flutter's transition.
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}
impl From<RotationTransition> for Widget {
    fn from(value: RotationTransition) -> Self {
        // Resolve alignment against the retained child size at compositor
        // update time; a turns change therefore does not relayout or repaint
        // the child subtree.
        Widget::controlled_rotation_with_alignment(
            value.controller,
            Some(value.alignment),
            value.child,
        )
    }
}

/// A compact composition surface for the retained transition primitives.
/// It intentionally merges fade and slide behavior instead of creating a
/// large hierarchy of narrowly different animation widget types.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    opacity: Option<OpacityController>,
    translation: Option<TranslationController>,
    child: Widget,
}
impl Transition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            opacity: None,
            translation: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn fade(mut self, controller: OpacityController) -> Self {
        self.opacity = Some(controller);
        self
    }
    #[must_use]
    pub fn slide(mut self, controller: TranslationController) -> Self {
        self.translation = Some(controller);
        self
    }
}
impl From<Transition> for Widget {
    fn from(value: Transition) -> Self {
        let child = match value.translation {
            Some(controller) => Widget::translate(controller, value.child),
            None => value.child,
        };
        match value.opacity {
            Some(controller) => Widget::controlled_opacity(controller, child),
            None => child,
        }
    }
}

/// Declarative Gaussian blur isolation. Sigma is in logical pixels and is
/// converted to physical pixels by the renderer at the current DPI.
#[derive(Clone, Debug, PartialEq)]
pub struct Blur {
    sigma_x: f32,
    sigma_y: f32,
    controller: Option<BlurController>,
    child: Widget,
}
impl Blur {
    #[must_use]
    pub fn new(sigma: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn asymmetric(sigma_x: f32, sigma_y: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: BlurController, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: controller.sigma(),
            sigma_y: controller.sigma(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn sigma_x(mut self, sigma: f32) -> Self {
        self.sigma_x = normalize_sigma(sigma);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn sigma_y(mut self, sigma: f32) -> Self {
        self.sigma_y = normalize_sigma(sigma);
        self.controller = None;
        self
    }
}
impl From<Blur> for Widget {
    fn from(value: Blur) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_blur(controller, value.child),
            None => Widget::asymmetric_blur(value.sigma_x, value.sigma_y, value.child),
        }
    }
}

/// Declarative arbitrary-subtree drop shadow. The source subtree is isolated
/// and its alpha is blurred, so text, images, gradients, and paths all share
/// the same shadow semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct DropShadow {
    offset: Offset,
    sigma_x: f32,
    sigma_y: f32,
    color: Color,
    controller: Option<DropShadowController>,
    child: Widget,
}
impl DropShadow {
    #[must_use]
    pub fn new(offset: Offset, sigma: f32, color: Color, child: impl Into<Widget>) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            color,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn asymmetric(
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            color,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: DropShadowController, child: impl Into<Widget>) -> Self {
        Self {
            offset: controller.offset(),
            sigma_x: controller.sigma(),
            sigma_y: controller.sigma(),
            color: controller.color(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn offset(mut self, offset: Offset) -> Self {
        self.offset = finite_offset(offset);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn sigma(mut self, sigma: f32) -> Self {
        self.sigma_x = normalize_sigma(sigma);
        self.sigma_y = normalize_sigma(sigma);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self.controller = None;
        self
    }
}
impl From<DropShadow> for Widget {
    fn from(value: DropShadow) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_drop_shadow(controller, value.child),
            None => Widget::drop_shadow(value.offset, value.sigma_x, value.color, value.child),
        }
    }
}

/// Declarative 4x5 color-matrix stage. The matrix is evaluated in straight
/// RGBA and converted back to the retained premultiplied texture format.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorFiltered {
    filter: ColorFilter,
    controller: Option<ColorFilterController>,
    child: Widget,
}
impl ColorFiltered {
    #[must_use]
    pub fn new(filter: ColorFilter, child: impl Into<Widget>) -> Self {
        Self {
            filter,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: ColorFilterController, child: impl Into<Widget>) -> Self {
        Self {
            filter: controller.filter(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn matrix(mut self, matrix: [f32; 20]) -> Self {
        self.filter = ColorFilter::matrix(matrix);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn filter(mut self, filter: ColorFilter) -> Self {
        self.filter = filter;
        self.controller = None;
        self
    }
    #[must_use]
    pub fn controller(mut self, controller: ColorFilterController) -> Self {
        self.filter = controller.filter();
        self.controller = Some(controller);
        self
    }
}
impl From<ColorFiltered> for Widget {
    fn from(value: ColorFiltered) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_color_filtered(controller, value.child),
            None => Widget::color_filtered(value.filter, value.child),
        }
    }
}

/// Declarative retained blend group. Destination-dependent modes promote only
/// the required composition scope to a sampleable intermediate target.
#[derive(Clone, Debug, PartialEq)]
pub struct Blend {
    mode: BlendMode,
    child: Widget,
}
impl Blend {
    #[must_use]
    pub fn new(mode: BlendMode, child: impl Into<Widget>) -> Self {
        Self {
            mode,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn mode(mut self, mode: BlendMode) -> Self {
        self.mode = mode;
        self
    }
}
impl From<Blend> for Widget {
    fn from(value: Blend) -> Self {
        Widget::blend(value.mode, value.child)
    }
}

/// Small composable builder for ordered retained effects. Each method wraps
/// the current child, so calls read in the same order as execution.
#[derive(Clone, Debug, PartialEq)]
pub struct Effects {
    child: Widget,
}
impl Effects {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
    #[must_use]
    pub fn color_filter(mut self, filter: ColorFilter) -> Self {
        self.child = Widget::color_filtered(filter, self.child);
        self
    }
    #[must_use]
    pub fn color_matrix(self, filter: ColorFilter) -> Self {
        self.color_filter(filter)
    }
    #[must_use]
    pub fn blur(mut self, sigma: f32) -> Self {
        self.child = Widget::blur(sigma, self.child);
        self
    }
    #[must_use]
    pub fn asymmetric_blur(mut self, sigma_x: f32, sigma_y: f32) -> Self {
        self.child = Widget::asymmetric_blur(sigma_x, sigma_y, self.child);
        self
    }
    #[must_use]
    pub fn opacity(mut self, alpha: f32) -> Self {
        self.child = Widget::opacity(alpha, self.child);
        self
    }
    #[must_use]
    pub fn drop_shadow(mut self, offset: Offset, sigma: f32, color: Color) -> Self {
        self.child = Widget::drop_shadow(offset, sigma, color, self.child);
        self
    }
    #[must_use]
    pub fn blend(mut self, mode: BlendMode) -> Self {
        self.child = Widget::blend(mode, self.child);
        self
    }
    #[must_use]
    pub fn build(self) -> Widget {
        self.child
    }
}
impl From<Effects> for Widget {
    fn from(value: Effects) -> Self {
        value.build()
    }
}

/// Vertical retained viewport. Keep a [`ScrollController`] outside a rebuild
/// when application code needs the position to survive a recreated description.
pub struct ScrollView;
impl ScrollView {
    #[must_use]
    pub fn vertical(controller: ScrollController, child: impl Into<Widget>) -> Widget {
        Widget::scroll_view(controller, child.into())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub mounts: u64,
    pub unmounts: u64,
    pub rebuilds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub composites: u64,
    pub scroll_events: u64,
    pub scroll_offset_updates: u64,
    pub animation_ticks: u64,
    pub lazy_layouts: u64,
    pub items_built: u64,
    pub items_mounted: u64,
    pub items_unmounted: u64,
    pub items_reused: u64,
    /// Reconciliation prefix/suffix fast-path hits (children matched in place
    /// without entering the keyed middle-diff).
    pub reconciliation_fast_paths: u64,
    // Task 15 structural breakdown. All cheap monotonic counters.
    /// `reconcile_children` invocations that reached scanning (parent changed).
    pub child_list_scans: u64,
    /// Child updates skipped because the retained element already equaled the
    /// desired widget (the unchanged-subtree bailout).
    pub identical_child_bailouts: u64,
    /// Widget type comparisons performed during compatibility checks.
    pub widget_type_comparisons: u64,
    /// Key comparisons performed during compatibility checks and lookups.
    pub key_comparisons: u64,
    /// Full widget configuration equality checks (deep ==).
    pub config_comparisons: u64,
    /// Old-key maps built for keyed middle ranges.
    pub key_maps_built: u64,
    /// Entries inserted into those key maps.
    pub key_map_entries: u64,
    /// Keyed lookups against those maps.
    pub key_lookups: u64,
    /// New elements mounted during reconciliation.
    pub elements_created: u64,
    /// Existing elements reused (compatible match) during reconciliation.
    pub elements_reused: u64,
    /// Elements unmounted during reconciliation.
    pub elements_removed: u64,
    /// Reused elements whose position changed relative to the previous list.
    pub elements_moved: u64,
    /// Every invalidation request (before deduplication).
    pub dirty_requests: u64,
    /// Requests that transitioned a retained object into a dirty state.
    pub dirty_queue_insertions: u64,
    /// Requests coalesced because the target was already dirty.
    pub dirty_queue_deduplicated: u64,
    /// LAYOUT passes skipped because constraints were unchanged.
    pub layout_cache_hits: u64,
    /// Retained per-render-object display lists replayed without recording.
    pub display_lists_reused: u64,
    /// Compositor-only mutations (transform/opacity/effect) that skipped
    /// BUILD/LAYOUT/PAINT entirely.
    pub compositor_only_updates: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum TreeError {
    MissingElement(ElementId),
    DuplicateKey(Key),
    /// No live window record matched the requested window identity.
    WindowUnknown,
}

pub struct Element {
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub widget: Widget,
    pub render: RenderObjectId,
    pub dirty: DirtyFlags,
    /// Parallel to `children` for a sliver viewport. IDs are viewport-scoped
    /// and remain stable while the cache window moves.
    sliver_child_ids: Vec<SliverChildId>,
    /// Pinned children are painted above normal flow children while logical
    /// accessibility order remains unchanged.
    sliver_pinned_ids: HashSet<SliverChildId>,
    /// RAII subscriptions for a NotificationListener. They are rebuilt after
    /// layout so lazily materialized sliver viewports are included without
    /// keeping dead controller listeners alive.
    notification_subscriptions: Vec<ScrollNotificationSubscription>,
    sliver_delegate_revision: u64,
    sliver_scroll_revision: u64,
    layout_builder_constraints: Option<Constraints>,
    layout_builder_revision: u64,
    /// Effective inherited retained builder environment for this element.
    environment: Option<Rc<dyn Any>>,
    /// Environment supplied directly by this element, if any. This lets a
    /// nested scope shadow its parent while descendants continue inheriting.
    environment_override: Option<Rc<dyn Any>>,
    /// DevTools-only instrumentation. Zero cost in production builds.
    #[cfg(feature = "devtools")]
    pub dev: ElementDevData,
}

/// Per-element counters and the latest invalidation cause, captured only
/// while the `devtools` feature is enabled.
#[cfg(feature = "devtools")]
#[derive(Default, Clone, Debug)]
pub struct ElementDevData {
    pub builds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub composites: u64,
    pub semantic_updates: u64,
    /// Monotonic per-node content revision for incremental tree deltas.
    pub revision: u64,
    pub last_cause: Option<InvalidationCause>,
    /// Small ordered cause set coalesced until the next observed build. This
    /// avoids inventing a single winner when several real invalidations land
    /// before a frame.
    pub invalidation_causes: Vec<InvalidationCause>,
    /// Structured, bounded configuration change set for Why Did This Rebuild.
    pub property_changes: Vec<incular_devtools_protocol::PropertyChange>,
    pub layout_history: Vec<LayoutHistoryRecord>,
    pub layout_reason: Option<String>,
    pub paint_reason: Option<String>,
    pub composite_reason: Option<String>,
}

#[cfg(feature = "devtools")]
#[derive(Clone, Debug)]
pub struct LayoutHistoryRecord {
    pub sequence: u64,
    pub old_constraints: Option<Constraints>,
    pub new_constraints: Constraints,
    pub old_size: Size,
    pub new_size: Size,
    pub cause: Option<String>,
}

/// Why an element last entered BUILD. Recorded by the runtime at the exact
/// invalidation sites; retained bounded summaries only.
#[cfg(feature = "devtools")]
#[derive(Clone, Debug)]
pub enum InvalidationCause {
    Signal {
        id: u64,
        name: Option<String>,
        old: Option<String>,
        new: Option<String>,
    },
    ParentReconciliation,
    WidgetConfigurationChanged,
    EnvironmentChanged,
    LocaleChanged,
    WindowMetricsChanged,
    ConstraintsChanged,
    Animation,
    TaskCompletion,
    Navigation,
    Restoration,
    Manual,
    Mounted,
}

#[cfg(feature = "devtools")]
impl InvalidationCause {
    pub fn summary(&self) -> String {
        match self {
            Self::Signal { name, .. } => {
                format!("Signal {}", name.as_deref().unwrap_or("<unnamed>"))
            }
            Self::ParentReconciliation => "parent reconciliation".into(),
            Self::WidgetConfigurationChanged => "widget changed".into(),
            Self::EnvironmentChanged => "environment".into(),
            Self::LocaleChanged => "locale".into(),
            Self::WindowMetricsChanged => "window metrics".into(),
            Self::ConstraintsChanged => "constraints".into(),
            Self::Animation => "animation".into(),
            Self::TaskCompletion => "task completion".into(),
            Self::Navigation => "navigation".into(),
            Self::Restoration => "restoration".into(),
            Self::Manual => "manual invalidation".into(),
            Self::Mounted => "mounted".into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum RenderKind {
    Box {
        desired: Size,
        color: Color,
    },
    Shape {
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        desired: Size,
    },
    CustomPaint {
        desired: Size,
        display_list: DisplayList,
    },
    Decorated {
        desired: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
    },
    Button {
        desired: Size,
        color: Color,
        hover_color: Option<Color>,
        pressed_color: Option<Color>,
        focused_color: Option<Color>,
        disabled_color: Option<Color>,
        enabled: bool,
        focusable_when_disabled: bool,
    },
    Padding {
        padding: EdgeInsets,
    },
    Constrained {
        constraints: Constraints,
    },
    Limited {
        max_width: f32,
        max_height: f32,
    },
    Overflow {
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
    },
    Unconstrained {
        constrained_axis: Option<Axis>,
    },
    Fractional {
        width_factor: Option<f32>,
        height_factor: Option<f32>,
    },
    Baseline {
        baseline: f32,
    },
    RepaintBoundary,
    Gesture,
    Align {
        alignment: Alignment,
        width_factor: Option<f32>,
        height_factor: Option<f32>,
    },
    Flex {
        flex: incular_layout::Flex,
    },
    Flexible {
        flex: u32,
        fit: incular_config::FlexFit,
    },
    Wrap {
        wrap: incular_layout::Wrap,
    },
    Table {
        columns: usize,
        column_spacing: f32,
        row_spacing: f32,
    },
    Stack {
        stack: incular_layout::Stack,
    },
    Positioned {
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
    },
    IndexedStack {
        alignment: Alignment,
        index: usize,
    },
    SafeArea {
        minimum: EdgeInsets,
        left: bool,
        top: bool,
        right: bool,
        bottom: bool,
        maintain_bottom_view_padding: bool,
    },
    ClipRect {
        clip_behavior: Clip,
    },
    ClipRRect {
        radius: CornerRadii,
        clip_behavior: Clip,
    },
    ClipOval {
        clip_behavior: Clip,
    },
    ClipPath {
        path: Arc<Path>,
        clip_behavior: Clip,
    },
    LayoutBuilder,
    Visibility {
        visible: bool,
    },
    AspectRatio {
        ratio: f32,
    },
    Text {
        text: String,
        style: TextStyle,
        align: TextAlign,
        soft_wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    },
    SelectableText {
        text: String,
        style: TextStyle,
        align: TextAlign,
    },
    SelectionArea,
    Image {
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        repeat: ImageRepeat,
        alignment: Alignment,
        sampling: ImageSampling,
    },
    TextField {
        controller: TextEditingController,
        desired: Size,
        style: TextStyle,
        placeholder: String,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
        cursor_width: f32,
        cursor_height: Option<f32>,
        cursor_radius: f32,
        show_cursor: bool,
        cursor_color: Color,
        selection_color: Color,
    },
    Scroll {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
    },
    PersistentHeader {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    },
    SliverViewport {
        config: Rc<SliverViewportConfig>,
    },
    Translate {
        controller: TranslationController,
    },
    Transform {
        transform: CoreTransform,
        origin: Option<Offset>,
    },
    Scale {
        controller: ScaleController,
        origin: Option<Offset>,
    },
    Rotation {
        controller: RotationController,
        origin: Option<Offset>,
        alignment: Option<Alignment>,
    },
    FittedBox {
        fit: ImageFit,
        alignment: Alignment,
    },
    Opacity {
        alpha: f32,
        controller: Option<OpacityController>,
    },
    Blur {
        sigma_x: f32,
        sigma_y: f32,
        controller: Option<BlurController>,
    },
    DropShadow {
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        controller: Option<DropShadowController>,
    },
    ColorFiltered {
        filter: ColorFilter,
        controller: Option<ColorFilterController>,
    },
    Blend {
        mode: BlendMode,
    },
}

/// Snapshot of one sliver viewport. Semantic integration can expose
/// `logical_item_count` plus these materialized item indices without creating
/// one semantic node per logical item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SliverViewportDiagnostics {
    pub logical_item_count: usize,
    pub materialized_range: std::ops::Range<usize>,
    pub materialized_item_count: usize,
    pub scroll_offset: f32,
    pub viewport_extent: f32,
    pub cache_extent: f32,
    pub element_count: usize,
    pub render_object_count: usize,
    pub picture_layer_count: usize,
}
pub struct RenderObject {
    pub parent: Option<RenderObjectId>,
    pub children: Vec<RenderObjectId>,
    kind: RenderKind,
    pub size: Size,
    pub offset: Offset,
    pub constraints: Option<Constraints>,
    dirty: DirtyFlags,
    cache: DisplayList,
    text_layout: Option<Arc<TextLayout>>,
    text_revision: u64,
    text_visual_revision: u64,
    text_scroll_x: f32,
    text_scroll_y: f32,
    scrollbar_hovered: bool,
    scrollbar_dragging: bool,
    focused: bool,
    pub(crate) baseline: Option<f32>,
    button_state: ButtonState,
    button_hovered: bool,
    button_pressed: bool,
    button_focused: bool,
    /// Static parent-relative layout placement. This is never used to store a
    /// scroll or animation displacement.
    layer: LayerId,
    picture: Option<LayerId>,
    /// Optional post-child picture used for shape-aware focus outlines on
    /// transparent compound-control hit surfaces.
    focus_picture: Option<LayerId>,
    pub(crate) clip_layer: Option<LayerId>,
    content_layer: Option<LayerId>,
    opacity_layer: Option<LayerId>,
    blur_layer: Option<LayerId>,
    shadow_layer: Option<LayerId>,
    color_filter_layer: Option<LayerId>,
    blend_layer: Option<LayerId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScrollbarDrag {
    render: RenderObjectId,
    /// Pointer position relative to the rendered thumb's top, fixed for this
    /// captured gesture. Keeping this anchor avoids snapping on a thumb press.
    grab_offset: f32,
}

/// Snapshot of the active thumb drag for deterministic diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarDragDiagnostics {
    pub active: bool,
    pub grab_offset: f32,
    pub track_extent: f32,
    pub thumb_extent: f32,
    pub thumb_travel: f32,
    pub thumb_top: f32,
    pub scroll_offset: f32,
}
struct SemanticBuild {
    element: ElementId,
    parent: Option<ElementId>,
    role: SemanticRole,
    label: Option<String>,
    value: Option<String>,
    description: Option<String>,
    bounds: Rect,
    state: SemanticState,
    actions: Vec<SemanticActionKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RetainedGestureKind {
    Pointer,
    Scale,
}

struct ActiveGestureMember {
    element: ElementId,
    member: GestureArenaMember,
    kind: RetainedGestureKind,
    recognizer: Option<PointerGestureRecognizer>,
    on_cancel: Option<Rc<dyn Fn()>>,
}

struct ActiveGesture {
    element: ElementId,
    members: Vec<ActiveGestureMember>,
    cancelled_elements: HashSet<ElementId>,
    start: Offset,
}

struct ActiveDrag {
    source: Rc<dyn RetainedDragSource>,
    target: Option<(ElementId, Rc<dyn RetainedDragTarget>)>,
    start: Offset,
}

fn disposition_for(
    entries: &[GestureArenaEntry],
    member: GestureArenaMember,
) -> GestureDisposition {
    entries
        .iter()
        .find(|entry| entry.member == member)
        .map_or(GestureDisposition::Cancelled, |entry| entry.disposition)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StaticSelectionPoint {
    element: ElementId,
    byte: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StaticSelection {
    area: ElementId,
    anchor: StaticSelectionPoint,
    extent: StaticSelectionPoint,
}

#[cfg(feature = "devtools")]
struct DeepTraceCapture {
    started: Instant,
    events: Vec<TraceEvent>,
    event_starts: Vec<Instant>,
    stack: Vec<u32>,
    max_events: usize,
    dropped_events: u32,
}

#[cfg(feature = "devtools")]
impl DeepTraceCapture {
    fn new(max_events: usize) -> Self {
        Self {
            started: Instant::now(),
            events: Vec::with_capacity(max_events.min(4_096)),
            event_starts: Vec::with_capacity(max_events.min(4_096)),
            stack: Vec::new(),
            max_events,
            dropped_events: 0,
        }
    }

    fn elapsed_us(duration: Duration) -> u32 {
        u32::try_from(duration.as_micros()).unwrap_or(u32::MAX)
    }

    fn begin(&mut self, node: DevWidgetId, phase: TracePhase) -> Option<u32> {
        if self.events.len() >= self.max_events {
            self.dropped_events = self.dropped_events.saturating_add(1);
            return None;
        }
        let now = Instant::now();
        let index = u32::try_from(self.events.len()).ok()?;
        self.events.push(TraceEvent {
            node,
            phase,
            parent: self.stack.last().copied(),
            start_us: Self::elapsed_us(now.saturating_duration_since(self.started)),
            duration_us: 0,
        });
        self.event_starts.push(now);
        self.stack.push(index);
        Some(index)
    }

    fn end(&mut self, token: Option<u32>) {
        let Some(token) = token else { return };
        let Some(event) = self.events.get_mut(token as usize) else {
            return;
        };
        if let Some(started) = self.event_starts.get(token as usize) {
            event.duration_us = Self::elapsed_us(started.elapsed());
        }
        if self.stack.last() == Some(&token) {
            self.stack.pop();
        } else if let Some(position) = self.stack.iter().rposition(|entry| *entry == token) {
            self.stack.remove(position);
        }
    }
}

/// Persistent UI state. IDs become invalid immediately after unmount.
pub struct WidgetTree {
    elements: Arena<Element>,
    renders: Arena<RenderObject>,
    root: Option<ElementId>,
    diagnostics: Diagnostics,
    unmounted: Vec<ElementId>,
    text_engine: TextEngine,
    compositor: LayerTree,
    compositor_initialized: bool,
    /// Maps the platform's monotonic frame clock onto the animation-only
    /// clock. Tokio, input, profiler timing and all other runtime clocks keep
    /// using real time.
    animation_time_scale: f32,
    animation_clock: Option<(Instant, Instant)>,
    next_action: u64,
    pending_handlers: Vec<(ActionId, Rc<dyn Fn()>)>,
    gesture_arena: GestureArena,
    active_gestures: HashMap<GestureArenaKey, ActiveGesture>,
    pointer_captures: HashMap<GestureArenaKey, ElementId>,
    active_drags: HashMap<GestureArenaKey, ActiveDrag>,
    scale_gestures: HashMap<ElementId, ScaleGestureDetector>,
    scrollbar_drag: Option<ScrollbarDrag>,
    semantics: SemanticsTree,
    semantic_ids: HashMap<ElementId, SemanticNodeId>,
    static_selection: Option<StaticSelection>,
    environment: RuntimeEnvironment,
    recursion_diagnostics: RecursionDiagnostics,
    #[cfg(feature = "devtools")]
    deep_trace: Option<DeepTraceCapture>,
}

struct FocusCandidate {
    id: ElementId,
    bounds: Rect,
    order: Option<f64>,
    sequence: usize,
}

enum FocusTraversalMember {
    Candidate(FocusCandidate),
    Group(FocusTraversalGroupMembers),
}

struct FocusTraversalGroupMembers {
    policy: FocusTraversalPolicyKind,
    members: Vec<FocusTraversalMember>,
}

impl FocusTraversalMember {
    fn first_candidate(&self) -> Option<&FocusCandidate> {
        match self {
            Self::Candidate(candidate) => Some(candidate),
            Self::Group(group) => group.members.iter().find_map(Self::first_candidate),
        }
    }

    fn representative_bounds(&self) -> Rect {
        self.first_candidate()
            .map_or(Rect::default(), |candidate| candidate.bounds)
    }

    fn representative_order(&self) -> Option<f64> {
        self.first_candidate().and_then(|candidate| candidate.order)
    }

    fn representative_sequence(&self) -> usize {
        self.first_candidate()
            .map_or(usize::MAX, |candidate| candidate.sequence)
    }
}

fn sort_focus_members(policy: FocusTraversalPolicyKind, members: &mut [FocusTraversalMember]) {
    match policy {
        FocusTraversalPolicyKind::WidgetOrder => {}
        FocusTraversalPolicyKind::ReadingOrder => {
            members.sort_by(|left, right| {
                let left_bounds = left.representative_bounds();
                let right_bounds = right.representative_bounds();
                let row_tolerance =
                    (left_bounds.size.height.max(right_bounds.size.height) * 0.5) + 1.0;
                let vertical = left_bounds.origin.y - right_bounds.origin.y;
                if vertical.abs() > row_tolerance {
                    left_bounds.origin.y.total_cmp(&right_bounds.origin.y)
                } else {
                    left_bounds.origin.x.total_cmp(&right_bounds.origin.x)
                }
                .then_with(|| {
                    left.representative_sequence()
                        .cmp(&right.representative_sequence())
                })
            });
        }
        FocusTraversalPolicyKind::Ordered => {
            members.sort_by(|left, right| {
                left.representative_order()
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&right.representative_order().unwrap_or(f64::INFINITY))
                    .then_with(|| {
                        left.representative_sequence()
                            .cmp(&right.representative_sequence())
                    })
            });
        }
    }
}

fn flatten_focus_group(group: FocusTraversalGroupMembers, out: &mut Vec<ElementId>) {
    let mut members = group.members;
    sort_focus_members(group.policy, &mut members);
    for member in members {
        match member {
            FocusTraversalMember::Candidate(candidate) => out.push(candidate.id),
            FocusTraversalMember::Group(group) => flatten_focus_group(group, out),
        }
    }
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}
impl WidgetTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            elements: Arena::new(),
            renders: Arena::new(),
            root: None,
            diagnostics: Diagnostics::default(),
            unmounted: Vec::new(),
            text_engine: TextEngine::new(),
            compositor: LayerTree::new(),
            compositor_initialized: false,
            animation_time_scale: 1.,
            animation_clock: None,
            next_action: 1,
            pending_handlers: Vec::new(),
            gesture_arena: GestureArena::new(),
            active_gestures: HashMap::new(),
            pointer_captures: HashMap::new(),
            active_drags: HashMap::new(),
            scale_gestures: HashMap::new(),
            scrollbar_drag: None,
            semantics: SemanticsTree::new(),
            semantic_ids: HashMap::new(),
            static_selection: None,
            environment: RuntimeEnvironment::default(),
            recursion_diagnostics: RecursionDiagnostics::new(),
            #[cfg(feature = "devtools")]
            deep_trace: None,
        }
    }

    /// Sets the ambient runtime environment (safe insets, scaling, etc.) and invalidates layout if needed.
    pub fn set_environment(&mut self, environment: RuntimeEnvironment) {
        let dirty_safe_area = self.environment.safe_insets != environment.safe_insets;
        self.environment = environment;
        if dirty_safe_area {
            for (_, render) in self.renders.iter_mut() {
                if matches!(render.kind, RenderKind::SafeArea { .. }) {
                    render.dirty.insert(DirtyFlags::LAYOUT);
                }
            }
        }
    }

    /// Returns a reference to the ambient runtime environment.
    #[must_use]
    pub fn environment(&self) -> &RuntimeEnvironment {
        &self.environment
    }

    /// Records the input or command responsible for subsequent frame work.
    /// The value is retained until another trigger replaces it so an input
    /// callback that merely schedules a frame remains attributable later.
    pub fn set_diagnostic_trigger(&self, trigger: impl Into<String>) {
        self.recursion_diagnostics.set_trigger(trigger);
    }

    /// Returns the most recent recursion report. Reports are also persisted to
    /// the platform crash-report directory before the guard panics.
    #[must_use]
    pub fn last_recursion_report(&self) -> Option<RecursionReport> {
        self.recursion_diagnostics.last_report()
    }

    /// Overrides the recursion crash-log directory, primarily for embedders
    /// and deterministic tests.
    pub fn set_recursion_report_directory(&self, path: impl Into<std::path::PathBuf>) {
        self.recursion_diagnostics.set_report_directory(path);
    }

    /// Sets the maximum nested pipeline depth. Debug and DevTools builds also
    /// detect exact retained-node re-entry before this fallback is reached.
    pub fn set_recursion_limit(&self, limit: usize) {
        self.recursion_diagnostics.set_limit(limit);
    }

    fn guard_phase_root(&self, phase: FramePhase) -> crate::recursion::ActivePhaseGuard {
        self.recursion_diagnostics
            .enter(DiagnosticNode::phase_root(phase))
    }

    fn guard_element(
        &self,
        phase: FramePhase,
        id: ElementId,
    ) -> crate::recursion::ActivePhaseGuard {
        let kind = self
            .elements
            .get(id.0)
            .map_or("UnmountedElement", |element| element.widget.type_().name());
        self.recursion_diagnostics.enter(DiagnosticNode {
            phase,
            id: Some(DiagnosticNodeId::Element(id)),
            kind,
            constraints: None,
        })
    }

    fn guard_render(
        &self,
        phase: FramePhase,
        id: RenderObjectId,
        constraints: Option<Constraints>,
    ) -> crate::recursion::ActivePhaseGuard {
        let kind = self
            .element_for_render(id)
            .and_then(|element| self.elements.get(element.0))
            .map_or("DetachedRenderObject", |element| {
                element.widget.type_().name()
            });
        self.recursion_diagnostics.enter(DiagnosticNode {
            phase,
            id: Some(DiagnosticNodeId::Render(id)),
            kind,
            constraints,
        })
    }

    /// Starts one bounded Deep-profiler frame. Calling this again discards an
    /// unfinished capture, which keeps stale target sessions from retaining
    /// trace data indefinitely.
    #[cfg(feature = "devtools")]
    pub fn begin_deep_trace(&mut self, max_events: usize) {
        self.deep_trace = Some(DeepTraceCapture::new(max_events));
    }

    /// Finishes a Deep-profiler frame and transfers its bounded storage.
    #[cfg(feature = "devtools")]
    pub fn take_deep_trace(&mut self) -> Option<(Vec<TraceEvent>, u32)> {
        self.deep_trace
            .take()
            .map(|capture| (capture.events, capture.dropped_events))
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_trace_begin_element(
        &mut self,
        id: ElementId,
        phase: TracePhase,
    ) -> Option<u32> {
        let raw = id.0;
        self.deep_trace.as_mut()?.begin(
            DevWidgetId::new(u64::from(raw.index()), u64::from(raw.generation())),
            phase,
        )
    }

    #[cfg(feature = "devtools")]
    pub fn devtools_trace_end(&mut self, token: Option<u32>) {
        if let Some(capture) = &mut self.deep_trace {
            capture.end(token);
        }
    }
    pub fn mount(&mut self, widget: Widget) -> Result<ElementId, TreeError> {
        let _phase_guard = self.guard_phase_root(FramePhase::Build);
        if let Some(root) = self.root {
            self.unmount(root)?;
        }
        let root = self.mount_element(None, widget)?;
        self.root = Some(root);
        self.refresh_notification_listeners();
        Ok(root)
    }
    #[must_use]
    pub fn root(&self) -> Option<ElementId> {
        self.root
    }
    /// Finds the first mounted element whose widget carries exactly `key`.
    #[must_use]
    pub fn element_with_key(&self, key: &Key) -> Option<ElementId> {
        self.elements.iter().find_map(|(raw, element)| {
            (element.widget.key() == Some(key)).then_some(ElementId(raw))
        })
    }
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }
    #[must_use]
    pub fn render_object_count(&self) -> usize {
        self.renders.len()
    }
    #[must_use]
    pub fn sliver_viewport_diagnostics(&self) -> Option<SliverViewportDiagnostics> {
        self.elements.iter().find_map(|(_raw, element)| {
            let WidgetKind::SliverViewport { config } = &element.widget.kind else {
                return None;
            };
            let render = self.renders.get(element.render.0)?;
            let mut indices = element
                .sliver_child_ids
                .iter()
                .filter_map(|id| id.item_index());
            let first = indices.next();
            let (start, end) = first.map_or((0, 0), |first| {
                indices.fold((first, first + 1), |(min, max), index| {
                    (min.min(index), max.max(index + 1))
                })
            });
            Some(SliverViewportDiagnostics {
                logical_item_count: config
                    .delegate
                    .child_count()
                    .unwrap_or(element.sliver_child_ids.len()),
                materialized_item_count: element.sliver_child_ids.len(),
                materialized_range: start..end,
                scroll_offset: config.controller.offset(),
                viewport_extent: config.axis.main_extent(render.size),
                cache_extent: config.cache_extent,
                element_count: self.elements.len(),
                render_object_count: self.renders.len(),
                picture_layer_count: self.compositor.diagnostics().layers as usize,
            })
        })
    }
    #[must_use]
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }
    /// Retained, renderer-independent semantic tree. It reflects meaningful
    /// controls, rather than paint commands or compositor pictures.
    #[must_use]
    pub fn semantics(&self) -> &SemanticsTree {
        &self.semantics
    }
    #[must_use]
    pub fn semantics_diagnostics(&self) -> SemanticsDiagnostics {
        self.semantics.diagnostics()
    }
    #[must_use]
    pub fn semantic_node_for_element(&self, element: ElementId) -> Option<SemanticNodeId> {
        self.semantic_ids.get(&element).copied()
    }
    #[must_use]
    pub fn element_for_semantic_node(&self, node: SemanticNodeId) -> Option<ElementId> {
        self.semantic_ids
            .iter()
            .find_map(|(element, current)| (*current == node).then_some(*element))
    }
    #[must_use]
    pub fn semantics_debug_dump(&self) -> String {
        self.semantics.debug_dump()
    }
    pub fn note_semantic_action(&mut self) {
        self.semantics.note_action();
    }
    pub fn take_unmounted(&mut self) -> Vec<ElementId> {
        std::mem::take(&mut self.unmounted)
    }
    #[must_use]
    pub fn element_exists(&self, id: ElementId) -> bool {
        self.elements.contains(id.0)
    }
    #[must_use]
    pub fn children(&self, id: ElementId) -> Option<&[ElementId]> {
        self.elements.get(id.0).map(|e| e.children.as_slice())
    }
    #[must_use]
    pub fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(id.0).and_then(|e| e.parent)
    }
    /// Records why an element is about to rebuild (DevTools builds only).
    #[cfg(feature = "devtools")]
    pub fn note_invalidation(&mut self, id: ElementId, cause: InvalidationCause) {
        if let Some(element) = self.elements.get_mut(id.0) {
            const MAX_CAUSES: usize = 8;
            if element.dev.invalidation_causes.len() == MAX_CAUSES {
                element.dev.invalidation_causes.remove(0);
            }
            element.dev.invalidation_causes.push(cause.clone());
            element.dev.last_cause = Some(cause);
        }
    }

    pub fn mark_build(&mut self, id: ElementId) -> Result<(), TreeError> {
        let element = self
            .elements
            .get_mut(id.0)
            .ok_or(TreeError::MissingElement(id))?;
        self.diagnostics.dirty_requests += 1;
        let was_dirty = element.dirty.contains(DirtyFlags::BUILD);
        element.dirty.insert(DirtyFlags::BUILD);
        if was_dirty {
            self.diagnostics.dirty_queue_deduplicated += 1;
        } else {
            self.diagnostics.dirty_queue_insertions += 1;
        }
        Ok(())
    }
    #[must_use]
    pub fn is_build_dirty(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| element.dirty.contains(DirtyFlags::BUILD))
    }
    #[must_use]
    pub fn render_id(&self, id: ElementId) -> Option<RenderObjectId> {
        self.elements.get(id.0).map(|e| e.render)
    }
    /// Current world-space bounds for a mounted element. This is useful for
    /// platform-neutral tooling and tests; it never exposes a render ID.
    #[must_use]
    pub fn element_bounds(&self, id: ElementId) -> Option<Rect> {
        let render = self.render_id(id)?;
        let size = self.renders.get(render.0)?.size;
        Some(
            self.render_world_transform(render)
                .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, size)),
        )
    }
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn dev_elements(&self) -> &Arena<Element> {
        &self.elements
    }

    /// Exact retained transforms for a live element, exposed only to the
    /// read-only DevTools snapshot adapter. Kurbo remains the geometry
    /// authority; this does not re-run layout or compositor work.
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_layout_transforms(
        &self,
        id: ElementId,
    ) -> Option<(CoreTransform, CoreTransform, CoreTransform)> {
        let render = self.render_id(id)?;
        let node = self.renders.get(render.0)?;
        Some((
            CoreTransform::translation(node.offset),
            self.render_world_transform(render),
            self.content_transform(render)
                .unwrap_or(CoreTransform::IDENTITY),
        ))
    }

    /// Line count from the existing Parley-backed retained result. It is
    /// intentionally an observation only: DevTools never asks the text
    /// engine to shape content for inspection.
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_text_line_count(&self, id: ElementId) -> Option<usize> {
        let render = self.render_id(id)?;
        Some(
            self.renders
                .get(render.0)?
                .text_layout
                .as_ref()?
                .lines
                .len(),
        )
    }

    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_is_layer_boundary(&self, id: ElementId) -> bool {
        self.render_id(id)
            .and_then(|render| self.renders.get(render.0))
            .is_some_and(|render| {
                render.picture.is_some()
                    || render.clip_layer.is_some()
                    || render.opacity_layer.is_some()
                    || render.blur_layer.is_some()
                    || render.shadow_layer.is_some()
                    || render.color_filter_layer.is_some()
                    || render.blend_layer.is_some()
            })
    }

    /// Per-viewport variant of [`Self::sliver_viewport_diagnostics`], used by
    /// the selected-node Layout Explorer rather than a global first-match
    /// query. Values are retained by the live sliver viewport.
    #[must_use]
    #[cfg(feature = "devtools")]
    pub(crate) fn devtools_sliver_viewport_diagnostics(
        &self,
        id: ElementId,
    ) -> Option<SliverViewportDiagnostics> {
        let element = self.elements.get(id.0)?;
        let WidgetKind::SliverViewport { config } = &element.widget.kind else {
            return None;
        };
        let render = self.renders.get(element.render.0)?;
        let mut indices = element
            .sliver_child_ids
            .iter()
            .filter_map(|id| id.item_index());
        let first = indices.next();
        let (start, end) = first.map_or((0, 0), |first| {
            indices.fold((first, first + 1), |(min, max), index| {
                (min.min(index), max.max(index + 1))
            })
        });
        Some(SliverViewportDiagnostics {
            logical_item_count: config
                .delegate
                .child_count()
                .unwrap_or(element.sliver_child_ids.len()),
            materialized_item_count: element.sliver_child_ids.len(),
            materialized_range: start..end,
            scroll_offset: config.controller.offset(),
            viewport_extent: config.axis.main_extent(render.size),
            cache_extent: config.cache_extent,
            element_count: self.elements.len(),
            render_object_count: self.renders.len(),
            picture_layer_count: self.compositor.diagnostics().layers as usize,
        })
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn dev_renders(&self) -> &Arena<RenderObject> {
        &self.renders
    }

    /// Applies a narrowly typed, temporary DevTools property override to a
    /// live retained element. Unsupported properties and stale IDs are
    /// rejected without mutating the tree.
    #[cfg(feature = "devtools")]
    pub fn devtools_edit_property(
        &mut self,
        target: DevWidgetId,
        name: &str,
        value: &DebugValue,
    ) -> bool {
        let Some(id) = self.devtools_resolve_id(target) else {
            return false;
        };
        let Some(value) = (match value {
            DebugValue::Float(value) if value.is_finite() => Some((*value as f32).clamp(0., 1.)),
            _ => None,
        }) else {
            return false;
        };
        let render = {
            let Some(element) = self.elements.get_mut(id.0) else {
                return false;
            };
            match (&mut element.widget.kind, name) {
                (
                    WidgetKind::Opacity {
                        alpha,
                        controller: None,
                        ..
                    },
                    "opacity",
                ) => {
                    if *alpha == value {
                        return true;
                    }
                    *alpha = value;
                }
                _ => return false,
            }
            element.dev.revision = element.dev.revision.wrapping_add(1);
            element.dev.composite_reason = Some("DevTools opacity override".into());
            element.render
        };
        let opacity_layer = {
            let Some(render_node) = self.renders.get_mut(render.0) else {
                return false;
            };
            let RenderKind::Opacity {
                alpha,
                controller: None,
            } = &mut render_node.kind
            else {
                return false;
            };
            *alpha = value;
            render_node.opacity_layer
        };
        if let Some(layer) = opacity_layer {
            let _ = self.compositor.update_opacity(layer, value);
        }
        self.diagnostics.compositor_only_updates =
            self.diagnostics.compositor_only_updates.wrapping_add(1);
        true
    }
    #[cfg(feature = "devtools")]
    pub(crate) fn dev_semantic_ids(&self) -> &HashMap<ElementId, SemanticNodeId> {
        &self.semantic_ids
    }
    #[cfg(feature = "devtools")]
    pub fn arena_index(&self, id: ElementId) -> u32 {
        id.0.index()
    }
    pub fn element_for_render(&self, render: RenderObjectId) -> Option<ElementId> {
        // Render IDs are opaque; a linear reverse lookup is only on input paths,
        // never layout/paint hot paths. A reverse arena index can be added when
        // profiling demonstrates it matters.
        self.elements
            .iter()
            .find_map(|(raw, element)| (element.render == render).then_some(ElementId(raw)))
    }
    #[must_use]
    pub fn action_for_element(&self, id: ElementId) -> Option<ActionId> {
        match &self.elements.get(id.0)?.widget.kind {
            WidgetKind::Button { action, .. } => Some(*action),
            _ => None,
        }
    }
    /// Returns an application-provided semantic action callback, if one was
    /// attached by [`Semantics`](crate::Semantics). The callback is cloned
    /// before the runtime invokes it, so it never runs while the tree is
    /// borrowed mutably.
    #[must_use]
    pub fn semantic_action_callback(
        &self,
        id: ElementId,
        action: SemanticActionKind,
    ) -> Option<Rc<dyn Fn() + 'static>> {
        self.elements
            .get(id.0)
            .and_then(|element| element.widget.semantics.callbacks.callback(action))
    }
    #[must_use]
    pub fn action_ids(&self) -> HashSet<ActionId> {
        self.elements
            .iter()
            .flat_map(|(_, element)| match element.widget.kind {
                WidgetKind::Button {
                    action,
                    hover_action,
                    exit_action,
                    ..
                } => [action, hover_action, exit_action]
                    .map(|action| (action.0 != 0).then_some(action)),
                _ => [None; 3],
            })
            .flatten()
            .collect()
    }
    #[must_use]
    pub fn hover_actions_for_element(&self, id: ElementId) -> (Option<ActionId>, Option<ActionId>) {
        match &self.elements.get(id.0).map(|element| &element.widget.kind) {
            Some(WidgetKind::Button {
                hover_action,
                exit_action,
                ..
            }) => (
                (hover_action.0 != 0).then_some(*hover_action),
                (exit_action.0 != 0).then_some(*exit_action),
            ),
            _ => (None, None),
        }
    }
    /// Allocates an opaque callback action. The runtime owns dispatch, while
    /// the tree uses this shared sequence for lazy children mounted during
    /// layout so IDs can never collide with eagerly prepared buttons.
    pub fn allocate_action(&mut self) -> ActionId {
        let action = ActionId(self.next_action);
        self.next_action += 1;
        action
    }
    pub fn take_pending_handlers(&mut self) -> Vec<(ActionId, Rc<dyn Fn()>)> {
        std::mem::take(&mut self.pending_handlers)
    }
    #[must_use]
    pub fn action_ancestor(&self, mut id: ElementId) -> Option<(ElementId, ActionId)> {
        loop {
            if let Some(action) = self.action_for_element(id) {
                return (action.0 != 0).then_some((id, action));
            }
            id = self.parent(id)?;
        }
    }
    #[must_use]
    pub fn button_ancestor(&self, mut id: ElementId) -> Option<(ElementId, Option<ActionId>)> {
        loop {
            if let Some(WidgetKind::Button {
                action, enabled, ..
            }) = self.elements.get(id.0).map(|element| &element.widget.kind)
            {
                if !*enabled {
                    return None;
                }
                return Some((id, (action.0 != 0).then_some(*action)));
            }
            id = self.parent(id)?;
        }
    }
    /// Requests retained pointer capture for an already active gesture member.
    /// The returned token is window-local and is released automatically on up,
    /// cancellation, or unmount. This is the portable guarantee; platform
    /// adapters may additionally request native OS capture.
    pub fn request_pointer_capture(
        &mut self,
        window: u64,
        pointer: u64,
        element: ElementId,
    ) -> Option<PointerCapture> {
        let key = GestureArenaKey { window, pointer };
        let active = self.active_gestures.get(&key)?;
        if !active
            .members
            .iter()
            .any(|candidate| candidate.element == element)
        {
            return None;
        }
        self.pointer_captures.insert(key, element);
        Some(PointerCapture { key })
    }
    /// Returns the retained target currently captured for a window/pointer.
    #[must_use]
    pub fn pointer_capture_target(&self, window: u64, pointer: u64) -> Option<ElementId> {
        self.pointer_captures
            .get(&GestureArenaKey { window, pointer })
            .copied()
    }
    /// Releases a capture token. Releasing a token from another window or a
    /// stale pointer sequence is harmless and returns `false`.
    pub fn release_pointer_capture(&mut self, capture: PointerCapture) -> bool {
        self.pointer_captures.remove(&capture.key).is_some()
    }
    /// Dispatches a pointer event in the standalone window (identity zero).
    /// Multi-window runtimes should use [`Self::dispatch_gesture_in_window`].
    pub fn dispatch_gesture(&mut self, event: PointerEvent) -> Option<ElementId> {
        self.dispatch_gesture_in_window(0, event)
    }
    /// Dispatches a pointer event through the retained gesture arena for one
    /// window. Every hit-tested gesture ancestor joins the stream pending;
    /// callbacks run only after its recognizer wins (or is explicitly
    /// compatible with) that stream's arena.
    pub fn dispatch_gesture_in_window(
        &mut self,
        window: u64,
        event: PointerEvent,
    ) -> Option<ElementId> {
        use incular_core::PointerPhase;

        let key = GestureArenaKey {
            window,
            pointer: event.pointer,
        };
        if matches!(event.phase, PointerPhase::Down) {
            self.cancel_gesture_stream(key, true);
            let hit = self
                .hit_test(event.position)
                .and_then(|render| self.element_for_render(render))?;
            let elements = self.gesture_ancestors(hit);
            let element = *elements.first()?;
            let mut active = ActiveGesture {
                element,
                members: Vec::new(),
                cancelled_elements: HashSet::new(),
                start: event.position,
            };
            for candidate in elements {
                let Some(callbacks) = self.gesture_callbacks(candidate) else {
                    continue;
                };
                if callbacks.has_pointer_recognizer() {
                    let member = self.gesture_arena.add(key, false);
                    let mut recognizer = PointerGestureRecognizer::new(callbacks.clone());
                    let _ = recognizer.observe(event);
                    active.members.push(ActiveGestureMember {
                        element: candidate,
                        member,
                        kind: RetainedGestureKind::Pointer,
                        recognizer: Some(recognizer),
                        on_cancel: callbacks.on_cancel.clone(),
                    });
                }
                if callbacks.on_scale_update.is_some()
                    || callbacks.on_scale_start.is_some()
                    || callbacks.on_scale_end.is_some()
                {
                    let on_update = callbacks.on_scale_update.clone();
                    let on_start = callbacks.on_scale_start.clone();
                    let on_end = callbacks.on_scale_end.clone();
                    let member = self.gesture_arena.add(key, true);
                    let mut scale = self.scale_gestures.remove(&candidate).unwrap_or_else(|| {
                        ScaleGestureDetector::with_callbacks(on_update, on_start, on_end)
                    });
                    let _ = scale.observe(event);
                    self.scale_gestures.insert(candidate, scale);
                    active.members.push(ActiveGestureMember {
                        element: candidate,
                        member,
                        kind: RetainedGestureKind::Scale,
                        recognizer: None,
                        on_cancel: callbacks.on_cancel.clone(),
                    });
                }
            }
            // A plain hit-tested label must continue through the runtime's
            // ordinary pointer route (for buttons, editable fields, and
            // read-only text selection). Only actual recognizers create an
            // arena stream or retain pointer capture.
            if active.members.is_empty() {
                return None;
            }
            self.active_gestures.insert(key, active);
            self.pointer_captures.insert(key, element);
            let scale_elements = self
                .active_gestures
                .get(&key)
                .map(|active| {
                    active
                        .members
                        .iter()
                        .filter(|candidate| candidate.kind == RetainedGestureKind::Scale)
                        .map(|candidate| candidate.element)
                        .collect()
                })
                .unwrap_or_default();
            self.activate_scale_pairs(key.window, scale_elements);
            return Some(element);
        }

        let element = self.active_gestures.get(&key)?.element;
        if !self.elements.contains(element.0) {
            self.cancel_gesture_stream(key, true);
            return None;
        }
        let dispositions = self.gesture_arena.entries(key);
        let mut accepts = Vec::new();
        let mut rejects = Vec::new();
        let mut callbacks = Vec::new();
        let mut scale_updates = Vec::new();
        if let Some(active) = self.active_gestures.get_mut(&key) {
            for candidate in &mut active.members {
                let disposition = disposition_for(&dispositions, candidate.member);
                match candidate.kind {
                    RetainedGestureKind::Pointer => {
                        let Some(recognizer) = candidate.recognizer.as_mut() else {
                            continue;
                        };
                        match recognizer.observe(event) {
                            GestureDecision::Accept(action) => match disposition {
                                GestureDisposition::Accepted => {
                                    callbacks.push((candidate.member, action))
                                }
                                GestureDisposition::Pending => {
                                    accepts.push((candidate.member, action))
                                }
                                GestureDisposition::Rejected | GestureDisposition::Cancelled => {}
                            },
                            GestureDecision::Reject | GestureDecision::Cancelled
                                if disposition == GestureDisposition::Pending =>
                            {
                                rejects.push(candidate.member);
                            }
                            GestureDecision::Pending
                            | GestureDecision::Reject
                            | GestureDecision::Cancelled => {}
                        }
                    }
                    RetainedGestureKind::Scale => {
                        if let Some(scale) = self.scale_gestures.get_mut(&candidate.element) {
                            if let Some(details) = scale.observe(event)
                                && disposition == GestureDisposition::Accepted
                            {
                                scale_updates.push((candidate.element, details));
                            }
                        }
                    }
                }
            }
        }
        for member in rejects {
            self.gesture_arena.reject(key, member);
            self.apply_arena_entries(key, self.gesture_arena.entries(key));
        }
        for (member, action) in accepts {
            let entries = self.gesture_arena.accept(key, member);
            self.apply_arena_entries(key, entries);
            if disposition_for(&self.gesture_arena.entries(key), member)
                == GestureDisposition::Accepted
            {
                callbacks.push((member, action));
            }
        }
        for (member, action) in callbacks {
            self.dispatch_gesture_action(key, member, action);
        }
        for (scale_element, details) in scale_updates {
            if let Some(scale) = self.scale_gestures.get(&scale_element) {
                scale.dispatch(details);
            }
        }
        let handled = !self.gesture_arena.entries(key).is_empty();
        if matches!(event.phase, PointerPhase::Up | PointerPhase::Cancel) {
            self.finish_drag(
                key,
                matches!(event.phase, PointerPhase::Cancel),
                event.position,
            );
            self.cancel_gesture_stream(key, matches!(event.phase, PointerPhase::Cancel));
        } else {
            self.remove_scale_recognizers_when_idle();
        }
        handled.then_some(element)
    }

    fn gesture_callbacks(&self, element: ElementId) -> Option<GestureCallbacks> {
        match &self.elements.get(element.0)?.widget.kind {
            WidgetKind::Gesture { callbacks, .. } => Some((**callbacks).clone()),
            WidgetKind::Draggable { .. } => Some(GestureCallbacks {
                on_pan_update: Some(Rc::new(|_| {})),
                ..GestureCallbacks::default()
            }),
            _ => None,
        }
    }
    fn gesture_ancestors(&self, mut id: ElementId) -> Vec<ElementId> {
        let mut ancestors = Vec::new();
        loop {
            if self.elements.get(id.0).is_none() {
                return ancestors;
            }
            if self.elements.get(id.0).is_some_and(|element| {
                matches!(
                    element.widget.kind,
                    WidgetKind::Gesture { .. } | WidgetKind::Draggable { .. }
                )
            }) {
                ancestors.push(id);
            }
            let Some(parent) = self.parent(id) else {
                return ancestors;
            };
            id = parent;
        }
    }
    fn activate_scale_pairs(&mut self, window: u64, elements: Vec<ElementId>) {
        for element in elements {
            let members: Vec<_> = self
                .active_gestures
                .iter()
                .filter(|(key, _)| key.window == window)
                .flat_map(|(key, active)| {
                    active.members.iter().filter_map(move |candidate| {
                        (candidate.element == element
                            && candidate.kind == RetainedGestureKind::Scale)
                            .then_some((*key, candidate.member))
                    })
                })
                .collect();
            if members.len() < 2 {
                continue;
            }
            for (key, member) in members {
                let entries = self.gesture_arena.accept(key, member);
                self.apply_arena_entries(key, entries);
            }
        }
    }
    fn apply_arena_entries(&mut self, key: GestureArenaKey, entries: Vec<GestureArenaEntry>) {
        let mut callbacks = Vec::new();
        if let Some(active) = self.active_gestures.get_mut(&key) {
            for entry in entries {
                if !matches!(
                    entry.disposition,
                    GestureDisposition::Rejected | GestureDisposition::Cancelled
                ) {
                    continue;
                }
                let Some(candidate) = active
                    .members
                    .iter()
                    .find(|candidate| candidate.member == entry.member)
                else {
                    continue;
                };
                if active.cancelled_elements.insert(candidate.element) {
                    if let Some(callback) = &candidate.on_cancel {
                        callbacks.push(callback.clone());
                    }
                }
            }
        }
        for callback in callbacks {
            callback();
        }
    }
    fn dispatch_gesture_action(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
        action: GestureAction,
    ) {
        self.update_drag_from_action(key, member, action);
        if let Some(recognizer) = self.active_gestures.get_mut(&key).and_then(|active| {
            active
                .members
                .iter_mut()
                .find(|candidate| candidate.member == member)
                .and_then(|candidate| candidate.recognizer.as_mut())
        }) {
            recognizer.dispatch(action);
        }
    }
    fn update_drag_from_action(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
        action: GestureAction,
    ) {
        let (GestureAction::Pan(delta)
        | GestureAction::HorizontalDrag(delta)
        | GestureAction::VerticalDrag(delta)) = action
        else {
            return;
        };
        let Some((source, start)) = self.active_gestures.get(&key).and_then(|active| {
            active
                .members
                .iter()
                .find(|candidate| candidate.member == member)
                .and_then(|candidate| self.drag_source(candidate.element))
                .map(|source| (source, active.start))
        }) else {
            return;
        };
        let position = start + delta;
        self.active_drags.entry(key).or_insert_with(|| {
            source.start(start);
            ActiveDrag {
                source: source.clone(),
                target: None,
                start,
            }
        });
        source.update(position);
        self.update_drag_target(key, position);
    }
    fn drag_source(&self, element: ElementId) -> Option<Rc<dyn RetainedDragSource>> {
        match &self.elements.get(element.0)?.widget.kind {
            WidgetKind::Draggable { source, .. } => Some(source.clone()),
            _ => None,
        }
    }
    fn drag_target(&self, element: ElementId) -> Option<Rc<dyn RetainedDragTarget>> {
        match &self.elements.get(element.0)?.widget.kind {
            WidgetKind::DragTarget { target, .. } => Some(target.clone()),
            _ => None,
        }
    }
    fn drag_target_ancestor(
        &self,
        mut element: ElementId,
    ) -> Option<(ElementId, Rc<dyn RetainedDragTarget>)> {
        loop {
            if let Some(target) = self.drag_target(element) {
                return Some((element, target));
            }
            element = self.parent(element)?;
        }
    }
    fn update_drag_target(&mut self, key: GestureArenaKey, position: Offset) {
        let next = self
            .hit_test(position)
            .and_then(|render| self.element_for_render(render))
            .and_then(|element| self.drag_target_ancestor(element));
        let Some(active) = self.active_drags.get_mut(&key) else {
            return;
        };
        let source_context = active.source.context_id();
        let next = next.filter(|(_, target)| target.context_id() == source_context);
        if active.target.as_ref().map(|(element, _)| *element)
            == next.as_ref().map(|(element, _)| *element)
        {
            if let Some((_, target)) = &active.target {
                target.update(position);
            }
            return;
        }
        if let Some((_, target)) = active.target.take() {
            target.leave();
        }
        if let Some((element, target)) = next {
            if target.enter() {
                target.update(position);
                active.target = Some((element, target));
            }
        }
    }
    fn finish_drag(&mut self, key: GestureArenaKey, cancelled: bool, position: Offset) {
        self.update_drag_target(key, position);
        let Some(active) = self.active_drags.remove(&key) else {
            return;
        };
        if cancelled {
            if let Some((_, target)) = active.target {
                target.leave();
            }
            active.source.cancel();
        } else {
            if let Some((_, target)) = active.target {
                target.drop_payload();
            }
            active.source.finish();
        }
    }
    fn cancel_gesture_stream(&mut self, key: GestureArenaKey, notify: bool) {
        let position = self
            .active_drags
            .get(&key)
            .map_or(Offset::ZERO, |drag| drag.start);
        self.finish_drag(key, true, position);
        let entries = self.gesture_arena.cancel(key);
        if notify {
            self.apply_arena_entries(key, entries);
        }
        self.active_gestures.remove(&key);
        self.pointer_captures.remove(&key);
        self.remove_scale_recognizers_when_idle();
    }
    fn remove_scale_recognizers_when_idle(&mut self) {
        self.scale_gestures.retain(|element, _| {
            self.active_gestures.values().any(|active| {
                active.members.iter().any(|candidate| {
                    candidate.element == *element && candidate.kind == RetainedGestureKind::Scale
                })
            })
        });
    }
    #[must_use]
    pub fn render_size(&self, id: RenderObjectId) -> Option<Size> {
        self.renders.get(id.0).map(|r| r.size)
    }
    /// Baseline in this render object's local logical coordinate system.
    #[must_use]
    pub fn baseline(&self, id: RenderObjectId) -> Option<f32> {
        self.renders.get(id.0).and_then(|render| render.baseline)
    }
    #[must_use]
    pub fn text_diagnostics(&self) -> TextDiagnostics {
        self.text_engine.diagnostics()
    }
    #[must_use]
    pub fn compositor_debug_tree(&self) -> String {
        self.compositor.debug_tree()
    }
    #[must_use]
    pub fn compositor_diagnostics(&self) -> incular_painting::CompositorDiagnostics {
        self.compositor.diagnostics()
    }
    #[must_use]
    pub fn button_state(&self, id: ElementId) -> Option<ButtonState> {
        self.render_id(id)
            .and_then(|render| self.renders.get(render.0))
            .map(|render| render.button_state)
    }
    pub fn set_button_state(&mut self, id: ElementId, state: ButtonState) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live render");
        if matches!(node.kind, RenderKind::Button { .. }) && node.button_state != state {
            node.button_state = state;
            node.button_hovered = matches!(state, ButtonState::Hovered);
            node.button_pressed = matches!(state, ButtonState::Pressed);
            node.button_focused = matches!(state, ButtonState::Focused);
            node.dirty.insert(DirtyFlags::PAINT);
        }
        Ok(())
    }

    /// Updates retained hover, press, and focus bits independently. The
    /// legacy [`set_button_state`](Self::set_button_state) API remains a
    /// compatibility setter for callers that want an exclusive state.
    pub fn set_button_interaction(
        &mut self,
        id: ElementId,
        hovered: Option<bool>,
        pressed: Option<bool>,
        focused: Option<bool>,
    ) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live render");
        if !matches!(node.kind, RenderKind::Button { .. }) {
            return Ok(());
        }
        if let Some(value) = hovered {
            node.button_hovered = value;
        }
        if let Some(value) = pressed {
            node.button_pressed = value;
        }
        if let Some(value) = focused {
            node.button_focused = value;
        }
        node.button_state = if node.button_pressed {
            ButtonState::Pressed
        } else if node.button_focused {
            ButtonState::Focused
        } else if node.button_hovered {
            ButtonState::Hovered
        } else {
            ButtonState::Normal
        };
        node.dirty.insert(DirtyFlags::PAINT);
        Ok(())
    }
    /// Changes only retained animation progression. The accumulated logical
    /// timestamp is preserved, so pausing or slowing an already-running
    /// controller never jumps it forward to wall-clock time.
    pub fn set_animation_time_scale(&mut self, scale: f32) {
        self.animation_time_scale = if scale.is_finite() {
            scale.clamp(0., 1.)
        } else {
            1.
        };
    }
    #[must_use]
    pub const fn animation_time_scale(&self) -> f32 {
        self.animation_time_scale
    }
    fn animation_now(&mut self, real_now: Instant) -> Instant {
        let (last_real, last_animation) = self.animation_clock.unwrap_or((real_now, real_now));
        let elapsed = real_now.saturating_duration_since(last_real);
        let scaled = elapsed.mul_f32(self.animation_time_scale);
        let animation_now = last_animation.checked_add(scaled).unwrap_or(last_animation);
        self.animation_clock = Some((real_now, animation_now));
        animation_now
    }
    /// Applies only retained compositor properties. It never marks a render
    /// object for build, layout, or paint.
    pub fn update_compositor(&mut self, now: Instant) -> (bool, bool) {
        let _phase_guard = self.guard_phase_root(FramePhase::Compositor);
        #[cfg(feature = "devtools")]
        let trace = self
            .root
            .and_then(|root| self.devtools_trace_begin_element(root, TracePhase::Composite));
        let now = self.animation_now(now);
        let nodes = self
            .renders
            .iter()
            .map(|(id, node)| {
                (
                    RenderObjectId(id),
                    node.kind.clone(),
                    node.content_layer,
                    node.opacity_layer,
                    node.blur_layer,
                    node.shadow_layer,
                    node.color_filter_layer,
                    node.blend_layer,
                )
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        let mut active = false;
        for (
            _render,
            kind,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
        ) in nodes
        {
            let constraints = self
                .renders
                .get(_render.0)
                .and_then(|render| render.constraints);
            let _node_guard = self.guard_render(FramePhase::Compositor, _render, constraints);
            #[cfg(feature = "devtools")]
            let changed_before_node = changed;
            match kind {
                RenderKind::Scroll {
                    controller,
                    axis,
                    reverse,
                    ..
                } => {
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            CoreTransform::translation(scroll_translation(
                                &controller,
                                axis,
                                reverse,
                            )),
                        )
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                        self.diagnostics.scroll_offset_updates += 1;
                        // Only this viewport's overlay picture changes; the
                        // retained content subtree remains compositor-only.
                        self.renders
                            .get_mut(_render.0)
                            .expect("live")
                            .dirty
                            .insert(DirtyFlags::PAINT);
                    }
                }
                RenderKind::SliverViewport { config } => {
                    if config.delegate.tick(now) {
                        changed = true;
                        self.diagnostics.animation_ticks += 1;
                        self.mark_render_dirty(
                            _render,
                            DirtyFlags::LAYOUT | DirtyFlags::PAINT,
                            true,
                        );
                    }
                    active |= config.delegate.is_animating();
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            CoreTransform::translation(scroll_translation(
                                &config.controller,
                                config.axis,
                                config.reverse,
                            )),
                        )
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                        self.diagnostics.scroll_offset_updates += 1;
                        self.renders
                            .get_mut(_render.0)
                            .expect("live")
                            .dirty
                            .insert(DirtyFlags::PAINT);
                        let has_pinned_children = self
                            .element_for_render(_render)
                            .and_then(|element| self.elements.get(element.0))
                            .is_some_and(|element| !element.sliver_pinned_ids.is_empty());
                        if has_pinned_children {
                            // Pinned placement is part of sliver layout rather
                            // than the generic box transform. Queue one
                            // retained layout refresh so its push-away
                            // geometry follows the new scroll offset.
                            self.mark_render_dirty(_render, DirtyFlags::LAYOUT, true);
                            if let Some(constraints) = self
                                .renders
                                .get(_render.0)
                                .and_then(|render| render.constraints)
                            {
                                // Standalone WidgetTree users do not have a
                                // runtime layout phase between a controller
                                // jump and this compositor call. Run the
                                // already-known viewport layout now so pinned
                                // placement is observable immediately; the
                                // runtime reuses the cached result on its next
                                // layout.
                                self.layout_render(_render, constraints);
                            }
                        }
                    }
                }
                RenderKind::PersistentHeader {
                    controller,
                    axis,
                    reverse,
                    pinned,
                } => {
                    let offset = self.persistent_header_translation(
                        _render,
                        &controller,
                        axis,
                        reverse,
                        pinned,
                    );
                    if let Some(content) = content_layer
                        && self
                            .compositor
                            .update_transform(content, CoreTransform::translation(offset))
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                        self.diagnostics.scroll_offset_updates += 1;
                    }
                }
                RenderKind::Translate { controller } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    // Layout placement lives on `layer`; the inner retained
                    // transform carries only the compositor-only movement.
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            CoreTransform::translation(controller.offset()),
                        )
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::Transform { transform, origin } => {
                    if let Some(content) = content_layer {
                        let size = self.renders.get(_render.0).expect("live").size;
                        if self
                            .compositor
                            .update_transform(content, transform_around(transform, origin, size))
                        {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::Scale { controller, origin } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    if let Some(content) = content_layer {
                        let size = self.renders.get(_render.0).expect("live").size;
                        if self.compositor.update_transform(
                            content,
                            transform_around(
                                CoreTransform::scale(controller.scale()),
                                origin,
                                size,
                            ),
                        ) {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::Rotation {
                    controller,
                    origin,
                    alignment,
                } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    if let Some(content) = content_layer {
                        let size = self.renders.get(_render.0).expect("live").size;
                        if self.compositor.update_transform(
                            content,
                            transform_around_alignment(
                                CoreTransform::rotation(controller.radians()),
                                origin,
                                alignment,
                                size,
                            ),
                        ) {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::FittedBox { fit, alignment } => {
                    if let (Some(content), Some(&child)) = (
                        content_layer,
                        self.renders.get(_render.0).expect("live").children.first(),
                    ) {
                        let size = self.renders.get(_render.0).expect("live").size;
                        let child_size = self.renders.get(child.0).expect("live").size;
                        if self.compositor.update_transform(
                            content,
                            fitted_transform(child_size, size, fit, alignment),
                        ) {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    }
                }
                RenderKind::Opacity { alpha, controller } => {
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        if let Some(opacity) = opacity_layer
                            && self
                                .compositor
                                .update_opacity(opacity, controller.opacity())
                        {
                            changed = true;
                            self.diagnostics.compositor_only_updates += 1;
                        }
                    } else if let Some(opacity) = opacity_layer
                        && self.compositor.update_opacity(opacity, alpha)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::Blur {
                    sigma_x,
                    sigma_y,
                    controller,
                } => {
                    let mut sigma_x = sigma_x;
                    let mut sigma_y = sigma_y;
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        sigma_x = controller.sigma();
                        sigma_y = sigma_x;
                    }
                    if let Some(layer) = blur_layer
                        && self
                            .compositor
                            .update_blur(layer, GaussianBlur::new(sigma_x, sigma_y))
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::DropShadow {
                    offset,
                    sigma_x,
                    sigma_y,
                    color,
                    controller,
                } => {
                    let mut shadow = DropShadowEffect::asymmetric(offset, sigma_x, sigma_y, color);
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        shadow = DropShadowEffect::new(
                            controller.offset(),
                            controller.sigma(),
                            controller.color(),
                        );
                    }
                    if let Some(layer) = shadow_layer
                        && self.compositor.update_drop_shadow(layer, shadow)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::ColorFiltered { filter, controller } => {
                    let mut filter = filter;
                    if let Some(controller) = controller {
                        if controller.tick(now) {
                            self.diagnostics.animation_ticks += 1;
                        }
                        active |= controller.is_active();
                        filter = controller.filter();
                    }
                    if let Some(layer) = color_filter_layer
                        && self.compositor.update_color_filter(layer, filter)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                RenderKind::Blend { mode } => {
                    if let Some(layer) = blend_layer
                        && self.compositor.update_blend(layer, mode)
                    {
                        changed = true;
                        self.diagnostics.compositor_only_updates += 1;
                    }
                }
                _ => {}
            }
            #[cfg(feature = "devtools")]
            if changed != changed_before_node
                && let Some(element) = self.element_for_render(_render)
                && let Some(element) = self.elements.get_mut(element.0)
            {
                element.dev.composites += 1;
                element.dev.composite_reason =
                    Some("retained compositor property changed; paint reused".into());
            }
        }
        if !self.compositor_initialized {
            changed = true;
            self.diagnostics.compositor_only_updates += 1;
            self.compositor_initialized = true;
        }
        if changed {
            self.diagnostics.composites += 1;
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        (changed, active)
    }
    pub fn scroll_at(&mut self, point: Offset, delta: Offset) -> bool {
        let Some(mut element) = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))
        else {
            return false;
        };
        // The hit-tested leaf walks outward through its retained ancestors.
        // Passing the resulting innermost-first chain to the coordinator
        // transfers only boundary remainder to an outer scroll viewport.
        let mut viewports = Vec::new();
        loop {
            let render = self.elements.get(element.0).expect("live element").render;
            match &self.renders.get(render.0).expect("live render").kind {
                RenderKind::Scroll {
                    controller,
                    axis,
                    reverse,
                    physics,
                } => viewports.push((controller.clone(), *axis, *reverse, *physics)),
                RenderKind::SliverViewport { config } => viewports.push((
                    config.controller.clone(),
                    config.axis,
                    config.reverse,
                    config.physics,
                )),
                _ => {}
            }
            let Some(parent) = self.parent(element) else {
                break;
            };
            element = parent;
        }
        let Some((_, first_axis, _, _)) = viewports.first() else {
            return false;
        };
        let mut physical_remaining = scroll_delta_for_axis(*first_axis, delta);
        let mut consumed = 0.;
        for (controller, _axis, reverse, physics) in viewports {
            // Nested viewports normally share an axis. If an application
            // nests orthogonal viewports, the innermost hit-tested axis owns
            // the physical input and outer viewports receive its remainder.
            let logical = if reverse {
                -physical_remaining
            } else {
                physical_remaining
            };
            controller.notify_user_scroll(logical);
            let result = controller.apply_physics(physics, logical);
            // Pointer-wheel samples are complete user activities in this
            // high-level adapter. Drag recognizers can keep an activity open
            // by calling the controller's begin/end methods directly.
            controller.end_activity();
            let physical_consumed = if reverse {
                -result.consumed
            } else {
                result.consumed
            };
            consumed += physical_consumed;
            physical_remaining = if reverse {
                -result.unconsumed
            } else {
                result.unconsumed
            };
            if physical_remaining == 0. {
                break;
            }
        }
        if consumed != 0. {
            self.diagnostics.scroll_events += 1;
            true
        } else {
            false
        }
    }
    /// Applies a semantic page action to the controller owned by this viewport.
    pub fn semantic_scroll(&mut self, id: ElementId, forward: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let (controller, reverse, physics) = match &self.renders.get(render.0).expect("live").kind {
            RenderKind::Scroll {
                controller,
                axis: _,
                reverse,
                physics,
            } => (controller.clone(), *reverse, *physics),
            RenderKind::SliverViewport { config } => {
                (config.controller.clone(), config.reverse, config.physics)
            }
            _ => return false,
        };
        let page_extent = physics
            .snap_extent(controller.viewport_extent())
            .unwrap_or_else(|| controller.viewport_extent());
        let mut delta = if forward { page_extent } else { -page_extent };
        if reverse {
            delta = -delta;
        }
        let changed = controller.apply_physics(physics, delta).consumed != 0.;
        if changed {
            self.diagnostics.scroll_events += 1;
        }
        changed
    }
    /// Handles overlay scrollbar hit testing and capture. A thumb drag maps
    /// directly to the same controller used by wheel input; a track click
    /// pages one viewport toward the pointer.
    pub fn scrollbar_pointer(&mut self, phase: incular_core::PointerPhase, point: Offset) -> bool {
        match phase {
            incular_core::PointerPhase::Down => {
                let Some(render) = self.scrollbar_at(point) else {
                    return false;
                };
                let (controller, geometry) = self
                    .scrollbar_local_geometry(render)
                    .expect("scrollbar render");
                let point = self
                    .scrollbar_local_point(render, point)
                    .expect("invertible scrollbar");
                if geometry.thumb.contains(point) {
                    let grab = point.y - geometry.thumb.origin.y;
                    self.scrollbar_drag = Some(ScrollbarDrag {
                        render,
                        grab_offset: grab.clamp(0., geometry.thumb.size.height),
                    });
                    self.renders
                        .get_mut(render.0)
                        .expect("live")
                        .scrollbar_dragging = true;
                } else {
                    let delta = if point.y < geometry.thumb.origin.y {
                        -controller.viewport_extent()
                    } else {
                        controller.viewport_extent()
                    };
                    let _ = controller.scroll_by(delta);
                }
                self.renders
                    .get_mut(render.0)
                    .expect("live")
                    .dirty
                    .insert(DirtyFlags::PAINT);
                true
            }
            incular_core::PointerPhase::Move => {
                if let Some(drag) = self.scrollbar_drag {
                    let (controller, geometry) = self
                        .scrollbar_local_geometry(drag.render)
                        .expect("live drag");
                    let point = self
                        .scrollbar_local_point(drag.render, point)
                        .expect("invertible scrollbar");
                    let thumb_top = point.y - drag.grab_offset;
                    let offset = geometry.offset_for_thumb_top(thumb_top);
                    let changed = controller.jump_to(offset);
                    if changed {
                        self.diagnostics.scroll_events += 1;
                    }
                    self.renders
                        .get_mut(drag.render.0)
                        .expect("live")
                        .dirty
                        .insert(DirtyFlags::PAINT);
                    return true;
                }
                let hovered = self.scrollbar_at(point);
                let mut changed = false;
                let ids = self
                    .renders
                    .iter()
                    .map(|(raw, _)| RenderObjectId(raw))
                    .collect::<Vec<_>>();
                for render in ids {
                    let node = self.renders.get_mut(render.0).expect("live");
                    let is_hovered = hovered == Some(render);
                    if node.scrollbar_hovered != is_hovered {
                        node.scrollbar_hovered = is_hovered;
                        node.dirty.insert(DirtyFlags::PAINT);
                        changed = true;
                    }
                }
                changed || hovered.is_some()
            }
            incular_core::PointerPhase::Up | incular_core::PointerPhase::Cancel => {
                let Some(drag) = self.scrollbar_drag.take() else {
                    return false;
                };
                let node = self.renders.get_mut(drag.render.0).expect("live");
                node.scrollbar_dragging = false;
                node.dirty.insert(DirtyFlags::PAINT);
                true
            }
        }
    }
    #[must_use]
    pub fn scrollbar_diagnostics(&self) -> Vec<ScrollbarGeometry> {
        self.renders
            .iter()
            .filter_map(|(raw, _)| {
                self.scrollbar_controller_and_geometry(RenderObjectId(raw))
                    .map(|(_, geometry)| geometry)
            })
            .collect()
    }
    #[must_use]
    pub fn scrollbar_drag_diagnostics(&self) -> ScrollbarDragDiagnostics {
        let Some(drag) = self.scrollbar_drag else {
            return ScrollbarDragDiagnostics::default();
        };
        let Some((controller, geometry)) = self.scrollbar_controller_and_geometry(drag.render)
        else {
            return ScrollbarDragDiagnostics::default();
        };
        ScrollbarDragDiagnostics {
            active: true,
            grab_offset: drag.grab_offset,
            track_extent: geometry.track.size.height,
            thumb_extent: geometry.thumb.size.height,
            thumb_travel: geometry.thumb_travel,
            thumb_top: geometry.thumb.origin.y,
            scroll_offset: controller.offset(),
        }
    }
    pub fn update(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        let result = self.update_existing(id, &widget);
        if result.is_ok() {
            self.refresh_notification_listeners();
        }
        result
    }
    pub fn mark_paint(&mut self, id: ElementId) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        self.mark_render_dirty(render, DirtyFlags::PAINT, false);
        Ok(())
    }
    pub fn layout(&mut self, constraints: Constraints) {
        let _phase_guard = self.guard_phase_root(FramePhase::Layout);
        self.refresh_text_fields();
        self.refresh_sliver_ranges();
        self.refresh_stateful_layout_builders();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.layout_render(root, constraints);
        }
        self.refresh_notification_listeners();
    }

    /// Marks local-state layout builders dirty before the normal retained
    /// layout cache runs.  Input callbacks can mutate a control's revision
    /// without replacing its parent widget description; this keeps that
    /// update local while still allowing a changed child size to propagate.
    fn refresh_stateful_layout_builders(&mut self) {
        let dirty = self
            .elements
            .iter()
            .filter_map(|(_, element)| {
                let WidgetKind::LayoutBuilder {
                    revision: Some(revision),
                    ..
                } = &element.widget.kind
                else {
                    return None;
                };
                (revision.get() != element.layout_builder_revision).then_some(element.render)
            })
            .collect::<Vec<_>>();
        for render in dirty {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
    /// Synchronizes the retained semantic arena after layout/compositor state
    /// is valid. Non-semantic layout widgets merge their descendants into the
    /// closest meaningful semantic ancestor.
    pub fn update_semantics(&mut self) {
        let _phase_guard = self.guard_phase_root(FramePhase::Semantics);
        #[cfg(feature = "devtools")]
        let trace = self
            .root
            .and_then(|root| self.devtools_trace_begin_element(root, TracePhase::Semantics));
        let mut built = Vec::new();
        if let Some(root) = self.root {
            self.collect_semantics(root, None, &mut built);
        }
        let live: HashSet<_> = built.iter().map(|node| node.element).collect();
        let stale: Vec<_> = self
            .semantic_ids
            .iter()
            .filter_map(|(element, node)| (!live.contains(element)).then_some((*element, *node)))
            .collect();
        for (element, node) in stale {
            self.semantic_ids.remove(&element);
            let _ = self.semantics.remove(node);
        }
        for build in &built {
            #[cfg(feature = "devtools")]
            let semantic_revision_before = self.semantics.revision();
            let id = *self.semantic_ids.entry(build.element).or_insert_with(|| {
                self.semantics.insert(SemanticNode {
                    id: SemanticNodeId(ArenaId::from_parts(0, 0)),
                    role: build.role,
                    label: build.label.clone(),
                    value: build.value.clone(),
                    description: build.description.clone(),
                    bounds: build.bounds,
                    state: build.state.clone(),
                    actions: build.actions.clone(),
                    children: Vec::new(),
                })
            });
            let children = built
                .iter()
                .filter(|child| child.parent == Some(build.element))
                .filter_map(|child| self.semantic_ids.get(&child.element).copied())
                .collect();
            let _ = self.semantics.update(
                id,
                SemanticNode {
                    id,
                    role: build.role,
                    label: build.label.clone(),
                    value: build.value.clone(),
                    description: build.description.clone(),
                    bounds: build.bounds,
                    state: build.state.clone(),
                    actions: build.actions.clone(),
                    children,
                },
            );
            #[cfg(feature = "devtools")]
            if self.semantics.revision() != semantic_revision_before
                && let Some(element) = self.elements.get_mut(build.element.0)
            {
                element.dev.semantic_updates = element.dev.semantic_updates.saturating_add(1);
            }
        }
        self.semantics.set_root(
            built
                .iter()
                .find(|node| node.parent.is_none())
                .and_then(|node| self.semantic_ids.get(&node.element).copied()),
        );
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
    }
    fn collect_semantics(
        &self,
        element: ElementId,
        semantic_parent: Option<ElementId>,
        out: &mut Vec<SemanticBuild>,
    ) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.collect_semantics_inner(element, semantic_parent, out);
        });
    }

    fn collect_semantics_inner(
        &self,
        element: ElementId,
        semantic_parent: Option<ElementId>,
        out: &mut Vec<SemanticBuild>,
    ) {
        let _node_guard = self.guard_element(FramePhase::Semantics, element);
        let Some(entry) = self.elements.get(element.0) else {
            return;
        };
        if entry.widget.semantics.hidden
            || matches!(
                entry.widget.kind,
                WidgetKind::Visibility { visible: false, .. }
            )
        {
            return;
        }
        let render = match self.renders.get(entry.render.0) {
            Some(render) => render,
            None => return,
        };
        let (mut role, mut default_label, mut value, mut state, mut actions) =
            match &entry.widget.kind {
                WidgetKind::Button {
                    has_callback,
                    enabled,
                    focusable_when_disabled,
                    ..
                } => (
                    Some(SemanticRole::Button),
                    widget_text(&entry.widget),
                    None,
                    SemanticState {
                        enabled: *enabled,
                        focused: render.button_focused,
                        focusable: *enabled || *focusable_when_disabled,
                        ..SemanticState::default()
                    },
                    if *has_callback && *enabled {
                        vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                    } else {
                        vec![SemanticActionKind::Focus]
                    },
                ),
                WidgetKind::Text { text, .. } => (
                    Some(SemanticRole::Text),
                    Some(text.clone()),
                    None,
                    SemanticState::default(),
                    vec![],
                ),
                WidgetKind::SelectableText { text, .. } => (
                    Some(SemanticRole::Text),
                    Some(text.clone()),
                    None,
                    SemanticState {
                        focusable: true,
                        focused: render.focused,
                        ..SemanticState::default()
                    },
                    vec![SemanticActionKind::Focus],
                ),
                WidgetKind::Image { .. } if entry.widget.semantics.label.is_some() => (
                    Some(SemanticRole::Image),
                    None,
                    None,
                    SemanticState::default(),
                    vec![],
                ),
                WidgetKind::TextField {
                    controller,
                    multiline,
                    enabled,
                    read_only,
                    obscure_text,
                    ..
                } => {
                    let value = controller.value();
                    let mut actions =
                        vec![SemanticActionKind::Focus, SemanticActionKind::SetSelection];
                    if *enabled && !*read_only {
                        actions.push(SemanticActionKind::SetText);
                    }
                    (
                        Some(if *multiline {
                            SemanticRole::TextArea
                        } else {
                            SemanticRole::TextField
                        }),
                        None,
                        Some(value.text),
                        SemanticState {
                            enabled: *enabled,
                            focused: render.focused,
                            focusable: *enabled,
                            editable: *enabled && !*read_only,
                            multiline: *multiline,
                            obscured: *obscure_text,
                            read_only: *read_only,
                            selection: Some(SemanticTextSelection {
                                base: value.selection.base,
                                extent: value.selection.extent,
                            }),
                            ..SemanticState::default()
                        },
                        actions,
                    )
                }
                WidgetKind::Scroll { controller, .. } => (
                    Some(SemanticRole::ScrollView),
                    None,
                    Some(format!(
                        "{:.0}/{:.0}",
                        controller.offset(),
                        controller.max_offset()
                    )),
                    SemanticState::default(),
                    vec![
                        SemanticActionKind::ScrollForward,
                        SemanticActionKind::ScrollBackward,
                    ],
                ),
                WidgetKind::SliverViewport { config } => (
                    Some(SemanticRole::ScrollView),
                    None,
                    Some(format!(
                        "{:.0}/{:.0}",
                        config.controller.offset(),
                        config.controller.max_offset()
                    )),
                    SemanticState::default(),
                    vec![
                        SemanticActionKind::ScrollForward,
                        SemanticActionKind::ScrollBackward,
                    ],
                ),
                _ => (None, None, None, SemanticState::default(), vec![]),
            };
        let explicit_description = entry
            .widget
            .semantics
            .explicit
            .as_ref()
            .and_then(|semantics| semantics.description.clone());
        if let Some(explicit) = &entry.widget.semantics.explicit {
            role = Some(explicit.role);
            default_label = explicit.label.clone().or(default_label);
            value = explicit.value.clone().or(value);
            state = explicit.state.clone();
            actions = explicit.actions.clone();
        }
        // A native adapter must never advertise an action that the retained
        // runtime cannot route to an existing controller/handler. Explicit
        // semantic metadata configures roles/state; the Semantics callbacks
        // below are the opt-in pathway for arbitrary painted controls.
        for action in [
            SemanticActionKind::Activate,
            SemanticActionKind::Increment,
            SemanticActionKind::Decrement,
            SemanticActionKind::ScrollForward,
            SemanticActionKind::ScrollBackward,
        ] {
            if entry.widget.semantics.callbacks.supports(action) && !actions.contains(&action) {
                actions.push(action);
            }
        }
        actions.retain(|action| {
            semantic_action_is_executable(
                &entry.widget.kind,
                *action,
                &entry.widget.semantics.callbacks,
            )
        });
        let this_parent = if let Some(role) = role {
            let mut state = state;
            if let Some(parent) = entry.parent.and_then(|parent| self.elements.get(parent.0)) {
                if let WidgetKind::SliverViewport { config } = &parent.widget.kind {
                    if let Some(slot) = parent.children.iter().position(|child| *child == element) {
                        state.item_index = parent
                            .sliver_child_ids
                            .get(slot)
                            .map(|id| (id.0 & u64::from(u32::MAX)) as usize)
                            .filter(|index| *index > 0)
                            .map(|index| index - 1);
                        state.set_size = config.delegate.child_count();
                    }
                }
            }
            out.push(SemanticBuild {
                element,
                parent: semantic_parent,
                role,
                label: entry
                    .widget
                    .semantics
                    .explicit
                    .as_ref()
                    .and_then(|semantics| semantics.label.clone())
                    .or_else(|| entry.widget.semantics.label.clone())
                    .or(default_label),
                value,
                description: entry
                    .widget
                    .semantics
                    .description
                    .clone()
                    .or(explicit_description),
                bounds: self.semantic_bounds(entry.render, render.size),
                state,
                actions,
            });
            Some(element)
        } else {
            semantic_parent
        };
        // A Button deliberately merges its visual label/icon subtree into the
        // one control node. Other containers preserve logical child order.
        if !matches!(entry.widget.kind, WidgetKind::Button { .. })
            && !entry.widget.semantics.merge_descendants
        {
            let first_visible_child = entry
                .children
                .iter()
                .rposition(|child| {
                    self.elements
                        .get(child.0)
                        .is_some_and(|child| child.widget.semantics.block_previous_siblings)
                })
                .unwrap_or(0);
            let semantic_children: Vec<_> = match entry.widget.kind {
                WidgetKind::IndexedStack { index, .. } => {
                    entry.children.get(index).copied().into_iter().collect()
                }
                _ => entry.children[first_visible_child..].to_vec(),
            };
            for child in semantic_children {
                self.collect_semantics(child, this_parent, out);
            }
        }
    }
    #[must_use]
    pub fn paint(&mut self) -> DisplayList {
        let _phase_guard = self.guard_phase_root(FramePhase::Paint);
        let mut ignored = DisplayList::new();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.paint_render(root, &mut ignored);
        }
        self.compositor.flatten()
    }
    #[must_use]
    pub fn hit_test(&self, point: Offset) -> Option<RenderObjectId> {
        self.root
            .and_then(|id| self.render_id(id))
            .and_then(|id| self.hit_test_render(id, point, Offset::ZERO))
    }
    #[must_use]
    pub fn focusable_elements(&self) -> Vec<ElementId> {
        let mut members = Vec::new();
        let mut sequence = 0;
        if let Some(root) = self.root {
            if self
                .elements
                .get(root.0)
                .is_some_and(|element| element.widget.semantics.focus_traversal_policy.is_some())
            {
                self.collect_focus_group(
                    root,
                    FocusTraversalPolicyKind::WidgetOrder,
                    false,
                    &mut members,
                    &mut sequence,
                );
            } else {
                self.append_focus_members(
                    root,
                    FocusTraversalPolicyKind::WidgetOrder,
                    false,
                    &mut members,
                    &mut sequence,
                );
            }
        }
        let mut result = Vec::new();
        flatten_focus_group(
            FocusTraversalGroupMembers {
                policy: FocusTraversalPolicyKind::WidgetOrder,
                members,
            },
            &mut result,
        );
        result
    }

    /// Dispatches a platform keyboard event through the nearest retained
    /// keyboard listeners surrounding the focused element. The tree owns the
    /// ordering; the runtime remains responsible for text-editing fallthrough
    /// when no listener consumes the event.
    #[must_use]
    pub fn dispatch_keyboard(&self, focused: Option<ElementId>, event: KeyboardEvent) -> bool {
        let mut current = focused;
        while let Some(id) = current {
            if let Some(element) = self.elements.get(id.0)
                && let WidgetKind::Gesture { callbacks, .. } = &element.widget.kind
                && callbacks.has_keyboard_listener()
                && callbacks.handle_keyboard(event.clone())
            {
                return true;
            }
            current = self.parent(id);
        }
        false
    }

    /// Routes a native accessibility increment/decrement through the same
    /// keyboard listener used by a slider. This keeps a control's value logic
    /// in its existing callback rather than creating a second action path.
    #[must_use]
    pub fn dispatch_semantic_increment(&self, element: ElementId, increment: bool) -> bool {
        let code = if increment {
            incular_core::Code::ArrowRight
        } else {
            incular_core::Code::ArrowLeft
        };
        self.dispatch_keyboard(
            Some(element),
            KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), code),
        )
    }

    /// Finds the retained element associated with an externally managed focus
    /// node. This lets a `FocusScopeNode` request focus without coupling the
    /// gestures crate to runtime element IDs.
    #[must_use]
    pub fn focused_keyboard_element(&self) -> Option<ElementId> {
        self.focusable_elements().into_iter().find(|id| {
            self.elements
                .get(id.0)
                .and_then(|element| match &element.widget.kind {
                    WidgetKind::Gesture { callbacks, .. } => callbacks.focus_node.as_ref(),
                    _ => None,
                })
                .is_some_and(|node| node.has_focus())
        })
    }

    /// Returns the first listener that requested autofocus and can currently
    /// receive focus.
    #[must_use]
    pub fn autofocus_element(&self) -> Option<ElementId> {
        let focusable = self.focusable_elements();
        if let Some(listener) = focusable.iter().find(|id| {
            self.elements
                .get(id.0)
                .and_then(|element| match &element.widget.kind {
                    WidgetKind::Gesture { callbacks, .. } => callbacks
                        .focus_node
                        .as_ref()
                        .filter(|node| callbacks.autofocus && node.can_request_focus()),
                    _ => None,
                })
                .is_some()
        }) {
            return Some(*listener);
        }
        self.elements
            .iter()
            .filter(|(_, element)| element.widget.semantics.focus_scope_autofocus)
            .map(|(raw, _)| ElementId(raw))
            .find_map(|scope| {
                focusable
                    .iter()
                    .copied()
                    .find(|candidate| self.is_descendant_or_self(*candidate, scope))
            })
    }

    /// Mirrors runtime focus changes onto a listener's external focus node.
    pub fn set_keyboard_focus(&self, id: ElementId, focused: bool) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        let WidgetKind::Gesture { callbacks, .. } = &element.widget.kind else {
            return;
        };
        let Some(node) = callbacks.focus_node.as_ref() else {
            return;
        };
        if focused {
            node.request_focus();
        } else {
            node.unfocus();
        }
    }

    #[must_use]
    pub fn text_field_at(&self, point: Offset) -> Option<ElementId> {
        let hit = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))?;
        self.text_field_ancestor(hit)
    }
    pub fn set_focused(
        &mut self,
        id: ElementId,
        focused: bool,
        now: Instant,
    ) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live");
        if matches!(node.kind, RenderKind::Button { .. }) {
            node.button_focused = focused;
            node.button_state = if node.button_pressed {
                ButtonState::Pressed
            } else if node.button_focused {
                ButtonState::Focused
            } else if node.button_hovered {
                ButtonState::Hovered
            } else {
                ButtonState::Normal
            };
            node.dirty.insert(DirtyFlags::PAINT);
        }
        if matches!(node.kind, RenderKind::TextField { .. }) && node.focused != focused {
            node.focused = focused;
            node.dirty.insert(DirtyFlags::PAINT);
            if focused {
                if let RenderKind::TextField { controller, .. } = &node.kind {
                    controller.reset_caret(now);
                }
            }
        }
        if matches!(node.kind, RenderKind::SelectableText { .. }) && node.focused != focused {
            node.focused = focused;
            node.dirty.insert(DirtyFlags::PAINT);
        }
        Ok(())
    }
    #[must_use]
    pub fn is_text_field(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| matches!(element.widget.kind, WidgetKind::TextField { .. }))
    }

    /// Returns whether the field is currently allowed to mutate its
    /// controller through keyboard/IME input.  Read-only fields intentionally
    /// remain focusable so selection and copy still work, while disabled
    /// fields are not editable or focusable.
    #[must_use]
    pub fn text_field_is_editable(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    enabled: true,
                    read_only: false,
                    ..
                }
            )
        })
    }

    #[must_use]
    pub fn text_field_is_read_only(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    enabled: true,
                    read_only: true,
                    ..
                }
            )
        })
    }
    #[must_use]
    pub fn is_multiline_text_field(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    multiline: true,
                    ..
                }
            )
        })
    }
    pub fn text_field_move_vertical(&mut self, id: ElementId, down: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let (
            RenderKind::TextField {
                controller,
                multiline,
                ..
            },
            Some(layout),
        ) = (&node.kind, node.text_layout.clone())
        else {
            return false;
        };
        if !multiline {
            return false;
        }
        let current = controller.value().selection.extent;
        let line = line_for_byte(&layout, current);
        let target_line = if down {
            line.saturating_add(1)
        } else {
            line.saturating_sub(1)
        };
        if layout.lines.get(target_line).is_none() {
            return false;
        }
        let x = controller.preferred_caret_x().unwrap_or_else(|| {
            line_caret_x_for_affinity(&layout, line, current, controller.caret_affinity())
        });
        let (target, affinity) = caret_for_line_position(&layout, target_line, x);
        controller.move_cursor_with_affinity(target, affinity, extend);
        controller.set_preferred_caret_x(x);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }

    /// Moves a text-field caret by visual screen order. Logical UTF-8 order is
    /// not sufficient for bidi paragraphs because the next screen stop can
    /// have a smaller byte offset.
    pub fn text_field_move_horizontal(&mut self, id: ElementId, right: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let RenderKind::TextField { controller, .. } = &node.kind else {
            return false;
        };
        let Some(layout) = node.text_layout.clone() else {
            return false;
        };
        if right {
            controller.move_right_visual(&layout, extend);
        } else {
            controller.move_left_visual(&layout, extend);
        }
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    pub fn text_field_move_line_edge(&mut self, id: ElementId, end: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let (
            RenderKind::TextField {
                controller,
                multiline,
                ..
            },
            Some(layout),
        ) = (&node.kind, node.text_layout.clone())
        else {
            return false;
        };
        if !multiline {
            return false;
        }
        let line = line_for_byte(&layout, controller.value().selection.extent);
        let target = if end {
            layout.lines[line].caret_end
        } else {
            layout.lines[line].start
        };
        let affinity = if end {
            incular_text::TextAffinity::Upstream
        } else {
            incular_text::TextAffinity::Downstream
        };
        controller.move_cursor_with_affinity(target, affinity, extend);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    pub fn text_field_set_caret(
        &mut self,
        id: ElementId,
        point: Offset,
        extend: bool,
        now: Instant,
    ) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let (layout, scroll_x, scroll_y, multiline, controller) =
            match self.renders.get(render.0).map(|node| {
                (
                    &node.kind,
                    node.text_layout.clone(),
                    node.text_scroll_x,
                    node.text_scroll_y,
                )
            }) {
                Some((
                    RenderKind::TextField {
                        controller,
                        multiline,
                        ..
                    },
                    layout,
                    scroll_x,
                    scroll_y,
                )) => (layout, scroll_x, scroll_y, *multiline, controller.clone()),
                _ => return false,
            };
        let local = self
            .render_world_transform(render)
            .inverse_transform_point(point)
            .unwrap_or(point);
        let x = local.x - 8. + scroll_x;
        let y = local.y - if multiline { 8. } else { 0. } + scroll_y;
        let text_len = controller.text().len();
        let (index, affinity) =
            layout
                .as_ref()
                .map_or((0, incular_text::TextAffinity::Downstream), |layout| {
                    let line_index = (y / layout.metrics.line_height).floor().max(0.) as usize;
                    let line_index = line_index.min(layout.lines.len().saturating_sub(1));
                    layout
                        .lines
                        .get(line_index)
                        .map_or((0, incular_text::TextAffinity::Downstream), |_| {
                            caret_for_line_position(layout, line_index, x)
                        })
                });
        let index = index.min(text_len);
        let previous = controller.value().selection;
        controller.set_selection_with_affinity(
            if extend {
                TextSelection {
                    base: previous.base,
                    extent: index,
                }
            } else {
                TextSelection::collapsed(index)
            },
            affinity,
        );
        controller.reset_caret(now);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    /// Returns the selectable label under `point`, if it belongs to a selection area.
    #[must_use]
    pub fn selectable_text_at(&self, point: Offset) -> Option<ElementId> {
        let hit = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))?;
        self.selectable_text_ancestor(hit)
    }
    #[must_use]
    pub fn is_selectable_text(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| matches!(element.widget.kind, WidgetKind::SelectableText { .. }))
    }
    /// Starts or extends read-only selection from a pointer position. The
    /// caret index is recovered from the cached Parley-backed layout; changing
    /// selection therefore never reshapes or rasterizes the text.
    pub fn selectable_text_set_selection(
        &mut self,
        id: ElementId,
        point: Offset,
        extend: bool,
    ) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(layout) = self
            .renders
            .get(render.0)
            .and_then(|node| node.text_layout.clone())
        else {
            return false;
        };
        let local = self
            .render_world_transform(render)
            .inverse_transform_point(point)
            .unwrap_or(point);
        let line = layout
            .lines
            .get((local.y / layout.metrics.line_height).floor().max(0.) as usize)
            .or_else(|| layout.lines.last());
        let byte = line.map_or(0, |line| caret_for_line_x(line, local.x));
        let point = StaticSelectionPoint { element: id, byte };
        if extend
            && self
                .static_selection
                .is_some_and(|selection| selection.area != area)
        {
            return false;
        }
        let anchor = if extend {
            self.static_selection
                .filter(|selection| selection.area == area)
                .map(|selection| selection.anchor)
                .unwrap_or(point)
        } else {
            point
        };
        self.static_selection = Some(StaticSelection {
            area,
            anchor,
            extent: point,
        });
        self.sync_static_selection(area);
        true
    }
    /// Moves the active static-text selection by one grapheme cluster. Only
    /// selection state changes; there is no editable caret or IME route.
    pub fn selectable_text_move(&mut self, id: ElementId, right: bool, extend: bool) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let Some(text) = self.selectable_text_value(id) else {
            return false;
        };
        let current = self
            .static_selection
            .filter(|selection| selection.area == area && selection.extent.element == id)
            .map(|selection| selection.extent.byte)
            .unwrap_or(0);
        let byte = if right {
            next_grapheme_boundary(&text, current)
        } else {
            previous_grapheme_boundary(&text, current)
        };
        let point = StaticSelectionPoint { element: id, byte };
        let anchor = if extend {
            self.static_selection
                .filter(|selection| selection.area == area)
                .map(|selection| selection.anchor)
                .unwrap_or(point)
        } else {
            point
        };
        self.static_selection = Some(StaticSelection {
            area,
            anchor,
            extent: point,
        });
        self.sync_static_selection(area);
        true
    }
    pub fn selectable_text_move_to_edge(&mut self, id: ElementId, end: bool, extend: bool) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let Some(text) = self.selectable_text_value(id) else {
            return false;
        };
        let point = StaticSelectionPoint {
            element: id,
            byte: if end { text.len() } else { 0 },
        };
        let anchor = if extend {
            self.static_selection
                .filter(|selection| selection.area == area)
                .map(|selection| selection.anchor)
                .unwrap_or(point)
        } else {
            point
        };
        self.static_selection = Some(StaticSelection {
            area,
            anchor,
            extent: point,
        });
        self.sync_static_selection(area);
        true
    }
    pub fn selectable_text_select_all(&mut self, id: ElementId) -> bool {
        let Some(area) = self.selection_area_ancestor(id) else {
            return false;
        };
        let entries = self.selectable_texts_in_area(area);
        let Some(first) = entries.first().copied() else {
            return false;
        };
        let Some(last) = entries.last().copied() else {
            return false;
        };
        let last_len = self
            .selectable_text_value(last)
            .map_or(0, |text| text.len());
        self.static_selection = Some(StaticSelection {
            area,
            anchor: StaticSelectionPoint {
                element: first,
                byte: 0,
            },
            extent: StaticSelectionPoint {
                element: last,
                byte: last_len,
            },
        });
        self.sync_static_selection(area);
        true
    }
    #[must_use]
    pub fn selectable_text_selected_text(&self, id: ElementId) -> Option<String> {
        let area = self.selection_area_ancestor(id)?;
        if let Some(controller) = self.selection_area_controller(area) {
            return Some(controller.selected_text());
        }
        let selection = self
            .static_selection
            .filter(|selection| selection.area == area)?;
        let entries = self.selectable_texts_in_area(area);
        Some(static_selection_text(self, &entries, selection))
    }
    fn selectable_text_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.elements.get(id.0).is_some_and(|element| {
                matches!(element.widget.kind, WidgetKind::SelectableText { .. })
            }) {
                return Some(id);
            }
            id = self.parent(id)?;
        }
    }
    fn selection_area_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        let selectable = id;
        loop {
            if self.elements.get(id.0).is_some_and(|element| {
                matches!(element.widget.kind, WidgetKind::SelectionArea { .. })
            }) {
                return Some(id);
            }
            let Some(parent) = self.parent(id) else {
                // A standalone SelectableText is its own one-label region.
                return self
                    .elements
                    .get(selectable.0)
                    .is_some_and(|element| {
                        matches!(element.widget.kind, WidgetKind::SelectableText { .. })
                    })
                    .then_some(selectable);
            };
            id = parent;
        }
    }
    fn selection_area_controller(&self, id: ElementId) -> Option<SelectionAreaController> {
        let WidgetKind::SelectionArea { controller, .. } = &self.elements.get(id.0)?.widget.kind
        else {
            return None;
        };
        Some(controller.clone())
    }
    fn selectable_text_value(&self, id: ElementId) -> Option<String> {
        let WidgetKind::SelectableText { text, .. } = &self.elements.get(id.0)?.widget.kind else {
            return None;
        };
        Some(text.clone())
    }
    fn selectable_texts_in_area(&self, area: ElementId) -> Vec<ElementId> {
        let mut entries = Vec::new();
        self.collect_selectable_texts(area, &mut entries);
        entries
    }
    fn collect_selectable_texts(&self, id: ElementId, entries: &mut Vec<ElementId>) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        if matches!(element.widget.kind, WidgetKind::SelectableText { .. }) {
            entries.push(id);
        }
        for child in &element.children {
            self.collect_selectable_texts(*child, entries);
        }
    }
    fn sync_static_selection(&mut self, area: ElementId) {
        let entries = self.selectable_texts_in_area(area);
        let Some(selection) = self
            .static_selection
            .filter(|selection| selection.area == area)
        else {
            return;
        };
        let selected = static_selection_text(self, &entries, selection);
        if let Some(controller) = self.selection_area_controller(area) {
            controller.set_selected_text(selected);
        }
        for element in entries {
            if let Some(render) = self.render_id(element) {
                self.renders
                    .get_mut(render.0)
                    .expect("live selectable text")
                    .dirty
                    .insert(DirtyFlags::PAINT);
            }
        }
    }
    fn static_selection_range(&self, element: ElementId) -> Option<TextRange> {
        let selection = self.static_selection?;
        let entries = self.selectable_texts_in_area(selection.area);
        static_selection_range(self, &entries, selection, element)
    }
    #[must_use]
    pub fn text_controller(&self, id: ElementId) -> Option<TextEditingController> {
        let render = self.render_id(id)?;
        match &self.renders.get(render.0)?.kind {
            RenderKind::TextField { controller, .. } => Some(controller.clone()),
            _ => None,
        }
    }

    /// Returns the nearest [`UndoHistory`](crate::UndoHistory) capacity that
    /// wraps this text field. The value is kept on widget metadata so the
    /// renderer-neutral Widgets crate does not depend on Runtime.
    #[must_use]
    pub fn text_field_history_max_entries(&self, mut id: ElementId) -> Option<usize> {
        loop {
            let element = self.elements.get(id.0)?;
            if let Some(max_entries) = element.widget.semantics.undo_history_max_entries {
                return Some(max_entries);
            }
            id = element.parent?;
        }
    }

    /// Captures the current text-input client state after layout. The runtime
    /// turns this into a platform command; native window handles never enter
    /// the retained tree.
    #[must_use]
    pub fn text_field_input_snapshot(&self, id: ElementId) -> Option<TextFieldInputSnapshot> {
        let element = self.elements.get(id.0)?;
        let WidgetKind::TextField {
            controller,
            multiline,
            enabled,
            read_only,
            obscure_text,
            ..
        } = &element.widget.kind
        else {
            return None;
        };
        let bounds = self.element_bounds(id).unwrap_or_default();
        let caret_rect = self
            .render_id(id)
            .and_then(|render| self.renders.get(render.0).map(|node| (render, node)))
            .and_then(|(render, node)| {
                let layout = node.text_layout.as_ref()?;
                let (x, y, height) = caret_geometry(
                    layout,
                    controller.selection().extent,
                    controller.caret_affinity(),
                );
                let top = if *multiline {
                    8.0
                } else {
                    ((node.size.height - height) / 2.0).max(0.0)
                };
                Some(self.render_world_transform(render).transform_rect_bbox(
                    Rect::from_origin_size(
                        Offset::new(x - node.text_scroll_x + 8.0, y - node.text_scroll_y + top),
                        Size::new(1.0, height.max(1.0)),
                    ),
                ))
            })
            .unwrap_or(bounds);
        Some(TextFieldInputSnapshot {
            client_id: (u64::from(id.0.generation()) << 32) | u64::from(id.0.index()),
            text: controller.text(),
            selection: controller.selection(),
            composing: controller.composing(),
            multiline: *multiline,
            enabled: *enabled,
            read_only: *read_only,
            obscure_text: *obscure_text,
            input_type: element.widget.semantics.text_input_type.unwrap_or_default(),
            input_action: element
                .widget
                .semantics
                .text_input_action
                .unwrap_or_default(),
            bounds,
            caret_rect,
        })
    }
    pub fn submit_text_field(&self, id: ElementId) -> bool {
        let Some(element) = self.elements.get(id.0) else {
            return false;
        };
        let WidgetKind::TextField {
            controller,
            on_submit,
            ..
        } = &element.widget.kind
        else {
            return false;
        };
        if let Some(callback) = on_submit {
            callback(controller.text());
        }
        true
    }

    fn mount_element(
        &mut self,
        parent: Option<ElementId>,
        widget: Widget,
    ) -> Result<ElementId, TreeError> {
        enum MountWork {
            Create {
                parent: Option<ElementId>,
                widget: Box<Widget>,
            },
            Finish(ElementId),
        }

        let mut work = vec![MountWork::Create {
            parent,
            widget: Box::new(widget),
        }];
        let mut root = None;
        while let Some(next) = work.pop() {
            match next {
                MountWork::Create { parent, widget } => {
                    let children = widget
                        .children_refs()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    let id = self.mount_element_node(parent, *widget)?;
                    if root.is_none() {
                        root = Some(id);
                    }
                    if let Some(parent) = parent {
                        self.elements
                            .get_mut(parent.0)
                            .expect("mount parent remains live")
                            .children
                            .push(id);
                    }
                    work.push(MountWork::Finish(id));
                    for child in children.into_iter().rev() {
                        work.push(MountWork::Create {
                            parent: Some(id),
                            widget: Box::new(child),
                        });
                    }
                }
                MountWork::Finish(id) => {
                    self.sync_render_children(id);
                    if self
                        .elements
                        .get(id.0)
                        .is_some_and(|element| element.parent.is_none())
                    {
                        let layer = self
                            .renders
                            .get(self.render_id(id).expect("mounted").0)
                            .expect("mounted render")
                            .layer;
                        self.compositor.set_root(layer);
                    }
                }
            }
        }
        Ok(root.expect("mount work always contains the root widget"))
    }

    fn mount_element_node(
        &mut self,
        parent: Option<ElementId>,
        widget: Widget,
    ) -> Result<ElementId, TreeError> {
        let inherited_environment = parent
            .and_then(|id| self.elements.get(id.0))
            .and_then(|element| element.environment.clone());
        let environment_override = match &widget.kind {
            WidgetKind::LayoutBuilder { environment, .. } => environment.clone(),
            _ => None,
        };
        let environment = compose_environment(environment_override.clone(), inherited_environment);
        self.check_keys_borrowed(widget.children_refs())?;
        let layer = self
            .compositor
            .create_transform(CoreTransform::translation(Offset::ZERO));
        let picture = matches!(
            widget.kind,
            WidgetKind::Box { .. }
                | WidgetKind::Shape { .. }
                | WidgetKind::CustomPaint { .. }
                | WidgetKind::RepaintBoundary { .. }
                | WidgetKind::Decorated { .. }
                | WidgetKind::Button { .. }
                | WidgetKind::Text { .. }
                | WidgetKind::SelectableText { .. }
                | WidgetKind::TextField { .. }
                | WidgetKind::Image { .. }
                | WidgetKind::Scroll { .. }
                | WidgetKind::SliverViewport { .. }
        )
        .then(|| {
            self.compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )
        });
        let focus_picture = match &widget.kind {
            WidgetKind::Button {
                color,
                focused_color: Some(_),
                ..
            } if color.alpha == 0 => Some(self.compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )),
            _ => None,
        };
        let (
            clip_layer,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
        ) = match &widget.kind {
            WidgetKind::Scroll { .. } | WidgetKind::SliverViewport { .. } => {
                let clip = self
                    .compositor
                    .create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                let content = self
                    .compositor
                    .create_transform(CoreTransform::translation(Offset::ZERO));
                self.compositor
                    .set_children(layer, std::iter::once(clip).chain(picture).collect());
                self.compositor.set_children(clip, vec![content]);
                (Some(clip), Some(content), None, None, None, None, None)
            }
            WidgetKind::PersistentHeader { .. } => {
                // Keep the pinning transform below static flow placement. The
                // header still occupies its normal sliver extent in layout.
                let content = self
                    .compositor
                    .create_transform(CoreTransform::translation(Offset::ZERO));
                self.compositor.set_children(layer, vec![content]);
                (None, Some(content), None, None, None, None, None)
            }
            WidgetKind::Translate { .. } => {
                // Keep dynamic movement structurally below static layout
                // placement: parent -> layout transform -> animated transform
                // -> normal child tree. This is also what lets animation avoid
                // repainting local picture commands.
                let content = self
                    .compositor
                    .create_transform(CoreTransform::translation(Offset::ZERO));
                self.compositor.set_children(layer, vec![content]);
                (None, Some(content), None, None, None, None, None)
            }
            WidgetKind::Transform { .. }
            | WidgetKind::Scale { .. }
            | WidgetKind::Rotation { .. }
            | WidgetKind::FittedBox { .. } => {
                let content = self.compositor.create_transform(CoreTransform::IDENTITY);
                self.compositor.set_children(layer, vec![content]);
                (None, Some(content), None, None, None, None, None)
            }
            WidgetKind::Opacity { alpha, .. } => {
                let opacity = self.compositor.create_opacity(*alpha);
                self.compositor.set_children(layer, vec![opacity]);
                (None, None, Some(opacity), None, None, None, None)
            }
            WidgetKind::Blur {
                sigma_x, sigma_y, ..
            } => {
                let blur = self
                    .compositor
                    .create_blur(GaussianBlur::new(*sigma_x, *sigma_y));
                self.compositor.set_children(layer, vec![blur]);
                (None, None, None, Some(blur), None, None, None)
            }
            WidgetKind::DropShadow {
                offset,
                sigma_x,
                sigma_y,
                color,
                ..
            } => {
                let shadow = self
                    .compositor
                    .create_drop_shadow(DropShadowEffect::asymmetric(
                        *offset, *sigma_x, *sigma_y, *color,
                    ));
                self.compositor.set_children(layer, vec![shadow]);
                (None, None, None, None, Some(shadow), None, None)
            }
            WidgetKind::ColorFiltered { filter, .. } => {
                let color_filter = self.compositor.create_color_filter(*filter);
                self.compositor.set_children(layer, vec![color_filter]);
                (None, None, None, None, None, Some(color_filter), None)
            }
            WidgetKind::Blend { mode, .. } => {
                let blend = self.compositor.create_blend(*mode);
                self.compositor.set_children(layer, vec![blend]);
                (None, None, None, None, None, None, Some(blend))
            }
            _ => {
                let mut layers = picture.into_iter().collect::<Vec<_>>();
                layers.extend(focus_picture);
                self.compositor.set_children(layer, layers);
                (None, None, None, None, None, None, None)
            }
        };
        let render = self.renders.insert(RenderObject {
            parent: None,
            children: Vec::new(),
            kind: render_kind(&widget, environment.as_ref()),
            size: Size::ZERO,
            offset: Offset::ZERO,
            constraints: None,
            dirty: DirtyFlags::LAYOUT | DirtyFlags::PAINT,
            cache: DisplayList::new(),
            text_layout: None,
            text_revision: 0,
            text_visual_revision: 0,
            text_scroll_x: 0.,
            text_scroll_y: 0.,
            scrollbar_hovered: false,
            scrollbar_dragging: false,
            focused: false,
            baseline: None,
            button_state: ButtonState::Normal,
            button_hovered: false,
            button_pressed: false,
            button_focused: false,
            layer,
            picture,
            focus_picture,
            clip_layer,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
        });
        let id = ElementId(self.elements.insert(Element {
            parent,
            children: Vec::new(),
            widget: widget.clone(),
            render: RenderObjectId(render),
            dirty: DirtyFlags::NONE,
            sliver_child_ids: Vec::new(),
            sliver_pinned_ids: HashSet::new(),
            notification_subscriptions: Vec::new(),
            sliver_delegate_revision: 0,
            sliver_scroll_revision: 0,
            layout_builder_constraints: None,
            layout_builder_revision: 0,
            environment,
            environment_override,
            #[cfg(feature = "devtools")]
            dev: ElementDevData::default(),
        }));
        self.elements.get_mut(id.0).expect("fresh element").children = Vec::new();
        self.diagnostics.mounts += 1;
        Ok(id)
    }

    fn propagate_environment(&mut self, id: ElementId) {
        let (inherited, children) = {
            let element = self.elements.get(id.0).expect("live environment element");
            (element.environment.clone(), element.children.clone())
        };
        for child in children {
            let (override_value, previous, widget, render) = {
                let element = self.elements.get(child.0).expect("live child");
                (
                    element.environment_override.clone(),
                    element.environment.clone(),
                    element.widget.clone(),
                    element.render,
                )
            };
            let effective = compose_environment(override_value, inherited.clone());
            let changed = match (&previous, &effective) {
                (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
                (None, None) => false,
                _ => true,
            };
            if changed {
                let new_kind = render_kind(&widget, effective.as_ref());
                self.renders
                    .get_mut(render.0)
                    .expect("live child render")
                    .kind = new_kind;
                {
                    let element = self.elements.get_mut(child.0).expect("live child");
                    element.environment = effective;
                    element.layout_builder_constraints = None;
                    element.layout_builder_revision = 0;
                }
                self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
            }
            self.propagate_environment(child);
        }
    }

    fn update_existing(&mut self, id: ElementId, widget: &Widget) -> Result<(), TreeError> {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.update_existing_inner(id, widget)
        })
    }

    fn update_existing_inner(&mut self, id: ElementId, widget: &Widget) -> Result<(), TreeError> {
        let _node_guard = self.guard_element(FramePhase::Build, id);
        // Borrow-compare first: the unchanged-subtree bailout must not clone
        // either widget. This is the dominant hot path for wide static trees
        // under a rebuilding parent (Task 15 measured bottleneck).
        if self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?
            .widget
            == *widget
        {
            self.diagnostics.identical_child_bailouts += 1;
            return Ok(());
        }
        #[cfg(feature = "devtools")]
        let trace = self.devtools_trace_begin_element(id, TracePhase::Build);
        let old = self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?
            .widget
            .clone();
        let old_environment = self
            .elements
            .get(id.0)
            .and_then(|element| element.environment.clone());
        let new_override = match &widget.kind {
            WidgetKind::LayoutBuilder { environment, .. } => environment.clone(),
            _ => None,
        };
        let parent_environment = self
            .elements
            .get(id.0)
            .and_then(|element| element.parent)
            .and_then(|parent| self.elements.get(parent.0))
            .and_then(|parent| parent.environment.clone());
        let new_environment = compose_environment(new_override.clone(), parent_environment.clone());
        let environment_changed = match (&old_environment, &new_environment) {
            (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        };
        #[cfg(feature = "devtools")]
        let property_changes = crate::devtools_props::diff_properties(&old.kind, &widget.kind);
        debug_assert_eq!(
            old.type_(),
            widget.type_(),
            "only compatible elements may update"
        );
        self.check_keys_borrowed(widget.children_refs())?;
        let render = self.elements.get(id.0).expect("present").render;
        let old_kind = render_kind(&old, old_environment.as_ref());
        let new_kind = render_kind(widget, new_environment.as_ref());
        carry_replaced_transition(&old_kind, &new_kind);
        #[cfg(feature = "devtools")]
        let mut work_reasons: (Option<String>, Option<String>, Option<String>) = (None, None, None);
        if old_kind != new_kind {
            let opacity_only = opacity_composite_only_change(&old_kind, &new_kind);
            let effect_only = effect_composite_only_change(&old_kind, &new_kind);
            let affine_only = affine_composite_only_change(&old_kind, &new_kind);
            self.renders.get_mut(render.0).expect("present").kind = new_kind.clone();
            if opacity_only {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.2 = Some("retained opacity property changed".into());
                }
                // Alpha is consumed by the retained compositor layer. Keep
                // paint/layout caches warm for opacity-only rebuilds.
                if let RenderKind::Opacity { alpha, .. } = new_kind {
                    if let Some(layer) = self.renders.get(render.0).and_then(|n| n.opacity_layer) {
                        self.compositor.update_opacity(layer, alpha);
                    }
                }
            } else if effect_only {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.2 = Some("retained effect property changed".into());
                }
                match new_kind {
                    RenderKind::Blur {
                        sigma_x, sigma_y, ..
                    } => {
                        if let Some(layer) = self.renders.get(render.0).and_then(|n| n.blur_layer) {
                            self.compositor
                                .update_blur(layer, GaussianBlur::new(sigma_x, sigma_y));
                        }
                    }
                    RenderKind::DropShadow {
                        offset,
                        sigma_x,
                        sigma_y,
                        color,
                        ..
                    } => {
                        if let Some(layer) = self.renders.get(render.0).and_then(|n| n.shadow_layer)
                        {
                            self.compositor.update_drop_shadow(
                                layer,
                                DropShadowEffect::asymmetric(offset, sigma_x, sigma_y, color),
                            );
                        }
                    }
                    RenderKind::ColorFiltered { filter, .. } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.color_filter_layer)
                        {
                            self.compositor.update_color_filter(layer, filter);
                        }
                    }
                    RenderKind::Blend { mode } => {
                        if let Some(layer) =
                            self.renders.get(render.0).and_then(|node| node.blend_layer)
                        {
                            self.compositor.update_blend(layer, mode);
                        }
                    }
                    _ => {}
                }
            } else if affine_only {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.2 = Some("retained affine transform changed".into());
                }
                if let (Some(layer), Some(transform)) = (
                    self.renders
                        .get(render.0)
                        .and_then(|node| node.content_layer),
                    self.content_transform(render),
                ) {
                    self.compositor.update_transform(layer, transform);
                }
            } else if text_paint_only_change(&old_kind, &new_kind)
                || custom_paint_only_change(&old_kind, &new_kind)
            {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.1 = Some("paint-only configuration changed".into());
                }
                self.mark_render_dirty(render, DirtyFlags::PAINT, false);
            } else {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.0 = Some("layout-affecting configuration changed".into());
                    work_reasons.1 = Some("layout result invalidated paint".into());
                }
                self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
            }
        }
        {
            let element = self.elements.get_mut(id.0).expect("present");
            element.widget = widget.clone();
            element.environment_override = new_override;
            element.environment = new_environment;
            if environment_changed {
                element.layout_builder_constraints = None;
            }
            element.dirty.remove(DirtyFlags::BUILD);
        }
        if environment_changed {
            self.propagate_environment(id);
        }
        if matches!(widget.kind, WidgetKind::LayoutBuilder { .. }) {
            // A new descriptor may carry a different builder closure while
            // retaining the same constraints/revision value. Force one
            // materialization so updates cannot leave the old child mounted.
            if let Some(element) = self.elements.get_mut(id.0) {
                element.layout_builder_constraints = None;
                element.layout_builder_revision = 0;
            }
        }
        self.diagnostics.rebuilds += 1;
        #[cfg(feature = "devtools")]
        {
            let element = self.elements.get_mut(id.0).expect("present");
            element.dev.builds += 1;
            element.dev.revision += 1;
            element.dev.property_changes = property_changes;
            element.dev.layout_reason = work_reasons.0;
            element.dev.paint_reason = work_reasons.1;
            element.dev.composite_reason = work_reasons.2;
        }
        // Lazy children are owned by the viewport's indexed materialization
        // map, not by `Widget::children()`. Recreating a sliver viewport
        // description must preserve every still-valid mounted child; the
        // next layout pass will add/drop only indices required by the delegate.
        if matches!(widget.kind, WidgetKind::SliverViewport { .. }) {
            #[cfg(feature = "devtools")]
            self.devtools_trace_end(trace);
            return Ok(());
        }
        if matches!(widget.kind, WidgetKind::LayoutBuilder { .. }) {
            #[cfg(feature = "devtools")]
            self.devtools_trace_end(trace);
            return Ok(());
        }
        let previous = self.elements.get(id.0).expect("present").children.clone();
        let desired = widget.children_refs();
        let reconciled = self.reconcile_children(id, previous.clone(), &desired)?;
        if reconciled != previous {
            self.elements.get_mut(id.0).expect("present").children = reconciled;
            self.sync_render_children(id);
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        Ok(())
    }
    fn compatible(&mut self, id: ElementId, widget: &Widget) -> bool {
        match self.elements.get(id.0) {
            Some(element) => {
                self.diagnostics.widget_type_comparisons += 1;
                element.widget.type_() == widget.type_() && {
                    self.diagnostics.key_comparisons += 1;
                    element.widget.key == widget.key
                }
            }
            None => false,
        }
    }
    fn reconcile_children(
        &mut self,
        parent: ElementId,
        previous: Vec<ElementId>,
        desired: &[&Widget],
    ) -> Result<Vec<ElementId>, TreeError> {
        self.check_keys_borrowed(desired.iter().copied())?;
        self.diagnostics.child_list_scans += 1;
        let mut start = 0;
        let mut old_end = previous.len();
        let mut new_end = desired.len();
        let mut next = Vec::with_capacity(new_end);
        while start < old_end && start < new_end && self.compatible(previous[start], desired[start])
        {
            let id = previous[start];
            self.update_existing(id, desired[start])?;
            next.push(id);
            start += 1;
            self.diagnostics.reconciliation_fast_paths += 1;
            self.diagnostics.elements_reused += 1;
        }
        while start < old_end
            && start < new_end
            && self.compatible(previous[old_end - 1], desired[new_end - 1])
        {
            old_end -= 1;
            new_end -= 1;
            self.diagnostics.reconciliation_fast_paths += 1;
            self.diagnostics.elements_reused += 1;
        }
        let old_middle = &previous[start..old_end];
        let desired_middle = &desired[start..new_end];
        // Track original positions to detect genuine moves (Task 15 counter).
        let mut keyed: HashMap<Key, (ElementId, usize)> = HashMap::new();
        let mut unkeyed_ids: Vec<(ElementId, usize)> = Vec::new();
        let any_keys = old_middle.iter().any(|id| {
            self.elements
                .get(id.0)
                .is_some_and(|e| e.widget.key.is_some())
        }) || desired_middle.iter().any(|w| w.key.is_some());
        if any_keys {
            self.diagnostics.key_maps_built += 1;
            for (position, id) in old_middle.iter().enumerate() {
                if let Some(key) = self.elements.get(id.0).and_then(|e| e.widget.key.clone()) {
                    keyed.insert(key, (*id, position));
                    self.diagnostics.key_map_entries += 1;
                } else {
                    unkeyed_ids.push((*id, position));
                }
            }
        } else {
            for (position, id) in old_middle.iter().enumerate() {
                unkeyed_ids.push((*id, position));
            }
        }
        let mut unkeyed = unkeyed_ids.into_iter();
        let mut used = std::collections::HashSet::new();
        for widget in desired_middle {
            let candidate = if let Some(key) = widget.key() {
                self.diagnostics.key_lookups += 1;
                keyed.get(key).copied()
            } else {
                unkeyed.next()
            };
            self.diagnostics.widget_type_comparisons += 1;
            if let Some((id, old_position)) = candidate.filter(|(id, _)| {
                self.elements
                    .get(id.0)
                    .is_some_and(|e| e.widget.type_() == widget.type_())
            }) {
                self.update_existing(id, widget)?;
                used.insert(id);
                // A reused child is "moved" when its previous middle position
                // differs from the position it is emitted at now.
                let emitted_index = next.len();
                let expected_index = start + old_position;
                if emitted_index != expected_index {
                    self.diagnostics.elements_moved += 1;
                }
                next.push(id);
                self.diagnostics.elements_reused += 1;
            } else {
                self.diagnostics.elements_created += 1;
                next.push(self.mount_element(Some(parent), (*widget).clone())?);
            }
        }
        for id in old_middle {
            if !used.contains(id) {
                self.unmount_element(*id);
                self.diagnostics.elements_removed += 1;
            }
        }
        let mut suffix = Vec::new();
        for index in new_end..desired.len() {
            let id = previous[old_end + (index - new_end)];
            self.update_existing(id, desired[index])?;
            suffix.push(id);
            self.diagnostics.elements_reused += 1;
        }
        next.extend(suffix);
        Ok(next)
    }
    /// Reconciles the indexed child window produced by the retained sliver
    /// protocol. Unlike a box list, each child carries a viewport-scoped
    /// identity and an explicit sliver placement/constraint record.
    fn materialize_sliver_children(
        &mut self,
        id: RenderObjectId,
        config: &SliverViewportConfig,
        layout: &SliverViewportLayout,
    ) {
        let element_id = self
            .element_for_render(id)
            .expect("sliver viewport element");
        let (old_ids, old_children) = {
            let element = self
                .elements
                .get(element_id.0)
                .expect("sliver viewport element");
            (element.sliver_child_ids.clone(), element.children.clone())
        };
        let existing = old_ids
            .into_iter()
            .zip(old_children.iter().copied())
            .collect::<HashMap<_, _>>();
        let mut next_ids = Vec::with_capacity(layout.children.len());
        let mut next_children = Vec::with_capacity(layout.children.len());
        let mut pinned = HashSet::new();
        for child in &layout.children {
            let child_id = child.id;
            let retained = if let Some(existing) = existing.get(&child_id).copied() {
                self.update_existing(existing, &child.widget)
                    .expect("sliver child update must remain valid");
                existing
            } else {
                let mut widget = child.widget.clone();
                let handlers = &mut self.pending_handlers;
                let next = &mut self.next_action;
                widget.bind_callbacks(&mut |callback| {
                    let action = ActionId(*next);
                    *next += 1;
                    handlers.push((action, callback));
                    action
                });
                self.diagnostics.items_built += 1;
                match self.mount_element(Some(element_id), widget) {
                    Ok(child) => {
                        self.diagnostics.items_mounted += 1;
                        child
                    }
                    Err(error) => panic!("sliver child {child_id:?} could not mount: {error:?}"),
                }
            };
            if child.pinned {
                pinned.insert(child_id);
            }
            next_ids.push(child_id);
            next_children.push(retained);
        }
        let retained = next_children.iter().copied().collect::<HashSet<_>>();
        for child in old_children {
            if !retained.contains(&child) {
                self.unmount_element(child);
                self.diagnostics.items_unmounted += 1;
            } else {
                self.diagnostics.items_reused += 1;
            }
        }
        let element = self
            .elements
            .get_mut(element_id.0)
            .expect("sliver viewport element");
        element.children = next_children;
        element.sliver_child_ids = next_ids;
        element.sliver_pinned_ids = pinned;
        element.sliver_delegate_revision = config.delegate.revision();
        element.sliver_scroll_revision = config.controller.revision();
        self.sync_render_children(element_id);
    }

    fn materialize_layout_builder(&mut self, id: RenderObjectId, constraints: Constraints) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.materialize_layout_builder_inner(id, constraints);
        });
    }

    fn materialize_layout_builder_inner(&mut self, id: RenderObjectId, constraints: Constraints) {
        let element_id = self.element_for_render(id).expect("layout builder element");
        let _build_guard = self.guard_element(FramePhase::Build, element_id);
        let (builder, revision, previous_constraints, previous_revision, previous_children) = {
            let element = self
                .elements
                .get(element_id.0)
                .expect("layout builder element");
            let WidgetKind::LayoutBuilder {
                builder,
                environment: _,
                revision,
            } = &element.widget.kind
            else {
                return;
            };
            (
                builder.clone(),
                revision.clone(),
                element.layout_builder_constraints,
                element.layout_builder_revision,
                element.children.clone(),
            )
        };
        let revision_value = revision.as_ref().map_or(0, |revision| revision.get());
        if previous_constraints == Some(constraints)
            && previous_revision == revision_value
            && previous_children.len() == 1
        {
            return;
        }
        let environment = self
            .elements
            .get(element_id.0)
            .and_then(|element| element.environment.clone());
        let mut child = with_build_environment(environment, || builder(constraints));
        let handlers = &mut self.pending_handlers;
        let next = &mut self.next_action;
        child.bind_callbacks(&mut |callback| {
            let action = ActionId(*next);
            *next += 1;
            handlers.push((action, callback));
            action
        });
        let children = self
            .reconcile_children(element_id, previous_children, &[&child])
            .expect("layout builder child must have unique sibling keys");
        let element = self
            .elements
            .get_mut(element_id.0)
            .expect("layout builder element");
        element.children = children;
        element.layout_builder_constraints = Some(constraints);
        element.layout_builder_revision = revision_value;
        self.sync_render_children(element_id);
    }
    /// Returns whether the current retained sliver children still cover the
    /// viewport's requested cache window. A small scroll can therefore stay a
    /// compositor-only transform; layout is needed only when a cache boundary
    /// is crossed or the delegate itself changed.
    fn sliver_cache_window_is_covered(
        &self,
        render_id: RenderObjectId,
        element: &Element,
        config: &SliverViewportConfig,
    ) -> bool {
        let Some(render) = self.renders.get(render_id.0) else {
            return false;
        };
        let viewport = config.axis.main_extent(render.size).max(0.);
        let cache = config.cache_extent.max(0.);
        let content_end = (config.controller.max_offset() + viewport).max(0.);
        let physical =
            physical_scroll_offset(&config.controller, config.reverse).clamp(0., content_end);
        if !physical.is_finite()
            || !viewport.is_finite()
            || !cache.is_finite()
            || !content_end.is_finite()
        {
            return false;
        }
        let wanted_start = (physical - cache).max(0.);
        let wanted_end = (physical + viewport + cache)
            .min(content_end)
            .max(wanted_start);
        let mut materialized_start = f32::INFINITY;
        let mut materialized_end = f32::NEG_INFINITY;
        for child in &element.children {
            let Some(child_element) = self.elements.get(child.0) else {
                return false;
            };
            let Some(child_render) = self.renders.get(child_element.render.0) else {
                return false;
            };
            let child_start = match config.axis {
                Axis::Horizontal => child_render.offset.x,
                Axis::Vertical => child_render.offset.y,
            };
            let child_extent = config.axis.main_extent(child_render.size).max(0.);
            if !child_start.is_finite() || !child_extent.is_finite() {
                return false;
            }
            materialized_start = materialized_start.min(child_start);
            materialized_end = materialized_end.max(child_start + child_extent);
        }
        if !materialized_start.is_finite() || !materialized_end.is_finite() {
            return content_end == 0.;
        }
        const RANGE_EPSILON: f32 = 0.001;
        materialized_start <= wanted_start + RANGE_EPSILON
            && materialized_end + RANGE_EPSILON >= wanted_end
    }

    /// A controller or sliver delegate can change independently of widget
    /// BUILD. Mark the retained viewport dirty only when its sliver protocol
    /// needs a new cache window or its delegate changed.
    fn refresh_sliver_ranges(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::SliverViewport { config } = &render.kind else {
                    return None;
                };
                let element = self.element_for_render(RenderObjectId(raw))?;
                let element = self.elements.get(element.0)?;
                let delegate_changed =
                    element.sliver_delegate_revision != config.delegate.revision();
                let scroll_changed = element.sliver_scroll_revision != config.controller.revision();
                (delegate_changed
                    || (scroll_changed
                        && !self.sliver_cache_window_is_covered(
                            RenderObjectId(raw),
                            element,
                            config,
                        )))
                .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT, true);
        }
    }

    /// Rebuilds the retained notification subscriptions after layout. A
    /// sliver viewport owns its materialized children, so installing these
    /// subscriptions after materialization lets an ancestor listener observe
    /// both ordinary and newly visible nested viewports without rebuilding the
    /// application widget description.
    fn refresh_notification_listeners(&mut self) {
        let mut listeners = self
            .elements
            .iter()
            .filter_map(|(raw, element)| {
                let WidgetKind::NotificationListener {
                    callback: Some(callback),
                    ..
                } = &element.widget.kind
                else {
                    return None;
                };
                let id = ElementId(raw);
                Some((id, callback.clone(), self.element_depth(id)))
            })
            .collect::<Vec<_>>();

        // Drop old registrations first. This also removes listeners for
        // children that left a lazy cache window.
        for (_, element) in self.elements.iter_mut() {
            element.notification_subscriptions.clear();
        }

        // Controller dispatch is stop-on-true. Register the deepest wrapper
        // first so an inner listener gets first refusal, matching bubbling up
        // through Flutter's notification tree.
        listeners.sort_by_key(|(_, _, depth)| std::cmp::Reverse(*depth));
        for (listener, callback, _) in listeners {
            let mut sources = Vec::new();
            self.collect_scroll_sources(listener, 0, &mut sources);
            let subscriptions = sources
                .into_iter()
                .map(|(controller, depth)| {
                    let callback = callback.clone();
                    controller.add_listener(move |mut notification| {
                        notification.depth = depth;
                        callback(notification)
                    })
                })
                .collect::<Vec<_>>();
            if let Some(element) = self.elements.get_mut(listener.0) {
                element.notification_subscriptions = subscriptions;
            }
        }
    }

    fn element_depth(&self, mut id: ElementId) -> usize {
        let mut depth = 0;
        while let Some(parent) = self.elements.get(id.0).and_then(|element| element.parent) {
            depth += 1;
            id = parent;
        }
        depth
    }

    fn scroll_controller_for_element(&self, id: ElementId) -> Option<ScrollController> {
        let render = self.elements.get(id.0)?.render;
        match &self.renders.get(render.0)?.kind {
            RenderKind::Scroll { controller, .. } => Some(controller.clone()),
            RenderKind::SliverViewport { config } => Some(config.controller.clone()),
            _ => None,
        }
    }

    fn collect_scroll_sources(
        &self,
        id: ElementId,
        viewport_depth: usize,
        out: &mut Vec<(ScrollController, usize)>,
    ) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        let next_depth = if let Some(controller) = self.scroll_controller_for_element(id) {
            if !out.iter().any(|(existing, _)| *existing == controller) {
                out.push((controller, viewport_depth));
            }
            viewport_depth + 1
        } else {
            viewport_depth
        };
        for child in &element.children {
            self.collect_scroll_sources(*child, next_depth, out);
        }
    }

    fn refresh_text_fields(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::TextField { controller, .. } = &render.kind else {
                    return None;
                };
                let (content, visual) = controller.revisions();
                ((content != render.text_revision) || (visual != render.text_visual_revision))
                    .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            // Text width can change the size seen by an unconstrained parent.
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
    fn collect_focus_group(
        &self,
        id: ElementId,
        inherited_policy: FocusTraversalPolicyKind,
        excluded_focus: bool,
        out: &mut Vec<FocusTraversalMember>,
        sequence: &mut usize,
    ) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        if element.widget.semantics.exclude_focus_traversal {
            return;
        }
        let policy = element
            .widget
            .semantics
            .focus_traversal_policy
            .unwrap_or(inherited_policy);
        let mut members = Vec::new();
        self.append_focus_members(id, policy, excluded_focus, &mut members, sequence);
        out.push(FocusTraversalMember::Group(FocusTraversalGroupMembers {
            policy,
            members,
        }));
    }

    fn append_focus_members(
        &self,
        id: ElementId,
        policy: FocusTraversalPolicyKind,
        excluded_focus: bool,
        out: &mut Vec<FocusTraversalMember>,
        sequence: &mut usize,
    ) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        if element.widget.semantics.exclude_focus_traversal {
            return;
        }
        let excluded_focus = excluded_focus || element.widget.semantics.exclude_focus;
        let focusable = match &element.widget.kind {
            WidgetKind::Button {
                enabled,
                focusable_when_disabled,
                ..
            } => *enabled || *focusable_when_disabled,
            WidgetKind::TextField { enabled, .. } => *enabled,
            WidgetKind::SelectableText { .. } => true,
            WidgetKind::Gesture { callbacks, .. } => callbacks
                .focus_node
                .as_ref()
                .is_some_and(|node| node.can_request_focus()),
            _ => false,
        };
        if focusable && !excluded_focus {
            let bounds = self.element_bounds(id).unwrap_or_default();
            let order = element.widget.semantics.focus_traversal_order;
            if let WidgetKind::Gesture { callbacks, .. } = &element.widget.kind
                && let Some(node) = callbacks.focus_node.as_ref()
            {
                node.set_rect(bounds);
                node.set_traversal_order(order);
            }
            out.push(FocusTraversalMember::Candidate(FocusCandidate {
                id,
                bounds,
                order,
                sequence: *sequence,
            }));
            *sequence = sequence.saturating_add(1);
        }
        let (descendants_are_focusable, descendants_are_traversable) = match &element.widget.kind {
            WidgetKind::Gesture { callbacks, .. } => {
                callbacks.focus_node.as_ref().map_or((true, true), |node| {
                    (
                        node.descendants_are_focusable(),
                        node.descendants_are_traversable(),
                    )
                })
            }
            _ => (true, true),
        };
        if !descendants_are_traversable {
            return;
        }
        let excluded_focus = excluded_focus || !descendants_are_focusable;
        let focus_children: Vec<_> = match element.widget.kind {
            WidgetKind::IndexedStack { index, .. } => {
                element.children.get(index).copied().into_iter().collect()
            }
            _ => element.children.clone(),
        };
        for child in focus_children {
            let child_has_policy = self
                .elements
                .get(child.0)
                .is_some_and(|child| child.widget.semantics.focus_traversal_policy.is_some());
            if child_has_policy {
                self.collect_focus_group(child, policy, excluded_focus, out, sequence);
            } else {
                self.append_focus_members(child, policy, excluded_focus, out, sequence);
            }
        }
    }
    fn is_descendant_or_self(&self, candidate: ElementId, ancestor: ElementId) -> bool {
        let mut current = Some(candidate);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.parent(id);
        }
        false
    }
    fn text_field_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.is_text_field(id) {
                return Some(id);
            }
            id = self.parent(id)?;
        }
    }
    fn content_transform(&self, id: RenderObjectId) -> Option<CoreTransform> {
        let node = self.renders.get(id.0)?;
        match &node.kind {
            RenderKind::Transform { transform, origin } => {
                Some(transform_around(*transform, *origin, node.size))
            }
            RenderKind::Scale { controller, origin } => Some(transform_around(
                CoreTransform::scale(controller.scale()),
                *origin,
                node.size,
            )),
            RenderKind::Rotation {
                controller,
                origin,
                alignment,
            } => Some(transform_around_alignment(
                CoreTransform::rotation(controller.radians()),
                *origin,
                *alignment,
                node.size,
            )),
            RenderKind::FittedBox { fit, alignment } => {
                let child = *node.children.first()?;
                let child_size = self.renders.get(child.0)?.size;
                Some(fitted_transform(child_size, node.size, *fit, *alignment))
            }
            _ => None,
        }
    }
    fn child_content_transform(&self, id: RenderObjectId) -> CoreTransform {
        let node = self.renders.get(id.0).expect("live render");
        match &node.kind {
            RenderKind::Scroll {
                controller,
                axis,
                reverse,
                ..
            } => CoreTransform::translation(scroll_translation(controller, *axis, *reverse)),
            RenderKind::SliverViewport { config } => CoreTransform::translation(
                scroll_translation(&config.controller, config.axis, config.reverse),
            ),
            RenderKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
            } => CoreTransform::translation(
                self.persistent_header_translation(id, controller, *axis, *reverse, *pinned),
            ),
            RenderKind::Translate { controller } => CoreTransform::translation(controller.offset()),
            _ => self
                .content_transform(id)
                .unwrap_or(CoreTransform::IDENTITY),
        }
    }
    fn render_world_transform(&self, id: RenderObjectId) -> CoreTransform {
        let mut path = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            path.push(current);
            cursor = self.renders.get(current.0).expect("live render").parent;
        }
        path.reverse();
        let mut world = CoreTransform::IDENTITY;
        for (index, current) in path.iter().enumerate() {
            let node = self.renders.get(current.0).expect("live render");
            world = world.then(CoreTransform::translation(node.offset));
            if index + 1 != path.len() {
                world = world.then(self.child_content_transform(*current));
            }
        }
        world
    }
    fn semantic_bounds(&self, id: RenderObjectId, size: Size) -> Rect {
        let mut world = self.render_world_transform(id);
        if self.content_transform(id).is_some() {
            world = world.then(self.child_content_transform(id));
        }
        world.transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, size))
    }
    #[cfg(test)]
    fn render_origin(&self, mut id: RenderObjectId) -> Offset {
        let mut origin = Offset::ZERO;
        loop {
            let node = self.renders.get(id.0).expect("live render");
            origin = origin + node.offset;
            match &node.kind {
                RenderKind::Scroll {
                    controller,
                    axis,
                    reverse,
                    ..
                } => origin = origin + scroll_translation(controller, *axis, *reverse),
                RenderKind::SliverViewport { config } => {
                    origin =
                        origin + scroll_translation(&config.controller, config.axis, config.reverse)
                }
                RenderKind::PersistentHeader {
                    controller,
                    axis,
                    reverse,
                    pinned,
                } => {
                    origin = origin
                        + self
                            .persistent_header_translation(id, controller, *axis, *reverse, *pinned)
                }
                RenderKind::Translate { controller } => origin = origin + controller.offset(),
                _ => {}
            }
            let Some(parent) = node.parent else {
                return origin;
            };
            id = parent;
        }
    }
    /// World origin of a render object's viewport/picture. Unlike
    /// `render_origin`, this deliberately does not apply the object's own
    /// scrolling transform: a scrollbar is attached to that viewport, not to
    /// its scrolling content. Ancestor transforms still apply.
    fn render_viewport_origin(&self, mut id: RenderObjectId) -> Offset {
        let mut origin = Offset::ZERO;
        let mut is_self = true;
        loop {
            let node = self.renders.get(id.0).expect("live render");
            origin = origin + node.offset;
            if !is_self {
                match &node.kind {
                    RenderKind::Scroll {
                        controller,
                        axis,
                        reverse,
                        ..
                    } => origin = origin + scroll_translation(controller, *axis, *reverse),
                    RenderKind::SliverViewport { config } => {
                        origin = origin
                            + scroll_translation(&config.controller, config.axis, config.reverse)
                    }
                    RenderKind::PersistentHeader {
                        controller,
                        axis,
                        reverse,
                        pinned,
                    } => {
                        origin = origin
                            + self.persistent_header_translation(
                                id, controller, *axis, *reverse, *pinned,
                            )
                    }
                    RenderKind::Translate { controller } => origin = origin + controller.offset(),
                    _ => {}
                }
            }
            let Some(parent) = node.parent else {
                return origin;
            };
            id = parent;
            is_self = false;
        }
    }
    /// Returns the compositor-only compensation for a persistent header. A
    /// header remains a normal flow child; when it reaches the viewport edge,
    /// this transform cancels its parent's scroll transform. The following
    /// persistent sibling limits the compensation and pushes it away.
    fn persistent_header_translation(
        &self,
        header: RenderObjectId,
        controller: &ScrollController,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    ) -> Offset {
        if !pinned {
            return Offset::ZERO;
        }
        let Some((scroll, flow_position)) = self.scroll_flow_position(header, controller, axis)
        else {
            return Offset::ZERO;
        };
        let header_extent = self
            .renders
            .get(header.0)
            .map_or(0., |node| axis.main_extent(node.size));
        let viewport_extent = self
            .renders
            .get(scroll.0)
            .map_or(0., |node| axis.main_extent(node.size));
        let physical_scroll = physical_scroll_offset(controller, reverse);
        let current_position = flow_position - physical_scroll;
        let leading_position = if reverse {
            (viewport_extent - header_extent).max(0.)
        } else {
            0.
        };
        let next_y = self
            .renders
            .iter()
            .filter_map(|(raw, node)| match &node.kind {
                RenderKind::PersistentHeader {
                    controller: candidate,
                    axis: candidate_axis,
                    reverse: candidate_reverse,
                    pinned: candidate_pinned,
                } if candidate == controller
                    && *candidate_axis == axis
                    && *candidate_reverse == reverse
                    && *candidate_pinned =>
                {
                    self.scroll_flow_position(RenderObjectId(raw), candidate, axis)
                        .filter(|(candidate_scroll, position)| {
                            *candidate_scroll == scroll && *position > flow_position
                        })
                        .map(|(_, position)| position)
                }
                _ => None,
            })
            .min_by(|left, right| left.total_cmp(right));
        let translation = (leading_position - current_position).max(0.);
        let translation = next_y.map_or(translation, |next| {
            // The push distance is a flow-space relationship between
            // neighboring headers.  Applying the current scroll offset here
            // would double-count the content transform and make a preceding
            // header disappear too early.
            translation.min((next - flow_position - header_extent).max(0.))
        });
        axis.offset(translation, 0.)
    }
    /// Static y-position within the scroll content and the matching scroll
    /// viewport. Dynamic scroll/pinning transforms are deliberately excluded.
    fn scroll_flow_position(
        &self,
        mut id: RenderObjectId,
        controller: &ScrollController,
        axis: Axis,
    ) -> Option<(RenderObjectId, f32)> {
        let mut position = 0.;
        loop {
            let node = self.renders.get(id.0)?;
            position += match axis {
                Axis::Horizontal => node.offset.x,
                Axis::Vertical => node.offset.y,
            };
            if let RenderKind::Scroll {
                controller: viewport,
                ..
            } = &node.kind
                && viewport == controller
            {
                return Some((id, position));
            }
            id = node.parent?;
        }
    }
    fn unmount(&mut self, id: ElementId) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        self.unmount_element(id);
        if self.root == Some(id) {
            self.root = None;
        }
        Ok(())
    }
    fn unmount_element(&mut self, id: ElementId) {
        let Some(element) = self.elements.remove(id.0) else {
            return;
        };
        self.active_gestures.retain(|_, active| {
            active
                .members
                .iter()
                .all(|candidate| candidate.element != id)
        });
        self.pointer_captures.retain(|_, target| *target != id);
        self.scale_gestures.remove(&id);
        for child in element.children {
            self.unmount_element(child);
        }
        if let Some(render) = self.renders.remove(element.render.0) {
            if let Some(picture) = render.picture {
                self.compositor.remove(picture);
            }
            if let Some(picture) = render.focus_picture {
                self.compositor.remove(picture);
            }
            if let Some(layer) = render.clip_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.content_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.opacity_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.blur_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.shadow_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.color_filter_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.blend_layer {
                self.compositor.remove(layer);
            }
            self.compositor.remove(render.layer);
        }
        self.unmounted.push(id);
        self.diagnostics.unmounts += 1;
    }
    fn check_keys_borrowed<'a>(
        &self,
        widgets: impl IntoIterator<Item = &'a Widget>,
    ) -> Result<(), TreeError> {
        let mut keys: HashSet<&Key> = HashSet::new();
        for key in widgets.into_iter().filter_map(Widget::key) {
            if !keys.insert(key) {
                return Err(TreeError::DuplicateKey(key.clone()));
            }
        }
        Ok(())
    }
    fn sync_render_children(&mut self, id: ElementId) {
        let (render, children) = {
            let e = self.elements.get(id.0).expect("mounted");
            (e.render, e.children.clone())
        };
        let render_children: Vec<_> = children
            .into_iter()
            .filter_map(|child| self.render_id(child))
            .collect();
        let children_changed = {
            let node = self.renders.get_mut(render.0).expect("mounted");
            let changed = node.children != render_children;
            node.children = render_children.clone();
            if changed {
                node.dirty.insert(DirtyFlags::LAYOUT | DirtyFlags::PAINT);
            }
            changed
        };
        let (
            layer,
            picture,
            focus_picture,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
            kind,
        ) = {
            let node = self.renders.get(render.0).expect("mounted");
            (
                node.layer,
                node.picture,
                node.focus_picture,
                node.content_layer,
                node.opacity_layer,
                node.blur_layer,
                node.shadow_layer,
                node.color_filter_layer,
                node.blend_layer,
                node.kind.clone(),
            )
        };
        let mut child_layers = render_children
            .iter()
            .filter_map(|child| self.renders.get(child.0).map(|render| render.layer))
            .collect::<Vec<_>>();
        if let RenderKind::IndexedStack { index, .. } = kind {
            child_layers = child_layers.get(index).copied().into_iter().collect();
        } else if matches!(kind, RenderKind::SliverViewport { .. }) {
            let pinned = self
                .element_for_render(render)
                .and_then(|element| self.elements.get(element.0))
                .map(|element| element.sliver_pinned_ids.clone())
                .unwrap_or_default();
            let mut order = (0..render_children.len()).collect::<Vec<_>>();
            order.sort_by_key(|index| {
                let child_id = self
                    .element_for_render(render)
                    .and_then(|element| self.elements.get(element.0))
                    .and_then(|element| element.sliver_child_ids.get(*index))
                    .copied();
                usize::from(child_id.is_some_and(|id| pinned.contains(&id)))
            });
            child_layers = order
                .into_iter()
                .filter_map(|index| child_layers.get(index).copied())
                .collect();
        }
        if let Some(opacity) = opacity_layer {
            self.compositor.set_children(opacity, child_layers);
            self.compositor.set_children(layer, vec![opacity]);
        } else if let Some(blur) = blur_layer {
            self.compositor.set_children(blur, child_layers);
            self.compositor.set_children(layer, vec![blur]);
        } else if let Some(shadow) = shadow_layer {
            self.compositor.set_children(shadow, child_layers);
            self.compositor.set_children(layer, vec![shadow]);
        } else if let Some(color_filter) = color_filter_layer {
            self.compositor.set_children(color_filter, child_layers);
            self.compositor.set_children(layer, vec![color_filter]);
        } else if let Some(blend) = blend_layer {
            self.compositor.set_children(blend, child_layers);
            self.compositor.set_children(layer, vec![blend]);
        } else if let Some(content) = content_layer {
            self.compositor.set_children(content, child_layers);
        } else {
            let mut layers =
                Vec::with_capacity(child_layers.len() + 1 + usize::from(focus_picture.is_some()));
            layers.extend(picture);
            layers.extend(child_layers);
            layers.extend(focus_picture);
            self.compositor.set_children(layer, layers);
        }
        for child in render_children {
            self.renders.get_mut(child.0).expect("mounted").parent = Some(render);
        }
        if children_changed {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
    fn mark_render_dirty(&mut self, id: RenderObjectId, flags: DirtyFlags, propagate_layout: bool) {
        let already_dirty = self
            .renders
            .get(id.0)
            .is_some_and(|node| node.dirty.contains(flags));
        self.diagnostics.dirty_requests += 1;
        if already_dirty {
            self.diagnostics.dirty_queue_deduplicated += 1;
        } else {
            self.diagnostics.dirty_queue_insertions += 1;
        }
        let mut current = Some(id);
        while let Some(render) = current {
            let node = self.renders.get_mut(render.0).expect("live render");
            node.dirty.insert(flags);
            current = if propagate_layout && flags.contains(DirtyFlags::LAYOUT) {
                node.parent
            } else {
                None
            };
        }
    }
    fn layout_render(&mut self, id: RenderObjectId, constraints: Constraints) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.layout_render_inner(id, constraints);
        });
    }

    fn layout_render_inner(&mut self, id: RenderObjectId, constraints: Constraints) {
        let _node_guard = self.guard_render(FramePhase::Layout, id, Some(constraints));
        let needs = self
            .renders
            .get(id.0)
            .expect("live")
            .dirty
            .contains(DirtyFlags::LAYOUT)
            || self.renders.get(id.0).expect("live").constraints != Some(constraints);
        if !needs {
            self.diagnostics.layout_cache_hits += 1;
            return;
        }
        #[cfg(feature = "devtools")]
        let trace = self
            .element_for_render(id)
            .and_then(|element| self.devtools_trace_begin_element(element, TracePhase::Layout));
        #[cfg(feature = "devtools")]
        let old_layout = self.deep_trace.as_ref().map(|_| {
            let render = self.renders.get(id.0).expect("live");
            (render.constraints, render.size)
        });
        if matches!(
            self.renders.get(id.0).expect("live").kind,
            RenderKind::LayoutBuilder
        ) {
            self.materialize_layout_builder(id, constraints);
        }
        let (kind, children) = {
            let n = self.renders.get(id.0).expect("live");
            (n.kind.clone(), n.children.clone())
        };
        let (size, offsets) = match kind {
            RenderKind::Box { desired, .. }
            | RenderKind::Shape { desired, .. }
            | RenderKind::CustomPaint { desired, .. } => {
                (constraints.constrain(desired), Vec::new())
            }
            RenderKind::Decorated { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let wanted = desired.unwrap_or(child_size);
                    let size = constraints.constrain(Size::new(
                        wanted.width.max(child_size.width),
                        wanted.height.max(child_size.height),
                    ));
                    (size, vec![Offset::ZERO])
                } else {
                    (
                        constraints.constrain(desired.unwrap_or(Size::ZERO)),
                        Vec::new(),
                    )
                }
            }
            RenderKind::Button { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let size = constraints.constrain(Size::new(
                        desired.width.max(child_size.width),
                        desired.height.max(child_size.height),
                    ));
                    (
                        size,
                        vec![Offset::new(
                            (size.width - child_size.width) / 2.0,
                            (size.height - child_size.height) / 2.0,
                        )],
                    )
                } else {
                    (constraints.constrain(desired), Vec::new())
                }
            }
            RenderKind::Padding { padding } => {
                let child_constraints =
                    constraints.deflate(padding.horizontal(), padding.vertical());
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let s = self.renders.get(child.0).expect("live").size;
                    (
                        constraints.constrain(Size::new(
                            s.width + padding.horizontal(),
                            s.height + padding.vertical(),
                        )),
                        vec![Offset::new(padding.left, padding.top)],
                    )
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Constrained {
                constraints: additional,
            } => {
                let child_constraints = enforced_constraints(constraints, additional);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Limited {
                max_width,
                max_height,
            } => {
                let child_constraints = Constraints::new(
                    constraints.min_width,
                    if constraints.max_width.is_infinite() {
                        max_width
                    } else {
                        constraints.max_width
                    },
                    constraints.min_height,
                    if constraints.max_height.is_infinite() {
                        max_height
                    } else {
                        constraints.max_height
                    },
                );
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Overflow {
                min_width,
                max_width,
                min_height,
                max_height,
            } => {
                let child_constraints = Constraints::new(
                    min_width.unwrap_or(constraints.min_width),
                    max_width
                        .unwrap_or(constraints.max_width)
                        .max(min_width.unwrap_or(constraints.min_width)),
                    min_height.unwrap_or(constraints.min_height),
                    max_height
                        .unwrap_or(constraints.max_height)
                        .max(min_height.unwrap_or(constraints.min_height)),
                );
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Unconstrained { constrained_axis } => {
                let child_constraints = unconstrained_constraints(constraints, constrained_axis);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Fractional {
                width_factor,
                height_factor,
            } => {
                let child_constraints =
                    fractional_constraints(constraints, width_factor, height_factor);
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Baseline { baseline } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child = self.renders.get(child.0).expect("live");
                    let result = incular_layout::layout_baseline(
                        constraints,
                        incular_layout::BaselineChild {
                            size: child.size,
                            baseline: child.baseline,
                        },
                        baseline,
                    );
                    self.renders.get_mut(id.0).expect("live").baseline = Some(baseline);
                    (result.size, vec![result.children[0].offset])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::RepaintBoundary | RenderKind::Gesture => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::PersistentHeader { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::SliverViewport { config } => {
                config
                    .controller
                    .set_metrics_context(config.axis, config.reverse);
                let mut size = sliver_viewport_size(config.axis, constraints, config.shrink_wrap);
                let mut viewport_extent = scroll_viewport_extent(config.axis, size);
                let cache_extent = config.cache_extent.max(0.);
                let cross_extent = config.axis.cross_extent(size);
                let make_constraints = |physical_offset: f32, viewport: f32| {
                    // Match RenderViewport's forward layout contract for
                    // overscroll: a negative physical position is represented
                    // as leading overlap and a reduced paint extent, while
                    // the sliver scroll offset itself stays non-negative.
                    let overlap = physical_offset.min(0.);
                    SliverConstraints::new(
                        config.axis,
                        config.reverse,
                        physical_offset.max(0.),
                        0.,
                        overlap,
                        (viewport + overlap).max(0.),
                        cross_extent,
                        viewport,
                        viewport + cache_extent * 2.,
                        -cache_extent,
                    )
                };
                let mut sliver_layout = config.delegate.perform_layout(make_constraints(
                    physical_scroll_offset(&config.controller, config.reverse),
                    viewport_extent,
                ));
                config.controller.update_extents_with_physics(
                    sliver_layout.geometry.scroll_extent,
                    viewport_extent,
                    config.physics,
                );

                // A shrink-wrapping viewport derives its own main-axis size
                // from the sliver geometry. The first pass supplies a
                // provisional zero/unbounded extent, then the real viewport
                // extent is laid out again before children are materialized.
                if config.shrink_wrap {
                    let content_extent = sliver_layout.geometry.scroll_extent.max(0.);
                    let main = if config.axis.is_vertical() {
                        content_extent.clamp(constraints.min_height, constraints.max_height)
                    } else {
                        content_extent.clamp(constraints.min_width, constraints.max_width)
                    };
                    size = constraints.constrain(config.axis.size(main, cross_extent));
                    viewport_extent = scroll_viewport_extent(config.axis, size);
                    sliver_layout = config.delegate.perform_layout(make_constraints(
                        physical_scroll_offset(&config.controller, config.reverse),
                        viewport_extent,
                    ));
                    config.controller.update_extents_with_physics(
                        sliver_layout.geometry.scroll_extent,
                        viewport_extent,
                        config.physics,
                    );
                }

                // Extent updates can clamp a restored/programmatic offset or
                // establish the physical origin of a reversed viewport. One
                // corrective layout keeps geometry and the retained range in
                // the same coordinate space from the first frame.
                sliver_layout = config.delegate.perform_layout(make_constraints(
                    physical_scroll_offset(&config.controller, config.reverse),
                    viewport_extent,
                ));
                config.controller.update_extents_with_physics(
                    sliver_layout.geometry.scroll_extent,
                    viewport_extent,
                    config.physics,
                );

                for _ in 0..3 {
                    let physical_before =
                        physical_scroll_offset(&config.controller, config.reverse);
                    let anchor_before = sliver_anchor(&sliver_layout, physical_before);
                    self.materialize_sliver_children(id, &config, &sliver_layout);
                    let materialized = self
                        .renders
                        .get(id.0)
                        .expect("sliver viewport render")
                        .children
                        .clone();
                    let mut pass_changed = false;
                    for (child, layout) in materialized.into_iter().zip(&sliver_layout.children) {
                        self.layout_render(child, layout.constraints);
                        let measured = config
                            .axis
                            .main_extent(self.renders.get(child.0).expect("sliver child").size);
                        pass_changed |= config.delegate.set_child_extent(layout.id, measured);
                        let child = self.renders.get_mut(child.0).expect("sliver child");
                        child.offset = config.axis.offset(layout.offset, layout.cross_offset);
                        self.compositor.update_transform(
                            child.layer,
                            CoreTransform::translation(child.offset),
                        );
                    }
                    if !pass_changed {
                        break;
                    }
                    let mut next_layout = config
                        .delegate
                        .perform_layout(make_constraints(physical_before, viewport_extent));
                    config.controller.update_extents_with_physics(
                        next_layout.geometry.scroll_extent,
                        viewport_extent,
                        config.physics,
                    );
                    // A reversed viewport's physical origin depends on the
                    // newly measured max offset. Recompute the layout after
                    // updating extents before comparing anchors; otherwise a
                    // first-frame estimate change can turn logical offset 0
                    // into an artificial scroll to the middle of the list.
                    let physical_after_extent =
                        physical_scroll_offset(&config.controller, config.reverse);
                    if physical_after_extent != physical_before {
                        next_layout = config.delegate.perform_layout(make_constraints(
                            physical_after_extent,
                            viewport_extent,
                        ));
                        config.controller.update_extents_with_physics(
                            next_layout.geometry.scroll_extent,
                            viewport_extent,
                            config.physics,
                        );
                    }
                    if let Some((anchor_id, anchor_offset)) = anchor_before
                        && let Some(updated) = next_layout
                            .children
                            .iter()
                            .find(|child| child.id == anchor_id)
                    {
                        // Compare visual positions, not raw content offsets.
                        // The physical origin may have changed when a reverse
                        // viewport learned its real content extent.
                        let delta = (updated.offset - physical_after_extent)
                            - (anchor_offset - physical_before);
                        if delta.is_finite() && delta.abs() > f32::EPSILON {
                            let corrected_physical = (physical_after_extent + delta).max(0.);
                            let corrected_logical = if config.reverse {
                                (config.controller.max_offset() - corrected_physical).max(0.)
                            } else {
                                corrected_physical
                            };
                            if config.controller.jump_to(corrected_logical) {
                                next_layout = config.delegate.perform_layout(make_constraints(
                                    physical_scroll_offset(&config.controller, config.reverse),
                                    viewport_extent,
                                ));
                                config.controller.update_extents_with_physics(
                                    next_layout.geometry.scroll_extent,
                                    viewport_extent,
                                    config.physics,
                                );
                            }
                        }
                    }
                    sliver_layout = next_layout;
                }
                if let Some(correction) = sliver_layout.geometry.scroll_offset_correction {
                    let physical = (physical_scroll_offset(&config.controller, config.reverse)
                        + correction)
                        .max(0.);
                    let logical = if config.reverse {
                        (config.controller.max_offset() - physical).max(0.)
                    } else {
                        physical
                    };
                    config.controller.jump_to(logical);
                }
                self.diagnostics.lazy_layouts += 1;
                (size, Vec::new())
            }
            RenderKind::Align {
                alignment,
                width_factor,
                height_factor,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let result = incular_layout::layout_align(
                        constraints,
                        child_size.into(),
                        incular_layout::Align {
                            alignment,
                            width_factor,
                            height_factor,
                        },
                    );
                    (
                        result.size,
                        result.children.into_iter().map(|c| c.offset).collect(),
                    )
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Flex { flex } => {
                let cross_max = match flex.direction {
                    Axis::Horizontal => constraints.max_height,
                    Axis::Vertical => constraints.max_width,
                };
                let main_max = match flex.direction {
                    Axis::Horizontal => constraints.max_width,
                    Axis::Vertical => constraints.max_height,
                };
                let loose = match flex.direction {
                    Axis::Horizontal => Constraints::new(0., f32::INFINITY, 0., cross_max),
                    Axis::Vertical => Constraints::new(0., cross_max, 0., f32::INFINITY),
                };
                let flex_meta = children
                    .iter()
                    .map(|child| {
                        let render_node = self.renders.get(child.0).expect("live");
                        match &render_node.kind {
                            RenderKind::Flexible { flex: f, fit } => (*f, *fit),
                            _ => (0, FlexFit::Loose),
                        }
                    })
                    .collect::<Vec<_>>();

                let mut flex_children = Vec::with_capacity(children.len());
                let mut occupied_non_flex = 0.0;
                for (child, (f, fit)) in children.iter().zip(&flex_meta) {
                    if *f == 0 || !main_max.is_finite() {
                        self.layout_render(*child, loose);
                        let size = self.renders.get(child.0).expect("live").size;
                        occupied_non_flex += flex.direction.main_extent(size);
                        flex_children.push(incular_layout::FlexChild::new(size));
                    } else {
                        flex_children.push(incular_layout::FlexChild::flexible(
                            Size::ZERO,
                            *f,
                            *fit,
                        ));
                    }
                }

                let total_flex: u32 = flex_meta.iter().map(|(f, _)| *f).sum();
                let spacing = flex.spacing.max(0.0) * children.len().saturating_sub(1) as f32;
                if main_max.is_finite() && total_flex > 0 {
                    let available_for_flex = (main_max - occupied_non_flex - spacing).max(0.0);
                    for (i, (child, (f, fit))) in children.iter().zip(&flex_meta).enumerate() {
                        if *f > 0 {
                            let allocation = available_for_flex * *f as f32 / total_flex as f32;
                            let child_constraints = match flex.direction {
                                Axis::Horizontal => Constraints::new(
                                    if *fit == FlexFit::Tight {
                                        allocation
                                    } else {
                                        0.0
                                    },
                                    allocation,
                                    0.0,
                                    cross_max,
                                ),
                                Axis::Vertical => Constraints::new(
                                    0.0,
                                    cross_max,
                                    if *fit == FlexFit::Tight {
                                        allocation
                                    } else {
                                        0.0
                                    },
                                    allocation,
                                ),
                            };
                            self.layout_render(*child, child_constraints);
                            let size = self.renders.get(child.0).expect("live").size;
                            flex_children[i] = incular_layout::FlexChild::flexible(size, *f, *fit);
                        }
                    }
                }

                let result = incular_layout::layout_flex(constraints, &flex_children, flex);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::Flexible { .. } | RenderKind::Positioned { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Wrap { wrap } => {
                let child_constraints = constraints.loosen();
                for child in &children {
                    self.layout_render(*child, child_constraints);
                }
                let wrap_children = children
                    .iter()
                    .map(|child| {
                        incular_layout::WrapChild::new(
                            self.renders.get(child.0).expect("live").size,
                        )
                    })
                    .collect::<Vec<_>>();

                let result = incular_layout::layout_wrap(constraints, &wrap_children, wrap);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::Table {
                columns,
                column_spacing,
                row_spacing,
            } => {
                for child in &children {
                    self.layout_render(*child, constraints.loosen());
                }
                let cells = children
                    .iter()
                    .map(|child| {
                        incular_layout::TableChild::new(
                            self.renders.get(child.0).expect("live").size,
                        )
                    })
                    .collect::<Vec<_>>();
                let config = incular_layout::Table {
                    columns,
                    column_spacing,
                    row_spacing,
                    alignment: Alignment::TOP_LEFT,
                };
                let result = incular_layout::layout_table(constraints, &cells, config);
                (
                    result.size,
                    result
                        .children
                        .into_iter()
                        .map(|cell| cell.offset)
                        .collect(),
                )
            }
            RenderKind::Stack { stack } => {
                let non_positioned_constraints = match stack.fit {
                    StackFit::Loose => constraints.loosen(),
                    StackFit::Expand => Constraints::tight(constraints.biggest()),
                    StackFit::Passthrough => constraints,
                };
                let mut max_non_pos_w: f32 = 0.0;
                let mut max_non_pos_h: f32 = 0.0;
                for child in &children {
                    let render_node = self.renders.get(child.0).expect("live");
                    if !matches!(render_node.kind, RenderKind::Positioned { .. }) {
                        self.layout_render(*child, non_positioned_constraints);
                        let size = self.renders.get(child.0).expect("live").size;
                        max_non_pos_w = max_non_pos_w.max(size.width);
                        max_non_pos_h = max_non_pos_h.max(size.height);
                    }
                }
                let stack_size = constraints.constrain(if matches!(stack.fit, StackFit::Expand) {
                    constraints.biggest()
                } else {
                    Size::new(max_non_pos_w, max_non_pos_h)
                });

                let mut stack_children = Vec::with_capacity(children.len());
                for child in &children {
                    let render_node = self.renders.get(child.0).expect("live");
                    if let RenderKind::Positioned {
                        left,
                        top,
                        right,
                        bottom,
                        width,
                        height,
                    } = render_node.kind
                    {
                        let pos = incular_layout::Positioned {
                            left,
                            top,
                            right,
                            bottom,
                            width,
                            height,
                        };
                        let child_w = width.or_else(|| {
                            left.zip(right)
                                .map(|(l, r)| (stack_size.width - l - r).max(0.0))
                        });
                        let child_h = height.or_else(|| {
                            top.zip(bottom)
                                .map(|(t, b)| (stack_size.height - t - b).max(0.0))
                        });
                        let child_constraints = Constraints::new(
                            0.0,
                            child_w.unwrap_or(stack_size.width),
                            0.0,
                            child_h.unwrap_or(stack_size.height),
                        );
                        self.layout_render(*child, child_constraints);
                        let size = self.renders.get(child.0).expect("live").size;
                        stack_children.push(incular_layout::StackChild::positioned(size, pos));
                    } else {
                        let size = self.renders.get(child.0).expect("live").size;
                        stack_children.push(incular_layout::StackChild::new(size));
                    }
                }

                let result = incular_layout::layout_stack(constraints, &stack_children, stack);
                (
                    result.size,
                    result.children.into_iter().map(|c| c.offset).collect(),
                )
            }
            RenderKind::IndexedStack { alignment, .. } => {
                let mut natural = Size::ZERO;
                for child in &children {
                    self.layout_render(*child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    natural = Size::new(
                        natural.width.max(child_size.width),
                        natural.height.max(child_size.height),
                    );
                }
                let size = constraints.constrain(natural);
                let offsets = children
                    .iter()
                    .map(|child| {
                        alignment.within(size, self.renders.get(child.0).expect("live").size)
                    })
                    .collect();
                (size, offsets)
            }
            RenderKind::SafeArea {
                minimum,
                left,
                top,
                right,
                bottom,
                maintain_bottom_view_padding: _,
            } => {
                let ambient = self.environment.safe_insets.normalized();
                let insets = EdgeInsets::only(
                    if left {
                        ambient.left.max(minimum.left)
                    } else {
                        minimum.left
                    },
                    if top {
                        ambient.top.max(minimum.top)
                    } else {
                        minimum.top
                    },
                    if right {
                        ambient.right.max(minimum.right)
                    } else {
                        minimum.right
                    },
                    if bottom {
                        ambient.bottom.max(minimum.bottom)
                    } else {
                        minimum.bottom
                    },
                );
                let child_constraints = constraints.deflate(insets.horizontal(), insets.vertical());
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let size = constraints.constrain(Size::new(
                        child_size.width + insets.horizontal(),
                        child_size.height + insets.vertical(),
                    ));
                    (size, vec![Offset::new(insets.left, insets.top)])
                } else {
                    let size =
                        constraints.constrain(Size::new(insets.horizontal(), insets.vertical()));
                    (size, Vec::new())
                }
            }
            RenderKind::ClipRect { .. }
            | RenderKind::ClipRRect { .. }
            | RenderKind::ClipOval { .. }
            | RenderKind::ClipPath { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::LayoutBuilder => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints);
                    let size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Visibility { visible } => {
                if visible {
                    if let Some(&child) = children.first() {
                        self.layout_render(child, constraints.loosen());
                        let size =
                            constraints.constrain(self.renders.get(child.0).expect("live").size);
                        (size, vec![Offset::ZERO])
                    } else {
                        (constraints.constrain(Size::ZERO), Vec::new())
                    }
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::AspectRatio { ratio } => {
                let mut size = if constraints.is_width_bounded() {
                    Size::new(constraints.max_width, constraints.max_width / ratio)
                } else if constraints.is_height_bounded() {
                    Size::new(constraints.max_height * ratio, constraints.max_height)
                } else {
                    Size::ZERO
                };
                if size.height > constraints.max_height {
                    size = Size::new(constraints.max_height * ratio, constraints.max_height);
                }
                size = constraints.constrain(size);
                if let Some(&child) = children.first() {
                    self.layout_render(child, Constraints::tight(size));
                    (size, vec![Offset::ZERO])
                } else {
                    (size, Vec::new())
                }
            }
            RenderKind::Text {
                text,
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            } => {
                let width = constraints
                    .is_width_bounded()
                    .then_some(constraints.max_width);
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout_with_options", &text));
                let layout = self.text_engine.layout_with_options(
                    &text,
                    &style,
                    TextLayoutOptions::new(width, align)
                        .soft_wrap(soft_wrap)
                        .max_lines(max_lines)
                        .overflow(overflow),
                );
                let size = constraints.constrain(layout.metrics.size);
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::SelectableText { text, style, align } => {
                let width = constraints
                    .is_width_bounded()
                    .then_some(constraints.max_width);
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout", &text));
                let layout = self.text_engine.layout(&text, &style, width, align);
                let size = constraints.constrain(layout.metrics.size);
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::SelectionArea => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints);
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Image {
                image,
                width,
                height,
                ..
            } => {
                let intrinsic = image.decoded();
                let ratio = intrinsic.width() as f32 / intrinsic.height() as f32;
                let natural = match (width, height) {
                    (Some(w), Some(h)) => Size::new(w, h),
                    (Some(w), None) => Size::new(w, w / ratio),
                    (None, Some(h)) => Size::new(h * ratio, h),
                    (None, None) => Size::new(intrinsic.width() as f32, intrinsic.height() as f32),
                };
                (constraints.constrain(natural), Vec::new())
            }
            RenderKind::TextField {
                controller,
                desired,
                style,
                placeholder,
                multiline,
                min_lines,
                max_lines,
                expands,
                text_align,
                obscure_text,
                ..
            } => {
                let display = text_field_display(&controller, &placeholder, obscure_text);
                let intrinsic_width = if desired.width > 0.0 {
                    desired.width
                } else if constraints.is_width_bounded() {
                    constraints.max_width
                } else {
                    260.0
                };
                let width_for_text = (intrinsic_width - 16.).max(0.);
                let _external_call = self
                    .recursion_diagnostics
                    .external_call(text_call_label("TextEngine::layout_with_options", &display));
                let layout = self.text_engine.layout_with_options(
                    &display,
                    &style,
                    TextLayoutOptions::new(Some(width_for_text), text_align)
                        .soft_wrap(multiline)
                        .max_lines(max_lines),
                );
                let line_height = layout.metrics.line_height.max(1.0);
                let minimum_lines = min_lines.unwrap_or(1).max(1) as f32;
                let minimum_height = (line_height * minimum_lines + 16.0).max(32.0);
                let intrinsic_height = if desired.height > 0.0 {
                    desired.height
                } else if expands && constraints.is_height_bounded() {
                    constraints.max_height
                } else {
                    layout.metrics.size.height.max(line_height) + 16.0
                };
                let size = constraints.constrain(Size::new(
                    intrinsic_width,
                    intrinsic_height.max(minimum_height),
                ));
                let (revision, visual_revision) = controller.revisions();
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.text_revision = revision;
                node.text_visual_revision = visual_revision;
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::Scroll {
                controller,
                axis,
                reverse: _,
                physics,
            } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, scroll_constraints(axis, constraints));
                    let content = self.renders.get(child.0).expect("live").size;
                    let size = scroll_size(axis, constraints, content);
                    controller.update_extents_with_physics(
                        axis.main_extent(content),
                        scroll_viewport_extent(axis, size),
                        physics,
                    );
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Translate { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Transform { .. }
            | RenderKind::Scale { .. }
            | RenderKind::Rotation { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::FittedBox { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(
                        child,
                        Constraints::new(0., f32::INFINITY, 0., f32::INFINITY),
                    );
                    let child_size = self.renders.get(child.0).expect("live").size;
                    (constraints.constrain(child_size), vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Opacity { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Blur { .. }
            | RenderKind::DropShadow { .. }
            | RenderKind::ColorFiltered { .. }
            | RenderKind::Blend { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
        };
        for (child, offset) in children.into_iter().zip(offsets) {
            // The parent computes this placement after the child has completed
            // its own layout. Update the retained placement layer here rather
            // than waiting for a later child layout pass (which may not occur).
            let child = self.renders.get_mut(child.0).expect("live");
            child.offset = offset;
            self.compositor
                .update_transform(child.layer, CoreTransform::translation(offset));
        }
        let node = self.renders.get_mut(id.0).expect("live");
        node.size = size;
        node.constraints = Some(constraints);
        node.dirty.remove(DirtyFlags::LAYOUT);
        node.dirty.insert(DirtyFlags::PAINT);
        self.compositor
            .update_transform(node.layer, CoreTransform::translation(node.offset));
        if let Some(clip) = node.clip_layer {
            self.compositor
                .update_clip(clip, Rect::from_origin_size(Offset::ZERO, node.size));
        }
        // Static affine wrappers and FittedBox receive their initial retained
        // transform during layout. Later controller changes are handled by
        // `update_compositor` without revisiting this path.
        let (content_layer, transform) = {
            let node = self.renders.get(id.0).expect("live");
            (node.content_layer, self.content_transform(id))
        };
        if let (Some(content), Some(transform)) = (content_layer, transform) {
            self.compositor.update_transform(content, transform);
        }
        self.diagnostics.layouts += 1;
        #[cfg(feature = "devtools")]
        if let Some(element) = self.element_for_render(id) {
            if let Some(element) = self.elements.get_mut(element.0) {
                element.dev.layouts += 1;
                if let Some((old_constraints, old_size)) = old_layout
                    && (old_constraints != Some(constraints) || old_size != size)
                {
                    if old_constraints != Some(constraints) {
                        element
                            .dev
                            .layout_reason
                            .get_or_insert_with(|| "incoming constraints changed".into());
                    }
                    element
                        .dev
                        .paint_reason
                        .get_or_insert_with(|| "layout result invalidated paint".into());
                    const MAX_LAYOUT_HISTORY: usize = 64;
                    if element.dev.layout_history.len() == MAX_LAYOUT_HISTORY {
                        element.dev.layout_history.remove(0);
                    }
                    element.dev.layout_history.push(LayoutHistoryRecord {
                        sequence: element.dev.layouts,
                        old_constraints,
                        new_constraints: constraints,
                        old_size,
                        new_size: size,
                        cause: element
                            .dev
                            .last_cause
                            .as_ref()
                            .map(InvalidationCause::summary),
                    });
                }
            }
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
    }
    fn paint_render(&mut self, id: RenderObjectId, output: &mut DisplayList) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.paint_render_inner(id, output);
        });
    }

    fn paint_render_inner(&mut self, id: RenderObjectId, output: &mut DisplayList) {
        let constraints = self.renders.get(id.0).and_then(|render| render.constraints);
        let _node_guard = self.guard_render(FramePhase::Paint, id, constraints);
        if matches!(
            self.renders.get(id.0).expect("live").kind,
            RenderKind::Visibility { visible: false }
        ) {
            return;
        }
        let (mut offset, cache, children, kind) = {
            let node = self.renders.get(id.0).expect("live");
            (
                node.offset,
                node.cache.clone(),
                node.children.clone(),
                node.kind.clone(),
            )
        };
        if let RenderKind::PersistentHeader {
            ref controller,
            axis,
            reverse,
            pinned,
        } = kind
        {
            offset =
                offset + self.persistent_header_translation(id, controller, axis, reverse, pinned);
        }
        let dirty = self
            .renders
            .get(id.0)
            .expect("live")
            .dirty
            .contains(DirtyFlags::PAINT);
        #[cfg(feature = "devtools")]
        let trace = dirty
            .then(|| self.element_for_render(id))
            .flatten()
            .and_then(|element| self.devtools_trace_begin_element(element, TracePhase::Paint));
        if dirty {
            let kind = self.renders.get(id.0).expect("live").kind.clone();
            let size = self.renders.get(id.0).expect("live").size;
            let focus_picture = self.renders.get(id.0).expect("live").focus_picture;
            let focus_ring = match &kind {
                RenderKind::Button {
                    color,
                    focused_color,
                    enabled,
                    ..
                } => {
                    let render = self.renders.get(id.0).expect("live");
                    (render.button_focused && *enabled && color.alpha == 0)
                        .then_some(*focused_color)
                        .flatten()
                }
                _ => None,
            };
            let mut cache = DisplayList::new();
            let mut focus_cache = DisplayList::new();
            match kind {
                RenderKind::Box { color, .. } => {
                    if color.alpha > 0 {
                        cache.push(PaintCommand::Rect {
                            rect: Rect::from_origin_size(Offset::ZERO, size),
                            color,
                        });
                    }
                }
                RenderKind::Shape {
                    path, fill, stroke, ..
                } => {
                    if let Some(brush) = fill {
                        cache.push(PaintCommand::FillPath {
                            path: path.clone(),
                            brush,
                            fill_rule: FillRule::NonZero,
                        });
                    }
                    if let Some((brush, stroke)) = stroke {
                        cache.push(PaintCommand::StrokePath {
                            path,
                            brush,
                            stroke,
                        });
                    }
                }
                RenderKind::CustomPaint { display_list, .. } => {
                    cache.extend_from(&display_list);
                }
                RenderKind::Decorated {
                    background,
                    border,
                    radius,
                    ..
                } => {
                    let rrect = RRect::new(Rect::from_origin_size(Offset::ZERO, size), radius);
                    if let Some(brush) = background {
                        cache.push(PaintCommand::RRect { rrect, brush });
                    }
                    if let Some(border) = border {
                        cache.push(PaintCommand::Border { rrect, border });
                    }
                }
                RenderKind::Button {
                    color,
                    hover_color,
                    pressed_color,
                    focused_color,
                    disabled_color,
                    enabled,
                    ..
                } => {
                    let render = self.renders.get(id.0).expect("live");
                    // Transparent buttons are commonly used as the retained
                    // hit/semantic surface for compound controls (checkboxes,
                    // switches, toggles, and radios).  Treat their focused
                    // color as a focus ring instead of filling the entire
                    // hit surface.  Filling a label row with the accent made
                    // keyboard focus look like a stuck hover highlight and
                    // obscured the control's actual state.
                    let state_color = if render.button_pressed {
                        pressed_color.or(hover_color).or(focused_color)
                    } else if render.button_hovered {
                        hover_color
                    } else if render.button_focused {
                        focus_ring.map(|_| Color::TRANSPARENT)
                    } else {
                        None
                    };
                    let base = if !enabled {
                        disabled_color.or(Some(color))
                    } else {
                        state_color.or(Some(color))
                    }
                    .unwrap_or(color);
                    if base.alpha > 0 {
                        cache.push(PaintCommand::RRect {
                            rrect: RRect::uniform(Rect::from_origin_size(Offset::ZERO, size), 4.),
                            brush: base.into(),
                        });
                    }
                }
                RenderKind::Text {
                    style, overflow, ..
                } => {
                    if let Some(layout) = self.renders.get(id.0).expect("live").text_layout.clone()
                    {
                        if overflow == TextOverflow::Clip {
                            cache.push(PaintCommand::PushClip {
                                rect: Rect::from_origin_size(Offset::ZERO, size),
                            });
                        }
                        for line in layout.lines.iter() {
                            for run in line.runs.iter() {
                                cache.push(PaintCommand::GlyphRun {
                                    run: run.clone(),
                                    color: style.color,
                                });
                            }
                        }
                        if overflow == TextOverflow::Clip {
                            cache.push(PaintCommand::PopClip);
                        }
                    }
                }
                RenderKind::SelectableText { style, .. } => {
                    let selection = self
                        .element_for_render(id)
                        .and_then(|element| self.static_selection_range(element));
                    if let (Some(layout), Some(selection)) = (
                        self.renders.get(id.0).expect("live").text_layout.clone(),
                        selection,
                    ) {
                        for rect in selection_rects(&layout, selection, 0., 0., 0.) {
                            cache.push(PaintCommand::Rect {
                                rect,
                                color: Color::rgba(72, 120, 220, 150),
                            });
                        }
                        for line in layout.lines.iter() {
                            for run in line.runs.iter() {
                                cache.push(PaintCommand::GlyphRun {
                                    run: run.clone(),
                                    color: style.color,
                                });
                            }
                        }
                    } else if let Some(layout) =
                        self.renders.get(id.0).expect("live").text_layout.clone()
                    {
                        for line in layout.lines.iter() {
                            for run in line.runs.iter() {
                                cache.push(PaintCommand::GlyphRun {
                                    run: run.clone(),
                                    color: style.color,
                                });
                            }
                        }
                    }
                }
                RenderKind::SelectionArea => {}
                RenderKind::Image {
                    image,
                    fit,
                    repeat,
                    alignment,
                    sampling,
                    ..
                } => {
                    let intrinsic = image.decoded();
                    let source = Rect::from_origin_size(
                        Offset::ZERO,
                        Size::new(intrinsic.width() as f32, intrinsic.height() as f32),
                    );
                    let (source, destination) = image_fit_rects(
                        source,
                        Rect::from_origin_size(Offset::ZERO, size),
                        fit,
                        alignment,
                    );
                    for destination in image_repeat_destinations(
                        destination,
                        Rect::from_origin_size(Offset::ZERO, size),
                        repeat,
                    ) {
                        cache.push(PaintCommand::Image {
                            image: image.clone(),
                            source,
                            destination,
                            sampling,
                        });
                    }
                }
                RenderKind::TextField {
                    controller,
                    style,
                    placeholder,
                    multiline,
                    obscure_text,
                    cursor_width,
                    cursor_height,
                    cursor_radius,
                    show_cursor,
                    cursor_color,
                    selection_color,
                    ..
                } => {
                    let (layout, focused, scroll_x, scroll_y) = {
                        let node = self.renders.get(id.0).expect("live");
                        (
                            node.text_layout.clone(),
                            node.focused,
                            node.text_scroll_x,
                            node.text_scroll_y,
                        )
                    };
                    let value = controller.value();
                    let display = text_field_display(&controller, &placeholder, obscure_text);
                    let mut active_scroll_x = scroll_x;
                    let mut active_scroll_y = scroll_y;
                    if let Some(layout) = layout {
                        let (caret, caret_y, caret_height) = caret_geometry(
                            &layout,
                            value.selection.extent,
                            controller.caret_affinity(),
                        );
                        let available = (size.width - 16.).max(0.);
                        if !multiline && caret - active_scroll_x > available {
                            active_scroll_x = caret - available;
                        } else if !multiline && caret < active_scroll_x {
                            active_scroll_x = caret;
                        }
                        active_scroll_x = active_scroll_x.max(0.);
                        let top = if multiline {
                            8.
                        } else {
                            ((size.height - layout.metrics.line_height) / 2.).max(0.)
                        };
                        if multiline {
                            let viewport = (size.height - 16.).max(0.);
                            if caret_y - active_scroll_y < 0. {
                                active_scroll_y = caret_y;
                            } else if caret_y + caret_height - active_scroll_y > viewport {
                                active_scroll_y = caret_y + caret_height - viewport;
                            }
                            active_scroll_y = active_scroll_y
                                .clamp(0., (layout.metrics.size.height - viewport).max(0.));
                        }
                        let selection = value.selection.range();
                        if focused && !selection.is_empty() {
                            for rect in selection_rects(
                                &layout,
                                selection,
                                active_scroll_x,
                                active_scroll_y,
                                top,
                            ) {
                                cache.push(PaintCommand::Rect {
                                    rect,
                                    color: selection_color,
                                });
                            }
                        }
                        cache.push(PaintCommand::PushClip {
                            rect: Rect::from_origin_size(
                                Offset::new(8., 2.),
                                Size::new((size.width - 16.).max(0.), (size.height - 4.).max(0.)),
                            ),
                        });
                        cache.push(PaintCommand::PushTransform {
                            transform: CoreTransform::translation(Offset::new(
                                8. - active_scroll_x,
                                top - active_scroll_y,
                            )),
                        });
                        let color = if display == placeholder && value.text.is_empty() {
                            Color::rgba(150, 154, 170, 255)
                        } else {
                            style.color
                        };
                        for line in layout.lines.iter() {
                            for run in line.runs.iter() {
                                cache.push(PaintCommand::GlyphRun {
                                    run: run.clone(),
                                    color,
                                });
                            }
                        }
                        cache.push(PaintCommand::PopTransform);
                        cache.push(PaintCommand::PopClip);
                        if focused && show_cursor && controller.caret_visible(Instant::now()) {
                            let height = cursor_height.unwrap_or(caret_height).max(0.0);
                            let y = top + caret_y - active_scroll_y
                                + (caret_height - height).max(0.0) * 0.5;
                            let rect = Rect::from_origin_size(
                                Offset::new(caret - active_scroll_x + 8., y),
                                Size::new(cursor_width.max(0.0), height),
                            );
                            if cursor_radius > 0.0 {
                                cache.push(PaintCommand::RRect {
                                    rrect: RRect::uniform(rect, cursor_radius),
                                    brush: cursor_color.into(),
                                });
                            } else if rect.size.width > 0.0 && rect.size.height > 0.0 {
                                cache.push(PaintCommand::Rect {
                                    rect,
                                    color: cursor_color,
                                });
                            }
                        }
                    }
                    let node = self.renders.get_mut(id.0).expect("live");
                    node.text_scroll_x = active_scroll_x;
                    node.text_scroll_y = active_scroll_y;
                }
                RenderKind::Scroll { controller, .. } => {
                    self.paint_scrollbar(id, size, &controller, &mut cache);
                }
                RenderKind::SliverViewport { config } => {
                    self.paint_scrollbar(id, size, &config.controller, &mut cache);
                }
                _ => {}
            }
            if let Some(focus_ring) = focus_ring.filter(|color| color.alpha > 0) {
                let inset = 1.0;
                let ring_size = Size::new(
                    (size.width - 2. * inset).max(0.),
                    (size.height - 2. * inset).max(0.),
                );
                focus_cache.push(PaintCommand::Border {
                    rrect: RRect::uniform(
                        Rect::from_origin_size(Offset::new(inset, inset), ring_size),
                        4.,
                    ),
                    border: Border::new(2.0, focus_ring),
                });
            }
            let node = self.renders.get_mut(id.0).expect("live");
            node.cache = cache;
            node.dirty.remove(DirtyFlags::PAINT);
            if let Some(picture) = node.picture {
                self.compositor.update_picture(
                    picture,
                    node.cache.clone(),
                    Rect::from_origin_size(Offset::ZERO, node.size),
                );
            }
            if let Some(picture) = focus_picture {
                self.compositor.update_picture(
                    picture,
                    focus_cache,
                    Rect::from_origin_size(Offset::ZERO, node.size),
                );
            }
            self.diagnostics.paints += 1;
            #[cfg(feature = "devtools")]
            if let Some(element) = self.element_for_render(id) {
                if let Some(element) = self.elements.get_mut(element.0) {
                    element.dev.paints += 1;
                }
            }
        }
        output.push(PaintCommand::PushTransform {
            transform: CoreTransform::translation(offset),
        });
        if dirty {
            output.extend_from(&self.renders.get(id.0).expect("live").cache);
        } else {
            self.diagnostics.display_lists_reused += 1;
            output.extend_from(&cache);
        }
        let pushed_clip = match &kind {
            RenderKind::ClipRect { .. }
            | RenderKind::ClipRRect { .. }
            | RenderKind::ClipOval { .. }
            | RenderKind::ClipPath { .. } => {
                let node_size = self.renders.get(id.0).expect("live").size;
                output.push(PaintCommand::PushClip {
                    rect: Rect::from_origin_size(Offset::ZERO, node_size),
                });
                true
            }
            _ => false,
        };
        for child in children {
            self.paint_render(child, output);
        }
        if pushed_clip {
            output.push(PaintCommand::PopClip);
        }
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        output.push(PaintCommand::PopTransform);
    }
    fn paint_scrollbar(
        &self,
        id: RenderObjectId,
        size: Size,
        controller: &ScrollController,
        cache: &mut DisplayList,
    ) {
        let style = controller.scrollbar_style();
        let geometry = scrollbar_geometry(size, controller, style);
        if !geometry.visible {
            return;
        }
        let node = self.renders.get(id.0).expect("live");
        if !controller.scrollbar_thumb_visibility()
            && !node.scrollbar_hovered
            && !node.scrollbar_dragging
        {
            return;
        }
        let thumb = style.thumb_color;
        if style.track_color.alpha > 0 {
            cache.push(PaintCommand::RRect {
                rrect: RRect::uniform(geometry.track, style.width * 0.5),
                brush: style.track_color.into(),
            });
        }
        if thumb.alpha > 0 {
            cache.push(PaintCommand::RRect {
                rrect: RRect::uniform(geometry.thumb, style.width * 0.5),
                brush: thumb.into(),
            });
        }
    }
    fn hit_test_render(
        &self,
        id: RenderObjectId,
        point: Offset,
        origin: Offset,
    ) -> Option<RenderObjectId> {
        let node = self.renders.get(id.0)?;
        match self
            .element_for_render(id)
            .and_then(|element| self.elements.get(element.0))
            .map(|element| &element.widget.kind)
        {
            Some(WidgetKind::IgnorePointer { ignoring: true, .. }) => return None,
            Some(WidgetKind::AbsorbPointer {
                absorbing: true, ..
            }) => return Some(id),
            _ => {}
        }
        if matches!(
            node.kind,
            RenderKind::Transform { .. }
                | RenderKind::Scale { .. }
                | RenderKind::Rotation { .. }
                | RenderKind::FittedBox { .. }
        ) {
            let current = origin + node.offset;
            let local = self
                .child_content_transform(id)
                .inverse_transform_point(point - current)?;
            let child_size = node
                .children
                .first()
                .and_then(|child| self.renders.get(child.0))
                .map_or(node.size, |child| child.size);
            if !Rect::from_origin_size(Offset::ZERO, child_size).contains(local) {
                return None;
            }
            for child in node.children.iter().rev() {
                if let Some(hit) = self.hit_test_render(*child, local, Offset::ZERO) {
                    return Some(hit);
                }
            }
            return None;
        }
        let current = match &node.kind {
            RenderKind::Translate { controller } => origin + node.offset + controller.offset(),
            RenderKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
            } => {
                origin
                    + node.offset
                    + self.persistent_header_translation(id, controller, *axis, *reverse, *pinned)
            }
            _ => origin + node.offset,
        };
        if !Rect::from_origin_size(current, node.size).contains(point) {
            return None;
        }
        if matches!(node.kind, RenderKind::Visibility { visible: false }) {
            return None;
        }
        let child_origin = match &node.kind {
            RenderKind::Scroll {
                controller,
                axis,
                reverse,
                ..
            } => current + scroll_translation(controller, *axis, *reverse),
            RenderKind::SliverViewport { config } => {
                current + scroll_translation(&config.controller, config.axis, config.reverse)
            }
            RenderKind::Translate { .. } => current,
            _ => current,
        };
        let hit_children: Vec<_> = if matches!(node.kind, RenderKind::SliverViewport { .. }) {
            let pinned = self
                .element_for_render(id)
                .and_then(|element| self.elements.get(element.0))
                .map(|element| element.sliver_pinned_ids.clone())
                .unwrap_or_default();
            let mut ordered = node.children.clone();
            ordered.sort_by_key(|child| {
                let child_element = self.element_for_render(*child);
                child_element
                    .and_then(|element| self.elements.get(element.0))
                    .and_then(|element| element.parent)
                    .and_then(|parent| self.elements.get(parent.0))
                    .and_then(|parent| {
                        parent
                            .children
                            .iter()
                            .position(|candidate| Some(*candidate) == child_element)
                            .and_then(|slot| parent.sliver_child_ids.get(slot))
                    })
                    .map_or(0, |child_id| usize::from(pinned.contains(child_id)))
            });
            ordered
        } else {
            match &node.kind {
                RenderKind::IndexedStack { index, .. } => {
                    node.children.get(*index).copied().into_iter().collect()
                }
                _ => node.children.clone(),
            }
        };
        for child in hit_children.iter().rev() {
            if let Some(hit) = self.hit_test_render(*child, point, child_origin) {
                return Some(hit);
            }
        }
        match self
            .element_for_render(id)
            .and_then(|element| self.elements.get(element.0))
            .map(|element| &element.widget.kind)
        {
            Some(WidgetKind::Gesture { behavior, .. })
                if *behavior == crate::gestures::HitTestBehavior::DeferToChild =>
            {
                None
            }
            _ => Some(id),
        }
    }
    fn scrollbar_controller_and_geometry(
        &self,
        render: RenderObjectId,
    ) -> Option<(ScrollController, ScrollbarGeometry)> {
        let node = self.renders.get(render.0)?;
        let controller = match &node.kind {
            RenderKind::Scroll { controller, .. } => controller.clone(),
            RenderKind::SliverViewport { config } => config.controller.clone(),
            _ => return None,
        };
        let mut geometry = scrollbar_geometry(node.size, &controller, controller.scrollbar_style());
        let origin = self.render_viewport_origin(render);
        geometry.track.origin = geometry.track.origin + origin;
        geometry.thumb.origin = geometry.thumb.origin + origin;
        Some((controller, geometry))
    }
    fn scrollbar_local_geometry(
        &self,
        render: RenderObjectId,
    ) -> Option<(ScrollController, ScrollbarGeometry)> {
        let node = self.renders.get(render.0)?;
        let controller = match &node.kind {
            RenderKind::Scroll { controller, .. } => controller.clone(),
            RenderKind::SliverViewport { config } => config.controller.clone(),
            _ => return None,
        };
        Some((
            controller.clone(),
            scrollbar_geometry(node.size, &controller, controller.scrollbar_style()),
        ))
    }
    fn scrollbar_local_point(&self, render: RenderObjectId, point: Offset) -> Option<Offset> {
        self.render_world_transform(render)
            .inverse_transform_point(point)
    }
    fn scrollbar_at(&self, point: Offset) -> Option<RenderObjectId> {
        self.renders.iter().fold(None, |found, (raw, _)| {
            let render = RenderObjectId(raw);
            found.or_else(|| {
                self.scrollbar_local_geometry(render)
                    .and_then(|(_, geometry)| {
                        self.scrollbar_local_point(render, point).and_then(|point| {
                            (geometry.visible && geometry.track.contains(point)).then_some(render)
                        })
                    })
            })
        })
    }
}

fn semantic_action_is_executable(
    kind: &WidgetKind,
    action: SemanticActionKind,
    callbacks: &SemanticCallbacks,
) -> bool {
    if callbacks.supports(action) {
        return true;
    }
    match action {
        SemanticActionKind::Focus => true,
        SemanticActionKind::Activate => {
            matches!(
                kind,
                WidgetKind::Button {
                    has_callback: true,
                    enabled: true,
                    ..
                }
            )
        }
        SemanticActionKind::SetText | SemanticActionKind::SetSelection => {
            matches!(kind, WidgetKind::TextField { .. })
        }
        SemanticActionKind::ScrollForward | SemanticActionKind::ScrollBackward => {
            matches!(
                kind,
                WidgetKind::Scroll { .. } | WidgetKind::SliverViewport { .. }
            )
        }
        SemanticActionKind::Increment | SemanticActionKind::Decrement => {
            matches!(
                kind,
                WidgetKind::Gesture { callbacks, .. } if callbacks.has_keyboard_listener()
            )
        }
    }
}

fn widget_text(widget: &Widget) -> Option<String> {
    match &widget.kind {
        WidgetKind::Text { text, .. } | WidgetKind::SelectableText { text, .. } => {
            Some(text.clone())
        }
        WidgetKind::Button { child, .. } => child.as_deref().and_then(widget_text),
        WidgetKind::Decorated { child, .. }
        | WidgetKind::Padding { child, .. }
        | WidgetKind::Constrained { child, .. }
        | WidgetKind::Limited { child, .. }
        | WidgetKind::Overflow { child, .. }
        | WidgetKind::Unconstrained { child, .. }
        | WidgetKind::Fractional { child, .. }
        | WidgetKind::Baseline { child, .. }
        | WidgetKind::RepaintBoundary { child }
        | WidgetKind::Gesture { child, .. }
        | WidgetKind::Draggable { child, .. }
        | WidgetKind::DragTarget { child, .. }
        | WidgetKind::IgnorePointer { child, .. }
        | WidgetKind::AbsorbPointer { child, .. }
        | WidgetKind::Align { child, .. }
        | WidgetKind::Flexible { child, .. }
        | WidgetKind::Positioned { child, .. }
        | WidgetKind::SafeArea { child, .. }
        | WidgetKind::ClipRect { child, .. }
        | WidgetKind::ClipRRect { child, .. }
        | WidgetKind::ClipOval { child, .. }
        | WidgetKind::ClipPath { child, .. }
        | WidgetKind::Visibility { child, .. }
        | WidgetKind::AspectRatio { child, .. }
        | WidgetKind::Scroll { child, .. }
        | WidgetKind::PersistentHeader { child, .. }
        | WidgetKind::NotificationListener { child, .. }
        | WidgetKind::Translate { child, .. }
        | WidgetKind::Transform { child, .. }
        | WidgetKind::Scale { child, .. }
        | WidgetKind::Rotation { child, .. }
        | WidgetKind::FittedBox { child, .. }
        | WidgetKind::Opacity { child, .. }
        | WidgetKind::Blur { child, .. }
        | WidgetKind::DropShadow { child, .. }
        | WidgetKind::ColorFiltered { child, .. }
        | WidgetKind::Blend { child, .. } => widget_text(child),
        WidgetKind::SelectionArea { child, .. } => widget_text(child),
        WidgetKind::Flex { children, .. }
        | WidgetKind::Stack { children, .. }
        | WidgetKind::IndexedStack { children, .. } => {
            let text: String = children
                .iter()
                .filter_map(widget_text)
                .collect::<Vec<_>>()
                .join(" ");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}
#[cfg(feature = "devtools")]
impl WidgetKind {
    pub fn dev_type_name_widget(&self) -> String {
        crate::devtools_props::kind_display_name(self)
    }
}

#[cfg(feature = "devtools")]
impl RenderKind {
    pub fn dev_type_name_render(&self) -> String {
        crate::devtools_props::kind_display_name_render(self)
    }

    pub fn dev_type_name(&self) -> String {
        let name = match self {
            RenderKind::Box { .. } => "Box",
            RenderKind::Shape { .. } => "Shape",
            RenderKind::CustomPaint { .. } => "CustomPaint",
            RenderKind::Decorated { .. } => "DecoratedBox",
            RenderKind::Button { .. } => "Button",
            RenderKind::Text { .. } => "Text",
            RenderKind::SelectableText { .. } => "SelectableText",
            RenderKind::SelectionArea => "SelectionArea",
            RenderKind::TextField { .. } => "TextField",
            RenderKind::Image { .. } => "Image",
            RenderKind::Padding { .. } => "Padding",
            RenderKind::Constrained { .. } => "ConstrainedBox",
            RenderKind::Limited { .. } => "LimitedBox",
            RenderKind::Overflow { .. } => "OverflowBox",
            RenderKind::Unconstrained { .. } => "UnconstrainedBox",
            RenderKind::Fractional { .. } => "FractionallySizedBox",
            RenderKind::Baseline { .. } => "Baseline",
            RenderKind::RepaintBoundary => "RepaintBoundary",
            RenderKind::Gesture => "GestureDetector",
            RenderKind::Align { .. } => "Align",
            RenderKind::Flex { flex, .. } => {
                return match flex.direction {
                    incular_config::Axis::Vertical => "Column".into(),
                    incular_config::Axis::Horizontal => "Row".into(),
                };
            }
            RenderKind::Wrap { .. } => "Wrap",
            RenderKind::Table { .. } => "Table",
            RenderKind::Stack { .. } => "Stack",
            RenderKind::IndexedStack { .. } => "IndexedStack",
            RenderKind::Positioned { .. } => "Positioned",
            RenderKind::SafeArea { .. } => "SafeArea",
            RenderKind::ClipRect { .. } => "ClipRect",
            RenderKind::ClipRRect { .. } => "ClipRRect",
            RenderKind::ClipOval { .. } => "ClipOval",
            RenderKind::ClipPath { .. } => "ClipPath",
            RenderKind::Visibility { .. } => "Visibility",
            RenderKind::AspectRatio { .. } => "AspectRatio",
            RenderKind::Scroll { .. } => "ScrollView",
            RenderKind::PersistentHeader { .. } => "PersistentHeader",
            RenderKind::SliverViewport { .. } => "SliverViewport",
            RenderKind::LayoutBuilder => "LayoutBuilder",
            RenderKind::Translate { .. } => "Translate",
            RenderKind::Transform { .. } => "Transform",
            RenderKind::Scale { .. } => "Scale",
            RenderKind::Rotation { .. } => "Rotation",
            RenderKind::FittedBox { .. } => "FittedBox",
            RenderKind::Opacity { .. } => "Opacity",
            RenderKind::Blur { .. } => "Blur",
            RenderKind::DropShadow { .. } => "DropShadow",
            RenderKind::ColorFiltered { .. } => "ColorFiltered",
            RenderKind::Blend { .. } => "Blend",
            RenderKind::Flexible { .. } => "Flexible",
        };
        name.to_owned()
    }
}

fn resolve_text_style(style: &TextStyle, environment: Option<&Rc<dyn Any>>) -> TextStyle {
    environment
        .and_then(environment_value::<TextStyle>)
        .map_or_else(|| style.clone(), |default| default.merge(style))
}

fn render_kind(widget: &Widget, environment: Option<&Rc<dyn Any>>) -> RenderKind {
    match &widget.kind {
        WidgetKind::Box { size, color } => RenderKind::Box {
            desired: *size,
            color: *color,
        },
        WidgetKind::Shape {
            path,
            fill,
            stroke,
            size,
        } => RenderKind::Shape {
            path: path.clone(),
            fill: fill.clone(),
            stroke: stroke.clone(),
            desired: size.unwrap_or_else(|| path.bounds().map_or(Size::ZERO, |bounds| bounds.size)),
        },
        WidgetKind::CustomPaint { size, display_list } => RenderKind::CustomPaint {
            desired: *size,
            display_list: display_list.clone(),
        },
        WidgetKind::Decorated {
            size,
            background,
            border,
            radius,
            ..
        } => RenderKind::Decorated {
            desired: *size,
            background: background.clone(),
            border: *border,
            radius: *radius,
        },
        WidgetKind::Button {
            size,
            color,
            hover_color,
            pressed_color,
            focused_color,
            disabled_color,
            enabled,
            focusable_when_disabled,
            ..
        } => RenderKind::Button {
            desired: *size,
            color: *color,
            hover_color: *hover_color,
            pressed_color: *pressed_color,
            focused_color: *focused_color,
            disabled_color: *disabled_color,
            enabled: *enabled,
            focusable_when_disabled: *focusable_when_disabled,
        },
        WidgetKind::Text {
            text,
            style,
            align,
            soft_wrap,
            max_lines,
            overflow,
        } => RenderKind::Text {
            text: text.clone(),
            style: resolve_text_style(style, environment),
            align: *align,
            soft_wrap: *soft_wrap,
            max_lines: *max_lines,
            overflow: *overflow,
        },
        WidgetKind::SelectableText { text, style, align } => RenderKind::SelectableText {
            text: text.clone(),
            style: resolve_text_style(style, environment),
            align: *align,
        },
        WidgetKind::SelectionArea { .. } => RenderKind::SelectionArea,
        WidgetKind::Image {
            image,
            width,
            height,
            fit,
            repeat,
            alignment,
            sampling,
        } => RenderKind::Image {
            image: image.clone(),
            width: *width,
            height: *height,
            fit: *fit,
            repeat: *repeat,
            alignment: *alignment,
            sampling: *sampling,
        },
        WidgetKind::TextField {
            controller,
            size,
            style,
            placeholder,
            on_submit: _,
            multiline,
            min_lines,
            max_lines,
            expands,
            text_align,
            enabled,
            read_only,
            obscure_text,
            cursor_width,
            cursor_height,
            cursor_radius,
            show_cursor,
            cursor_color,
            selection_color,
        } => RenderKind::TextField {
            controller: controller.clone(),
            desired: *size,
            style: resolve_text_style(style, environment),
            placeholder: placeholder.clone(),
            multiline: *multiline,
            min_lines: *min_lines,
            max_lines: *max_lines,
            expands: *expands,
            text_align: *text_align,
            enabled: *enabled,
            read_only: *read_only,
            obscure_text: *obscure_text,
            cursor_width: *cursor_width,
            cursor_height: *cursor_height,
            cursor_radius: *cursor_radius,
            show_cursor: *show_cursor,
            cursor_color: *cursor_color,
            selection_color: *selection_color,
        },
        WidgetKind::Padding { padding, .. } => RenderKind::Padding { padding: *padding },
        WidgetKind::Constrained { constraints, .. } => RenderKind::Constrained {
            constraints: *constraints,
        },
        WidgetKind::Limited {
            max_width,
            max_height,
            ..
        } => RenderKind::Limited {
            max_width: *max_width,
            max_height: *max_height,
        },
        WidgetKind::Overflow {
            min_width,
            max_width,
            min_height,
            max_height,
            ..
        } => RenderKind::Overflow {
            min_width: *min_width,
            max_width: *max_width,
            min_height: *min_height,
            max_height: *max_height,
        },
        WidgetKind::Unconstrained {
            constrained_axis, ..
        } => RenderKind::Unconstrained {
            constrained_axis: *constrained_axis,
        },
        WidgetKind::Fractional {
            width_factor,
            height_factor,
            ..
        } => RenderKind::Fractional {
            width_factor: *width_factor,
            height_factor: *height_factor,
        },
        WidgetKind::Baseline { baseline, .. } => RenderKind::Baseline {
            baseline: *baseline,
        },
        WidgetKind::RepaintBoundary { .. } => RenderKind::RepaintBoundary,
        WidgetKind::Gesture { .. } => RenderKind::Gesture,
        WidgetKind::Draggable { .. } | WidgetKind::DragTarget { .. } => RenderKind::Gesture,
        WidgetKind::IgnorePointer { .. } | WidgetKind::AbsorbPointer { .. } => RenderKind::Gesture,
        WidgetKind::Align {
            alignment,
            width_factor,
            height_factor,
            ..
        } => RenderKind::Align {
            alignment: *alignment,
            width_factor: *width_factor,
            height_factor: *height_factor,
        },
        WidgetKind::Flex {
            axis,
            main_axis_alignment,
            main_axis_size,
            cross_axis_alignment,
            text_direction,
            vertical_direction,
            spacing,
            ..
        } => RenderKind::Flex {
            flex: incular_layout::Flex {
                direction: *axis,
                main_axis_alignment: *main_axis_alignment,
                main_axis_size: *main_axis_size,
                cross_axis_alignment: *cross_axis_alignment,
                text_direction: *text_direction,
                vertical_direction: *vertical_direction,
                spacing: *spacing,
            },
        },
        WidgetKind::Flexible { flex, fit, .. } => RenderKind::Flexible {
            flex: *flex,
            fit: *fit,
        },
        WidgetKind::Wrap {
            axis,
            alignment,
            spacing,
            run_alignment,
            run_spacing,
            cross_axis_alignment,
            text_direction,
            vertical_direction,
            ..
        } => RenderKind::Wrap {
            wrap: incular_layout::Wrap {
                direction: *axis,
                alignment: *alignment,
                spacing: *spacing,
                run_alignment: *run_alignment,
                run_spacing: *run_spacing,
                cross_axis_alignment: *cross_axis_alignment,
                text_direction: *text_direction,
                vertical_direction: *vertical_direction,
            },
        },
        WidgetKind::Table {
            columns,
            column_spacing,
            row_spacing,
            ..
        } => RenderKind::Table {
            columns: *columns,
            column_spacing: *column_spacing,
            row_spacing: *row_spacing,
        },
        WidgetKind::Stack {
            alignment,
            text_direction,
            fit,
            clip_behavior,
            ..
        } => RenderKind::Stack {
            stack: incular_layout::Stack {
                alignment: *alignment,
                text_direction: *text_direction,
                fit: *fit,
                clip_behavior: *clip_behavior,
            },
        },
        WidgetKind::SafeArea {
            minimum,
            left,
            top,
            right,
            bottom,
            maintain_bottom_view_padding,
            ..
        } => RenderKind::SafeArea {
            minimum: *minimum,
            left: *left,
            top: *top,
            right: *right,
            bottom: *bottom,
            maintain_bottom_view_padding: *maintain_bottom_view_padding,
        },
        WidgetKind::ClipRect { clip_behavior, .. } => RenderKind::ClipRect {
            clip_behavior: *clip_behavior,
        },
        WidgetKind::ClipRRect {
            radius,
            clip_behavior,
            ..
        } => RenderKind::ClipRRect {
            radius: *radius,
            clip_behavior: *clip_behavior,
        },
        WidgetKind::ClipOval { clip_behavior, .. } => RenderKind::ClipOval {
            clip_behavior: *clip_behavior,
        },
        WidgetKind::ClipPath {
            path,
            clip_behavior,
            ..
        } => RenderKind::ClipPath {
            path: path.clone(),
            clip_behavior: *clip_behavior,
        },
        WidgetKind::Positioned {
            left,
            top,
            right,
            bottom,
            width,
            height,
            ..
        } => RenderKind::Positioned {
            left: *left,
            top: *top,
            right: *right,
            bottom: *bottom,
            width: *width,
            height: *height,
        },
        WidgetKind::IndexedStack {
            alignment, index, ..
        } => RenderKind::IndexedStack {
            alignment: *alignment,
            index: *index,
        },
        WidgetKind::LayoutBuilder { .. } => RenderKind::LayoutBuilder,
        WidgetKind::Visibility { visible, .. } => RenderKind::Visibility { visible: *visible },
        WidgetKind::AspectRatio { ratio, .. } => RenderKind::AspectRatio { ratio: *ratio },
        WidgetKind::Scroll {
            controller,
            axis,
            reverse,
            physics,
            ..
        } => RenderKind::Scroll {
            controller: controller.clone(),
            axis: *axis,
            reverse: *reverse,
            physics: *physics,
        },
        WidgetKind::PersistentHeader {
            controller,
            axis,
            reverse,
            pinned,
            ..
        } => RenderKind::PersistentHeader {
            controller: controller.clone(),
            axis: *axis,
            reverse: *reverse,
            pinned: *pinned,
        },
        WidgetKind::NotificationListener { .. } => RenderKind::Gesture,
        WidgetKind::SliverViewport { config } => RenderKind::SliverViewport {
            config: config.clone(),
        },
        WidgetKind::Translate { controller, .. } => RenderKind::Translate {
            controller: controller.clone(),
        },
        WidgetKind::Transform {
            transform, origin, ..
        } => RenderKind::Transform {
            transform: *transform,
            origin: *origin,
        },
        WidgetKind::Scale {
            controller, origin, ..
        } => RenderKind::Scale {
            controller: controller.clone(),
            origin: *origin,
        },
        WidgetKind::Rotation {
            controller,
            origin,
            alignment,
            ..
        } => RenderKind::Rotation {
            controller: controller.clone(),
            origin: *origin,
            alignment: *alignment,
        },
        WidgetKind::FittedBox { fit, alignment, .. } => RenderKind::FittedBox {
            fit: *fit,
            alignment: *alignment,
        },
        WidgetKind::Opacity {
            alpha, controller, ..
        } => RenderKind::Opacity {
            alpha: *alpha,
            controller: controller.clone(),
        },
        WidgetKind::Blur {
            sigma_x,
            sigma_y,
            controller,
            ..
        } => RenderKind::Blur {
            sigma_x: *sigma_x,
            sigma_y: *sigma_y,
            controller: controller.clone(),
        },
        WidgetKind::DropShadow {
            offset,
            sigma_x,
            sigma_y,
            color,
            controller,
            ..
        } => RenderKind::DropShadow {
            offset: *offset,
            sigma_x: *sigma_x,
            sigma_y: *sigma_y,
            color: *color,
            controller: controller.clone(),
        },
        WidgetKind::ColorFiltered {
            filter, controller, ..
        } => RenderKind::ColorFiltered {
            filter: *filter,
            controller: controller.clone(),
        },
        WidgetKind::Blend { mode, .. } => RenderKind::Blend { mode: *mode },
    }
}

/// Carries a compositor transition across a declarative rebuild when a
/// controlled application creates a fresh controller value.  The widget
/// descriptor is replaced, but the retained render object is still the same
/// semantic/control node; jumping directly to the new controller value would
/// make externally owned toggles and switches visibly snap.
fn carry_replaced_transition(old: &RenderKind, new: &RenderKind) {
    const REPLACED_TRANSITION: Duration = Duration::from_millis(140);

    match (old, new) {
        (
            RenderKind::Opacity {
                alpha: old_alpha,
                controller: old_controller,
            },
            RenderKind::Opacity {
                alpha: new_alpha,
                controller: Some(new_controller),
            },
        ) => {
            let replaced = match old_controller {
                Some(old_controller) => old_controller != new_controller,
                None => true,
            };
            let current_alpha = old_controller
                .as_ref()
                .map_or(*old_alpha, OpacityController::opacity);
            if replaced && (current_alpha - new_alpha).abs() > f32::EPSILON {
                new_controller.set_opacity(current_alpha);
                new_controller.animate_to(*new_alpha, REPLACED_TRANSITION, Instant::now());
            }
        }
        (
            RenderKind::Translate {
                controller: old_controller,
            },
            RenderKind::Translate {
                controller: new_controller,
            },
        ) if old_controller != new_controller => {
            let old_offset = old_controller.offset();
            let new_offset = new_controller.offset();
            if old_offset != new_offset {
                new_controller.set_offset(old_offset);
                new_controller.animate_to(new_offset, REPLACED_TRANSITION, Instant::now());
            }
        }
        (
            RenderKind::Scale {
                controller: old_controller,
                ..
            },
            RenderKind::Scale {
                controller: new_controller,
                ..
            },
        ) if old_controller != new_controller => {
            let old_scale = old_controller.scale();
            let new_scale = new_controller.scale();
            if (old_scale - new_scale).abs() > f32::EPSILON {
                new_controller.set_scale(old_scale);
                new_controller.animate_to(new_scale, REPLACED_TRANSITION, Instant::now());
            }
        }
        (
            RenderKind::Rotation {
                controller: old_controller,
                ..
            },
            RenderKind::Rotation {
                controller: new_controller,
                ..
            },
        ) if old_controller != new_controller => {
            let old_radians = old_controller.radians();
            let new_radians = new_controller.radians();
            if (old_radians - new_radians).abs() > f32::EPSILON {
                new_controller.set_radians(old_radians);
                new_controller.animate_to(new_radians, REPLACED_TRANSITION, Instant::now());
            }
        }
        _ => {}
    }
}

fn transform_around(transform: CoreTransform, origin: Option<Offset>, size: Size) -> CoreTransform {
    let origin = origin.unwrap_or(Offset::new(size.width * 0.5, size.height * 0.5));
    CoreTransform::translation(origin)
        .then(transform)
        .then(CoreTransform::translation(Offset::new(
            -origin.x, -origin.y,
        )))
}

fn transform_around_alignment(
    transform: CoreTransform,
    origin: Option<Offset>,
    alignment: Option<Alignment>,
    size: Size,
) -> CoreTransform {
    let origin = origin.or_else(|| alignment.map(|alignment| alignment.within(size, Size::ZERO)));
    transform_around(transform, origin, size)
}

fn fitted_transform(
    source: Size,
    bounds: Size,
    fit: ImageFit,
    alignment: Alignment,
) -> CoreTransform {
    if source.width <= 0. || source.height <= 0. || bounds.width <= 0. || bounds.height <= 0. {
        return CoreTransform::IDENTITY;
    }
    let sx = bounds.width / source.width;
    let sy = bounds.height / source.height;
    let (scale_x, scale_y) = match fit {
        ImageFit::Fill => (sx, sy),
        ImageFit::Cover => {
            let scale = sx.max(sy);
            (scale, scale)
        }
        ImageFit::FitWidth => (sx, sx),
        ImageFit::FitHeight => (sy, sy),
        ImageFit::None => (1., 1.),
        ImageFit::ScaleDown => {
            let scale = sx.min(sy).min(1.);
            (scale, scale)
        }
        ImageFit::Contain => {
            let scale = sx.min(sy);
            (scale, scale)
        }
    };
    let fitted = Size::new(source.width * scale_x, source.height * scale_y);
    let offset = alignment.within(bounds, fitted);
    CoreTransform::translation(offset).then(CoreTransform::scale_non_uniform(scale_x, scale_y))
}

/// Returns the pixel source crop and logical destination for a fit operation.
#[must_use]
pub fn image_fit_rects(
    source: Rect,
    bounds: Rect,
    fit: ImageFit,
    alignment: Alignment,
) -> (Rect, Rect) {
    if source.size.width == 0.
        || source.size.height == 0.
        || bounds.size.width == 0.
        || bounds.size.height == 0.
    {
        return (source, Rect::from_origin_size(bounds.origin, Size::ZERO));
    }
    if fit == ImageFit::Fill {
        return (source, bounds);
    }
    let sx = bounds.size.width / source.size.width;
    let sy = bounds.size.height / source.size.height;
    let (scale_x, scale_y) = match fit {
        ImageFit::Cover => {
            let s = sx.max(sy);
            (s, s)
        }
        ImageFit::FitWidth => (sx, sx),
        ImageFit::FitHeight => (sy, sy),
        ImageFit::None => (1., 1.),
        ImageFit::ScaleDown => {
            let s = sx.min(sy).min(1.);
            (s, s)
        }
        _ => {
            let s = sx.min(sy);
            (s, s)
        }
    };
    let scale = scale_x.min(scale_y);
    let rendered = Size::new(source.size.width * scale, source.size.height * scale);
    if fit == ImageFit::Cover {
        let crop = Size::new(
            (bounds.size.width / scale).min(source.size.width),
            (bounds.size.height / scale).min(source.size.height),
        );
        let x = source.origin.x + (source.size.width - crop.width) * (alignment.x + 1.) / 2.;
        let y = source.origin.y + (source.size.height - crop.height) * (alignment.y + 1.) / 2.;
        return (Rect::from_origin_size(Offset::new(x, y), crop), bounds);
    }
    let origin = Offset::new(
        bounds.origin.x + (bounds.size.width - rendered.width) * (alignment.x + 1.) / 2.,
        bounds.origin.y + (bounds.size.height - rendered.height) * (alignment.y + 1.) / 2.,
    );
    (source, Rect::from_origin_size(origin, rendered))
}

/// Returns all local destinations required to repeat one image tile within a
/// bounded paint rectangle. Non-repeated axes retain the fitted destination.
#[must_use]
pub fn image_repeat_destinations(
    destination: Rect,
    bounds: Rect,
    repeat: ImageRepeat,
) -> Vec<Rect> {
    if matches!(repeat, ImageRepeat::NoRepeat)
        || destination.size.width <= 0.
        || destination.size.height <= 0.
    {
        return vec![destination];
    }
    let repeat_x = matches!(repeat, ImageRepeat::RepeatX | ImageRepeat::Repeat);
    let repeat_y = matches!(repeat, ImageRepeat::RepeatY | ImageRepeat::Repeat);
    let start_x = if repeat_x {
        destination.origin.x
            - ((destination.origin.x - bounds.origin.x) / destination.size.width).ceil()
                * destination.size.width
    } else {
        destination.origin.x
    };
    let start_y = if repeat_y {
        destination.origin.y
            - ((destination.origin.y - bounds.origin.y) / destination.size.height).ceil()
                * destination.size.height
    } else {
        destination.origin.y
    };
    let x_limit = if repeat_x {
        bounds.origin.x + bounds.size.width
    } else {
        start_x + destination.size.width
    };
    let y_limit = if repeat_y {
        bounds.origin.y + bounds.size.height
    } else {
        start_y + destination.size.height
    };
    let mut tiles = Vec::new();
    let mut y = start_y;
    while y < y_limit && tiles.len() < 16_384 {
        let mut x = start_x;
        while x < x_limit && tiles.len() < 16_384 {
            tiles.push(Rect::from_origin_size(Offset::new(x, y), destination.size));
            x += destination.size.width;
        }
        y += destination.size.height;
    }
    tiles
}

#[cfg(test)]
mod image_fit_tests {
    use super::*;
    #[test]
    fn contain_cover_fill_and_scale_down_are_deterministic() {
        let source = Rect::from_origin_size(Offset::ZERO, Size::new(400., 200.));
        let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(200., 200.));
        let (_, contain) = image_fit_rects(source, bounds, ImageFit::Contain, Alignment::CENTER);
        assert_eq!(contain.size, Size::new(200., 100.));
        let (cover_source, cover_destination) =
            image_fit_rects(source, bounds, ImageFit::Cover, Alignment::CENTER);
        assert_eq!(cover_destination, bounds);
        assert_eq!(cover_source.size, Size::new(200., 200.));
        assert_eq!(
            image_fit_rects(source, bounds, ImageFit::Fill, Alignment::CENTER).1,
            bounds
        );
        assert_eq!(
            image_fit_rects(
                Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
                bounds,
                ImageFit::ScaleDown,
                Alignment::CENTER
            )
            .1
            .size,
            Size::new(40., 20.)
        );
    }

    #[test]
    fn repeat_destinations_cover_requested_axes_from_the_fitted_origin() {
        let destination = Rect::from_origin_size(Offset::new(5., 2.), Size::new(10., 4.));
        let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(30., 10.));
        let tiles = image_repeat_destinations(destination, bounds, ImageRepeat::RepeatX);
        assert_eq!(tiles.len(), 4);
        assert_eq!(tiles[0].origin, Offset::new(-5., 2.));
        assert_eq!(tiles[3].origin, Offset::new(25., 2.));
        assert_eq!(
            image_repeat_destinations(destination, bounds, ImageRepeat::NoRepeat),
            vec![destination]
        );
    }
}
fn text_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    let (
        RenderKind::Text {
            text: old_text,
            style: old_style,
            align: old_align,
            soft_wrap: old_soft_wrap,
            max_lines: old_max_lines,
            overflow: old_overflow,
        },
        RenderKind::Text {
            text: new_text,
            style: new_style,
            align: new_align,
            soft_wrap: new_soft_wrap,
            max_lines: new_max_lines,
            overflow: new_overflow,
        },
    ) = (old, new)
    else {
        return false;
    };
    old_text == new_text
        && old_align == new_align
        && old_soft_wrap == new_soft_wrap
        && old_max_lines == new_max_lines
        && old_overflow == new_overflow
        && old_style.family == new_style.family
        && old_style.size == new_style.size
        && old_style.weight == new_style.weight
        && old_style.style == new_style.style
        && old_style.line_height == new_style.line_height
        && old_style.letter_spacing == new_style.letter_spacing
}

fn custom_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (
            RenderKind::CustomPaint {
                desired: old_size,
                ..
            },
            RenderKind::CustomPaint {
                desired: new_size,
                ..
            }
        ) if old_size == new_size
    )
}

fn opacity_composite_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Opacity { .. }, RenderKind::Opacity { .. })
    )
}

fn effect_composite_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Blur { .. }, RenderKind::Blur { .. })
            | (RenderKind::DropShadow { .. }, RenderKind::DropShadow { .. })
            | (
                RenderKind::ColorFiltered { .. },
                RenderKind::ColorFiltered { .. }
            )
            | (RenderKind::Blend { .. }, RenderKind::Blend { .. })
    )
}

fn affine_composite_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Transform { .. }, RenderKind::Transform { .. })
            | (RenderKind::Scale { .. }, RenderKind::Scale { .. })
            | (RenderKind::Rotation { .. }, RenderKind::Rotation { .. })
    )
}

fn text_field_display(
    controller: &TextEditingController,
    placeholder: &str,
    obscure_text: bool,
) -> String {
    let value = controller.value();
    let mask = |text: String| {
        if obscure_text {
            // Keep one ASCII glyph per source byte.  Editing selections are
            // byte-indexed throughout the retained core, so this preserves
            // caret/selection geometry while still ensuring that the source
            // value never reaches the paint list.  For ordinary passwords
            // (the overwhelmingly common case) this is one mask glyph per
            // character; non-ASCII input remains safely obscured as well.
            "*".repeat(text.len())
        } else {
            text
        }
    };
    if let Some(preedit) = controller.preedit() {
        let range = value.selection.range();
        let mut text = value.text;
        text.replace_range(range.as_range(), &preedit);
        mask(text)
    } else if value.text.is_empty() {
        placeholder.to_owned()
    } else {
        mask(value.text.clone())
    }
}

fn valid_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}

fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(text)
        .take_while(|index| *index < offset)
        .last()
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(text)
        .find(|index| *index > offset)
        .unwrap_or(text.len())
}
fn line_caret_x(line: &incular_text::TextLine, byte: usize) -> f32 {
    if byte >= line.caret_end {
        return line.width;
    }
    let mut x = 0.;
    for glyph in line.glyphs.iter() {
        if glyph.cluster as usize >= byte {
            break;
        }
        x = (glyph.offset.x + glyph.advance).max(x);
    }
    x
}
fn line_caret_x_for_affinity(
    layout: &TextLayout,
    line: usize,
    byte: usize,
    affinity: incular_text::TextAffinity,
) -> f32 {
    layout
        .line_caret_positions(line)
        .iter()
        .find(|position| position.offset == byte && position.affinity == affinity)
        .or_else(|| {
            layout
                .line_caret_positions(line)
                .iter()
                .find(|position| position.offset == byte)
        })
        .map_or_else(
            || {
                layout
                    .lines
                    .get(line)
                    .map_or(0.0, |line| line_caret_x(line, byte))
            },
            |position| position.x,
        )
}
fn caret_for_line_x(line: &incular_text::TextLine, x: f32) -> usize {
    if x >= line.width {
        return line.caret_end;
    }
    let mut best = 0usize;
    for glyph in line.glyphs.iter() {
        let midpoint = glyph.offset.x + glyph.advance / 2.;
        if x < midpoint {
            return glyph.cluster as usize;
        }
        best = (glyph.cluster as usize).max(best);
    }
    best.max(line.start)
}
fn caret_for_line_position(
    layout: &TextLayout,
    line: usize,
    x: f32,
) -> (usize, incular_text::TextAffinity) {
    let Some(line_data) = layout.lines.get(line) else {
        return (0, incular_text::TextAffinity::Downstream);
    };
    let positions = layout.line_caret_positions(line);
    positions
        .iter()
        .min_by(|left, right| {
            (left.x - x)
                .abs()
                .total_cmp(&(right.x - x).abs())
                .then_with(|| left.x.total_cmp(&right.x))
        })
        .map_or_else(
            || {
                (
                    caret_for_line_x(line_data, x),
                    incular_text::TextAffinity::Downstream,
                )
            },
            |position| (position.offset, position.affinity),
        )
}
fn line_for_byte(layout: &TextLayout, byte: usize) -> usize {
    layout
        .lines
        .iter()
        .position(|line| byte <= line.end)
        .unwrap_or_else(|| layout.lines.len().saturating_sub(1))
}
fn caret_geometry(
    layout: &TextLayout,
    byte: usize,
    affinity: incular_text::TextAffinity,
) -> (f32, f32, f32) {
    let index = line_for_byte(layout, byte);
    let line = layout.lines.get(index);
    (
        line.map_or(0., |_| {
            line_caret_x_for_affinity(layout, index, byte, affinity)
        }),
        index as f32 * layout.metrics.line_height,
        layout.metrics.line_height,
    )
}
fn selection_rects(
    layout: &TextLayout,
    selection: TextRange,
    scroll_x: f32,
    scroll_y: f32,
    top: f32,
) -> Vec<Rect> {
    layout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let start = selection.start.max(line.start);
            let end = selection.end.min(line.end);
            (start < end
                || (line.start == line.end
                    && selection.start <= line.start
                    && selection.end >= line.end))
                .then(|| {
                    Rect::from_origin_size(
                        Offset::new(
                            line_caret_x(line, start) - scroll_x + 8.,
                            index as f32 * layout.metrics.line_height - scroll_y + top,
                        ),
                        Size::new(
                            (line_caret_x(line, end) - line_caret_x(line, start)).max(1.),
                            layout.metrics.line_height,
                        ),
                    )
                })
        })
        .collect()
}

fn static_selection_range(
    tree: &WidgetTree,
    entries: &[ElementId],
    selection: StaticSelection,
    element: ElementId,
) -> Option<TextRange> {
    let anchor_index = entries
        .iter()
        .position(|entry| *entry == selection.anchor.element)?;
    let extent_index = entries
        .iter()
        .position(|entry| *entry == selection.extent.element)?;
    let element_index = entries.iter().position(|entry| *entry == element)?;
    let (first_index, last_index, first_byte, last_byte) = if anchor_index <= extent_index {
        (
            anchor_index,
            extent_index,
            selection.anchor.byte,
            selection.extent.byte,
        )
    } else {
        (
            extent_index,
            anchor_index,
            selection.extent.byte,
            selection.anchor.byte,
        )
    };
    if !(first_index..=last_index).contains(&element_index) {
        return None;
    }
    let text = tree.selectable_text_value(element)?;
    let start = if element_index == first_index {
        valid_boundary(&text, first_byte)
    } else {
        0
    };
    let end = if element_index == last_index {
        valid_boundary(&text, last_byte)
    } else {
        text.len()
    };
    (start < end).then_some(TextRange::new(start, end))
}

fn static_selection_text(
    tree: &WidgetTree,
    entries: &[ElementId],
    selection: StaticSelection,
) -> String {
    entries
        .iter()
        .filter_map(|element| {
            let range = static_selection_range(tree, entries, selection, *element)?;
            let text = tree.selectable_text_value(*element)?;
            Some(text[range.start..range.end].to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    use serde_json::{Value, json};

    use super::*;
    use crate::drag_drop::DragDropContext;
    use crate::internal::ActionSurface;
    use crate::scrolling::{
        CustomScrollView, SliverFixedExtentList, SliverList, SliverVariedExtentList,
    };
    use crate::{
        AbsorbPointer, DismissDirection, Dismissible, DragTarget, Draggable, Expanded,
        ExplicitSemantics, Flexible, IgnorePointer, IndexedStack, Positioned, SizedBox, Spacer,
    };

    #[derive(Default)]
    struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

    fn fixed_sliver_list<W>(
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

    fn restoration_key(value: &str) -> RestorationKey {
        RestorationKey::new(value).unwrap()
    }

    fn restoration_scope() -> RestorationScope {
        RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
            .child_unchecked(restoration_key("window"))
            .child_unchecked(restoration_key("main"))
    }

    #[test]
    fn nested_environment_scopes_keep_all_typed_values() {
        let outer = Color::rgba(12, 34, 56, 255);
        let inner = TextStyle::new().font_size(24.0);
        let environment = compose_environment(
            Some(Rc::new(inner.clone()) as Rc<dyn Any>),
            Some(Rc::new(outer) as Rc<dyn Any>),
        )
        .expect("nested environment");

        with_build_environment(Some(environment), || {
            assert_eq!(
                current_build_environment::<TextStyle>(),
                Some(inner.clone())
            );
            assert_eq!(current_build_environment::<Color>(), Some(outer));
        });
    }

    #[test]
    fn default_text_style_is_resolved_for_descendant_text() {
        let default_style = TextStyle::new().font_size(24.0).color(Color::BLACK);
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(crate::DefaultTextStyle::new(default_style.clone(), Text::new("hello")).into())
            .expect("mount default text style");
        tree.layout(Constraints::tight(Size::new(200.0, 80.0)));

        let child = tree.children(root).expect("materialized text")[0];
        let render = tree
            .renders
            .get(tree.render_id(child).expect("text render").0)
            .expect("text render object");
        let RenderKind::Text { style, .. } = &render.kind else {
            panic!("expected text render kind");
        };
        assert_eq!(style.size, default_style.size);
        assert_eq!(style.color, Color::BLACK);
    }

    fn rect_origins(list: &DisplayList) -> Vec<Offset> {
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
    fn glyph_origins(list: &DisplayList) -> Vec<Offset> {
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
    fn rrect_origins(list: &DisplayList) -> Vec<Offset> {
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
    fn box_(key: u64) -> Widget {
        Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(key)
    }
    #[test]
    fn limited_and_overflow_boxes_apply_their_distinct_constraint_policies() {
        let mut limited = WidgetTree::new();
        let root = limited
            .mount(Widget::limited_box(
                20.,
                30.,
                Widget::fixed_box(Size::new(80., 80.), Color::WHITE),
            ))
            .unwrap();
        limited.layout(Constraints::new(0., f32::INFINITY, 0., f32::INFINITY));
        assert_eq!(
            limited.render_size(limited.render_id(root).unwrap()),
            Some(Size::new(20., 30.))
        );
        limited.layout(Constraints::new(0., 100., 0., 100.));
        assert_eq!(
            limited.render_size(limited.render_id(root).unwrap()),
            Some(Size::new(80., 80.))
        );

        let mut overflow = WidgetTree::new();
        let root = overflow
            .mount(Widget::overflow_box(
                None,
                Some(80.),
                None,
                Some(80.),
                Widget::fixed_box(Size::new(80., 80.), Color::WHITE),
            ))
            .unwrap();
        overflow.layout(Constraints::tight(Size::new(20., 20.)));
        let child = overflow.children(root).unwrap()[0];
        assert_eq!(
            overflow.render_size(overflow.render_id(root).unwrap()),
            Some(Size::new(20., 20.))
        );
        assert_eq!(
            overflow.render_size(overflow.render_id(child).unwrap()),
            Some(Size::new(80., 80.))
        );
    }
    #[test]
    fn flexible_expanded_and_spacer_allocate_bounded_main_axis_space() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::row(vec![
                Widget::fixed_box(Size::new(10., 10.), Color::WHITE),
                Expanded::new(Widget::fixed_box(Size::new(1., 10.), Color::WHITE))
                    .flex(2)
                    .into(),
                Flexible::new(Widget::fixed_box(Size::new(15., 10.), Color::WHITE))
                    .flex(1)
                    .into(),
                Spacer::new().flex(1).into(),
            ]))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 20.)));
        let children = tree.children(root).unwrap().to_vec();
        assert_eq!(
            tree.render_size(tree.render_id(children[1]).unwrap()),
            Some(Size::new(45., 10.))
        );
        assert_eq!(
            tree.render_size(tree.render_id(children[2]).unwrap()),
            Some(Size::new(15., 10.))
        );
        assert_eq!(
            tree.render_size(tree.render_id(children[3]).unwrap()),
            Some(Size::new(22.5, 0.))
        );
    }
    #[test]
    fn positioned_and_indexed_stacks_keep_only_the_selected_branch_interactive_and_semantic() {
        let positioned = Positioned::new(Widget::fixed_box(Size::new(100., 100.), Color::WHITE))
            .left(10.)
            .right(20.)
            .top(5.)
            .bottom(15.);
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::stack(Alignment::TOP_LEFT, vec![positioned.into()]))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let positioned = tree.children(root).unwrap()[0];
        assert_eq!(
            tree.render_size(tree.render_id(positioned).unwrap()),
            Some(Size::new(70., 80.))
        );
        assert_eq!(
            tree.render_origin(tree.render_id(positioned).unwrap()),
            Offset::new(10., 5.)
        );

        let mut indexed = WidgetTree::new();
        let _root = indexed
            .mount(
                IndexedStack::new([Widget::text("hidden"), Widget::text("shown")])
                    .index(1)
                    .into(),
            )
            .unwrap();
        indexed.layout(Constraints::tight(Size::new(100., 40.)));
        indexed.update_semantics();
        assert_eq!(indexed.semantics().len(), 1);
        assert!(indexed.semantics_debug_dump().contains("shown"));
        assert!(!indexed.semantics_debug_dump().contains("hidden"));
    }
    #[test]
    fn layout_builder_rebuilds_only_when_constraints_change() {
        let builds = Rc::new(Cell::new(0));
        let observed = builds.clone();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::layout_builder(move |constraints| {
                observed.set(observed.get() + 1);
                Widget::fixed_box(Size::new(constraints.max_width, 10.), Color::WHITE)
            }))
            .unwrap();
        tree.layout(Constraints::new(0., 30., 0., 20.));
        let child = tree.children(root).unwrap()[0];
        assert_eq!(
            tree.render_size(tree.render_id(child).unwrap()),
            Some(Size::new(30., 10.))
        );
        tree.layout(Constraints::new(0., 30., 0., 20.));
        assert_eq!(builds.get(), 1);
        tree.layout(Constraints::new(0., 40., 0., 20.));
        assert_eq!(builds.get(), 2);
    }
    #[test]
    fn plain_text_uses_the_documented_natural_default_style() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Text::new("hello").into()).unwrap();
        tree.layout(Constraints::loose(Size::new(400., 100.)));
        let render = tree.render_id(root).unwrap();
        let size = tree.render_size(render).unwrap();
        assert!(size.width > 0.0 && size.width < 100.0);
        assert!(size.height >= 16.0);
    }
    #[test]
    fn icons_fit_and_center_their_declared_logical_box() {
        let icon: Widget = Icon::new(icons::check()).size(12.).into();
        let RenderKind::Shape { path, desired, .. } = render_kind(&icon, None) else {
            panic!("Icon should retain a shape render kind");
        };
        let bounds = path.bounds().expect("check path has geometry");
        assert_eq!(desired, Size::new(12., 12.));
        assert!(bounds.origin.x.abs() < 0.001);
        assert!(bounds.size.width <= 12.001);
        assert!(bounds.size.height <= 12.001);
        assert!((bounds.origin.x + bounds.size.width - 12.).abs() < 0.001);
        assert!((bounds.origin.y + bounds.size.height * 0.5 - 6.).abs() < 0.001);
    }
    #[test]
    fn stateful_layout_builder_rebuilds_when_local_revision_changes() {
        let builds = Rc::new(Cell::new(0));
        let revision = Rc::new(Cell::new(0));
        let observed = builds.clone();
        let state = revision.clone();
        let builder_state = state.clone();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::stateful_layout_builder(revision, move |_| {
                observed.set(observed.get() + 1);
                Widget::fixed_box(
                    Size::new(10. + builder_state.get() as f32, 10.),
                    Color::WHITE,
                )
            }))
            .unwrap();
        let constraints = Constraints::loose(Size::new(80., 20.));
        tree.layout(constraints);
        assert_eq!(builds.get(), 1);
        assert_eq!(
            tree.render_size(tree.render_id(tree.children(root).unwrap()[0]).unwrap()),
            Some(Size::new(10., 10.))
        );
        state.set(3);
        tree.layout(constraints);
        assert_eq!(builds.get(), 2);
        assert_eq!(
            tree.render_size(tree.render_id(tree.children(root).unwrap()[0]).unwrap()),
            Some(Size::new(13., 10.))
        );
    }
    #[test]
    fn affine_transform_uses_inverse_hit_testing_and_transformed_semantics() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::transform(
                CoreTransform::translation(Offset::new(20., 10.)),
                Widget::button(Size::new(10., 10.), Color::WHITE, ActionId(1)),
            ))
            .unwrap();
        let button = tree.children(root).unwrap()[0];
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let (changed, _) = tree.update_compositor(Instant::now());
        assert!(changed);
        assert_eq!(
            tree.element_for_render(tree.hit_test(Offset::new(25., 15.)).unwrap()),
            Some(button)
        );
        assert!(tree.hit_test(Offset::new(5., 5.)).is_none());
        tree.update_semantics();
        let semantic = tree
            .semantic_node_for_element(button)
            .and_then(|id| tree.semantics().node(id))
            .expect("button semantics");
        assert_eq!(semantic.bounds.origin, Offset::new(20., 10.));
        assert_eq!(semantic.bounds.size, Size::new(10., 10.));
    }
    #[test]
    fn fitted_box_scales_hits_into_the_child_coordinate_space() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::fitted_box(
                ImageFit::Contain,
                Alignment::CENTER,
                Widget::box_(Size::new(10., 20.), Color::WHITE),
            ))
            .unwrap();
        let child = tree.children(root).unwrap()[0];
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            tree.element_for_render(tree.hit_test(Offset::new(30., 10.)).unwrap()),
            Some(child)
        );
        assert!(tree.hit_test(Offset::new(20., 10.)).is_none());
    }

    #[test]
    fn selectable_text_drag_uses_cached_parley_layout_and_copies_across_widgets() {
        let controller = SelectionAreaController::new();
        let mut tree = WidgetTree::new();
        let area = tree
            .mount(Widget::selection_area(
                controller.clone(),
                Widget::column(vec![
                    Widget::selectable_text_styled(
                        "Latin café",
                        TextStyle::default(),
                        TextAlign::Start,
                    ),
                    Widget::selectable_text_styled(
                        "עברית mixed 世界",
                        TextStyle::default(),
                        TextAlign::Start,
                    ),
                ]),
            ))
            .expect("mount selection area");
        tree.layout(Constraints::tight(Size::new(180., 80.)));
        let column = tree.children(area).unwrap()[0];
        let labels = tree.children(column).unwrap();
        let first = labels[0];
        let second = labels[1];
        let first_render = tree.render_id(first).unwrap();
        let second_render = tree.render_id(second).unwrap();
        let first_origin = tree
            .render_world_transform(first_render)
            .transform_point(Offset::ZERO);
        let second_origin = tree
            .render_world_transform(second_render)
            .transform_point(Offset::ZERO);
        assert!(tree.selectable_text_set_selection(first, first_origin, false));
        assert!(tree.selectable_text_set_selection(
            second,
            second_origin + Offset::new(10_000., 0.),
            true,
        ));
        assert_eq!(controller.selected_text(), "Latin café\nעברית mixed 世界");
        let before = tree.text_engine.diagnostics().layouts_requested;
        let _ = tree.paint();
        assert_eq!(tree.text_engine.diagnostics().layouts_requested, before);
        assert!(tree.static_selection_range(first).is_some());
        assert!(tree.static_selection_range(second).is_some());
    }

    #[test]
    fn selectable_text_keyboard_motion_stays_on_grapheme_boundaries() {
        let controller = SelectionAreaController::new();
        let mut tree = WidgetTree::new();
        let area = tree
            .mount(Widget::selection_area(
                controller.clone(),
                Widget::selectable_text_styled(
                    "a👩\u{200d}💻b",
                    TextStyle::default(),
                    TextAlign::Start,
                ),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(180., 40.)));
        let label = tree.children(area).unwrap()[0];
        assert!(tree.selectable_text_move(label, true, false));
        assert!(tree.selectable_text_move(label, true, true));
        assert_eq!(controller.selected_text(), "👩\u{200d}💻");
        assert!(tree.selectable_text_select_all(label));
        assert_eq!(controller.selected_text(), "a👩\u{200d}💻b");
    }

    #[test]
    fn standalone_selectable_text_has_its_own_read_only_selection_region() {
        let mut tree = WidgetTree::new();
        let label = tree
            .mount(Widget::selectable_text_styled(
                "copy me",
                TextStyle::default(),
                TextAlign::Start,
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 30.)));
        assert!(tree.selectable_text_select_all(label));
        assert_eq!(
            tree.selectable_text_selected_text(label),
            Some("copy me".into())
        );
    }

    #[test]
    fn retained_text_honors_wrap_line_limit_overflow_and_rich_text_conversion() {
        let mut tree = WidgetTree::new();
        let label = tree
            .mount(
                Text::new("one two three four five six seven")
                    .style(TextStyle::default().font_size(18.))
                    .max_lines(Some(1))
                    .overflow(TextOverflow::Ellipsis)
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(75., 30.)));
        let render = tree.render_id(label).unwrap();
        let layout = tree
            .renders
            .get(render.0)
            .unwrap()
            .text_layout
            .clone()
            .unwrap();
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.overflowed);

        let rich: Widget = RichText::new(incular_text::TextSpan::new("one two three four"))
            .max_lines(Some(1))
            .overflow(TextOverflow::Clip)
            .into();
        let rich_label = tree.mount(rich).unwrap();
        tree.layout(Constraints::tight(Size::new(60., 30.)));
        let rich_render = tree.render_id(rich_label).unwrap();
        assert!(
            tree.renders
                .get(rich_render.0)
                .unwrap()
                .text_layout
                .as_ref()
                .unwrap()
                .overflowed
        );
    }
    #[test]
    fn scale_and_rotation_transitions_update_only_retained_compositor_layers() {
        let scale = ScaleController::new();
        let rotation = RotationController::new();
        let mut tree = WidgetTree::new();
        tree.mount(
            RotationTransition::new(
                rotation.clone(),
                ScaleTransition::new(
                    scale.clone(),
                    Widget::box_(Size::new(20., 20.), Color::WHITE),
                ),
            )
            .into(),
        )
        .unwrap();
        tree.layout(Constraints::tight(Size::new(80., 80.)));
        let _ = tree.paint();
        let before = tree.diagnostics();
        scale.set_scale(1.5);
        rotation.set_radians(0.25);
        assert!(tree.update_compositor(Instant::now()).0);
        let after = tree.diagnostics();
        assert_eq!(after.layouts, before.layouts);
        assert_eq!(after.paints, before.paints);
    }

    #[test]
    fn rotation_transition_quarter_turn_uses_compositor_transform() {
        let controller = RotationController::new();
        assert!(controller.set_turns(0.25));
        assert!((controller.turns() - 0.25).abs() < 0.0001);
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                RotationTransition::new(
                    controller,
                    Widget::box_(Size::new(20., 10.), Color::WHITE),
                )
                .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(80., 80.)));
        let render = tree.render_id(root).unwrap();
        let transform = tree.content_transform(render).expect("rotation transform");
        let bounds = transform
            .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, Size::new(20., 10.)));
        assert!((bounds.size.width - 10.).abs() < 0.001);
        assert!((bounds.size.height - 20.).abs() < 0.001);
    }

    #[test]
    fn rotation_transition_alignment_changes_pivot_and_hit_testing_follows_it() {
        let center = RotationController::new();
        center.set_turns(0.25);
        let top_left = RotationController::new();
        top_left.set_turns(0.25);
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::row(vec![
                RotationTransition::new(center, Widget::box_(Size::new(20., 10.), Color::WHITE))
                    .into(),
                RotationTransition::new(top_left, Widget::box_(Size::new(20., 10.), Color::WHITE))
                    .alignment(Alignment::TOP_LEFT)
                    .into(),
            ]))
            .unwrap();
        let rotations = tree.children(root).expect("row children");
        let centered = rotations[0];
        let aligned = rotations[1];
        tree.layout(Constraints::tight(Size::new(80., 80.)));
        let centered_transform = tree
            .content_transform(tree.render_id(centered).unwrap())
            .expect("centered transform");
        let aligned_transform = tree
            .content_transform(tree.render_id(aligned).unwrap())
            .expect("aligned transform");
        assert_ne!(
            centered_transform.translation_offset(),
            aligned_transform.translation_offset()
        );

        let child = tree.children(aligned).expect("rotation child")[0];
        let aligned_bounds = aligned_transform
            .transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, Size::new(20., 10.)));
        let hit_point = tree.render_origin(tree.render_id(aligned).unwrap())
            + aligned_bounds.origin
            + Offset::new(
                aligned_bounds.size.width * 0.5,
                aligned_bounds.size.height * 0.5,
            );
        let hit = tree.hit_test(hit_point).expect("rotated hit");
        assert_eq!(tree.element_for_render(hit), Some(child));
    }
    #[test]
    fn replacement_opacity_controller_keeps_a_controlled_transition_alive() {
        let old_controller = OpacityController::new();
        old_controller.set_opacity(0.);
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                Opacity::controlled(
                    old_controller,
                    Widget::box_(Size::new(20., 20.), Color::WHITE),
                )
                .into(),
            )
            .unwrap();
        let constraints = Constraints::tight(Size::new(40., 40.));
        let start = Instant::now();
        tree.layout(constraints);
        tree.update_compositor(start);

        let replacement = OpacityController::new();
        tree.update(
            root,
            Opacity::controlled(
                replacement.clone(),
                Widget::box_(Size::new(20., 20.), Color::WHITE),
            )
            .into(),
        )
        .unwrap();
        assert!(replacement.opacity() < 0.01);
        tree.update_compositor(start + Duration::from_millis(70));
        assert!(replacement.opacity() > 0.01 && replacement.opacity() < 0.99);
    }
    #[test]
    fn keyed_reorder_reuses_elements() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::row(vec![box_(1), box_(2), box_(3)]))
            .unwrap();
        let before = tree.children(root).unwrap().to_vec();
        tree.update(root, Widget::row(vec![box_(3), box_(1), box_(2)]))
            .unwrap();
        let after = tree.children(root).unwrap();
        assert_eq!(after, &[before[2], before[0], before[1]]);
    }
    #[test]
    fn removed_ids_are_stale_and_unmounted_once() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Widget::row(vec![box_(1), box_(2)])).unwrap();
        let removed = tree.children(root).unwrap()[1];
        tree.update(root, Widget::row(vec![box_(1)])).unwrap();
        assert!(!tree.element_exists(removed));
        assert_eq!(tree.diagnostics().unmounts, 1);
    }
    #[test]
    fn nested_layout_and_paint_cache_are_incremental() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::padding(
                EdgeInsets::all(2.),
                Widget::box_(Size::new(10., 5.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(20., 20.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(20., 20.))
        );
        let first = tree.paint();
        let paints = tree.diagnostics().paints;
        let _ = tree.paint();
        assert_eq!(tree.diagnostics().paints, paints);
        assert!(!first.is_empty());
    }

    #[test]
    fn constrained_box_tightens_child_bounds_without_escaping_the_parent() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::constrained(
                Constraints::new(10., 15., 6., 12.),
                Widget::fixed_box(Size::new(5., 20.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::loose(Size::new(20., 20.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(10., 12.))
        );

        tree.layout(Constraints::tight(Size::new(8., 8.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(8., 8.))
        );
    }

    #[test]
    fn unconstrained_box_uses_natural_child_size_but_stays_parent_bounded() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::unconstrained(
                None,
                Widget::fixed_box(Size::new(30., 5.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::loose(Size::new(20., 20.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(20., 5.))
        );
        let child = tree.children(root).unwrap()[0];
        assert_eq!(
            tree.render_size(tree.render_id(child).unwrap()),
            Some(Size::new(30., 5.))
        );
    }

    #[test]
    fn wrap_starts_a_new_run_when_a_child_exceeds_the_remaining_main_axis() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::wrap(
                Axis::Horizontal,
                2.,
                3.,
                vec![
                    Widget::fixed_box(Size::new(8., 4.), Color::WHITE),
                    Widget::fixed_box(Size::new(8., 6.), Color::WHITE),
                ],
            ))
            .unwrap();
        tree.layout(Constraints::loose(Size::new(17., 20.)));
        let children = tree.children(root).unwrap();
        assert_eq!(
            tree.render_origin(tree.render_id(children[0]).unwrap()),
            Offset::ZERO
        );
        assert_eq!(
            tree.render_origin(tree.render_id(children[1]).unwrap()),
            Offset::new(0., 7.)
        );
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(8., 13.))
        );
    }

    #[test]
    fn fractional_box_tightens_requested_axes_to_parent_factors() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::fractionally_sized(
                Some(0.5),
                Some(0.25),
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(40., 20.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(40., 20.))
        );
        let child = tree.children(root).unwrap()[0];
        assert_eq!(
            tree.render_size(tree.render_id(child).unwrap()),
            Some(Size::new(20., 5.))
        );
    }

    #[test]
    fn table_uses_max_content_cell_sizes_and_row_major_offsets() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::table(
                2,
                2.,
                3.,
                vec![
                    Widget::fixed_box(Size::new(10., 4.), Color::WHITE),
                    Widget::fixed_box(Size::new(5., 8.), Color::WHITE),
                    Widget::fixed_box(Size::new(7., 6.), Color::WHITE),
                ],
            ))
            .unwrap();
        tree.layout(Constraints::loose(Size::new(40., 40.)));
        let children = tree.children(root).unwrap();
        assert_eq!(
            tree.render_origin(tree.render_id(children[1]).unwrap()),
            Offset::new(12., 0.)
        );
        assert_eq!(
            tree.render_origin(tree.render_id(children[2]).unwrap()),
            Offset::new(0., 11.)
        );
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(17., 17.))
        );
    }

    #[test]
    fn baseline_offsets_a_child_using_its_bottom_as_the_default_baseline() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::baseline(
                10.,
                Widget::fixed_box(Size::new(8., 5.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::loose(Size::new(20., 20.)));
        let child = tree.children(root).unwrap()[0];
        assert_eq!(
            tree.render_origin(tree.render_id(child).unwrap()),
            Offset::new(0., 5.)
        );
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(8., 10.))
        );
    }

    #[test]
    fn merged_transition_composes_retained_fade_and_slide_layers() {
        let opacity = OpacityController::new();
        let translation = TranslationController::new();
        let _: Widget = Transition::new(Widget::fixed_box(Size::new(1., 1.), Color::WHITE))
            .fade(opacity)
            .slide(translation)
            .into();
    }

    #[test]
    fn custom_paint_replays_its_display_list_in_the_retained_picture() {
        let mut display_list = DisplayList::new();
        display_list.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 3.)),
            color: Color::WHITE,
        });
        let mut tree = WidgetTree::new();
        tree.mount(Widget::custom_paint(Size::new(10., 8.), display_list))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(10., 8.)));
        assert!(tree.paint().commands().iter().any(|command| {
            matches!(command, PaintCommand::Rect { rect, .. } if rect.size == Size::new(4., 3.))
        }));
    }

    #[test]
    fn repaint_boundary_keeps_its_picture_when_child_custom_paint_changes() {
        let list = |color| {
            let mut list = DisplayList::new();
            list.push(PaintCommand::Rect {
                rect: Rect::from_origin_size(Offset::ZERO, Size::new(4., 3.)),
                color,
            });
            list
        };
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::repaint_boundary(Widget::custom_paint(
                Size::new(10., 8.),
                list(Color::WHITE),
            )))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(10., 8.)));
        let _ = tree.paint();
        let before = tree.diagnostics().paints;
        tree.update(
            root,
            Widget::repaint_boundary(Widget::custom_paint(Size::new(10., 8.), list(Color::BLACK))),
        )
        .unwrap();
        let _ = tree.paint();
        assert_eq!(tree.diagnostics().paints, before + 1);
    }

    #[test]
    fn stack_aligns_children_and_hits_the_frontmost_child() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::stack(
                Alignment::CENTER,
                vec![
                    Widget::fixed_box(Size::new(20., 20.), Color::BLACK),
                    Widget::button(Size::new(10., 10.), Color::WHITE, ActionId(1)),
                ],
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(20., 20.)));
        let children = tree.children(root).unwrap();
        let front = children[1];
        assert_eq!(
            tree.render_origin(tree.render_id(front).unwrap()),
            Offset::new(5., 5.)
        );
        assert_eq!(
            tree.element_for_render(tree.hit_test(Offset::new(10., 10.)).unwrap()),
            Some(front)
        );
    }

    #[test]
    fn invisible_widgets_skip_child_layout_hit_testing_and_semantics() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::visibility(
                false,
                Widget::button(Size::new(20., 20.), Color::WHITE, ActionId(1)),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(20., 20.)));
        tree.update_semantics();
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(20., 20.))
        );
        assert_eq!(tree.hit_test(Offset::new(10., 10.)), None);
        assert_eq!(tree.semantics().len(), 0);
    }

    #[test]
    fn aspect_ratio_uses_the_largest_fitting_box() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::aspect_ratio(
                2.,
                Widget::fixed_box(Size::new(1., 1.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::loose(Size::new(100., 80.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(100., 50.))
        );
    }
    #[test]
    fn duplicate_local_keys_are_rejected() {
        let mut tree = WidgetTree::new();
        assert_eq!(
            tree.mount(Widget::row(vec![box_(1), box_(1)])).unwrap_err(),
            TreeError::DuplicateKey(Key::Value(1))
        );
    }
    #[test]
    fn hit_test_uses_reverse_paint_order_and_nested_offsets() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::padding(
                EdgeInsets::all(2.),
                Widget::row(vec![
                    Widget::box_(Size::new(10., 10.), Color::WHITE),
                    Widget::box_(Size::new(10., 10.), Color::BLACK),
                ]),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(30., 20.)));
        let hit = tree.hit_test(Offset::new(13., 5.)).unwrap();
        let row = tree.children(root).unwrap()[0];
        let second = tree.children(row).unwrap()[1];
        assert_eq!(tree.element_for_render(hit), Some(second));
    }
    #[test]
    fn retained_pictures_apply_column_row_and_padding_offsets_once() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::padding(
            EdgeInsets {
                left: 20.,
                top: 10.,
                right: 0.,
                bottom: 0.,
            },
            Widget::column(vec![
                Widget::box_(Size::new(100., 20.), Color::WHITE),
                Widget::row(vec![
                    Widget::box_(Size::new(30., 30.), Color::WHITE),
                    Widget::box_(Size::new(40., 30.), Color::WHITE),
                ]),
                Widget::box_(Size::new(100., 40.), Color::WHITE),
            ]),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 200.)));
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(20., 10.),
                Offset::new(20., 30.),
                Offset::new(50., 30.),
                Offset::new(20., 60.),
            ]
        );
    }
    #[test]
    fn retained_text_pictures_keep_independent_column_origins() {
        let style = TextStyle {
            size: 20.,
            line_height: Some(incular_text::LineHeight::Absolute(30.)),
            ..TextStyle::default()
        };
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::column(vec![
                Widget::text_styled("A", style.clone(), TextAlign::Start),
                Widget::text_styled("B", style.clone(), TextAlign::Start),
                Widget::text_styled("C", style, TextAlign::Start),
            ]))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 200.)));
        let origins = glyph_origins(&tree.paint());
        assert_eq!(origins.len(), 3);
        let children = tree.children(root).unwrap();
        let a_height = tree
            .render_size(tree.render_id(children[0]).unwrap())
            .unwrap()
            .height;
        let b_height = tree
            .render_size(tree.render_id(children[1]).unwrap())
            .unwrap()
            .height;
        assert_eq!(origins[1].y - origins[0].y, a_height);
        assert_eq!(origins[2].y - origins[1].y, b_height);
        assert!(origins[0].y < origins[1].y && origins[1].y < origins[2].y);
    }
    #[test]
    fn button_label_receives_the_button_parent_placement() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::column(vec![
            Widget::box_(Size::new(100., 30.), Color::BLACK),
            ActionSurface::new("Placed label")
                .color(Color::rgba(70, 120, 220, 255))
                .into(),
        ]))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 120.)));
        let list = tree.paint();
        let button_origin = rrect_origins(&list)[0];
        let label_origin = glyph_origins(&list)[0];
        assert_eq!(button_origin.y, 30.);
        assert!(label_origin.y >= button_origin.y);
    }
    #[test]
    fn compositional_button_keeps_configured_size_and_content_semantics() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                ActionSurface::new("Inspector row")
                    .size(Size::new(180., 34.))
                    .content(Widget::row([
                        Widget::text("Inspector"),
                        Widget::text("row"),
                    ]))
                    .into(),
            )
            .unwrap();

        tree.layout(Constraints::new(0., 300., 0., 100.));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(180., 34.))
        );

        tree.update_semantics();
        let semantic = tree
            .semantic_node_for_element(root)
            .and_then(|id| tree.semantics().node(id))
            .expect("button semantics");
        assert_eq!(semantic.role, SemanticRole::Button);
        assert_eq!(semantic.label.as_deref(), Some("Inspector row"));
    }
    #[test]
    fn scroll_and_animation_compose_with_static_layout_placement() {
        let scroll = ScrollController::new();
        let translation = TranslationController::new();
        translation.set_offset(Offset::new(15., 0.));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::padding(
            EdgeInsets {
                left: 0.,
                top: 10.,
                right: 0.,
                bottom: 0.,
            },
            Widget::scroll_view(
                scroll.clone(),
                Widget::translate(
                    translation.clone(),
                    Widget::column(vec![
                        Widget::box_(Size::new(40., 20.), Color::WHITE),
                        Widget::box_(Size::new(40., 30.), Color::WHITE),
                    ]),
                ),
            ),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 40.));
        tree.layout(constraints);
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![Offset::new(15., 10.), Offset::new(15., 30.)]
        );
        assert!(scroll.jump_to(10.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![Offset::new(15., 0.), Offset::new(15., 20.)]
        );
    }
    #[test]
    fn transparent_opacity_keeps_hit_testing_and_semantics() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::opacity(
            0.,
            ActionSurface::new("Still active").into(),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(140., 60.)));
        tree.update_semantics();
        assert!(tree.hit_test(Offset::new(10., 10.)).is_some());
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.role == SemanticRole::Button)
        );
    }
    #[test]
    fn effect_parameter_animation_is_compositor_only_and_keeps_semantics() {
        let blur = BlurController::new(2.);
        let mut tree = WidgetTree::new();
        tree.mount(Blur::controlled(blur.clone(), ActionSurface::new("Still active")).into())
            .unwrap();
        tree.layout(Constraints::tight(Size::new(140., 60.)));
        let _ = tree.paint();
        let paints = tree.diagnostics().paints;
        let _ = tree.update_compositor(Instant::now());
        assert!(blur.set_sigma(14.));
        let (changed, _) = tree.update_compositor(Instant::now());
        assert!(changed);
        let list = tree.paint();
        assert!(list.commands().iter().any(|command| {
            matches!(command, PaintCommand::PushBlur { blur, .. } if blur.sigma_x == 14.)
        }));
        assert_eq!(tree.diagnostics().paints, paints);
        tree.update_semantics();
        assert!(tree.hit_test(Offset::new(10., 10.)).is_some());
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.role == SemanticRole::Button)
        );
    }
    #[test]
    fn text_picture_replacement_and_root_unmount_do_not_leak_layers() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Widget::text("Count: 0")).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.paint();
        let before = tree.compositor_diagnostics().layers;
        tree.update(root, Widget::text("Count: 1")).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.paint();
        assert_eq!(tree.compositor_diagnostics().layers, before);
        tree.update(root, Widget::text("Count: 2")).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.paint();
        assert_eq!(tree.compositor_diagnostics().layers, before);
        tree.mount(Widget::box_(Size::new(1., 1.), Color::WHITE))
            .unwrap();
        assert_eq!(tree.compositor_diagnostics().layers, 2);
    }
    #[test]
    fn scroll_positions_are_logical_clamped_and_clip_the_viewport() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::column(
                (0..5)
                    .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(controller.max_offset(), 100.);
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(0., 0.),
                Offset::new(0., 40.),
                Offset::new(0., 80.),
            ]
        );
        assert!(controller.jump_to(50.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(0., -10.),
                Offset::new(0., 30.),
                Offset::new(0., 70.),
            ]
        );
        assert!(controller.jump_to(10_000.));
        assert_eq!(controller.offset(), 100.);
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(0., -20.),
                Offset::new(0., 20.),
                Offset::new(0., 60.),
            ]
        );
        assert!(controller.jump_to(-1.));
        assert_eq!(controller.offset(), 0.);
    }

    #[test]
    fn horizontal_scroll_preserves_axis_in_layout_transform_and_hit_testing() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                crate::scrolling::SingleChildScrollView::new(Widget::row(
                    (0..5)
                        .map(|_| Widget::box_(Size::new(40., 30.), Color::WHITE))
                        .collect::<Vec<_>>(),
                ))
                .scroll_direction(Axis::Horizontal)
                .controller(controller.clone())
                .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(controller.max_offset(), 100.);
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(100., 40.))
        );
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.y < 40.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(0., 0.),
                Offset::new(40., 0.),
                Offset::new(80., 0.)
            ]
        );
        assert!(controller.jump_to(20.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.y < 40.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(-20., 0.),
                Offset::new(20., 0.),
                Offset::new(60., 0.)
            ]
        );
        // The retained hit test uses the same horizontal content transform.
        let row = tree.children(root).unwrap()[0];
        let second = tree.children(row).unwrap()[1];
        assert_eq!(
            tree.element_for_render(tree.hit_test(Offset::new(30., 10.)).unwrap()),
            Some(second)
        );
        // Wheel input follows the configured axis; a horizontal delta must
        // not be discarded in favor of the vertical component.
        assert!(tree.scroll_at(Offset::new(30., 10.), Offset::new(20., 0.)));
        assert_eq!(controller.offset(), 40.);
    }

    #[test]
    fn reverse_scroll_starts_at_the_physical_trailing_edge() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                crate::scrolling::ListView::builder(5, |_| {
                    Widget::box_(Size::new(80., 40.), Color::WHITE)
                })
                .item_extent(40.)
                .reverse(true)
                .controller(controller.clone())
                .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(controller.max_offset(), 100.);
        let children = tree.children(root).unwrap().to_vec();
        assert_eq!(children.len(), 5);
        // Logical offset zero maps to the physical end for reverse views.
        assert_eq!(
            tree.render_origin(tree.render_id(children[0]).unwrap()).y,
            -100.
        );
        assert_eq!(
            tree.render_origin(tree.render_id(children[4]).unwrap()).y,
            60.
        );
        assert!(controller.jump_to(100.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            tree.render_origin(tree.render_id(children[0]).unwrap()).y,
            0.
        );
        assert_eq!(
            tree.render_origin(tree.render_id(children[4]).unwrap()).y,
            160.
        );
    }

    #[test]
    fn page_view_static_children_fill_viewport_and_preserve_reverse_direction() {
        let controller = crate::scrolling::PageController::new();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                crate::scrolling::PageView::new([
                    Widget::fixed_box(Size::new(20., 20.), Color::WHITE),
                    Widget::fixed_box(Size::new(20., 20.), Color::BLACK),
                ])
                .scroll_direction(Axis::Horizontal)
                .controller(controller.clone())
                .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        assert_eq!(controller.max_offset(), 100.);
        let pages = tree.children(root).expect("page children");
        assert_eq!(
            tree.render_size(tree.render_id(pages[0]).unwrap()),
            Some(Size::new(100., 40.))
        );
        assert_eq!(
            tree.render_size(tree.render_id(pages[1]).unwrap()),
            Some(Size::new(100., 40.))
        );
        assert!(controller.jump_to(100.));
        assert_eq!(
            tree.render_origin(tree.render_id(pages[0]).unwrap()).x,
            -100.
        );
        assert_eq!(tree.render_origin(tree.render_id(pages[1]).unwrap()).x, 0.);

        let reverse_controller = crate::scrolling::PageController::new();
        let mut reverse_tree = WidgetTree::new();
        let reverse_root = reverse_tree
            .mount(
                crate::scrolling::PageView::new([
                    Widget::fixed_box(Size::new(20., 20.), Color::WHITE),
                    Widget::fixed_box(Size::new(20., 20.), Color::BLACK),
                ])
                .scroll_direction(Axis::Horizontal)
                .reverse(true)
                .controller(reverse_controller.clone())
                .into(),
            )
            .unwrap();
        reverse_tree.layout(Constraints::tight(Size::new(100., 40.)));
        let reverse_pages = reverse_tree
            .children(reverse_root)
            .expect("reverse page children");
        assert_eq!(
            reverse_tree
                .render_origin(reverse_tree.render_id(reverse_pages[0]).unwrap())
                .x,
            -100.
        );
        assert_eq!(
            reverse_tree
                .render_origin(reverse_tree.render_id(reverse_pages[1]).unwrap())
                .x,
            0.
        );
    }

    #[test]
    fn scroll_physics_survives_descriptor_lowering_and_controls_input() {
        let controller = ScrollController::new();
        let physics = ScrollPhysics::clamping().never_scrollable();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                crate::scrolling::SingleChildScrollView::new(Widget::column(
                    (0..4)
                        .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                        .collect::<Vec<_>>(),
                ))
                .physics(physics)
                .controller(controller.clone())
                .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let render = tree.render_id(root).unwrap();
        assert!(matches!(
            &tree.renders.get(render.0).expect("scroll render").kind,
            RenderKind::Scroll { physics: retained, .. } if *retained == physics
        ));
        assert!(!tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 20.)));
        assert_eq!(controller.offset(), 0.);
    }

    #[test]
    fn persistent_headers_pin_in_flow_and_are_pushed_by_the_next_header() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::scroll_view(
                controller.clone(),
                Widget::column(vec![
                    Widget::fixed_box(Size::new(80., 40.), Color::BLACK),
                    Widget::persistent_header(
                        controller.clone(),
                        Widget::fixed_box(Size::new(80., 20.), Color::rgba(255, 0, 0, 255)),
                    ),
                    Widget::fixed_box(Size::new(80., 60.), Color::WHITE),
                    Widget::persistent_header(
                        controller.clone(),
                        Widget::fixed_box(Size::new(80., 20.), Color::rgba(0, 255, 0, 255)),
                    ),
                    Widget::fixed_box(Size::new(80., 200.), Color::BLACK),
                ]),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let flow = tree.children(root).unwrap()[0];
        let children = tree.children(flow).unwrap();
        let first = tree.render_id(children[1]).unwrap();
        let second = tree.render_id(children[3]).unwrap();
        assert_eq!(tree.render_origin(first), Offset::new(0., 40.));
        assert_eq!(tree.render_origin(second), Offset::new(0., 120.));

        assert!(controller.jump_to(50.));
        assert!(tree.update_compositor(Instant::now()).0);
        assert_eq!(tree.render_origin(first), Offset::ZERO);
        assert_eq!(tree.render_origin(second), Offset::new(0., 70.));

        assert!(controller.jump_to(130.));
        assert!(tree.update_compositor(Instant::now()).0);
        assert_eq!(tree.render_origin(first), Offset::new(0., -30.));
        assert_eq!(tree.render_origin(second), Offset::ZERO);
        // The flattened retained display list applies the same dynamic
        // transforms and clips the displaced predecessor out of the viewport.
        assert!(!rect_origins(&tree.paint()).contains(&Offset::new(0., -30.)));
        assert!(rect_origins(&tree.paint()).contains(&Offset::ZERO));
    }

    #[test]
    fn pinned_header_sliver_pins_vertical_without_rebuilding_on_scroll() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let slivers: Vec<Box<dyn crate::scrolling::Sliver>> = vec![
            Box::new(crate::scrolling::PinnedHeaderSliver::new(
                Widget::fixed_box(Size::new(100., 20.), Color::WHITE),
            )),
            Box::new(crate::scrolling::SliverToBoxAdapter::new(
                Widget::fixed_box(Size::new(100., 220.), Color::BLACK),
            )),
        ];
        let root = tree
            .mount(
                crate::scrolling::CustomScrollView::new(slivers)
                    .controller(controller.clone())
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        // A real sliver viewport retains its materialized sliver children
        // directly; there is no synthetic Column/flow box between the
        // viewport and its sliver children.
        let header = tree.children(root).expect("sliver children")[0];
        let header_render = tree.render_id(header).unwrap();
        assert_eq!(tree.render_size(header_render), Some(Size::new(100., 20.)));
        assert_eq!(tree.render_origin(header_render), Offset::ZERO);
        let before = tree.diagnostics();
        assert!(controller.jump_to(80.));
        assert!(tree.update_compositor(Instant::now()).0);
        assert_eq!(tree.render_origin(header_render), Offset::ZERO);
        assert_eq!(tree.diagnostics().rebuilds, before.rebuilds);
    }

    #[test]
    fn pinned_header_sliver_supports_horizontal_and_reverse_viewports() {
        let make_tree = |reverse| {
            let controller = ScrollController::new();
            let mut tree = WidgetTree::new();
            let slivers: Vec<Box<dyn crate::scrolling::Sliver>> = vec![
                Box::new(crate::scrolling::PinnedHeaderSliver::new(
                    Widget::fixed_box(Size::new(20., 40.), Color::WHITE),
                )),
                Box::new(crate::scrolling::SliverToBoxAdapter::new(
                    Widget::fixed_box(Size::new(220., 40.), Color::BLACK),
                )),
            ];
            let root = tree
                .mount(
                    crate::scrolling::CustomScrollView::new(slivers)
                        .scroll_direction(Axis::Horizontal)
                        .reverse(reverse)
                        .controller(controller.clone())
                        .into(),
                )
                .unwrap();
            tree.layout(Constraints::tight(Size::new(100., 40.)));
            let header = tree.children(root).expect("sliver children")[0];
            (tree, controller, header)
        };

        let (mut forward, controller, header) = make_tree(false);
        assert!(controller.jump_to(80.));
        forward.update_compositor(Instant::now());
        assert_eq!(
            forward.render_origin(forward.render_id(header).unwrap()).x,
            0.
        );

        let (mut reverse, controller, header) = make_tree(true);
        assert_eq!(controller.offset(), 0.);
        reverse.update_compositor(Instant::now());
        // Reverse content enters from the physical trailing edge, so the
        // pinned header's leading edge is the viewport's trailing edge.
        assert_eq!(
            reverse.render_origin(reverse.render_id(header).unwrap()).x,
            80.
        );
    }

    #[test]
    fn notification_listener_bubbles_sliver_events_and_honors_stop() {
        use incular_scroll::ScrollNotificationType;

        let controller = ScrollController::new();
        let inner_events = Rc::new(RefCell::new(Vec::new()));
        let outer_events = Rc::new(RefCell::new(Vec::new()));
        let observed_inner = inner_events.clone();
        let observed_outer = outer_events.clone();
        let sliver: Box<dyn crate::scrolling::Sliver> =
            Box::new(crate::scrolling::SliverToBoxAdapter::new(
                Widget::fixed_box(Size::new(100., 240.), Color::WHITE),
            ));
        let view: Widget = crate::scrolling::CustomScrollView::new(vec![sliver])
            .controller(controller.clone())
            .into();
        let inner: Widget = crate::scrolling::NotificationListener::new(view)
            .on_notification(move |notification| {
                observed_inner
                    .borrow_mut()
                    .push((notification.kind, notification.depth));
                true
            })
            .into();
        let mut tree = WidgetTree::new();
        let _root = tree
            .mount(
                crate::scrolling::NotificationListener::new(inner)
                    .on_notification(move |notification| {
                        observed_outer
                            .borrow_mut()
                            .push((notification.kind, notification.depth));
                        false
                    })
                    .into(),
            )
            .expect("notification listener mount");
        tree.layout(Constraints::tight(Size::new(100., 100.)));

        assert!(controller.jump_to(40.));
        assert!(
            inner_events
                .borrow()
                .iter()
                .any(|(kind, depth)| *kind == ScrollNotificationType::Update && *depth == 0)
        );
        assert!(outer_events.borrow().is_empty());
    }

    #[test]
    fn million_item_sliver_list_materializes_only_viewport_and_cache() {
        let controller = ScrollController::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut tree = WidgetTree::new();
        tree.mount(fixed_sliver_list(
            1_000_000,
            40.,
            controller.clone(),
            move |index| {
                observed.set(observed.get() + 1);
                Widget::box_(Size::new(80., 40.), Color::rgba(index as u8, 0, 0, 255))
            },
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let initial = tree.sliver_viewport_diagnostics().unwrap();
        assert!(initial.materialized_item_count < 100);
        assert_eq!(calls.get(), initial.materialized_item_count);
        assert!(controller.jump_to(900_000. * 40.));
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let jumped = tree.sliver_viewport_diagnostics().unwrap();
        assert!(jumped.materialized_range.contains(&900_000));
        assert!(jumped.materialized_item_count < 100);
        // Direct arithmetic builds only the destination cache range, never
        // every preceding logical item.
        assert!(calls.get() < 200);
        assert!(jumped.element_count < 100);
        assert!(jumped.render_object_count < 100);
    }

    #[test]
    fn million_item_sliver_list_deep_jump_builds_only_destination_rows() {
        let controller = ScrollController::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut tree = WidgetTree::new();
        tree.mount(
            CustomScrollView::new(vec![Box::new(SliverVariedExtentList::new(
                1_000_000,
                |item| {
                    if item % 2 == 0 { 32. } else { 56. }
                },
                move |item| {
                    observed.set(observed.get() + 1);
                    Widget::box_(
                        Size::new(80., if item % 2 == 0 { 32. } else { 56. }),
                        Color::rgba(item as u8, 0, 0, 255),
                    )
                },
            )) as Box<dyn crate::scrolling::Sliver>])
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 600.));
        tree.layout(constraints);
        // The explicit extent builder has an exact 44px mean for the
        // alternating 32px/56px sequence, so target the real prefix offset.
        assert!(controller.jump_to(900_000. * 44.));
        tree.layout(constraints);
        let diagnostics = tree.sliver_viewport_diagnostics().unwrap();
        assert!(diagnostics.materialized_range.contains(&900_000));
        assert!(diagnostics.materialized_item_count < 100);
        assert!(calls.get() < 200);
        // The cache window, not the skipped prefix, is what becomes exact.
        assert!(tree.children(tree.root().unwrap()).unwrap().len() < 100);
    }

    #[test]
    fn variable_measurements_above_visible_anchor_compensate_scroll_offset() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(
            CustomScrollView::new(vec![Box::new(SliverList::builder(1_000_000, |item| {
                Widget::box_(
                    Size::new(80., if item < 900_000 { 80. } else { 40. }),
                    Color::WHITE,
                )
            })) as Box<dyn crate::scrolling::Sliver>])
            .controller(controller.clone())
            .into(),
        )
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 600.));
        tree.layout(constraints);
        let target = 900_000. * 48.;
        assert!(controller.jump_to(target));
        tree.layout(constraints);
        // Cached rows before the first visible row grow from their 48px
        // estimates to their 80px laid-out height. The controller compensates
        // so the logical anchor stays in place.
        assert!(controller.offset() > target);
    }

    #[test]
    fn restored_sliver_list_offset_stays_viewport_bounded() {
        let scope = restoration_scope();
        let key = restoration_key("million-items");
        scope.set_json(&key, json!({ "offset": 900_000. * 40. }));
        let controller = ScrollController::restored(scope, key);
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut tree = WidgetTree::new();
        tree.mount(fixed_sliver_list(
            1_000_000,
            40.,
            controller.clone(),
            move |index| {
                observed.set(observed.get() + 1);
                Widget::box_(Size::new(80., 40.), Color::rgba(index as u8, 0, 0, 255))
            },
        ))
        .unwrap();

        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let diagnostics = tree.sliver_viewport_diagnostics().unwrap();
        assert!(diagnostics.materialized_range.contains(&900_000));
        assert!(diagnostics.materialized_item_count < 100);
        assert!(calls.get() < 100);
    }

    #[test]
    fn page_controller_alias_restores_its_logical_position() {
        let scope = restoration_scope();
        let key = restoration_key("pager");
        scope.set_json(&key, json!({ "offset": 200. }));

        let controller = crate::PageController::restored(scope, key);
        controller.update_extents(500., 100.);
        assert_eq!(controller.offset(), 200.);
    }

    #[test]
    fn sliver_children_retain_identity_inside_the_cache_and_release_outside() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(fixed_sliver_list(10_000, 40., controller.clone(), |i| {
                Widget::box_(Size::new(80., 40.), Color::rgba(i as u8, 0, 0, 255))
            }))
            .unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        tree.layout(constraints);
        let before = tree.children(root).unwrap().to_vec();
        assert!(controller.jump_to(3.));
        tree.layout(constraints);
        assert_eq!(tree.children(root).unwrap(), before.as_slice());
        assert!(controller.jump_to(400.));
        tree.layout(constraints);
        assert!(before.iter().any(|id| !tree.element_exists(*id)));
        assert!(before.iter().any(|id| tree.element_exists(*id)));
        let after = tree.sliver_viewport_diagnostics().unwrap();
        assert!(after.element_count < 30);
        assert!(tree.diagnostics().items_unmounted > 0);
    }

    #[test]
    fn sliver_count_changes_retain_valid_rows_and_release_invalid_ones() {
        let controller = ScrollController::new();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(fixed_sliver_list(100, 40., controller.clone(), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            }))
            .unwrap();
        tree.layout(constraints);
        let retained = tree.children(root).unwrap()[0];
        tree.update(
            root,
            fixed_sliver_list(80, 40., controller.clone(), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            }),
        )
        .unwrap();
        tree.layout(constraints);
        assert!(tree.element_exists(retained));
        assert!(controller.jump_to(1_000.));
        tree.layout(constraints);
        tree.update(
            root,
            fixed_sliver_list(0, 40., controller.clone(), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            }),
        )
        .unwrap();
        tree.layout(constraints);
        assert_eq!(controller.offset(), 0.);
        assert!(tree.children(root).unwrap().is_empty());
    }

    #[test]
    fn editing_uses_graphemes_and_keeps_utf8_boundaries() {
        let controller = TextEditingController::with_text("a👩‍💻é");
        controller.move_end(false);
        controller.backspace();
        assert_eq!(controller.text(), "a👩‍💻");
        controller.backspace();
        assert_eq!(controller.text(), "a");
        controller.set_selection(TextSelection { base: 1, extent: 1 });
        controller.insert("नमस्ते");
        assert_eq!(controller.text(), "aनमस्ते");
        let value = controller.value();
        assert!(controller.text().is_char_boundary(value.selection.extent));
    }

    #[test]
    fn ime_preedit_does_not_mutate_committed_text() {
        let controller = TextEditingController::with_text("hello");
        controller.set_preedit("世界", Some(TextRange::new(0, 3)));
        assert_eq!(controller.text(), "hello");
        assert_eq!(controller.preedit().as_deref(), Some("世界"));
        controller.commit_preedit("世界");
        assert_eq!(controller.text(), "hello世界");
        assert!(controller.preedit().is_none());
    }

    #[test]
    fn text_editing_restoration_round_trips_committed_text_and_selection_only() {
        let scope = restoration_scope();
        let key = restoration_key("document");
        let controller = TextEditingController::with_text("seed");
        controller.bind_restoration(scope.clone(), key.clone());
        controller.set_selection(TextSelection::collapsed(1));
        controller.insert("β");
        controller.set_preedit("transient", Some(TextRange::new(0, 3)));

        assert_eq!(
            scope.get_json(&key),
            Some(json!({
                "text": "sβeed",
                "selection": { "base": 3, "extent": 3 },
            }))
        );

        let restored = TextEditingController::restored(scope, key);
        assert_eq!(restored.text(), "sβeed");
        assert_eq!(restored.value().selection, TextSelection::collapsed(3));
        assert!(restored.preedit().is_none());
    }

    #[test]
    fn text_editing_restoration_normalizes_stale_utf8_selection_offsets() {
        let scope = restoration_scope();
        let key = restoration_key("document");
        scope.set_json(
            &key,
            json!({
                "text": "é",
                "selection": { "base": 99, "extent": 1 },
            }),
        );

        let restored = TextEditingController::restored(scope, key);
        assert_eq!(restored.text(), "é");
        // `base` clamps to the text end and `extent` to its preceding UTF-8
        // boundary; no invalid byte index reaches the editor.
        assert_eq!(
            restored.value().selection,
            TextSelection { base: 2, extent: 0 }
        );
    }

    #[test]
    fn editing_newlines_and_boundaries_are_grapheme_safe() {
        let controller = TextEditingController::with_text("hello\nworld");
        controller.set_selection(TextSelection::collapsed(6));
        controller.backspace();
        assert_eq!(controller.text(), "helloworld");
        controller.set_selection(TextSelection { base: 2, extent: 5 });
        controller.insert("\n");
        assert_eq!(controller.text(), "he\nworld");
        controller.set_selection(TextSelection::collapsed(2));
        controller.delete();
        assert_eq!(controller.text(), "heworld");
    }

    #[test]
    fn scrollbar_geometry_and_drag_share_the_controller() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert!(geometry.visible);
        assert_eq!(geometry.thumb.size.height, 24.);
        assert_eq!(geometry.thumb.origin.y, 0.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 10.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 60.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 60.)));
        assert!(controller.offset() > 0.);
        assert!(controller.offset() <= controller.max_offset());
        assert!(controller.scroll_by(0.25));
        assert!(controller.offset().fract() > 0.);
    }

    #[test]
    fn scrollbar_geometry_round_trips_offsets_and_thumb_tops() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 4_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(geometry.thumb.size.height, 90.);
        assert_eq!(geometry.thumb_travel, 510.);
        for fraction in [0., 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
            let offset = fraction * geometry.max_scroll_extent;
            let thumb_top = geometry.thumb_top_for_offset(offset);
            assert!((geometry.offset_for_thumb_top(thumb_top) - offset).abs() < 0.01);
            assert!(
                (geometry.thumb_top_for_offset(geometry.offset_for_thumb_top(thumb_top))
                    - thumb_top)
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn sliver_list_small_thumb_drag_is_continuous_and_reversible() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(fixed_sliver_list(100, 40., controller.clone(), |_| {
            Widget::fixed_box(Size::new(100., 40.), Color::WHITE)
        }))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(controller.max_offset(), 3_400.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 5.)));
        for y in [10., 15., 25.] {
            assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., y)));
        }
        let down = controller.offset();
        assert!(down > 0. && down < controller.max_offset() * 0.1);
        assert!((down - geometry.offset_for_thumb_top(20.)).abs() < 0.01);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 10.)));
        assert!(controller.offset() < down);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 10.)));
        assert!(!tree.scrollbar_drag_diagnostics().active);
    }

    #[test]
    fn sliver_list_minimum_thumb_drag_uses_actual_travel_and_stays_bounded() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(fixed_sliver_list(
            1_000_000,
            40.,
            controller.clone(),
            |_| Widget::fixed_box(Size::new(100., 40.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(geometry.thumb.size.height, 24.);
        assert_eq!(geometry.thumb_travel, 576.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 12.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 13.)));
        let one_pixel = controller.offset();
        assert!((one_pixel - geometry.max_scroll_extent / geometry.thumb_travel).abs() < 0.1);
        assert!(one_pixel < controller.max_offset());
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 300.)));
        let middle = controller.offset();
        assert!(middle > controller.max_offset() * 0.45 && middle < controller.max_offset() * 0.55);
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let middle_rows = tree.sliver_viewport_diagnostics().unwrap();
        assert!(middle_rows.materialized_range.contains(&(500_000usize)));
        assert!(middle_rows.materialized_item_count < 100);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 588.)));
        assert_eq!(controller.offset(), controller.max_offset());
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 300.)));
        assert!(controller.offset() < controller.max_offset());
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 300.)));
    }

    #[test]
    fn scrollbar_does_not_scroll_when_thumb_has_no_travel() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 20.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(geometry.thumb_travel, 0.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 10.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 100.)));
        assert_eq!(controller.offset(), 0.);
    }

    #[test]
    fn scrollbar_stays_synchronized_after_wheel_and_programmatic_offset_changes() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 4_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 600.)));
        let after_wheel = tree.scrollbar_diagnostics().pop().unwrap();
        assert!(after_wheel.thumb.origin.y > after_wheel.track.origin.y);
        assert!(controller.jump_to(controller.max_offset() * 0.5));
        let middle = tree.scrollbar_diagnostics().pop().unwrap();
        let grab_y = middle.thumb.origin.y + middle.thumb.size.height * 0.5;
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., grab_y)));
        assert!(tree.scrollbar_pointer(
            incular_core::PointerPhase::Move,
            Offset::new(95., grab_y - 10.)
        ));
        assert!(controller.offset() < controller.max_offset() * 0.5);
        let events = tree.diagnostics().scroll_events;
        assert!(tree.scrollbar_pointer(
            incular_core::PointerPhase::Move,
            Offset::new(95., grab_y - 10.)
        ));
        assert_eq!(tree.diagnostics().scroll_events, events);
    }

    #[test]
    fn wheel_at_nested_scroll_transfers_child_boundary_remainder_to_parent_once() {
        let outer = ScrollController::new();
        let inner = ScrollController::new();
        let nested: Widget = SizedBox::from_size(Size::new(100., 100.))
            .child(Widget::scroll_view(
                inner.clone(),
                Widget::fixed_box(Size::new(100., 300.), Color::WHITE),
            ))
            .into();
        let content = Widget::column(vec![
            Widget::fixed_box(Size::new(100., 10.), Color::BLACK),
            nested,
            Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
        ]);
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(outer.clone(), content))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        assert!(outer.jump_to(100.));
        assert_eq!(inner.offset(), 0.);
        // At the inner top, upward wheel delta is consumed by the outer
        // viewport only; it is never duplicated into both controllers.
        assert!(tree.scroll_at(Offset::new(50., 20.), Offset::new(0., -40.)));
        assert_eq!(inner.offset(), 0.);
        assert_eq!(outer.offset(), 60.);
    }

    #[test]
    fn textarea_uses_shaped_lines_for_pointer_and_vertical_navigation() {
        let controller = TextEditingController::with_text("abcdef\nxy\n123456");
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                EditableText::new(controller.clone())
                    .multiline(true)
                    .size(Size::new(120., 100.))
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(120., 100.)));
        tree.text_field_set_caret(root, Offset::new(30., 10.), false, Instant::now());
        let initial = controller.value().selection.extent;
        assert!(initial > 0 && initial <= 6);
        assert!(tree.text_field_move_vertical(root, true, true));
        let selection = controller.value().selection;
        assert_eq!(selection.base, initial);
        assert!(
            selection.extent > 6 && selection.extent <= 9,
            "vertical move should land on the second shaped line: {selection:?}"
        );
        assert!(tree.text_field_move_line_edge(root, true, false));
        assert_eq!(controller.value().selection.extent, 9);
        tree.text_field_set_caret(root, Offset::new(5., 55.), false, Instant::now());
        assert!(controller.value().selection.extent >= 10);
    }

    #[test]
    fn transparent_button_focus_is_an_outline_not_a_surface_fill() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                ActionSurface::with_child(Widget::fixed_box(Size::new(96., 32.), Color::WHITE))
                    .color(Color::TRANSPARENT)
                    .focused_color(Color::rgba(85, 150, 255, 200))
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(96., 32.)));
        tree.set_focused(root, true, Instant::now()).unwrap();
        let list = tree.paint();

        assert!(list.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::Border { border, .. }
                    if border.width == 2.0 && border.color == Color::rgba(85, 150, 255, 200)
            )
        }));
        assert!(!list.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::RRect {
                    brush: Brush::Solid(color),
                    ..
                } if *color == Color::rgba(85, 150, 255, 200)
            )
        }));
        let content_index = list
            .commands()
            .iter()
            .position(|command| {
                matches!(
                    command,
                    PaintCommand::Rect { color, .. } if *color == Color::WHITE
                )
            })
            .expect("focused button content should be painted");
        let ring_index = list
            .commands()
            .iter()
            .position(|command| {
                matches!(
                    command,
                    PaintCommand::Border { border, .. }
                        if border.color == Color::rgba(85, 150, 255, 200)
                )
            })
            .expect("focused button should paint its outline");
        assert!(
            ring_index > content_index,
            "focus outline must overlay content"
        );
    }

    #[test]
    fn focused_text_field_does_not_paint_framework_outline() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                EditableText::new(TextEditingController::with_text("value"))
                    .size(Size::new(180., 32.))
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(180., 32.)));
        tree.set_focused(root, true, Instant::now()).unwrap();
        let list = tree.paint();

        assert!(
            !list
                .commands()
                .iter()
                .any(|command| matches!(command, PaintCommand::Border { .. }))
        );
        assert!(!list.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::Rect { rect, color }
                    if color == &Color::rgba(120, 170, 245, 220)
                        && (rect.size.width - 180.).abs() < f32::EPSILON
            )
        }));
    }

    #[test]
    fn color_filter_and_blend_updates_stay_in_the_retained_compositor() {
        let controller = ColorFilterController::new(ColorFilter::grayscale(0.));
        let widget = ColorFiltered::controlled(
            controller.clone(),
            Blend::new(
                BlendMode::Multiply,
                Widget::box_(Size::new(80., 40.), Color::rgba(200, 80, 40, 255)),
            ),
        );
        let mut tree = WidgetTree::new();
        tree.mount(widget.into()).expect("mount effect tree");
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let _ = tree.paint();
        let before = tree.diagnostics();
        assert!(controller.set_filter(ColorFilter::sepia(1.)));
        let (changed, _) = tree.update_compositor(Instant::now());
        let after = tree.diagnostics();
        assert!(changed);
        assert_eq!(after.paints, before.paints);
        assert!(after.composites > before.composites);
        let debug = tree.compositor_debug_tree();
        assert!(debug.contains("ColorFilter"));
        assert!(debug.contains("Blend"));
    }

    #[test]
    fn gesture_region_captures_a_hit_tested_pointer_sequence() {
        let taps = Rc::new(Cell::new(0));
        let observed = taps.clone();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::gesture(
                GestureCallbacks {
                    on_tap: Some(Rc::new(move || observed.set(observed.get() + 1))),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(40., 40.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        assert_eq!(
            tree.dispatch_gesture(PointerEvent {
                pointer: 7,
                position: Offset::new(10., 10.),
                phase: incular_core::PointerPhase::Down,
                time: now,
            }),
            Some(root)
        );
        assert_eq!(
            tree.dispatch_gesture(PointerEvent {
                pointer: 7,
                position: Offset::new(80., 80.),
                phase: incular_core::PointerPhase::Up,
                time: now + Duration::from_millis(20),
            }),
            Some(root)
        );
        assert_eq!(taps.get(), 1);
    }

    #[test]
    fn gesture_region_combines_identified_contacts_for_scale_updates() {
        let scale = Rc::new(Cell::new(0.));
        let observed = scale.clone();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::gesture(
                GestureCallbacks {
                    on_scale_update: Some(Rc::new(move |details| observed.set(details.scale))),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        for (pointer, position) in [(10, Offset::new(10., 10.)), (11, Offset::new(20., 10.))] {
            assert_eq!(
                tree.dispatch_gesture(PointerEvent {
                    pointer,
                    position,
                    phase: incular_core::PointerPhase::Down,
                    time: now,
                }),
                Some(root)
            );
        }
        assert_eq!(
            tree.dispatch_gesture(PointerEvent {
                pointer: 11,
                position: Offset::new(30., 10.),
                phase: incular_core::PointerPhase::Move,
                time: now + Duration::from_millis(16),
            }),
            Some(root)
        );
        assert_eq!(scale.get(), 2.);
        assert_eq!(
            tree.dispatch_gesture(PointerEvent {
                pointer: 10,
                position: Offset::new(10., 10.),
                phase: incular_core::PointerPhase::Up,
                time: now + Duration::from_millis(32),
            }),
            Some(root)
        );
    }

    #[test]
    fn retained_arena_allows_drag_to_defeat_nested_tap_before_callbacks() {
        let taps = Rc::new(Cell::new(0));
        let pans = Rc::new(Cell::new(0));
        let cancelled = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::gesture(
            GestureCallbacks {
                on_pan_update: Some({
                    let pans = pans.clone();
                    Rc::new(move |_| pans.set(pans.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::gesture(
                GestureCallbacks {
                    on_tap: Some({
                        let taps = taps.clone();
                        Rc::new(move || taps.set(taps.get() + 1))
                    }),
                    on_cancel: Some({
                        let cancelled = cancelled.clone();
                        Rc::new(move || cancelled.set(cancelled.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        for (phase, position) in [
            (incular_core::PointerPhase::Down, Offset::new(10., 10.)),
            (incular_core::PointerPhase::Move, Offset::new(40., 10.)),
            (incular_core::PointerPhase::Up, Offset::new(40., 10.)),
        ] {
            assert!(
                tree.dispatch_gesture(PointerEvent {
                    pointer: 1,
                    position,
                    phase,
                    time: now
                })
                .is_some()
            );
        }
        assert_eq!(pans.get(), 1);
        assert_eq!(taps.get(), 0);
        assert_eq!(cancelled.get(), 1);
    }

    #[test]
    fn retained_arena_arbitrates_horizontal_against_vertical_drag() {
        let horizontal = Rc::new(Cell::new(0));
        let vertical = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::gesture(
            GestureCallbacks {
                on_vertical_drag_update: Some({
                    let vertical = vertical.clone();
                    Rc::new(move |_| vertical.set(vertical.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::gesture(
                GestureCallbacks {
                    on_horizontal_drag_update: Some({
                        let horizontal = horizontal.clone();
                        Rc::new(move |_| horizontal.set(horizontal.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        for (phase, position) in [
            (incular_core::PointerPhase::Down, Offset::new(10., 10.)),
            (incular_core::PointerPhase::Move, Offset::new(12., 40.)),
            (incular_core::PointerPhase::Up, Offset::new(12., 40.)),
        ] {
            let _ = tree.dispatch_gesture(PointerEvent {
                pointer: 2,
                position,
                phase,
                time: now,
            });
        }
        assert_eq!(horizontal.get(), 0);
        assert_eq!(vertical.get(), 1);
    }

    #[test]
    fn retained_arena_cancels_long_press_when_drag_claims_stream() {
        let long_presses = Rc::new(Cell::new(0));
        let cancellations = Rc::new(Cell::new(0));
        let pans = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::gesture(
            GestureCallbacks {
                on_pan_update: Some({
                    let pans = pans.clone();
                    Rc::new(move |_| pans.set(pans.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::gesture(
                GestureCallbacks {
                    on_long_press: Some({
                        let long_presses = long_presses.clone();
                        Rc::new(move || long_presses.set(long_presses.get() + 1))
                    }),
                    on_cancel: Some({
                        let cancellations = cancellations.clone();
                        Rc::new(move || cancellations.set(cancellations.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 3,
            position: Offset::new(10., 10.),
            phase: incular_core::PointerPhase::Down,
            time: now,
        });
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 3,
            position: Offset::new(40., 10.),
            phase: incular_core::PointerPhase::Move,
            time: now + Duration::from_millis(100),
        });
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 3,
            position: Offset::new(40., 10.),
            phase: incular_core::PointerPhase::Up,
            time: now + PointerGestureRecognizer::LONG_PRESS_TIMEOUT + Duration::from_millis(1),
        });
        assert_eq!(pans.get(), 1);
        assert_eq!(long_presses.get(), 0);
        assert_eq!(cancellations.get(), 1);
    }

    #[test]
    fn retained_arena_allows_scale_to_defeat_pan_and_share_two_contacts() {
        let pans = Rc::new(Cell::new(0));
        let scales = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::gesture(
            GestureCallbacks {
                on_pan_update: Some({
                    let pans = pans.clone();
                    Rc::new(move |_| pans.set(pans.get() + 1))
                }),
                ..GestureCallbacks::default()
            },
            Widget::gesture(
                GestureCallbacks {
                    on_scale_update: Some({
                        let scales = scales.clone();
                        Rc::new(move |_| scales.set(scales.get() + 1))
                    }),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        for (pointer, position) in [(10, Offset::new(10., 10.)), (11, Offset::new(20., 10.))] {
            let _ = tree.dispatch_gesture(PointerEvent {
                pointer,
                position,
                phase: incular_core::PointerPhase::Down,
                time: now,
            });
        }
        let _ = tree.dispatch_gesture(PointerEvent {
            pointer: 11,
            position: Offset::new(40., 10.),
            phase: incular_core::PointerPhase::Move,
            time: now + Duration::from_millis(16),
        });
        assert_eq!(pans.get(), 0);
        assert_eq!(scales.get(), 1);
    }

    #[test]
    fn ignore_pointer_skips_its_subtree_and_reveals_a_stacked_target() {
        let behind = Rc::new(Cell::new(0));
        let ignored = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::stack(
            Alignment::CENTER,
            vec![
                Widget::gesture(
                    GestureCallbacks {
                        on_tap: Some({
                            let behind = behind.clone();
                            Rc::new(move || behind.set(behind.get() + 1))
                        }),
                        ..GestureCallbacks::default()
                    },
                    Widget::box_(Size::new(100., 100.), Color::WHITE),
                ),
                IgnorePointer::new(Widget::gesture(
                    GestureCallbacks {
                        on_tap: Some({
                            let ignored = ignored.clone();
                            Rc::new(move || ignored.set(ignored.get() + 1))
                        }),
                        ..GestureCallbacks::default()
                    },
                    Widget::box_(Size::new(100., 100.), Color::BLACK),
                ))
                .into(),
            ],
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        for phase in [
            incular_core::PointerPhase::Down,
            incular_core::PointerPhase::Up,
        ] {
            let _ = tree.dispatch_gesture(PointerEvent {
                pointer: 20,
                position: Offset::new(20., 20.),
                phase,
                time: now,
            });
        }
        assert_eq!(behind.get(), 1);
        assert_eq!(ignored.get(), 0);
    }

    #[test]
    fn absorb_pointer_blocks_descendant_and_stacked_gesture_targets() {
        let behind = Rc::new(Cell::new(0));
        let absorbed_child = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::stack(
            Alignment::CENTER,
            vec![
                Widget::gesture(
                    GestureCallbacks {
                        on_tap: Some({
                            let behind = behind.clone();
                            Rc::new(move || behind.set(behind.get() + 1))
                        }),
                        ..GestureCallbacks::default()
                    },
                    Widget::box_(Size::new(100., 100.), Color::WHITE),
                ),
                AbsorbPointer::new(Widget::gesture(
                    GestureCallbacks {
                        on_tap: Some({
                            let absorbed_child = absorbed_child.clone();
                            Rc::new(move || absorbed_child.set(absorbed_child.get() + 1))
                        }),
                        ..GestureCallbacks::default()
                    },
                    Widget::box_(Size::new(100., 100.), Color::BLACK),
                ))
                .into(),
            ],
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        for phase in [
            incular_core::PointerPhase::Down,
            incular_core::PointerPhase::Up,
        ] {
            assert!(
                tree.dispatch_gesture(PointerEvent {
                    pointer: 21,
                    position: Offset::new(20., 20.),
                    phase,
                    time: now,
                })
                .is_none()
            );
        }
        assert_eq!(behind.get(), 0);
        assert_eq!(absorbed_child.get(), 0);
    }

    #[test]
    fn retained_pointer_capture_is_window_local_and_released_with_the_stream() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::gesture(
                GestureCallbacks {
                    on_tap: Some(Rc::new(|| {})),
                    ..GestureCallbacks::default()
                },
                Widget::box_(Size::new(100., 100.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let now = Instant::now();
        let _ = tree.dispatch_gesture_in_window(
            9,
            PointerEvent {
                pointer: 5,
                position: Offset::new(10., 10.),
                phase: incular_core::PointerPhase::Down,
                time: now,
            },
        );
        assert_eq!(tree.pointer_capture_target(9, 5), Some(root));
        assert_eq!(tree.pointer_capture_target(10, 5), None);
        let capture = tree.request_pointer_capture(9, 5, root).unwrap();
        assert_eq!(capture.window(), 9);
        assert_eq!(capture.pointer(), 5);
        assert!(tree.release_pointer_capture(capture));
        assert_eq!(tree.pointer_capture_target(9, 5), None);
        let _ = tree.dispatch_gesture_in_window(
            9,
            PointerEvent {
                pointer: 5,
                position: Offset::new(90., 90.),
                phase: incular_core::PointerPhase::Up,
                time: now,
            },
        );
        assert_eq!(tree.pointer_capture_target(9, 5), None);
    }

    #[test]
    fn typed_local_drag_drop_enters_updates_and_drops_through_the_arena() {
        let context = DragDropContext::new();
        let entered = Rc::new(Cell::new(0));
        let updates = Rc::new(Cell::new(0));
        let dropped = Rc::new(Cell::new(0));
        let ended = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::row(vec![
            Draggable::new(
                context.clone(),
                String::from("card"),
                Widget::box_(Size::new(100., 80.), Color::WHITE),
            )
            .feedback(|payload| Text::new(payload).into())
            .on_end({
                let ended = ended.clone();
                move |_| ended.set(ended.get() + 1)
            })
            .into(),
            DragTarget::new(
                context.clone(),
                Widget::box_(Size::new(100., 80.), Color::BLACK),
            )
            .on_enter({
                let entered = entered.clone();
                move |_| entered.set(entered.get() + 1)
            })
            .on_update({
                let updates = updates.clone();
                move |_, _| updates.set(updates.get() + 1)
            })
            .on_drop({
                let dropped = dropped.clone();
                move |payload| {
                    assert_eq!(payload, "card");
                    dropped.set(dropped.get() + 1);
                }
            })
            .into(),
        ]))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 80.)));
        let now = Instant::now();
        for (phase, position) in [
            (incular_core::PointerPhase::Down, Offset::new(10., 20.)),
            (incular_core::PointerPhase::Move, Offset::new(35., 20.)),
            (incular_core::PointerPhase::Move, Offset::new(140., 20.)),
            (incular_core::PointerPhase::Up, Offset::new(140., 20.)),
        ] {
            let _ = tree.dispatch_gesture(PointerEvent {
                pointer: 30,
                position,
                phase,
                time: now,
            });
        }
        assert_eq!(entered.get(), 1);
        assert!(updates.get() >= 1);
        assert_eq!(dropped.get(), 1);
        assert_eq!(ended.get(), 1);
        assert!(!context.is_dragging());
        assert!(context.feedback().is_none());
    }

    #[test]
    fn typed_local_drag_drop_leaves_and_cancels_without_drop() {
        let context = DragDropContext::new();
        let left = Rc::new(Cell::new(0));
        let cancelled = Rc::new(Cell::new(0));
        let dropped = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::row(vec![
            Draggable::new(
                context.clone(),
                7_u32,
                Widget::box_(Size::new(100., 80.), Color::WHITE),
            )
            .on_cancel({
                let cancelled = cancelled.clone();
                move |_| cancelled.set(cancelled.get() + 1)
            })
            .into(),
            DragTarget::new(context, Widget::box_(Size::new(100., 80.), Color::BLACK))
                .on_leave({
                    let left = left.clone();
                    move |_| left.set(left.get() + 1)
                })
                .on_drop({
                    let dropped = dropped.clone();
                    move |_| dropped.set(dropped.get() + 1)
                })
                .into(),
        ]))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 80.)));
        let now = Instant::now();
        for (phase, position) in [
            (incular_core::PointerPhase::Down, Offset::new(10., 20.)),
            (incular_core::PointerPhase::Move, Offset::new(35., 20.)),
            (incular_core::PointerPhase::Move, Offset::new(140., 20.)),
            (incular_core::PointerPhase::Move, Offset::new(230., 20.)),
            (incular_core::PointerPhase::Cancel, Offset::new(230., 20.)),
        ] {
            let _ = tree.dispatch_gesture(PointerEvent {
                pointer: 31,
                position,
                phase,
                time: now,
            });
        }
        assert_eq!(left.get(), 1);
        assert_eq!(cancelled.get(), 1);
        assert_eq!(dropped.get(), 0);
    }

    #[test]
    fn dismissible_claims_its_directional_drag_before_callback() {
        let dismissals = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        tree.mount(
            Dismissible::new(
                DismissDirection::Horizontal,
                Widget::box_(Size::new(120., 80.), Color::WHITE),
                {
                    let dismissals = dismissals.clone();
                    move |_| dismissals.set(dismissals.get() + 1)
                },
            )
            .threshold(40.)
            .into(),
        )
        .unwrap();
        tree.layout(Constraints::tight(Size::new(120., 80.)));
        let now = Instant::now();
        for (phase, position) in [
            (incular_core::PointerPhase::Down, Offset::new(10., 20.)),
            (incular_core::PointerPhase::Move, Offset::new(70., 20.)),
            (incular_core::PointerPhase::Up, Offset::new(70., 20.)),
        ] {
            let _ = tree.dispatch_gesture(PointerEvent {
                pointer: 32,
                position,
                phase,
                time: now,
            });
        }
        assert_eq!(dismissals.get(), 1);
    }

    #[test]
    fn explicit_merge_exclude_and_block_semantics_transform_the_retained_tree() {
        let dialog = Widget::box_(Size::new(80., 40.), Color::WHITE)
            .semantics(
                ExplicitSemantics::new(SemanticRole::Dialog)
                    .label("Delete document")
                    .actions([SemanticActionKind::Focus]),
            )
            .merge_semantics()
            .block_semantics();
        let background = ActionSurface::new("Save").into();
        let decorative: Widget = Text::new("sparkle").into();
        let decorative = decorative.exclude_semantics();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::stack(
            Alignment::CENTER,
            vec![background, decorative, dialog],
        ))
        .expect("mount modal semantics");
        tree.layout(Constraints::tight(Size::new(160., 100.)));
        tree.update_semantics();
        let nodes: Vec<_> = tree.semantics().iter().collect();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].1.role, SemanticRole::Dialog);
        assert_eq!(nodes[0].1.label.as_deref(), Some("Delete document"));
        assert_eq!(nodes[0].1.children.len(), 0);
    }

    #[test]
    fn meaningful_images_are_semantic_but_unlabelled_images_are_decorative() {
        let image = ImageHandle::from_rgba8(1, 1, vec![255, 255, 255, 255]).unwrap();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::row(vec![
            Image::new(image.clone()).into(),
            Widget::from(Image::new(image)).accessibility_label("Incular logo"),
        ]))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(80., 40.)));
        tree.update_semantics();
        let images: Vec<_> = tree
            .semantics()
            .iter()
            .filter(|(_, node)| node.role == SemanticRole::Image)
            .collect();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].1.label.as_deref(), Some("Incular logo"));
    }
}

/// Widget key marking the performance-overlay mount point. Applications place
/// a placeholder with this key; `Runtime::install_performance_overlay`
/// (runtime crate) replaces it with the live overlay.
pub const PERFORMANCE_OVERLAY_KEY: &str = "incular-performance-overlay";

/// Mount-point placeholder for `Runtime::install_performance_overlay`
/// (runtime crate). Place this anywhere in an application tree; installing
/// swaps its contents for the live overlay while keeping the same top-level
/// widget kind so the retained update path stays compatible.
#[must_use]
pub fn performance_overlay_placeholder() -> Widget {
    Widget::repaint_boundary(Widget::box_(Size::ZERO, incular_core::Color::TRANSPARENT))
        .with_key(Key::String(PERFORMANCE_OVERLAY_KEY.to_owned()))
}

/// Property tests for retained child reconciliation: random operation
/// sequences applied to both the real reconciliation path and a trivial
/// reference model must agree on logical order, key identity, and retained
/// state after every step.
#[cfg(test)]
mod reconciliation_property {
    use super::*;
    use proptest::prelude::*;

    #[derive(Clone, Debug, PartialEq)]
    struct ModelChild {
        key: Option<u64>,
        payload: String,
    }
    type Model = Vec<ModelChild>;

    fn frame() -> Constraints {
        Constraints::tight(Size::new(400., 4000.))
    }

    fn widget_for(child: &ModelChild) -> Widget {
        let mut widget = Widget::from(Text::new(format!(
            "{}|{}",
            child.payload,
            child.key.unwrap_or(u64::MAX)
        )));
        if let Some(key) = child.key {
            widget = widget.with_key(Key::Value(key));
        }
        widget
    }

    fn observed_keys(tree: &WidgetTree, parent: ElementId) -> Vec<Option<Key>> {
        tree.children(parent)
            .expect("children")
            .iter()
            .map(|&child| {
                tree.elements
                    .get(child.0)
                    .map(|e| e.widget.key.clone())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn model_keys(model: &Model) -> Vec<Option<Key>> {
        model
            .iter()
            .map(|child| child.key.map(Key::Value))
            .collect()
    }

    /// Retained-state surrogate: payload travels with identity across moves.
    fn observed_payloads(tree: &WidgetTree, parent: ElementId) -> Vec<String> {
        tree.children(parent)
            .expect("children")
            .iter()
            .map(
                |&child| match &tree.elements.get(child.0).expect("live").widget.kind {
                    WidgetKind::Text { text, .. } => text.clone(),
                    _ => String::new(),
                },
            )
            .collect()
    }

    fn model_payloads(model: &Model) -> Vec<String> {
        model
            .iter()
            .map(|child| match widget_for(child).kind {
                WidgetKind::Text { text, .. } => text,
                _ => String::new(),
            })
            .collect()
    }

    #[derive(Clone, Debug)]
    enum TestOp {
        Insert(usize, Option<u64>, u64),
        Remove(usize),
        Move(usize, usize),
        Replace(usize, u64),
        ChangeKey(usize, Option<u64>),
    }

    fn apply(model: &mut Model, op: &TestOp) {
        match op {
            TestOp::Insert(position, key, payload) => {
                if key.is_some_and(|key| model.iter().any(|c| c.key == Some(key))) {
                    return;
                }
                model.insert(
                    (*position).min(model.len()),
                    ModelChild {
                        key: *key,
                        payload: format!("p{payload}"),
                    },
                );
            }
            TestOp::Remove(position) => {
                if !model.is_empty() {
                    model.remove((*position).min(model.len() - 1));
                }
            }
            TestOp::Move(from, to) => {
                if !model.is_empty() && from != to {
                    let from = *from % model.len();
                    let child = model.remove(from);
                    model.insert((*to).min(model.len()), child);
                }
            }
            TestOp::Replace(position, payload) => {
                if !model.is_empty() {
                    let position = (*position).min(model.len() - 1);
                    model[position].payload = format!("r{payload}");
                }
            }
            TestOp::ChangeKey(position, key) => {
                if !model.is_empty() {
                    let position = (*position).min(model.len() - 1);
                    let free = key.is_none_or(|key| {
                        !model
                            .iter()
                            .enumerate()
                            .any(|(index, c)| index != position && c.key == Some(key))
                    });
                    if free {
                        model[position].key = *key;
                    }
                }
            }
        }
    }

    fn reconcile(tree: &mut WidgetTree, parent: ElementId, model: &Model) {
        let desired: Vec<Widget> = model.iter().map(widget_for).collect();
        let mut parent_widget = tree.elements.get(parent.0).expect("parent").widget.clone();
        match &mut parent_widget.kind {
            WidgetKind::Flex { children, .. } => *children = desired,
            other => panic!("test parent is a column, found {other:?}"),
        }
        tree.update(parent, parent_widget).expect("reconcile");
        tree.layout(frame());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn reconciliation_matches_reference_model(
            ops in proptest::collection::vec(
                prop_oneof![
                    (0usize..8, proptest::option::of(0u64..4u64), 0u64..1_000u64)
                        .prop_map(|(position, key, payload)| TestOp::Insert(position, key, payload)),
                    (0usize..8usize).prop_map(TestOp::Remove),
                    (0usize..8usize, 0usize..8usize).prop_map(|(from, to)| TestOp::Move(from, to)),
                    (0usize..8usize, 0u64..1_000u64)
                        .prop_map(|(position, payload)| TestOp::Replace(position, payload)),
                    (0usize..8usize, proptest::option::of(0u64..4u64))
                        .prop_map(|(position, key)| TestOp::ChangeKey(position, key)),
                ],
                0..40,
            )
        ) {
            let mut model: Model = Vec::new();
            let mut tree = WidgetTree::default();
            let parent = tree.mount(Widget::column(Vec::new())).expect("mount");
            tree.layout(frame());

            for op in &ops {
                apply(&mut model, op);
                reconcile(&mut tree, parent, &model);

                prop_assert_eq!(observed_keys(&tree, parent), model_keys(&model));
                prop_assert_eq!(observed_payloads(&tree, parent), model_payloads(&model));
                prop_assert_eq!(tree.children(parent).unwrap().len(), model.len());
            }
        }
    }
}

/// Task 15 structural reconciliation contracts: operation-count based, no
/// timing thresholds.
#[cfg(test)]
mod reconciliation_structural_contracts {

    use super::*;

    fn frame() -> Constraints {
        Constraints::tight(Size::new(600., 6000.))
    }

    fn keyed_row(items: usize, generation: u64) -> Widget {
        let children = (0..items)
            .map(|index| {
                let label = if index == items / 2 && generation > 0 {
                    format!("changed {generation}")
                } else {
                    format!("row {index}")
                };
                Widget::from(Text::new(label)).with_key(Key::Value(index as u64))
            })
            .collect::<Vec<_>>();
        Widget::column(children)
    }

    fn prepared(root: Widget) -> (WidgetTree, ElementId) {
        let mut tree = WidgetTree::default();
        let root_id = tree.mount(root).expect("mount");
        tree.layout(frame());
        (tree, root_id)
    }

    #[test]
    fn ten_thousand_unchanged_children_perform_no_mutation_work() {
        // The PARENT differs (spacing), so its child list is rescanned; every
        // child is byte-identical and must cost nothing but a comparison.
        let build = |spacing: f32| {
            Widget::wrap(
                incular_config::Axis::Vertical,
                spacing,
                spacing,
                (0..10_000)
                    .map(|index| {
                        Widget::from(Text::new(format!("row {index}")))
                            .with_key(Key::Value(index as u64))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let (mut tree, root) = prepared(build(8.));
        let before = tree.diagnostics();

        tree.update(root, build(9.)).expect("update");
        tree.layout(frame());
        let _ = tree.paint();

        let after = tree.diagnostics();
        assert_eq!(
            after.elements_created - before.elements_created,
            0,
            "created"
        );
        assert_eq!(
            after.elements_removed - before.elements_removed,
            0,
            "removed"
        );
        assert_eq!(
            after.rebuilds - before.rebuilds,
            1,
            "only the parent itself rebuilds"
        );
        assert_eq!(
            after.identical_child_bailouts - before.identical_child_bailouts,
            10_000,
            "every child must hit the identical-widget bailout"
        );
    }

    #[test]
    fn single_changed_child_touches_only_that_child() {
        let (mut tree, root) = prepared(keyed_row(10_000, 0));
        let before = tree.diagnostics();

        tree.update(root, keyed_row(10_000, 1)).expect("update");
        tree.layout(frame());
        let _ = tree.paint();

        let after = tree.diagnostics();
        assert_eq!(after.elements_created - before.elements_created, 0);
        assert_eq!(after.elements_removed - before.elements_removed, 0);
        // Root + the one changed descendant; nothing else rebuilds.
        assert_eq!(after.rebuilds - before.rebuilds, 2);
        assert_eq!(
            after.identical_child_bailouts - before.identical_child_bailouts,
            9_999
        );
        // Only the changed text and its column re-resolve layout.
        assert_eq!(after.layouts - before.layouts, 2);
        assert!(after.paints - before.paints >= 1, "changed text repaints");
    }

    #[test]
    fn key_reorder_preserves_state_and_counts_moves() {
        let build = |order: &[usize]| {
            Widget::column(
                order
                    .iter()
                    .map(|&index| {
                        Widget::from(Text::new(format!("state {index}")))
                            .with_key(Key::Value(index as u64))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let (mut tree, root) = prepared(build(&[0, 1, 2, 3, 4]));
        let before = tree.diagnostics();

        // Pure reorder: [4,3,2,1,0].
        tree.update(root, build(&[4, 3, 2, 1, 0])).expect("reorder");
        tree.layout(frame());

        let after = tree.diagnostics();
        assert_eq!(
            after.elements_created - before.elements_created,
            0,
            "no remounts"
        );
        assert_eq!(
            after.elements_removed - before.elements_removed,
            0,
            "no unmounts"
        );
        assert_eq!(
            after.elements_moved - before.elements_moved,
            4,
            "four positions moved"
        );

        // Retained state identity: each moved child keeps its payload.
        let payloads: Vec<String> = tree
            .children(root)
            .expect("children")
            .iter()
            .map(
                |&child| match &tree.elements.get(child.0).expect("live").widget.kind {
                    WidgetKind::Text { text, .. } => text.clone(),
                    _ => String::new(),
                },
            )
            .collect();
        assert_eq!(
            payloads,
            ["state 4", "state 3", "state 2", "state 1", "state 0"]
        );
    }

    #[test]
    fn incompatible_key_replacement_does_not_inherit_state() {
        let build = |kind: u8| {
            let child = if kind == 0 {
                Widget::from(Text::new("text 0"))
            } else {
                Widget::box_(Size::new(20., 20.), Color::WHITE)
            };
            Widget::column(vec![child.with_key(Key::Value(7))])
        };
        let (mut tree, root) = prepared(build(0));
        let before = tree.diagnostics();

        // Same key, different widget type: incompatible, must remount fresh.
        tree.update(root, build(1)).expect("replace");

        let after = tree.diagnostics();
        assert_eq!(
            after.mounts - before.mounts,
            1,
            "replacement mounts a new element"
        );
        assert_eq!(after.unmounts - before.unmounts, 1, "old element unmounts");
        // The retained element is genuinely new: a Box, not the old Text.
        for &child in tree.children(root).expect("children") {
            assert!(matches!(
                tree.elements.get(child.0).expect("live").widget.kind,
                WidgetKind::Box { .. }
            ));
        }
    }
}
