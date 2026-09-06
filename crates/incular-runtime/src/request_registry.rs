//! Host-owned request phases. Domain code retains ownership and scheduling policy.
use std::{collections::HashMap, hash::Hash};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestPhase {
    Queued,
    Dispatched,
    Abandoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompletionRejection {
    Unknown,
    NotDispatched,
    TargetMismatch,
}

struct Entry<T> {
    value: T,
    phase: RequestPhase,
}
pub(crate) struct RequestRegistry<I, T> {
    entries: HashMap<I, Entry<T>>,
}
impl<I, T> Default for RequestRegistry<I, T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}
impl<I: Eq + Hash, T> RequestRegistry<I, T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn insert(&mut self, id: I, value: T) {
        assert!(
            !self.entries.contains_key(&id),
            "native request identity reused"
        );
        self.entries.insert(
            id,
            Entry {
                value,
                phase: RequestPhase::Queued,
            },
        );
    }
    pub(crate) fn get(&self, id: &I) -> Option<&T> {
        self.entries.get(id).map(|entry| &entry.value)
    }
    pub(crate) fn get_mut(&mut self, id: &I) -> Option<&mut T> {
        self.entries.get_mut(id).map(|entry| &mut entry.value)
    }
    pub(crate) fn phase(&self, id: &I) -> Option<RequestPhase> {
        self.entries.get(id).map(|entry| entry.phase)
    }
    pub(crate) fn dispatch(&mut self, id: &I) -> bool {
        let Some(entry) = self.entries.get_mut(id) else {
            return false;
        };
        if entry.phase != RequestPhase::Queued {
            return false;
        }
        entry.phase = RequestPhase::Dispatched;
        true
    }
    pub(crate) fn abandon(&mut self, id: &I) {
        if let Some(entry) = self.entries.get_mut(id) {
            assert_ne!(
                entry.phase,
                RequestPhase::Queued,
                "queued abandonment must remove the request"
            );
            entry.phase = RequestPhase::Abandoned;
        }
    }
    pub(crate) fn complete(
        &mut self,
        id: &I,
        matches: impl FnOnce(&T) -> bool,
    ) -> Result<T, CompletionRejection> {
        let entry = self.entries.get(id).ok_or(CompletionRejection::Unknown)?;
        if !matches(&entry.value) {
            return Err(CompletionRejection::TargetMismatch);
        }
        if entry.phase == RequestPhase::Queued {
            return Err(CompletionRejection::NotDispatched);
        }
        Ok(self
            .entries
            .remove(id)
            .expect("validated native request")
            .value)
    }
    pub(crate) fn remove(&mut self, id: &I) -> Option<T> {
        self.entries.remove(id).map(|entry| entry.value)
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&I, &T)> {
        self.entries.iter().map(|(id, entry)| (id, &entry.value))
    }
    pub(crate) fn drain(&mut self) -> impl Iterator<Item = (I, T)> {
        self.entries.drain().map(|(id, entry)| (id, entry.value))
    }
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}
