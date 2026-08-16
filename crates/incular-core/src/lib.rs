//! Renderer- and platform-independent primitives shared by Incular.
//!
//! This crate deliberately contains values and opaque storage identities only;
//! it does not know about widgets, layout policy, or GPU resources.

use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        assert!(
            width.is_finite() && height.is_finite() && width >= 0.0 && height >= 0.0,
            "sizes must be finite and non-negative"
        );
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
    pub x: f32,
    pub y: f32,
}
impl Offset {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
impl std::ops::Add for Offset {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}
impl std::ops::Sub for Offset {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Offset,
    pub size: Size,
}
impl Rect {
    #[must_use]
    pub fn from_origin_size(origin: Offset, size: Size) -> Self {
        Self { origin, size }
    }
    #[must_use]
    pub fn contains(self, point: Offset) -> bool {
        point.x >= self.origin.x
            && point.x <= self.origin.x + self.size.width
            && point.y >= self.origin.y
            && point.y <= self.origin.y + self.size.height
    }
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.origin.x < other.origin.x + other.size.width
            && other.origin.x < self.origin.x + self.size.width
            && self.origin.y < other.origin.y + other.size.height
            && other.origin.y < self.origin.y + self.size.height
    }
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.origin.x.max(other.origin.x);
        let top = self.origin.y.max(other.origin.y);
        let right = (self.origin.x + self.size.width).min(other.origin.x + other.size.width);
        let bottom = (self.origin.y + self.size.height).min(other.origin.y + other.size.height);
        (right >= left && bottom >= top).then(|| {
            Self::from_origin_size(
                Offset::new(left, top),
                Size::new(right - left, bottom - top),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}
impl Color {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);
    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
    #[must_use]
    pub const fn to_linear_rgba(self) -> [f32; 4] {
        [
            self.red as f32 / 255.0,
            self.green as f32 / 255.0,
            self.blue as f32 / 255.0,
            self.alpha as f32 / 255.0,
        ]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub translation: Offset,
}
impl Transform {
    #[must_use]
    pub const fn translation(offset: Offset) -> Self {
        Self {
            translation: offset,
        }
    }
    #[must_use]
    pub const fn inverse_translation(self) -> Self {
        Self::translation(Offset::new(-self.translation.x, -self.translation.y))
    }
    #[must_use]
    pub const fn transform_point(self, point: Offset) -> Offset {
        Offset::new(point.x + self.translation.x, point.y + self.translation.y)
    }
    #[must_use]
    pub const fn inverse_transform_point(self, point: Offset) -> Offset {
        Offset::new(point.x - self.translation.x, point.y - self.translation.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyFlags(u8);
impl DirtyFlags {
    pub const NONE: Self = Self(0);
    pub const BUILD: Self = Self(1);
    pub const LAYOUT: Self = Self(2);
    pub const PAINT: Self = Self(4);
    pub const COMPOSITE: Self = Self(8);
    pub const SEMANTICS: Self = Self(16);
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}
impl std::ops::BitOr for DirtyFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Stable slot identity. A removed slot increments its generation before reuse.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArenaId {
    index: u32,
    generation: u32,
}
impl ArenaId {
    #[must_use]
    pub const fn from_parts(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}
impl fmt::Debug for ArenaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArenaId({}, {})", self.index, self.generation)
    }
}
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}
/// Compact safe arena used by persistent framework trees. It never exposes references
/// across mutations, so tree algorithms can remain ordinary `&mut self` code.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}
impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Arena<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }
    pub fn insert(&mut self, value: T) -> ArenaId {
        self.len += 1;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(slot.value.is_none());
            slot.value = Some(value);
            ArenaId::from_parts(index, slot.generation)
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 0,
                value: Some(value),
            });
            ArenaId::from_parts(index, 0)
        }
    }
    #[must_use]
    pub fn get(&self, id: ArenaId) -> Option<&T> {
        let slot = self.slots.get(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }
    pub fn get_mut(&mut self, id: ArenaId) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        (slot.generation == id.generation)
            .then_some(slot.value.as_mut())
            .flatten()
    }
    pub fn remove(&mut self, id: ArenaId) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        self.len -= 1;
        Some(value)
    }
    #[must_use]
    pub fn contains(&self, id: ArenaId) -> bool {
        self.get(id).is_some()
    }
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = (ArenaId, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value
                .as_ref()
                .map(|value| (ArenaId::from_parts(index as u32, slot.generation), value))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerPhase {
    Move,
    Down,
    Up,
    Cancel,
}
/// Platform-neutral modifiers sampled with a keyboard event. `command` is a
/// semantic shortcut modifier: Control on Linux/Windows and Command on macOS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
    pub command: bool,
}
/// Physical command keys understood by the first desktop input slice. Text is
/// deliberately absent: printable Unicode arrives through [`InputEvent::Text`]
/// or IME commit rather than a key-to-character mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Tab,
    Enter,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    KeyA,
    KeyC,
    KeyV,
    KeyX,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub pressed: bool,
    pub repeat: bool,
    pub modifiers: Modifiers,
}
/// IME composition is separate from committed text. Byte ranges always refer
/// to valid UTF-8 boundaries in the preedit string when supplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeEvent {
    Preedit {
        text: String,
        selection: Option<(usize, usize)>,
    },
    Commit(String),
    End,
}
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer {
        phase: PointerPhase,
        position: Offset,
    },
    Scroll {
        delta: Offset,
    },
    Key(KeyEvent),
    Text(String),
    Ime(ImeEvent),
    WindowResized {
        size: Size,
        scale_factor: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_arena_id_is_stale_after_reuse() {
        let mut arena = Arena::new();
        let first = arena.insert(1);
        assert_eq!(arena.remove(first), Some(1));
        let second = arena.insert(2);
        assert_ne!(first, second);
        assert_eq!(arena.get(first), None);
        assert_eq!(arena.get(second), Some(&2));
    }
    #[test]
    fn rect_includes_its_edges() {
        assert!(
            Rect::from_origin_size(Offset::ZERO, Size::new(2.0, 2.0))
                .contains(Offset::new(2.0, 2.0))
        );
    }
}
