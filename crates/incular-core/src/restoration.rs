//! The renderer-independent boundary used by opt-in state restoration.
//!
//! `incular-runtime` owns restoration hierarchy, persistence, diagnostics,
//! and duplicate-scope detection. Lower-level crates use this small boundary
//! to restore and update JSON values without depending on the runtime.

use std::{error::Error, fmt, rc::Rc};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::Value;

/// A stable, application-provided segment in a restoration path.
///
/// A key is a segment, not a pre-concatenated path. In particular, a slash is
/// ordinary key content rather than an implicit hierarchy separator; runtime
/// stores paths structurally and applies its deterministic escaping only when
/// it needs a textual representation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RestorationKey(String);

impl RestorationKey {
    /// Creates one stable restoration path segment.
    pub fn new(segment: impl Into<String>) -> Result<Self, RestorationKeyError> {
        let segment = segment.into();
        if segment.is_empty() {
            return Err(RestorationKeyError::Empty);
        }
        if segment.contains('\0') {
            return Err(RestorationKeyError::ContainsNul);
        }
        Ok(Self(segment))
    }

    /// Borrows the application-provided segment.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes this key into its application-provided segment.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for RestorationKey {
    type Error = RestorationKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for RestorationKey {
    type Error = RestorationKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for RestorationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for RestorationKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RestorationKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|segment| Self::new(segment).map_err(de::Error::custom))
    }
}

/// Why a [`RestorationKey`] could not be created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestorationKeyError {
    /// A path segment must identify something explicitly.
    Empty,
    /// NUL is rejected so a key remains suitable for portable storage paths.
    ContainsNul,
}

impl fmt::Display for RestorationKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a restoration key cannot be empty"),
            Self::ContainsNul => formatter.write_str("a restoration key cannot contain NUL"),
        }
    }
}

impl Error for RestorationKeyError {}

/// Raw JSON storage implemented by the runtime's restoration manager.
///
/// Implementations are UI-thread-affine by design. They update in-memory
/// state synchronously and are responsible for coalescing any asynchronous
/// persistence work. Paths are always structured key segments, avoiding
/// ambiguous string concatenation.
pub trait RestorationBackend {
    /// Reads a value at the fully-qualified path.
    fn read_value(&self, path: &[RestorationKey]) -> Option<Value>;

    /// Replaces a value at the fully-qualified path.
    fn write_value(&self, path: &[RestorationKey], value: Value);

    /// Removes a value at the fully-qualified path.
    fn remove_value(&self, path: &[RestorationKey]);
}

/// A restoration location supplied by the runtime to an opt-in state owner.
///
/// Application code normally receives scopes from runtime build APIs rather
/// than constructing them. `new` remains public so embedded runtimes and
/// deterministic in-memory tests can provide their own backend.
#[derive(Clone)]
pub struct RestorationScope {
    backend: Rc<dyn RestorationBackend>,
    path: Vec<RestorationKey>,
}

impl fmt::Debug for RestorationScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestorationScope")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl RestorationScope {
    /// Creates the application-root restoration scope for a backend.
    #[must_use]
    pub fn root(backend: Rc<dyn RestorationBackend>) -> Self {
        Self {
            backend,
            path: Vec::new(),
        }
    }

    /// Creates a scope at an already-validated, fully-qualified path.
    #[must_use]
    pub fn new(
        backend: Rc<dyn RestorationBackend>,
        path: impl IntoIterator<Item = RestorationKey>,
    ) -> Self {
        Self {
            backend,
            path: path.into_iter().collect(),
        }
    }

    /// Extends this scope with one already-registered stable child segment.
    ///
    /// Runtime is responsible for registering and checking sibling scope
    /// identities before it exposes this scope. This method deliberately does
    /// no implicit child numbering or position-based identity assignment.
    #[must_use]
    pub fn child_unchecked(&self, key: RestorationKey) -> Self {
        let mut path = self.path.clone();
        path.push(key);
        Self {
            backend: self.backend.clone(),
            path,
        }
    }

    /// Returns the stable hierarchical path owned by this scope.
    #[must_use]
    pub fn path(&self) -> &[RestorationKey] {
        &self.path
    }

    /// Reads the JSON value stored under a stable value key.
    #[must_use]
    pub fn get_json(&self, key: &RestorationKey) -> Option<Value> {
        self.backend.read_value(&self.value_path(key))
    }

    /// Stores a JSON value under a stable value key.
    ///
    /// This only updates the runtime manager's in-memory snapshot. Its
    /// persistence policy determines when an asynchronous disk save occurs.
    pub fn set_json(&self, key: &RestorationKey, value: Value) {
        self.backend.write_value(&self.value_path(key), value);
    }

    /// Removes the JSON value stored under a stable value key.
    pub fn remove(&self, key: &RestorationKey) {
        self.backend.remove_value(&self.value_path(key));
    }

    fn value_path(&self, key: &RestorationKey) -> Vec<RestorationKey> {
        let mut path = self.path.clone();
        path.push(key.clone());
        path
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap};

    use serde_json::json;

    use super::*;

    #[derive(Default)]
    struct MemoryBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

    impl RestorationBackend for MemoryBackend {
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

    fn key(value: &str) -> RestorationKey {
        RestorationKey::new(value).unwrap()
    }

    #[test]
    fn scopes_keep_path_segments_unambiguous() {
        let backend = Rc::new(MemoryBackend::default());
        let scope = RestorationScope::new(backend, [key("window/main"), key("editor")]);
        let value = key("document");

        scope.set_json(&value, json!({"body": "hello"}));

        assert_eq!(scope.path(), &[key("window/main"), key("editor")]);
        assert_eq!(scope.get_json(&value), Some(json!({"body": "hello"})));
    }

    #[test]
    fn keys_validate_and_deserialize_through_the_same_rules() {
        assert_eq!(RestorationKey::new(""), Err(RestorationKeyError::Empty));
        assert_eq!(
            RestorationKey::new("contains\0nul"),
            Err(RestorationKeyError::ContainsNul)
        );
        assert!(serde_json::from_str::<RestorationKey>("\"\"").is_err());
        assert_eq!(
            serde_json::to_string(&key("main/sidebar")).unwrap(),
            "\"main/sidebar\""
        );
    }

    #[test]
    fn scopes_can_remove_an_opt_in_value() {
        let backend = Rc::new(MemoryBackend::default());
        let scope = RestorationScope::new(backend, [key("application")]);
        let value = key("theme");
        scope.set_json(&value, json!("night"));
        scope.remove(&value);
        assert_eq!(scope.get_json(&value), None);
    }
}
