//! Build-time inherited environment access.
//!
//! This module owns borrowed build context views and inherited descriptor values; retained dependency tracking remains owned by `WidgetTree`.

use super::*;

/// A borrowed view of the retained inherited environment for one live build
/// operation. The reference cannot escape the callback that receives it.
pub struct BuildContext<'a> {
    inner: &'a DependencyContext,
}

pub(crate) type LayoutBuilderCallback = dyn for<'a> Fn(&BuildContext<'a>, Constraints) -> Widget;

impl<'a> BuildContext<'a> {
    pub(super) fn new(inner: &'a DependencyContext) -> Self {
        Self { inner }
    }

    /// Reads the nearest inherited value and registers this element as a
    /// dependent of the exact retained scope that supplied it.
    #[must_use]
    pub fn depend_on<T: Any + Clone>(&self) -> Option<T> {
        self.inner.depend::<T>()
    }

    /// Reads the nearest inherited value without creating an invalidation edge.
    #[must_use]
    pub fn find<T: Any + Clone>(&self) -> Option<T> {
        self.inner.read::<T>()
    }

    /// Shared-ownership form of [`BuildContext::depend_on`].
    #[must_use]
    pub fn depend_on_shared<T: Any>(&self) -> Option<Rc<T>> {
        self.inner.depend_shared::<T>()
    }

    /// Shared-ownership form of [`BuildContext::find`].
    #[must_use]
    pub fn find_shared<T: Any>(&self) -> Option<Rc<T>> {
        self.inner.read_shared::<T>()
    }
}

#[derive(Clone)]
#[doc(hidden)]
pub struct InheritedScopeValue {
    pub(crate) type_id: TypeId,
    pub(crate) value: Rc<dyn Any>,
}

impl InheritedScopeValue {
    pub(super) fn new<T: Any>(value: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            value: Rc::new(value),
        }
    }
}
