//! Accessible semantic annotation widgets.

use incular_semantics::{SemanticAction, SemanticRole};

use crate::Widget;

/// An accessible semantic annotation on a widget subtree.
#[derive(Clone, Default)]
pub struct Semantics {
    role: Option<SemanticRole>,
    label: Option<String>,
    value: Option<String>,
    description: Option<String>,
    actions: Vec<SemanticAction>,
    child: Option<Widget>,
}

impl Semantics {
    /// Creates a Semantics annotation.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            role: None,
            label: None,
            value: None,
            description: None,
            actions: Vec::new(),
            child: Some(child.into()),
        }
    }

    /// Sets the semantic role (e.g. Button, Header, TextField).
    #[must_use]
    pub fn role(mut self, role: SemanticRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Sets the accessible label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the accessible current value.
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets the accessible extended description.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds a supported semantic action.
    #[must_use]
    pub fn action(mut self, action: SemanticAction) -> Self {
        self.actions.push(action);
        self
    }

    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.description = Some(hint.into());
        self
    }

    #[must_use]
    pub fn increased_value(mut self, val: impl Into<String>) -> Self {
        self.value = Some(val.into());
        self
    }

    #[must_use]
    pub fn decreased_value(mut self, val: impl Into<String>) -> Self {
        self.value = Some(val.into());
        self
    }

    #[must_use]
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.description = Some(tooltip.into());
        self
    }

    #[must_use]
    pub const fn text_direction(self, _dir: incular_config::TextDirection) -> Self {
        self
    }

    #[must_use]
    pub const fn selected(self, _selected: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn enabled(self, _enabled: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn checked(self, _checked: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn toggled(self, _toggled: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn focused(self, _focused: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn read_only(self, _read_only: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn obscured(self, _obscured: bool) -> Self {
        self
    }

    #[must_use]
    pub const fn multiline(self, _multiline: bool) -> Self {
        self
    }

    #[must_use]
    pub fn header(mut self, is_header: bool) -> Self {
        if is_header {
            self.role = Some(SemanticRole::Heading);
        }
        self
    }

    #[must_use]
    pub fn button(mut self, is_button: bool) -> Self {
        if is_button {
            self.role = Some(SemanticRole::Button);
        }
        self
    }

    #[must_use]
    pub fn slider(mut self, is_slider: bool) -> Self {
        if is_slider {
            self.role = Some(SemanticRole::Slider);
        }
        self
    }

    #[must_use]
    pub fn link(mut self, is_link: bool) -> Self {
        if is_link {
            self.role = Some(SemanticRole::Link);
        }
        self
    }

    #[must_use]
    pub fn image(mut self, is_image: bool) -> Self {
        if is_image {
            self.role = Some(SemanticRole::Image);
        }
        self
    }

    #[must_use]
    pub fn on_tap(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_dismiss(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_increase(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_decrease(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_scroll_left(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_scroll_right(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_scroll_up(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_scroll_down(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_copy(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_cut(self, _callback: impl Fn() + 'static) -> Self {
        self
    }

    #[must_use]
    pub fn on_paste(self, _callback: impl Fn() + 'static) -> Self {
        self
    }
}

impl From<Semantics> for Widget {
    fn from(value: Semantics) -> Self {
        let mut widget: Widget = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        if let Some(role) = value.role {
            let mut explicit = crate::ExplicitSemantics::new(role);
            if let Some(l) = value.label {
                explicit = explicit.label(l);
            }
            if let Some(v) = value.value {
                explicit = explicit.value(v);
            }
            if let Some(d) = value.description {
                explicit = explicit.description(d);
            }
            widget = widget.semantics(explicit);
        } else {
            if let Some(l) = value.label {
                widget = widget.accessibility_label(l);
            }
            if let Some(d) = value.description {
                widget = widget.accessibility_description(d);
            }
        }
        widget
    }
}

/// Merges descendant accessibility nodes into a single semantic element.
#[derive(Clone, Default)]
pub struct MergeSemantics {
    child: Option<Widget>,
}

impl MergeSemantics {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
        }
    }
}

impl From<MergeSemantics> for Widget {
    fn from(value: MergeSemantics) -> Self {
        let widget: Widget = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        widget.merge_semantics()
    }
}

/// Excludes a subtree from the accessibility tree.
#[derive(Clone, Default)]
pub struct ExcludeSemantics {
    excluding: bool,
    child: Option<Widget>,
}

impl ExcludeSemantics {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            excluding: true,
            child: Some(child.into()),
        }
    }

    #[must_use]
    pub fn excluding(mut self, excluding: bool) -> Self {
        self.excluding = excluding;
        self
    }
}

impl From<ExcludeSemantics> for Widget {
    fn from(value: ExcludeSemantics) -> Self {
        let widget: Widget = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        if value.excluding {
            widget.exclude_semantics()
        } else {
            widget
        }
    }
}

/// Blocks preceding semantic siblings at the same stacking level (e.g. for modal dialogs).
#[derive(Clone, Default)]
pub struct BlockSemantics {
    blocking: bool,
    child: Option<Widget>,
}

impl BlockSemantics {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            blocking: true,
            child: Some(child.into()),
        }
    }

    #[must_use]
    pub fn blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }
}

impl From<BlockSemantics> for Widget {
    fn from(value: BlockSemantics) -> Self {
        let widget: Widget = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        if value.blocking {
            widget.block_semantics()
        } else {
            widget
        }
    }
}
