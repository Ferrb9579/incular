//! Keyed page storage retained independently from the element tree.

use crate::Widget;
use incular_core::{Key, KeyHandle, KeyId, LocalKey};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    rc::Rc,
};

/// A value key that participates in a page-storage identifier chain and can
/// also be used as a retained widget key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PageStorageKey {
    key: KeyHandle,
}

impl PageStorageKey {
    /// Creates a page-storage key from any Incular key value (including a
    /// string, `ValueKey<T>`, or `UniqueKey`).
    #[must_use]
    pub fn new(key: impl Into<KeyHandle>) -> Self {
        Self { key: key.into() }
    }

    /// Returns the underlying erased key identity.
    #[must_use]
    pub fn key(&self) -> &dyn Key {
        self.key.as_key()
    }

    /// Returns the key identity used by retained widget reconciliation.
    #[must_use]
    pub fn key_id(&self) -> KeyId {
        self.key.key_id()
    }
}

impl Key for PageStorageKey {
    fn key_id(&self) -> KeyId {
        self.key.key_id()
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LocalKey for PageStorageKey {}

/// The ancestor-key chain used to address one page-storage value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PageStorageIdentifier {
    keys: Vec<PageStorageKey>,
}

impl PageStorageIdentifier {
    /// Creates an identifier from an ordered ancestor-key chain.
    #[must_use]
    pub fn new(keys: impl IntoIterator<Item = PageStorageKey>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }

    /// Creates a one-key identifier.
    #[must_use]
    pub fn key(key: impl Into<PageStorageKey>) -> Self {
        Self::new([key.into()])
    }

    /// Borrows the ordered key chain.
    #[must_use]
    pub fn keys(&self) -> &[PageStorageKey] {
        &self.keys
    }

    /// Returns whether the identifier has no key and therefore cannot store
    /// state.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Appends one nested ancestor key.
    pub fn push(&mut self, key: impl Into<PageStorageKey>) {
        self.keys.push(key.into());
    }
}

impl From<PageStorageKey> for PageStorageIdentifier {
    fn from(value: PageStorageKey) -> Self {
        Self::key(value)
    }
}

impl From<&PageStorageKey> for PageStorageIdentifier {
    fn from(value: &PageStorageKey) -> Self {
        Self::key(value.clone())
    }
}

impl From<Vec<PageStorageKey>> for PageStorageIdentifier {
    fn from(value: Vec<PageStorageKey>) -> Self {
        Self::new(value)
    }
}

impl From<&[PageStorageKey]> for PageStorageIdentifier {
    fn from(value: &[PageStorageKey]) -> Self {
        Self::new(value.iter().cloned())
    }
}

/// Shared keyed storage for pages that leave the active retained subtree.
#[derive(Clone)]
pub struct PageStorageBucket {
    state: Rc<RefCell<HashMap<PageStorageIdentifier, Value>>>,
    revision: Rc<Cell<u64>>,
}

impl Default for PageStorageBucket {
    fn default() -> Self {
        Self {
            state: Rc::new(RefCell::new(HashMap::new())),
            revision: Rc::new(Cell::new(0)),
        }
    }
}

impl fmt::Debug for PageStorageBucket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PageStorageBucket")
            .field("entries", &self.len())
            .field("revision", &self.revision())
            .finish()
    }
}

impl PartialEq for PageStorageBucket {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for PageStorageBucket {}

impl PageStorageBucket {
    /// Writes a JSON value under an ordered key chain.  Empty identifiers are
    /// rejected because Flutter's PageStorage only saves keyed state.
    pub fn write_state<I, V>(&self, identifier: I, value: V) -> bool
    where
        I: Into<PageStorageIdentifier>,
        V: Into<Value>,
    {
        let identifier = identifier.into();
        if identifier.is_empty() {
            return false;
        }
        let value = value.into();
        let changed = self.state.borrow().get(&identifier) != Some(&value);
        if changed {
            self.state.borrow_mut().insert(identifier, value);
            self.bump_revision();
        }
        changed
    }

    /// Reads a previously stored JSON value.
    #[must_use]
    pub fn read_state<I>(&self, identifier: I) -> Option<Value>
    where
        I: Into<PageStorageIdentifier>,
    {
        let identifier = identifier.into();
        (!identifier.is_empty())
            .then(|| self.state.borrow().get(&identifier).cloned())
            .flatten()
    }

    /// Removes one keyed state value.
    pub fn remove_state<I>(&self, identifier: I) -> Option<Value>
    where
        I: Into<PageStorageIdentifier>,
    {
        let identifier = identifier.into();
        if identifier.is_empty() {
            return None;
        }
        let removed = self.state.borrow_mut().remove(&identifier);
        if removed.is_some() {
            self.bump_revision();
        }
        removed
    }

    /// Removes all saved page state and returns the number of entries removed.
    pub fn clear(&self) -> usize {
        let removed = self.state.borrow().len();
        if removed != 0 {
            self.state.borrow_mut().clear();
            self.bump_revision();
        }
        removed
    }

    /// Returns whether a keyed value is present.
    #[must_use]
    pub fn contains<I>(&self, identifier: I) -> bool
    where
        I: Into<PageStorageIdentifier>,
    {
        let identifier = identifier.into();
        !identifier.is_empty() && self.state.borrow().contains_key(&identifier)
    }

    /// Returns the number of keyed values in this bucket.
    #[must_use]
    pub fn len(&self) -> usize {
        self.state.borrow().len()
    }

    /// Returns whether this bucket has no keyed values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Monotonic revision for retained consumers that need to refresh a page.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn bump_revision(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }
}

/// Provides a shared [`PageStorageBucket`] to deferred descendants.
#[derive(Clone, Debug, PartialEq)]
pub struct PageStorage {
    bucket: PageStorageBucket,
    child: Widget,
}

impl PageStorage {
    /// Creates a page-storage scope with a new bucket.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::with_bucket(PageStorageBucket::default(), child)
    }

    /// Creates a page-storage scope with explicit retained bucket ownership.
    #[must_use]
    pub fn with_bucket(bucket: PageStorageBucket, child: impl Into<Widget>) -> Self {
        Self {
            bucket,
            child: child.into(),
        }
    }

    /// Returns the bucket owned by this scope.
    #[must_use]
    pub fn bucket(&self) -> PageStorageBucket {
        self.bucket.clone()
    }

    /// Returns the wrapped child descriptor.
    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }

    /// Reads the nearest page-storage bucket while a deferred descendant is
    /// being materialized.
    #[must_use]
    pub fn maybe_of() -> Option<PageStorageBucket> {
        current_page_storage_bucket()
    }

    /// Reads the nearest page-storage bucket and panics when no scope exists.
    #[must_use]
    pub fn of() -> PageStorageBucket {
        Self::maybe_of().expect("PageStorage.of() called without a PageStorage scope")
    }
}

/// Reads the nearest retained page-storage scope.
#[must_use]
pub fn current_page_storage_bucket() -> Option<PageStorageBucket> {
    crate::tree::current_build_environment::<PageStorageBucket>()
}

impl From<PageStorage> for Widget {
    fn from(value: PageStorage) -> Self {
        Widget::environment_scope(value.bucket, value.child)
    }
}
