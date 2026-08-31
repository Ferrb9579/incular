//! Declarative widget values, kinds, and constructors.
//!
//! This module owns the immutable widget description consumed by the retained tree.

use super::*;

mod constructors;
mod geometry;
mod structure;

pub(in crate::tree) use geometry::{
    enforced_constraints, fractional_constraints, physical_scroll_offset, scroll_constraints,
    scroll_delta_for_axis, scroll_size, scroll_translation, scroll_viewport_extent, sliver_anchor,
    sliver_viewport_size, unconstrained_constraints,
};
pub(crate) use structure::{LoweringFamily, WidgetChildren, WidgetType};

/// Immutable declarative descriptor node shared by cheap [`Widget`] handles.
///
/// The type is doc-hidden because `Widget` is the public transport value; the
/// node exists only as the reference-counted ownership boundary. Its fields are
/// crate-private so framework implementation code can keep exhaustive matching
/// local without exposing representation details to applications.
#[derive(Clone)]
pub(crate) struct WidgetNode {
    pub(crate) key: Option<Key>,
    pub(crate) kind: WidgetKind,
    pub(crate) semantics: SemanticProperties,
}

/// A cheap immutable declarative widget handle.
///
/// Cloning a `Widget` clones one reference-counted descriptor pointer; retained
/// identity continues to belong exclusively to [`Element`].
pub struct Widget {
    node: Option<Rc<WidgetNode>>,
}

impl Clone for Widget {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
        }
    }
}

struct WidgetDropQueue {
    active: bool,
    pending: Vec<Rc<WidgetNode>>,
}

thread_local! {
    static WIDGET_DROP_QUEUE: RefCell<WidgetDropQueue> = const {
        RefCell::new(WidgetDropQueue {
            active: false,
            pending: Vec::new(),
        })
    };
}

impl Drop for Widget {
    fn drop(&mut self) {
        let Some(node) = self.node.take() else {
            return;
        };
        WIDGET_DROP_QUEUE.with(|queue| {
            {
                let mut queue = queue.borrow_mut();
                queue.pending.push(node);
                if queue.active {
                    return;
                }
                queue.active = true;
            }

            struct ResetDropQueue<'a>(&'a RefCell<WidgetDropQueue>);
            impl Drop for ResetDropQueue<'_> {
                fn drop(&mut self) {
                    self.0.borrow_mut().active = false;
                }
            }
            let _reset = ResetDropQueue(queue);

            loop {
                let next = queue.borrow_mut().pending.pop();
                let Some(node) = next else {
                    break;
                };
                match Rc::try_unwrap(node) {
                    // Dropping the unique node invokes Drop for its child Widget
                    // handles. Because this queue is active, those drops enqueue
                    // their nodes instead of recursively destroying them.
                    Ok(node) => drop(node),
                    // Another descriptor/clone still owns this node. Releasing
                    // this edge cannot destroy its descendants yet.
                    Err(node) => drop(node),
                }
            }
        });
    }
}

impl std::fmt::Debug for Widget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Widget")
            .field("key", &self.key())
            .field("kind", self.kind())
            .finish()
    }
}
impl PartialEq for Widget {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
            && self.kind() == other.kind()
            && self.semantic_properties() == other.semantic_properties()
    }
}
