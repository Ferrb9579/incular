use std::rc::Rc;

use super::{
    controls::{MenuCloseScope, MenuController},
    popup::menu_panel,
    style::MenuStyle,
};
use incular_config::Clip;
use incular_controls::{Button as ControlButton, ButtonStyle, current_control_theme};
use incular_core::{Color, Offset};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use incular_widgets::{
    Container, ExcludeSemantics, GestureDetector, HitTestBehavior, IgnorePointer, OverlayPortal,
    Positioned, SizedBox, Text, TransientPlacement, TransientRole, Widget,
};
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
#[builder(builder_method(name = typed_builder))]
pub struct MenuAnchor {
    #[builder(
        setter(transform = |menu_children: impl IntoIterator<Item = impl Into<Widget>>| {
            menu_children
                .into_iter()
                .map(Into::into)
                .collect::<Vec<Widget>>()
        })
    )]
    menu_children: Vec<Widget>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(
        default,
        setter(
            fn transform<F>(builder: F) -> Option<Rc<dyn Fn(bool) -> Widget + 'static>>
            where
                F: Fn(bool) -> Widget + 'static,
            {
                Some(Rc::new(builder))
            }
        )
    )]
    builder: Option<Rc<dyn Fn(bool) -> Widget + 'static>>,
    #[builder(default, setter(strip_option))]
    style: Option<MenuStyle>,
    #[builder(default, setter(strip_option))]
    item_style: Option<ButtonStyle>,
    #[builder(default)]
    alignment_offset: Offset,
    #[builder(default, setter(skip))]
    submenu: bool,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
    #[builder(default)]
    consume_outside_tap: bool,
    #[builder(default = true)]
    cross_axis_unconstrained: bool,
    #[builder(default)]
    use_root_overlay: bool,
    #[builder(default)]
    animated: bool,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    controller: MenuController,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_open: Option<Rc<dyn Fn() + 'static>>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_close: Option<Rc<dyn Fn() + 'static>>,
}

impl Default for MenuAnchor {
    fn default() -> Self {
        Self::typed_builder()
            .menu_children(Vec::<Widget>::new())
            .build()
    }
}

