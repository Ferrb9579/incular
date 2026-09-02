//! Native-menu coordination shared by desktop facade crates.
//!
//! This module deliberately owns no HWND/NSMenu/X11 APIs. It only supplies the
//! generation-safe command registry and portable shortcut matching that every
//! native backend must obey.

use incular_widgets::{
    MenuItemId, PlatformMenuShortcut, PlatformMenuSnapshot, PlatformMenuSnapshotNode,
    ShortcutModifiers,
};
use std::collections::{BTreeMap, BTreeSet};

/// Backend-private command identity. Application code always sees
/// [`MenuItemId`], never this integer.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeMenuCommandId(u32);

impl NativeMenuCommandId {
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Failure to construct a safe native command mapping.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuRegistryError {
    DuplicateId(MenuItemId),
    CommandSpaceExhausted,
}

/// Stable command mapping for one native application-menu delegate.
///
/// IDs that survive a consecutive snapshot keep their native command integer.
/// Once an ID disappears, its integer is retired permanently for this delegate
/// lifetime. This prevents a delayed native message from an older menu
/// generation from ever resolving to a different retained command.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct NativeMenuCommandRegistry {
    generation: u64,
    next_command: u32,
    max_command: u32,
    active_by_id: BTreeMap<MenuItemId, NativeMenuCommandId>,
    active_by_command: BTreeMap<NativeMenuCommandId, MenuItemId>,
}

impl Default for NativeMenuCommandRegistry {
    fn default() -> Self {
        Self::with_max_command_id(u32::MAX)
    }
}

impl NativeMenuCommandRegistry {
    #[must_use]
    pub fn with_max_command_id(max_command: u32) -> Self {
        Self {
            generation: 0,
            next_command: 1,
            max_command,
            active_by_id: BTreeMap::new(),
            active_by_command: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Replaces the active snapshot transactionally.
    pub fn synchronize(
        &mut self,
        snapshot: &PlatformMenuSnapshot,
    ) -> Result<(), NativeMenuRegistryError> {
        let ids = selectable_ids(snapshot)?;
        let needed = ids
            .iter()
            .filter(|id| !self.active_by_id.contains_key(*id))
            .count();
        if needed > 0 {
            let last = self
                .next_command
                .checked_add(u32::try_from(needed - 1).unwrap_or(u32::MAX))
                .ok_or(NativeMenuRegistryError::CommandSpaceExhausted)?;
            if last > self.max_command {
                return Err(NativeMenuRegistryError::CommandSpaceExhausted);
            }
        }

        let mut next_command = self.next_command;
        let mut next_by_id = BTreeMap::new();
        let mut next_by_command = BTreeMap::new();
        for id in ids {
            let command = self.active_by_id.get(&id).copied().unwrap_or_else(|| {
                let command = NativeMenuCommandId(next_command);
                next_command += 1;
                command
            });
            next_by_command.insert(command, id.clone());
            next_by_id.insert(id, command);
        }

        if self.active_by_id != next_by_id {
            self.generation = self.generation.wrapping_add(1);
        }
        self.next_command = next_command;
        self.active_by_id = next_by_id;
        self.active_by_command = next_by_command;
        Ok(())
    }

    #[must_use]
    pub fn command_for(&self, id: &MenuItemId) -> Option<NativeMenuCommandId> {
        self.active_by_id.get(id).copied()
    }

    #[must_use]
    pub fn resolve(&self, command: NativeMenuCommandId) -> Option<&MenuItemId> {
        self.active_by_command.get(&command)
    }

    #[must_use]
    pub fn resolve_raw(&self, command: u32) -> Option<&MenuItemId> {
        self.resolve(NativeMenuCommandId(command))
    }
}

/// Returns the enabled leaf command matching one desktop key event.
#[doc(hidden)]
#[must_use]
pub fn matching_menu_shortcut(
    snapshot: &PlatformMenuSnapshot,
    key: &str,
    modifiers: ShortcutModifiers,
) -> Option<MenuItemId> {
    fn walk(
        nodes: &[PlatformMenuSnapshotNode],
        key: &str,
        modifiers: ShortcutModifiers,
    ) -> Option<MenuItemId> {
        for node in nodes {
            if node.enabled
                && !node.separator
                && node.selectable
                && node.shortcut.as_ref().is_some_and(|shortcut| {
                    shortcut.modifiers == modifiers && shortcut.key.eq_ignore_ascii_case(key)
                })
            {
                return Some(node.id.clone());
            }
            if let Some(found) = walk(&node.children, key, modifiers) {
                return Some(found);
            }
        }
        None
    }
    walk(&snapshot.menus, key, modifiers)
}

/// Formats a portable shortcut for native menu labels on platforms whose menu
/// API does not separately render accelerators (notably Win32 without HACCEL).
#[doc(hidden)]
#[must_use]
pub fn format_menu_shortcut(shortcut: &PlatformMenuShortcut) -> String {
    let mut parts = Vec::with_capacity(5);
    if shortcut.modifiers.contains(ShortcutModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if shortcut.modifiers.contains(ShortcutModifiers::ALT) {
        parts.push("Alt");
    }
    if shortcut.modifiers.contains(ShortcutModifiers::SHIFT) {
        parts.push("Shift");
    }
    if shortcut.modifiers.contains(ShortcutModifiers::META) {
        parts.push("Meta");
    }
    parts.push(shortcut.key.as_str());
    parts.join("+")
}

/// Returns whether two snapshots can reuse an already-materialized native menu
/// tree. Labels, enabled state, tooltips, and shortcuts are attributes and may
/// be updated in place; retained IDs, separator positions, and submenu topology
/// define native identity.
#[doc(hidden)]
#[must_use]
pub fn native_menu_structure_equal(
    left: &PlatformMenuSnapshot,
    right: &PlatformMenuSnapshot,
) -> bool {
    fn nodes(left: &[PlatformMenuSnapshotNode], right: &[PlatformMenuSnapshotNode]) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                left.id == right.id
                    && left.separator == right.separator
                    && left.selectable == right.selectable
                    && nodes(&left.children, &right.children)
            })
    }

    nodes(&left.menus, &right.menus)
}

fn selectable_ids(
    snapshot: &PlatformMenuSnapshot,
) -> Result<Vec<MenuItemId>, NativeMenuRegistryError> {
    fn walk(
        nodes: &[PlatformMenuSnapshotNode],
        seen: &mut BTreeSet<MenuItemId>,
        selectable: &mut Vec<MenuItemId>,
    ) -> Result<(), NativeMenuRegistryError> {
        for node in nodes {
            if !seen.insert(node.id.clone()) {
                return Err(NativeMenuRegistryError::DuplicateId(node.id.clone()));
            }
            if !node.separator && node.selectable {
                selectable.push(node.id.clone());
            }
            walk(&node.children, seen, selectable)?;
        }
        Ok(())
    }

    let mut seen = BTreeSet::new();
    let mut selectable = Vec::new();
    walk(&snapshot.menus, &mut seen, &mut selectable)?;
    Ok(selectable)
}
