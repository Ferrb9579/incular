//! Widget identity keys.
//!
//! Keys are intentionally independent from a particular element tree. A
//! widget can expose a borrowed [`Key`] while the tree stores a cloneable
//! [`KeyHandle`] for reconciliation. [`UniqueKey`] values are local keys:
//! every call to [`UniqueKey::new`] produces a distinct identity, including
//! when two values have the same type and are otherwise empty.

use std::{
    any::{Any, TypeId},
    fmt,
    hash::{Hash, Hasher},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_UNIQUE_KEY: AtomicU64 = AtomicU64::new(1);

/// A compact, hashable description of a concrete key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyId {
    type_id: TypeId,
    hash: u64,
}

impl KeyId {
    #[must_use]
    pub const fn type_id(self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn hash(self) -> u64 {
        self.hash
    }
}

/// A type-erased identity used by widgets and reconciliation algorithms.
/// Implementors should be immutable value types.
pub trait Key: Any + fmt::Debug {
    /// Returns a stable, process-local identity for this key value.
    fn key_id(&self) -> KeyId;

    /// Supports equality after type erasure without requiring `dyn Key` to
    /// implement `Eq` itself.
    fn eq_key(&self, other: &dyn Key) -> bool;

    /// Clones a concrete key through a trait object.
    fn clone_box(&self) -> Box<dyn Key>;

    /// Provides a downcast hook for custom key implementations.
    fn as_any(&self) -> &dyn Any;
}

/// Marker for keys whose identity is scoped to a local widget tree.
pub trait LocalKey: Key {}

/// A globally unique local key. It is useful for forcing a fresh element when
/// a widget is rebuilt, and is intentionally not based on object addresses.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UniqueKey(u64);

impl UniqueKey {
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_UNIQUE_KEY.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    pub const fn id(self) -> u64 {
        self.0
    }
}

impl Default for UniqueKey {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for UniqueKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("UniqueKey").field(&self.0).finish()
    }
}

impl Key for UniqueKey {
    fn key_id(&self) -> KeyId {
        KeyId {
            type_id: TypeId::of::<Self>(),
            hash: self.0,
        }
    }

    fn eq_key(&self, other: &dyn Key) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self == other)
    }

    fn clone_box(&self) -> Box<dyn Key> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LocalKey for UniqueKey {}

/// A value-based local key. `ValueKey<T>` is the Rust equivalent of Flutter's
/// `ValueKey<T>` while remaining useful for any `Eq + Hash` value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValueKey<T>(pub T);

impl<T> ValueKey<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(&self) -> &T {
        &self.0
    }
}

impl<T> Key for ValueKey<T>
where
    T: Any + Clone + Eq + Hash + fmt::Debug,
{
    fn key_id(&self) -> KeyId {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        KeyId {
            type_id: TypeId::of::<Self>(),
            hash: hasher.finish(),
        }
    }

    fn eq_key(&self, other: &dyn Key) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self == other)
    }

    fn clone_box(&self) -> Box<dyn Key> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T> LocalKey for ValueKey<T> where T: Any + Clone + Eq + Hash + fmt::Debug {}

/// Convenient string key constructor.
pub type StringKey = ValueKey<String>;

/// Compatibility name for value/object keys.
pub type KeyObject<T> = ValueKey<T>;

/// Cloneable type-erased key. This is the type to use in a heterogeneous key
/// field or collection.
pub struct KeyHandle(Box<dyn Key>);

impl KeyHandle {
    #[must_use]
    pub fn new<K>(key: K) -> Self
    where
        K: Key,
    {
        Self(Box::new(key))
    }

    #[must_use]
    pub fn as_key(&self) -> &dyn Key {
        &*self.0
    }

    #[must_use]
    pub fn key_id(&self) -> KeyId {
        self.0.key_id()
    }
}

impl Clone for KeyHandle {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl fmt::Debug for KeyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("KeyHandle").field(&self.0).finish()
    }
}

impl PartialEq for KeyHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_key(&*other.0)
    }
}

impl Eq for KeyHandle {}

impl Hash for KeyHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.key_id(), state);
    }
}

impl Key for KeyHandle {
    fn key_id(&self) -> KeyId {
        self.0.key_id()
    }

    fn eq_key(&self, other: &dyn Key) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self == other)
    }

    fn clone_box(&self) -> Box<dyn Key> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LocalKey for KeyHandle {}

impl From<String> for KeyHandle {
    fn from(value: String) -> Self {
        Self::new(StringKey::new(value))
    }
}

impl From<&str> for KeyHandle {
    fn from(value: &str) -> Self {
        Self::new(StringKey::new(value.to_owned()))
    }
}

impl From<UniqueKey> for KeyHandle {
    fn from(value: UniqueKey) -> Self {
        Self::new(value)
    }
}

impl<T> From<ValueKey<T>> for KeyHandle
where
    T: Any + Clone + Eq + Hash + fmt::Debug,
{
    fn from(value: ValueKey<T>) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_keys_are_distinct_and_cloneable() {
        let first = UniqueKey::new();
        let second = UniqueKey::new();
        assert_ne!(first, second);
        assert_eq!(first, first);
        assert_eq!(KeyHandle::from(first), KeyHandle::from(first));
    }

    #[test]
    fn value_keys_compare_by_value_after_erasure() {
        let first = KeyHandle::from(ValueKey::new(42_u32));
        let second = KeyHandle::from(ValueKey::new(42_u32));
        let other = KeyHandle::from(ValueKey::new(7_u32));
        assert_eq!(first, second);
        assert_ne!(first, other);
        assert_eq!(first.key_id(), second.key_id());
    }
}
