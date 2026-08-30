//! Public widget identity, input, and painting controllers.
//!
//! These value types are kept separate from the retained tree implementation.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    pub(super) key: GestureArenaKey,
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
    pub(super) offset: Rc<Cell<Offset>>,
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
    pub(super) fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        self.set_scale(self.from.get() + (self.to.get() - self.from.get()) * animation.value())
    }
    pub(super) fn is_active(&self) -> bool {
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
    pub(super) fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        self.set_radians(self.from.get() + (self.to.get() - self.from.get()) * animation.value())
    }
    pub(super) fn is_active(&self) -> bool {
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
    pub(super) fn tick(&self, now: Instant) -> bool {
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
    pub(super) fn is_active(&self) -> bool {
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
    pub(super) fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        self.set_opacity(self.from.get() + (self.to.get() - self.from.get()) * t)
    }
    pub(super) fn is_active(&self) -> bool {
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
    pub(super) fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        self.set_sigma(self.from.get() + (self.to.get() - self.from.get()) * t)
    }
    pub(super) fn is_active(&self) -> bool {
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
    pub(super) fn tick(&self, now: Instant) -> bool {
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
    pub(super) fn is_active(&self) -> bool {
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
    pub(super) fn tick(&self, now: Instant) -> bool {
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
    pub(super) fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}
impl Default for DropShadowController {
    fn default() -> Self {
        Self::new(Offset::new(0., 4.), 8., Color::rgba(0, 0, 0, 96))
    }
}

pub(super) fn finite_offset(offset: Offset) -> Offset {
    Offset::new(
        if offset.x.is_finite() { offset.x } else { 0. },
        if offset.y.is_finite() { offset.y } else { 0. },
    )
}

pub(super) fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() { value.max(0.) } else { 0. }
}