impl MenuAnchor {
    #[must_use]
    pub fn new(menu_children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            menu_children: menu_children.into_iter().map(Into::into).collect(),
            child: None,
            builder: None,
            style: None,
            item_style: None,
            alignment_offset: Offset::ZERO,
            submenu: false,
            clip_behavior: Clip::HardEdge,
            consume_outside_tap: false,
            cross_axis_unconstrained: true,
            use_root_overlay: false,
            animated: false,
            enabled: true,
            controller: MenuController::new(),
            on_open: None,
            on_close: None,
        }
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    /// Supplies a builder that receives the current open state.
    #[must_use]
    pub fn builder(mut self, value: impl Fn(bool) -> Widget + 'static) -> Self {
        self.builder = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn style(mut self, value: MenuStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn item_style(mut self, value: ButtonStyle) -> Self {
        self.item_style = Some(value);
        self
    }

    #[must_use]
    pub fn alignment_offset(mut self, value: Offset) -> Self {
        self.alignment_offset = value;
        self
    }

    #[must_use]
    pub(crate) fn submenu(mut self, value: bool) -> Self {
        self.submenu = value;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, value: Clip) -> Self {
        self.clip_behavior = value;
        self
    }

    #[must_use]
    pub fn consume_outside_tap(mut self, value: bool) -> Self {
        self.consume_outside_tap = value;
        self
    }

    #[must_use]
    pub fn cross_axis_unconstrained(mut self, value: bool) -> Self {
        self.cross_axis_unconstrained = value;
        self
    }

    #[must_use]
    pub fn use_root_overlay(mut self, value: bool) -> Self {
        self.use_root_overlay = value;
        self
    }

    #[must_use]
    pub fn animated(mut self, value: bool) -> Self {
        self.animated = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn open(self, value: bool) -> Self {
        self.controller.set_open(value);
        self
    }

    #[must_use]
    pub fn controller(mut self, value: MenuController) -> Self {
        self.controller = value;
        self
    }

    #[must_use]
    pub fn on_open(mut self, value: impl Fn() + 'static) -> Self {
        self.on_open = Some(Rc::new(value));
        self
    }

    #[must_use]
    pub fn on_close(mut self, value: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(value));
        self
    }

    fn build(&self, context: &incular_widgets::BuildContext<'_>) -> Widget {
        let open = self.controller.is_open();
        let anchor_child = self.builder.as_ref().map_or_else(
            || {
                self.child
                    .clone()
                    .unwrap_or_else(|| Text::new("Menu").into())
            },
            |builder| builder(open),
        );
        let controller = self.controller.clone();
        let on_open = self.on_open.clone();
        let on_close = self.on_close.clone();
        let anchor_label = anchor_child.semantic_text();
        // `MenuAnchor` owns the trigger interaction. Treat the supplied child
        // as presentation even when callers use a Material Button for its
        // visuals; otherwise the nested inner Button wins retained hit testing
        // and the menu trigger never receives the activation.
        let anchor_child: Widget = IgnorePointer::new(ExcludeSemantics::new(anchor_child)).into();
        let mut anchor = ActionSurface::with_child(anchor_child)
            .color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if self.enabled {
            anchor = anchor.on_click(move || {
                let next = !controller.is_open();
                controller.set_open(next);
                if next {
                    if let Some(callback) = on_open.as_ref() {
                        callback();
                    }
                } else if let Some(callback) = on_close.as_ref() {
                    callback();
                }
            });
        }
        let anchor: Widget = anchor.into();
        let semantic_role = if self.submenu {
            SemanticRole::MenuItem
        } else {
            SemanticRole::Button
        };
        let mut semantics = ExplicitSemantics::new(semantic_role).state(SemanticState {
            enabled: self.enabled,
            focusable: self.enabled,
            expanded: Some(open),
            ..SemanticState::default()
        });
        if let Some(label) = anchor_label {
            semantics = semantics.label(label);
        }
        let anchor = anchor.semantics(semantics.actions(if self.enabled {
            vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
        } else {
            Vec::new()
        }));

        if !open {
            return anchor;
        }

        let theme = current_control_theme(context);
        let style = self.style.clone().unwrap_or_default();
        let items = if let Some(item_style) = self.item_style.clone() {
            self.menu_children
                .iter()
                .cloned()
                .map(|item| {
                    ControlButton::with_child(item)
                        .style(item_style.clone())
                        .into()
                })
                .collect::<Vec<Widget>>()
        } else {
            self.menu_children.clone()
        };
        let (panel, panel_height) = menu_panel(items, &style, &theme);
        let panel: Widget = SizedBox::new().height(panel_height).child(panel).into();
        // Leaf menu items close the complete retained menu chain, not merely
        // the nearest popup. A nested MenuAnchor inherits its parent's close
        // scope and composes its own controller into the same callback, so a
        // selection inside a cascading submenu deterministically closes the
        // submenu and every owning menu exactly once.
        let parent_close = context.find::<MenuCloseScope>().map(|scope| scope.0);
        let selection_controller = self.controller.clone();
        let selection_on_close = self.on_close.clone();
        let close_chain = Rc::new(move |reason| {
            let was_open = selection_controller.is_open();
            selection_controller.close();
            if was_open && let Some(callback) = selection_on_close.as_ref() {
                callback();
            }
            if let Some(parent_close) = parent_close.as_ref() {
                parent_close(reason);
            }
        })
            as Rc<dyn Fn(incular_widgets::TransientDismissReason) + 'static>;
        let panel = Widget::environment_scope(MenuCloseScope(close_chain), panel);
        // The lower-level overlay portal is retained here.  When requested,
        // add a transparent, full-bounds barrier beneath the anchor/panel so
        // an outside activation closes the menu while the visible rows remain
        // the top hit-test targets.
        let barrier_behavior = if self.consume_outside_tap {
            HitTestBehavior::Opaque
        } else {
            HitTestBehavior::Translucent
        };
        let barrier: Widget = Positioned::fill(
            GestureDetector::new(Container::new().color(Color::TRANSPARENT))
                .behavior(barrier_behavior)
                // Runtime owns outside-dismissal on pointer-down so the same
                // policy works across parent and native transient surfaces.
                // This retained barrier only preserves overlay hit consumption.
                .on_tap(|| {}),
        )
        .into();
        let controller = self.controller.clone();
        let on_close = self.on_close.clone();
        let overlay: Widget = OverlayPortal::new(anchor)
            .barrier_child(barrier)
            .overlay_child(panel)
            .role(TransientRole::Menu)
            .placement(
                TransientPlacement::default()
                    .submenu(self.submenu)
                    .alignment_offset(self.alignment_offset),
            )
            .on_dismiss(move |_| {
                controller.close();
                if let Some(callback) = on_close.as_ref() {
                    callback();
                }
            })
            .show(true)
            .into();
        let _ = (
            self.clip_behavior,
            self.cross_axis_unconstrained,
            self.use_root_overlay,
            self.animated,
        );
        overlay
    }
}

impl From<MenuAnchor> for Widget {
    fn from(value: MenuAnchor) -> Self {
        let value = Rc::new(value);
        let revision = value.controller.revision();
        Widget::stateful_layout_builder(revision, move |context, _| value.build(context))
    }
}
