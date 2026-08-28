//! Accessible semantic annotation widgets.

use incular_semantics::{SemanticAction, SemanticRole, SemanticState};
use typed_builder::TypedBuilder;

use crate::Widget;

/// An accessible semantic annotation on a widget subtree.
#[derive(Clone, Default, TypedBuilder)]
pub struct Semantics {
    #[builder(default, setter(strip_option))]
    role: Option<SemanticRole>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(strip_option, into))]
    value: Option<String>,
    #[builder(default, setter(strip_option, into))]
    description: Option<String>,
    #[builder(default)]
    actions: Vec<SemanticAction>,
    #[builder(default)]
    state: SemanticState,
    #[builder(default, setter(strip_option, into))]
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
            state: SemanticState::default(),
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

    /// Sets the complete retained semantic state for this annotation.
    ///
    /// The state is copied into the retained [`crate::ExplicitSemantics`]
    /// descriptor when this widget is converted. This keeps semantic state
    /// renderer-independent and lets native accessibility projections expose
    /// the same values as the widget tree.
    #[must_use]
    pub fn state(mut self, state: SemanticState) -> Self {
        self.state = state;
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
    pub fn selected(mut self, selected: bool) -> Self {
        self.state.selected = selected;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.state.enabled = enabled;
        self
    }

    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.state.checked = Some(checked);
        self
    }

    #[must_use]
    pub fn toggled(mut self, toggled: bool) -> Self {
        self.state.checked = Some(toggled);
        self
    }

    #[must_use]
    pub fn focused(mut self, focused: bool) -> Self {
        self.state.focused = focused;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }

    #[must_use]
    pub const fn obscured(self, _obscured: bool) -> Self {
        self
    }

    #[must_use]
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.state.multiline = multiline;
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

    fn explicit(&self) -> Option<crate::ExplicitSemantics> {
        let role = self.role?;
        let mut explicit = crate::ExplicitSemantics::new(role)
            .state(self.state.clone())
            .actions(self.actions.iter().map(SemanticAction::kind));
        if let Some(label) = &self.label {
            explicit = explicit.label(label.clone());
        }
        if let Some(value) = &self.value {
            explicit = explicit.value(value.clone());
        }
        if let Some(description) = &self.description {
            explicit = explicit.description(description.clone());
        }
        Some(explicit)
    }
}

impl From<Semantics> for Widget {
    fn from(value: Semantics) -> Self {
        let explicit = value.explicit();
        let mut widget: Widget = value
            .child
            .unwrap_or_else(|| crate::SizedBox::shrink().into());
        if let Some(explicit) = explicit {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_state_and_explicit_actions_survive_widget_conversion() {
        let semantics = Semantics::new(crate::SizedBox::shrink())
            .role(SemanticRole::Checkbox)
            .label("Remember me")
            .value("on")
            .enabled(true)
            .selected(true)
            .checked(true)
            .focused(true)
            .read_only(true)
            .multiline(true)
            .action(SemanticAction::Focus)
            .action(SemanticAction::Activate);

        let explicit = semantics
            .explicit()
            .expect("role creates explicit semantics");
        assert_eq!(explicit.role, SemanticRole::Checkbox);
        assert_eq!(explicit.label.as_deref(), Some("Remember me"));
        assert_eq!(explicit.value.as_deref(), Some("on"));
        assert!(explicit.state.enabled);
        assert!(explicit.state.selected);
        assert_eq!(explicit.state.checked, Some(true));
        assert!(explicit.state.focused);
        assert!(explicit.state.read_only);
        assert!(explicit.state.multiline);
        assert_eq!(
            explicit.actions,
            vec![
                incular_semantics::SemanticActionKind::Focus,
                incular_semantics::SemanticActionKind::Activate,
            ]
        );
    }
}

/// Merges descendant accessibility nodes into a single semantic element.
#[derive(Clone, Default, TypedBuilder)]
pub struct MergeSemantics {
    #[builder(default, setter(strip_option, into))]
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
#[derive(Clone, TypedBuilder)]
pub struct ExcludeSemantics {
    #[builder(default = true)]
    excluding: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Default for ExcludeSemantics {
    fn default() -> Self {
        Self {
            excluding: true,
            child: None,
        }
    }
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
#[derive(Clone, TypedBuilder)]
pub struct BlockSemantics {
    #[builder(default = true)]
    blocking: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Default for BlockSemantics {
    fn default() -> Self {
        Self {
            blocking: true,
            child: None,
        }
    }
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
