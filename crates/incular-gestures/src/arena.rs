use std::collections::HashMap;

/// Identifies one pointer stream within a native window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GestureArenaKey {
    pub window: u64,
    pub pointer: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureDisposition {
    Pending,
    Accepted,
    Rejected,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GestureArenaMember(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureArenaEntry {
    pub member: GestureArenaMember,
    pub disposition: GestureDisposition,
}
#[derive(Default)]
pub struct GestureArena {
    next: u64,
    streams: HashMap<GestureArenaKey, Vec<(GestureArenaMember, bool, GestureDisposition)>>,
}
impl GestureArena {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, key: GestureArenaKey, simultaneous: bool) -> GestureArenaMember {
        self.next += 1;
        let member = GestureArenaMember(self.next);
        self.streams.entry(key).or_default().push((
            member,
            simultaneous,
            GestureDisposition::Pending,
        ));
        member
    }
    pub fn accept(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
    ) -> Vec<GestureArenaEntry> {
        let Some(entries) = self.streams.get_mut(&key) else {
            return Vec::new();
        };
        let Some(index) = entries.iter().position(|entry| entry.0 == member) else {
            return Vec::new();
        };
        if entries[index].2 != GestureDisposition::Pending {
            return entries
                .iter()
                .map(|entry| GestureArenaEntry {
                    member: entry.0,
                    disposition: entry.2,
                })
                .collect();
        }
        let simultaneous = entries[index].1;
        let blocked = entries
            .iter()
            .any(|entry| entry.2 == GestureDisposition::Accepted && !(simultaneous && entry.1));
        if blocked {
            entries[index].2 = GestureDisposition::Rejected;
        } else {
            entries[index].2 = GestureDisposition::Accepted;
            for entry in entries.iter_mut() {
                if entry.2 == GestureDisposition::Pending && !(simultaneous && entry.1) {
                    entry.2 = GestureDisposition::Rejected;
                }
            }
        }
        entries
            .iter()
            .map(|e| GestureArenaEntry {
                member: e.0,
                disposition: e.2,
            })
            .collect()
    }
    pub fn reject(&mut self, key: GestureArenaKey, member: GestureArenaMember) {
        if let Some(entries) = self.streams.get_mut(&key) {
            if let Some(entry) = entries.iter_mut().find(|e| e.0 == member) {
                entry.2 = GestureDisposition::Rejected;
            }
        }
    }
    pub fn cancel(&mut self, key: GestureArenaKey) -> Vec<GestureArenaEntry> {
        self.streams
            .remove(&key)
            .unwrap_or_default()
            .into_iter()
            .map(|(member, _, _)| GestureArenaEntry {
                member,
                disposition: GestureDisposition::Cancelled,
            })
            .collect()
    }
    #[must_use]
    pub fn entries(&self, key: GestureArenaKey) -> Vec<GestureArenaEntry> {
        self.streams
            .get(&key)
            .map(|v| {
                v.iter()
                    .map(|e| GestureArenaEntry {
                        member: e.0,
                        disposition: e.2,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
