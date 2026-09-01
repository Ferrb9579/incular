use incular_platform::DisplayId;

struct DisplaySlot<K> {
    generation: u32,
    key: Option<K>,
    retired: bool,
}

/// Generational identity allocator used by the native monitor registry.
///
/// This stays crate-private; integration tests include this small production
/// module directly so the repository can keep all test bodies under `tests/`.
pub(crate) struct GenerationalDisplayRegistry<K> {
    slots: Vec<DisplaySlot<K>>,
}

impl<K> Default for GenerationalDisplayRegistry<K> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<K: Clone + Eq> GenerationalDisplayRegistry<K> {
    pub(crate) fn synchronize(&mut self, keys: &[K]) {
        for slot in &mut self.slots {
            let still_connected = slot
                .key
                .as_ref()
                .is_some_and(|key| keys.iter().any(|candidate| candidate == key));
            if slot.key.is_some() && !still_connected {
                slot.key = None;
                if let Some(next) = slot.generation.checked_add(1) {
                    slot.generation = next;
                } else {
                    // Never wrap back to a previously valid public identity.
                    slot.retired = true;
                }
            }
        }

        for key in keys {
            if self.id_for(key).is_some() {
                continue;
            }
            if let Some(slot) = self
                .slots
                .iter_mut()
                .find(|slot| slot.key.is_none() && !slot.retired)
            {
                slot.key = Some(key.clone());
                continue;
            }
            // A process cannot realistically discover more than u32::MAX
            // monitor generations. If it somehow does, leave the excess
            // monitor unrepresented rather than aliasing an existing ID.
            if u32::try_from(self.slots.len()).is_ok() {
                self.slots.push(DisplaySlot {
                    generation: 0,
                    key: Some(key.clone()),
                    retired: false,
                });
            }
        }
    }

    pub(crate) fn id_for(&self, key: &K) -> Option<DisplayId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            (slot.key.as_ref() == Some(key)).then(|| {
                DisplayId::from_parts(
                    u32::try_from(index).expect("display slot was admitted as u32"),
                    slot.generation,
                )
            })
        })
    }

    pub(crate) fn connected(&self) -> impl Iterator<Item = (DisplayId, &K)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.key.as_ref().map(|key| {
                (
                    DisplayId::from_parts(
                        u32::try_from(index).expect("display slot was admitted as u32"),
                        slot.generation,
                    ),
                    key,
                )
            })
        })
    }
}
