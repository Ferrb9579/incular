//! Renderer-neutral widget contracts.

use crate::{BuildContext, Key};
use std::rc::Rc;

/// Base contract shared by stateless and stateful widget descriptions.
///
/// Core deliberately does not prescribe how a widget is mounted, laid out, or
/// painted. Higher-level crates can add those concerns while still accepting
/// any type implementing this identity contract. The default key is absent,
/// which gives a widget positional identity in a local reconciliation pass.
pub trait Widget: 'static {
    /// Returns the optional local identity used by reconciliation.
    fn key(&self) -> Option<&dyn Key> {
        None
    }
}

/// A type-erased widget description suitable for child collections.
pub type WidgetRef = Rc<dyn Widget>;

/// Optional build contract for widget crates that want to construct a child
/// description from a context. It is separate from [`Widget`] so core does
/// not force every renderer-specific widget to choose a child representation.
pub trait BuildableWidget: Widget {
    fn build(&self, context: &BuildContext) -> WidgetRef;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KeyHandle, StringKey};

    #[derive(Debug)]
    struct ExampleWidget {
        key: KeyHandle,
    }

    impl Widget for ExampleWidget {
        fn key(&self) -> Option<&dyn Key> {
            Some(self.key.as_key())
        }
    }

    #[test]
    fn widget_contract_exposes_an_optional_key() {
        let widget = ExampleWidget {
            key: StringKey::new("home".to_owned()).into(),
        };
        assert_eq!(widget.key().expect("key").key_id(), widget.key.key_id());
    }
}
