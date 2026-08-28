//! Material feedback surfaces and presentation descriptors.
//!
//! This module owns the visual and semantic policy for feedback widgets.  It
//! deliberately does not own an application shell, navigator, or notification
//! queue: those services belong to the runtime/navigation layers.  The types in
//! this file are ordinary retained descriptors and can be composed into any
//! widget tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::extras::SnackBarBehavior;
use incular_config::{
    Alignment, Clip, CrossAxisAlignment, EdgeInsets, MainAxisAlignment, MainAxisSize,
};
use incular_controls::{ControlTheme, current_control_theme};
use incular_core::{Color, Offset, Size};
use incular_semantics::{Role as SemanticRole, SemanticAction, SemanticState};
use incular_text::TextStyle;
use incular_widgets::internal::ActionSurface;
use incular_widgets::{
    Align, BorderRadius, BoxDecoration, Column, Container, Focus, FocusNode, OverlayPortal,
    Padding, Positioned, Row, Semantics, SizedBox, Stack, Text, Widget,
};
use typed_builder::TypedBuilder;

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn blend_color(base: Color, overlay: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| {
        (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::rgba(
        mix(base.red, overlay.red),
        mix(base.green, overlay.green),
        mix(base.blue, overlay.blue),
        mix(base.alpha, overlay.alpha),
    )
}

fn semantic_state(enabled: bool) -> SemanticState {
    SemanticState {
        enabled,
        focusable: enabled,
        ..SemanticState::default()
    }
}

/// A Material dialog surface.
///
/// `Dialog` only describes the surface itself.  Use [`show_dialog`] and
/// [`DialogHandle::present`] when a retained overlay presentation is desired;
/// navigation and application-shell policy remain outside this type.
#[derive(Clone, TypedBuilder)]
pub struct Dialog {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = EdgeInsets::symmetric(40.0, 24.0))]
    inset_padding: EdgeInsets,
    #[builder(default = BorderRadius::circular(4.0))]
    shape: BorderRadius,
    #[builder(default = Alignment::CENTER)]
    alignment: Alignment,
    #[builder(default = Clip::None)]
    clip_behavior: Clip,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

/// A Material alert dialog with the common title/content/actions slots.
///
/// The modal mechanics are intentionally provided by [`DialogHandle`]; this
/// type only composes the Material surface.  Keeping the slots as ordinary
/// widgets makes it possible to use custom controls without introducing a
/// second route or focus implementation.
#[derive(Clone, TypedBuilder)]
pub struct AlertDialog {
    #[builder(default, setter(strip_option, into))]
    title: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    content: Option<Widget>,
    #[builder(
        default = Vec::new(),
        setter(transform = |actions: impl IntoIterator<Item = impl Into<Widget>>| {
            actions.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    actions: Vec<Widget>,
    #[builder(default)]
    scrollable: bool,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    surface_tint_color: Option<Color>,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = EdgeInsets::symmetric(40.0, 24.0))]
    inset_padding: EdgeInsets,
    #[builder(default, setter(strip_option))]
    title_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    content_padding: Option<EdgeInsets>,
    #[builder(default = EdgeInsets::symmetric(24.0, 8.0))]
    actions_padding: EdgeInsets,
    #[builder(default = BorderRadius::circular(4.0))]
    shape: BorderRadius,
    #[builder(default = Alignment::CENTER)]
    alignment: Alignment,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl Default for AlertDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            content: None,
            actions: Vec::new(),
            scrollable: false,
            background_color: None,
            surface_tint_color: None,
            elevation: 24.0,
            inset_padding: EdgeInsets::symmetric(40.0, 24.0),
            title_padding: None,
            content_padding: None,
            actions_padding: EdgeInsets::symmetric(24.0, 8.0),
            shape: BorderRadius::circular(4.0),
            alignment: Alignment::CENTER,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<Widget>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn title_text(self, title: impl Into<String>) -> Self {
        self.title(Text::new(title))
    }

    #[must_use]
    pub fn content(mut self, content: impl Into<Widget>) -> Self {
        self.content = Some(content.into());
        self
    }

    #[must_use]
    pub fn actions(mut self, actions: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.actions = actions.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn action(mut self, action: impl Into<Widget>) -> Self {
        self.actions.push(action.into());
        self
    }

    #[must_use]
    pub fn scrollable(mut self, value: bool) -> Self {
        self.scrollable = value;
        self
    }

    #[must_use]
    pub fn background_color(mut self, value: Color) -> Self {
        self.background_color = Some(value);
        self
    }

    #[must_use]
    pub fn surface_tint_color(mut self, value: Color) -> Self {
        self.surface_tint_color = Some(value);
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = finite_non_negative(value);
        self
    }

    #[must_use]
    pub fn inset_padding(mut self, value: EdgeInsets) -> Self {
        self.inset_padding = value;
        self
    }

    #[must_use]
    pub fn title_padding(mut self, value: EdgeInsets) -> Self {
        self.title_padding = Some(value);
        self
    }

    #[must_use]
    pub fn content_padding(mut self, value: EdgeInsets) -> Self {
        self.content_padding = Some(value);
        self
    }

    #[must_use]
    pub fn actions_padding(mut self, value: EdgeInsets) -> Self {
        self.actions_padding = value;
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = value;
        self
    }

    #[must_use]
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, value: impl Into<String>) -> Self {
        self.semantic_label = Some(value.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let mut children: Vec<Widget> = Vec::with_capacity(3);
        if let Some(title) = self.title.clone() {
            children.push(
                Container::with_child(title)
                    .padding(
                        self.title_padding
                            .unwrap_or_else(|| EdgeInsets::only(24.0, 24.0, 24.0, 16.0)),
                    )
                    .into(),
            );
        }
        if let Some(content) = self.content.clone() {
            let content = if self.scrollable {
                incular_widgets::SingleChildScrollView::new(content).into()
            } else {
                content
            };
            children.push(
                Container::with_child(content)
                    .padding(
                        self.content_padding
                            .unwrap_or_else(|| EdgeInsets::only(24.0, 0.0, 24.0, 20.0)),
                    )
                    .into(),
            );
        }
        if !self.actions.is_empty() {
            children.push(
                Container::with_child(
                    Row::new(self.actions.clone())
                        .spacing(8.0)
                        .main_axis_alignment(MainAxisAlignment::End)
                        .cross_axis_alignment(CrossAxisAlignment::Center),
                )
                .padding(self.actions_padding)
                .into(),
            );
        }
        let content: Widget = Column::new(children)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into();
        let mut dialog = Dialog::new(content)
            .background_color(
                self.background_color
                    .unwrap_or(theme.colors.surface_elevated),
            )
            .elevation(self.elevation)
            .inset_padding(self.inset_padding)
            .shape(self.shape)
            .alignment(self.alignment)
            .semantic_label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| "Alert dialog".to_owned()),
            );
        if let Some(tint) = self.surface_tint_color {
            dialog = dialog.surface_tint_color(tint);
        }
        dialog.build(theme)
    }
}

impl From<AlertDialog> for Widget {
    fn from(value: AlertDialog) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

impl Dialog {
    /// Creates a dialog around `child`.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            background_color: None,
            surface_tint_color: None,
            elevation: 24.0,
            inset_padding: EdgeInsets::symmetric(40.0, 24.0),
            shape: BorderRadius::circular(4.0),
            alignment: Alignment::CENTER,
            clip_behavior: Clip::None,
            semantic_label: None,
        }
    }

    /// Creates a dialog with an empty child for incremental composition.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(SizedBox::shrink())
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Alias for [`Dialog::background_color`].
    #[must_use]
    pub fn color(self, color: Color) -> Self {
        self.background_color(color)
    }

    #[must_use]
    pub fn surface_tint_color(mut self, color: Color) -> Self {
        self.surface_tint_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn inset_padding(mut self, padding: EdgeInsets) -> Self {
        self.inset_padding = padding;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.shape = BorderRadius::circular(finite_non_negative(radius));
        self
    }

    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip_behavior: Clip) -> Self {
        self.clip_behavior = clip_behavior;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    /// Materializes this dialog using the current control theme.
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let mut surface_color = self
            .background_color
            .unwrap_or(theme.colors.surface_elevated);
        if let Some(tint) = self.surface_tint_color {
            // Material surface tint is intentionally subtle at the default
            // elevation. The blend stays renderer-independent and keeps the
            // descriptor useful to non-GPU hosts.
            surface_color = blend_color(surface_color, tint, (self.elevation / 24.0) * 0.12);
        }

        let surface: Widget = Container::with_child(self.child.clone())
            .color(surface_color)
            .decoration(BoxDecoration::new().border_radius(self.shape))
            .clip_behavior(self.clip_behavior)
            .into();
        let surface = if self.elevation > 0.0 {
            Widget::drop_shadow(
                Offset::new(0.0, self.elevation * 0.16),
                (self.elevation * 0.45).max(1.0),
                Color::rgba(0, 0, 0, 90),
                surface,
            )
        } else {
            surface
        };
        let surface = Padding::new(self.inset_padding, surface);
        let mut semantics = Semantics::new(surface)
            .role(SemanticRole::Dialog)
            .state(semantic_state(true))
            .action(SemanticAction::Focus);
        if let Some(label) = self.semantic_label.clone() {
            semantics = semantics.label(label);
        }
        Align::new(self.alignment, semantics).into()
    }
}

impl From<Dialog> for Widget {
    fn from(value: Dialog) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A retained, local dialog presentation handle.
///
/// The handle is intentionally independent of a navigator.  It can be placed
/// over an arbitrary child with [`DialogHandle::present`], while a navigation
/// implementation may use the same `Dialog` descriptor in its own route.
#[derive(Clone)]
pub struct DialogHandle {
    dialog: Dialog,
    open: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    barrier_dismissible: bool,
    barrier_color: Color,
}

/// Rust-native spelling of Flutter's `DialogRoute`.
///
/// The retained handle is the route/presentation state in Incular: it owns
/// barrier policy and lifecycle while the runtime/navigation layer remains in
/// charge of scheduling and route stacks.  Keeping this alias means code that
/// names a dialog route can migrate without introducing a second route type.
pub type DialogRoute = DialogHandle;

impl DialogHandle {
    #[must_use]
    pub fn new(dialog: Dialog) -> Self {
        Self {
            dialog,
            open: Rc::new(Cell::new(false)),
            revision: Rc::new(Cell::new(0)),
            barrier_dismissible: true,
            barrier_color: Color::rgba(0, 0, 0, 96),
        }
    }

    #[must_use]
    pub fn dialog(&self) -> Dialog {
        self.dialog.clone()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn open(&self) {
        self.set_open(true);
    }

    pub fn close(&self) {
        self.set_open(false);
    }

    /// Alias for [`DialogHandle::close`].
    pub fn dismiss(&self) {
        self.close();
    }

    #[must_use]
    pub fn barrier_dismissible(mut self, dismissible: bool) -> Self {
        self.barrier_dismissible = dismissible;
        self
    }

    #[must_use]
    pub fn barrier_color(mut self, color: Color) -> Self {
        self.barrier_color = color;
        self
    }

    fn set_open(&self, open: bool) {
        if self.open.replace(open) != open {
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }

    fn build_overlay(&self) -> Widget {
        let dismiss = self.clone();
        let barrier = ActionSurface::with_child(Container::new().color(self.barrier_color))
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .focused_color(Color::TRANSPARENT)
            .enabled(self.barrier_dismissible)
            .focusable_when_disabled(false);
        let barrier = if self.barrier_dismissible {
            barrier.on_click(move || dismiss.dismiss())
        } else {
            barrier
        };
        let barrier: Widget = Positioned::fill(barrier).into();
        Stack::new([barrier, self.dialog.build(&current_control_theme())]).into()
    }

    /// Presents the dialog over `child` when this handle is open.
    #[must_use]
    pub fn present(&self, child: impl Into<Widget>) -> Widget {
        let child = child.into();
        let handle = self.clone();
        Widget::stateful_layout_builder(self.revision.clone(), move |_| {
            let base = OverlayPortal::new(child.clone())
                .overlay_child(handle.build_overlay())
                .show(handle.is_open());
            let presented: Widget = base.into();
            if handle.is_open() {
                presented.block_semantics()
            } else {
                presented
            }
        })
    }

    /// Returns a presentation rooted at an empty child.
    #[must_use]
    pub fn widget(&self) -> Widget {
        self.present(SizedBox::shrink())
    }
}

/// Creates a local retained dialog presentation descriptor.
#[must_use]
pub fn show_dialog(dialog: Dialog) -> DialogHandle {
    DialogHandle::new(dialog)
}

/// A typed result handle for callers that want a dialog result without tying
/// the feedback layer to a particular navigation/future implementation.
#[derive(Clone)]
pub struct DialogResultHandle<T> {
    handle: DialogHandle,
    result: Rc<RefCell<Option<T>>>,
}

impl<T> DialogResultHandle<T> {
    #[must_use]
    pub fn handle(&self) -> DialogHandle {
        self.handle.clone()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    pub fn open(&self) {
        self.handle.open();
    }

    pub fn close(&self) {
        self.handle.close();
    }

    pub fn complete(&self, value: T) {
        *self.result.borrow_mut() = Some(value);
        self.handle.close();
    }

    #[must_use]
    pub fn take_result(&self) -> Option<T> {
        self.result.borrow_mut().take()
    }

    #[must_use]
    pub fn present(&self, child: impl Into<Widget>) -> Widget {
        self.handle.present(child)
    }
}

/// Creates a typed local dialog result descriptor.
#[must_use]
pub fn show_dialog_result<T>(dialog: Dialog) -> DialogResultHandle<T> {
    DialogResultHandle {
        handle: DialogHandle::new(dialog),
        result: Rc::new(RefCell::new(None)),
    }
}

/// A selectable option used by [`SimpleDialog`].
#[derive(Clone, TypedBuilder)]
pub struct SimpleDialogOption {
    #[builder(setter(into))]
    child: Widget,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn()>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_pressed: Option<Rc<dyn Fn()>>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl SimpleDialogOption {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            on_pressed: None,
            enabled: true,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn text(label: impl Into<String>) -> Self {
        Self::new(Text::new(label))
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed = Some(Rc::new(callback));
        self
    }

    /// Alias matching the naming used by the Flutter API.
    #[must_use]
    pub fn on_click(self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed(callback)
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let content: Widget = Container::with_child(self.child.clone())
            .height(48.0)
            .padding(EdgeInsets::symmetric(24.0, 0.0))
            .alignment(Alignment::CENTER_LEFT)
            .into();
        let mut surface = ActionSurface::with_child(content)
            .color(Color::TRANSPARENT)
            .hover_color(theme.colors.hover_overlay)
            .pressed_color(theme.colors.pressed_overlay)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if let Some(callback) = self.on_pressed.clone() {
            if self.enabled {
                surface = surface.on_click(move || callback());
            }
        }
        let mut semantics = Semantics::new(surface)
            .role(SemanticRole::Button)
            .enabled(self.enabled)
            .action(SemanticAction::Activate);
        if let Some(label) = self.semantic_label.clone() {
            semantics = semantics.label(label);
        }
        semantics.into()
    }
}

impl From<SimpleDialogOption> for Widget {
    fn from(value: SimpleDialogOption) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A Material simple dialog made from a title and selectable options.
#[derive(Clone, TypedBuilder)]
pub struct SimpleDialog {
    #[builder(default, setter(strip_option, into))]
    title: Option<Widget>,
    #[builder(
        default = Vec::new(),
        setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
            children.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    children: Vec<Widget>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default = 24.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default = EdgeInsets::symmetric(40.0, 24.0))]
    inset_padding: EdgeInsets,
    #[builder(default = EdgeInsets::symmetric(8.0, 12.0))]
    content_padding: EdgeInsets,
    #[builder(default = EdgeInsets::symmetric(24.0, 16.0))]
    title_padding: EdgeInsets,
    #[builder(default = BorderRadius::circular(4.0))]
    shape: BorderRadius,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl Default for SimpleDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            children: Vec::new(),
            background_color: None,
            elevation: 24.0,
            inset_padding: EdgeInsets::symmetric(40.0, 24.0),
            content_padding: EdgeInsets::symmetric(8.0, 12.0),
            title_padding: EdgeInsets::symmetric(24.0, 16.0),
            shape: BorderRadius::circular(4.0),
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn with_children(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self::new().children(children)
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<Widget>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn title_text(self, title: impl Into<String>) -> Self {
        self.title(Text::new(title))
    }

    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.children.push(child.into());
        self
    }

    #[must_use]
    pub fn option(self, option: SimpleDialogOption) -> Self {
        self.child(option)
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn inset_padding(mut self, padding: EdgeInsets) -> Self {
        self.inset_padding = padding;
        self
    }

    #[must_use]
    pub fn content_padding(mut self, padding: EdgeInsets) -> Self {
        self.content_padding = padding;
        self
    }

    #[must_use]
    pub fn title_padding(mut self, padding: EdgeInsets) -> Self {
        self.title_padding = padding;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let mut items: Vec<Widget> =
            Vec::with_capacity(self.children.len() + usize::from(self.title.is_some()));
        if let Some(title) = self.title.clone() {
            items.push(
                Container::with_child(title)
                    .padding(self.title_padding)
                    .into(),
            );
        }
        items.extend(self.children.iter().cloned());
        let content: Widget = Container::with_child(
            Column::new(items)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .padding(self.content_padding)
        .into();
        Dialog::new(content)
            .background_color(
                self.background_color
                    .unwrap_or(theme.colors.surface_elevated),
            )
            .elevation(self.elevation)
            .inset_padding(self.inset_padding)
            .shape(self.shape)
            .semantic_label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| "Simple dialog".to_owned()),
            )
            .build(theme)
    }
}

impl From<SimpleDialog> for Widget {
    fn from(value: SimpleDialog) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A Material snackbar action.
#[derive(Clone, TypedBuilder)]
pub struct SnackBarAction {
    #[builder(setter(into))]
    label: String,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn()>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_pressed: Option<Rc<dyn Fn()>>,
    #[builder(default, setter(strip_option))]
    text_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    disabled_text_color: Option<Color>,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl SnackBarAction {
    #[must_use]
    pub fn new(label: impl Into<String>, on_pressed: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            on_pressed: Some(Rc::new(on_pressed)),
            text_color: None,
            disabled_text_color: None,
            enabled: true,
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    #[must_use]
    pub fn on_pressed(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_pressed = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    #[must_use]
    pub fn disabled_text_color(mut self, color: Color) -> Self {
        self.disabled_text_color = Some(color);
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn label_text(&self) -> &str {
        &self.label
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let color = if self.enabled {
            self.text_color.unwrap_or(theme.colors.accent)
        } else {
            self.disabled_text_color
                .unwrap_or(theme.colors.foreground_disabled)
        };
        let mut surface = ActionSurface::new(self.label.clone())
            .padding(EdgeInsets::symmetric(12.0, 8.0))
            .label_style(TextStyle::new().font_size(14.0).bold().color(color))
            .color(Color::TRANSPARENT)
            .hover_color(theme.colors.hover_overlay)
            .pressed_color(theme.colors.pressed_overlay)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if let Some(callback) = self.on_pressed.clone() {
            if self.enabled {
                surface = surface.on_click(move || callback());
            }
        }
        let mut semantics = Semantics::new(surface)
            .role(SemanticRole::Button)
            .label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| self.label.clone()),
            )
            .enabled(self.enabled)
            .action(SemanticAction::Activate);
        if !self.enabled {
            semantics = semantics.state(SemanticState {
                enabled: false,
                ..SemanticState::default()
            });
        }
        semantics.into()
    }
}

impl From<SnackBarAction> for Widget {
    fn from(value: SnackBarAction) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// A Material snackbar surface.
#[derive(Clone, TypedBuilder)]
pub struct SnackBar {
    #[builder(setter(into))]
    content: Widget,
    #[builder(default, setter(strip_option))]
    action: Option<SnackBarAction>,
    #[builder(default, setter(strip_option))]
    background_color: Option<Color>,
    #[builder(default = 6.0, setter(transform = |value: f32| finite_non_negative(value)))]
    elevation: f32,
    #[builder(default, setter(strip_option))]
    shape: Option<BorderRadius>,
    #[builder(default = EdgeInsets::symmetric(16.0, 14.0))]
    padding: EdgeInsets,
    #[builder(default, setter(strip_option))]
    margin: Option<EdgeInsets>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    width: Option<f32>,
    #[builder(default = SnackBarBehavior::Fixed)]
    behavior: SnackBarBehavior,
    #[builder(default = Duration::from_millis(4_000))]
    duration: Duration,
    #[builder(default, setter(strip_option, into))]
    semantic_label: Option<String>,
}

impl SnackBar {
    #[must_use]
    pub fn new(content: impl Into<Widget>) -> Self {
        Self {
            content: content.into(),
            action: None,
            background_color: None,
            elevation: 6.0,
            shape: None,
            padding: EdgeInsets::symmetric(16.0, 14.0),
            margin: None,
            width: None,
            behavior: SnackBarBehavior::Fixed,
            duration: Duration::from_millis(4_000),
            semantic_label: None,
        }
    }

    #[must_use]
    pub fn text(message: impl Into<String>) -> Self {
        Self::new(Text::new(message))
    }

    #[must_use]
    pub fn content(mut self, content: impl Into<Widget>) -> Self {
        self.content = content.into();
        self
    }

    #[must_use]
    pub fn action(mut self, action: SnackBarAction) -> Self {
        self.action = Some(action);
        self
    }

    #[must_use]
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    #[must_use]
    pub fn elevation(mut self, elevation: f32) -> Self {
        self.elevation = finite_non_negative(elevation);
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: BorderRadius) -> Self {
        self.shape = Some(shape);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.shape = Some(BorderRadius::circular(finite_non_negative(radius)));
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = Some(margin);
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(finite_non_negative(width));
        self
    }

    #[must_use]
    pub fn behavior(mut self, behavior: SnackBarBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    #[must_use]
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    #[must_use]
    pub fn action_ref(&self) -> Option<&SnackBarAction> {
        self.action.as_ref()
    }

    #[must_use]
    pub fn behavior_value(&self) -> SnackBarBehavior {
        self.behavior
    }

    #[must_use]
    pub fn duration_value(&self) -> Duration {
        self.duration
    }

    pub(crate) fn build(&self, theme: &ControlTheme) -> Widget {
        let mut children = vec![self.content.clone()];
        if let Some(action) = self.action.as_ref() {
            children.push(action.build(theme));
        }
        let content: Widget = Row::new(children)
            .spacing(8.0)
            .main_axis_size(MainAxisSize::Min)
            .main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into();
        let shape = self.shape.unwrap_or_else(|| {
            if self.behavior == SnackBarBehavior::Floating {
                BorderRadius::circular(theme.toast.radius)
            } else {
                BorderRadius::ZERO
            }
        });
        let mut surface = Container::with_child(content)
            .padding(self.padding)
            .color(
                self.background_color
                    .unwrap_or(theme.colors.surface_elevated),
            )
            .decoration(BoxDecoration::new().border_radius(shape));
        if let Some(width) = self.width {
            surface = surface.width(width);
        }
        if let Some(margin) = self.margin {
            surface = surface.margin(margin);
        }
        let visual: Widget = if self.elevation > 0.0 {
            Widget::drop_shadow(
                Offset::new(0.0, self.elevation * 0.2),
                (self.elevation * 0.5).max(1.0),
                Color::rgba(0, 0, 0, 90),
                surface.into(),
            )
        } else {
            surface.into()
        };
        let mut semantics = Semantics::new(visual)
            .role(SemanticRole::GenericContainer)
            .state(semantic_state(true));
        if let Some(label) = self.semantic_label.clone() {
            semantics = semantics.label(label);
        }
        semantics.into()
    }
}

impl From<SnackBar> for Widget {
    fn from(value: SnackBar) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&current_control_theme()))
    }
}

/// Controls whether a tooltip is currently visible.
#[derive(Clone, Default)]
pub struct TooltipController {
    visible: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    hide_at: Rc<Cell<Option<Instant>>>,
}

impl TooltipController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&self) {
        self.hide_at.set(None);
        self.set_visible(true);
    }

    pub fn hide(&self) {
        self.hide_at.set(None);
        self.set_visible(false);
    }

    /// Shows the tooltip for a bounded lifetime. The runtime can call
    /// [`Self::poll`] from its normal frame tick without allocating a timer
    /// task for every tooltip.
    pub fn show_for(&self, duration: Duration, now: Instant) {
        self.hide_at
            .set((duration > Duration::ZERO).then_some(now + duration));
        self.set_visible(true);
    }

    /// Applies a scheduled hide, returning whether visibility changed.
    pub fn poll(&self, now: Instant) -> bool {
        if self.hide_at.get().is_some_and(|deadline| now >= deadline) {
            self.hide_at.set(None);
            self.set_visible(false);
            true
        } else {
            false
        }
    }

    pub fn toggle(&self) {
        self.set_visible(!self.is_visible());
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible.get()
    }

    fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }
}

/// Input policy for a [`Tooltip`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipTriggerMode {
    #[default]
    Hover,
    Focus,
    Tap,
    Manual,
}

/// A Material tooltip attached to a child widget.
#[derive(Clone, TypedBuilder)]
pub struct Tooltip {
    #[builder(setter(into))]
    message: String,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = TooltipController::new())]
    controller: TooltipController,
    #[builder(default = FocusNode::new())]
    focus_node: FocusNode,
    #[builder(default = TooltipTriggerMode::Hover)]
    trigger_mode: TooltipTriggerMode,
    #[builder(default = true)]
    enabled: bool,
    #[builder(default = EdgeInsets::symmetric(16.0, 8.0))]
    padding: EdgeInsets,
    #[builder(default = EdgeInsets::all(0.0))]
    margin: EdgeInsets,
    #[builder(default, setter(strip_option))]
    text_style: Option<TextStyle>,
    #[builder(default, setter(strip_option))]
    decoration_color: Option<Color>,
    #[builder(default = true)]
    prefer_below: bool,
    #[builder(default = 14.0, setter(transform = |value: f32| finite_non_negative(value)))]
    vertical_offset: f32,
    #[builder(default = Duration::from_millis(500))]
    wait_duration: Duration,
    #[builder(default = Duration::from_millis(0))]
    show_duration: Duration,
    #[builder(default = Duration::from_millis(100))]
    exit_duration: Duration,
    #[builder(default)]
    exclude_from_semantics: bool,
}

impl Tooltip {
    #[must_use]
    pub fn new(message: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            message: message.into(),
            child: child.into(),
            controller: TooltipController::new(),
            focus_node: FocusNode::new(),
            trigger_mode: TooltipTriggerMode::Hover,
            enabled: true,
            padding: EdgeInsets::symmetric(16.0, 8.0),
            margin: EdgeInsets::all(0.0),
            text_style: None,
            decoration_color: None,
            prefer_below: true,
            vertical_offset: 14.0,
            wait_duration: Duration::from_millis(500),
            show_duration: Duration::from_millis(0),
            exit_duration: Duration::from_millis(100),
            exclude_from_semantics: false,
        }
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn controller(mut self, controller: TooltipController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn focus_node(mut self, node: FocusNode) -> Self {
        self.focus_node = node;
        self
    }

    #[must_use]
    pub fn controller_ref(&self) -> TooltipController {
        self.controller.clone()
    }

    #[must_use]
    pub fn trigger_mode(mut self, mode: TooltipTriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = margin;
        self
    }

    #[must_use]
    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.text_style = Some(style);
        self
    }

    #[must_use]
    pub fn decoration_color(mut self, color: Color) -> Self {
        self.decoration_color = Some(color);
        self
    }

    #[must_use]
    pub fn prefer_below(mut self, prefer_below: bool) -> Self {
        self.prefer_below = prefer_below;
        self
    }

    #[must_use]
    pub fn vertical_offset(mut self, offset: f32) -> Self {
        self.vertical_offset = finite_non_negative(offset);
        self
    }

    #[must_use]
    pub fn wait_duration(mut self, duration: Duration) -> Self {
        self.wait_duration = duration;
        self
    }

    #[must_use]
    pub fn show_duration(mut self, duration: Duration) -> Self {
        self.show_duration = duration;
        self
    }

    #[must_use]
    pub fn exit_duration(mut self, duration: Duration) -> Self {
        self.exit_duration = duration;
        self
    }

    #[must_use]
    pub fn exclude_from_semantics(mut self, exclude: bool) -> Self {
        self.exclude_from_semantics = exclude;
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn wait_duration_value(&self) -> Duration {
        self.wait_duration
    }

    #[must_use]
    pub fn show_duration_value(&self) -> Duration {
        self.show_duration
    }

    #[must_use]
    pub fn exit_duration_value(&self) -> Duration {
        self.exit_duration
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let controller = self.controller.clone();
        let mut anchor = ActionSurface::with_child(self.child.clone())
            .color(Color::TRANSPARENT)
            .hover_color(Color::TRANSPARENT)
            .pressed_color(Color::TRANSPARENT)
            .focused_color(Color::TRANSPARENT)
            .disabled_color(Color::TRANSPARENT)
            .enabled(self.enabled);
        if self.enabled {
            match self.trigger_mode {
                TooltipTriggerMode::Hover => {
                    let show = controller.clone();
                    let hide = controller.clone();
                    anchor = anchor
                        .on_hover(move || show.show())
                        .on_exit(move || hide.hide());
                }
                TooltipTriggerMode::Tap => {
                    let toggle = controller.clone();
                    anchor = anchor.on_click(move || toggle.toggle());
                }
                TooltipTriggerMode::Focus => {
                    if self.focus_node.has_focus() {
                        controller.show();
                    } else {
                        controller.hide();
                    }
                }
                TooltipTriggerMode::Manual => {}
            }
        }
        let anchor: Widget = if self.exclude_from_semantics {
            anchor.into()
        } else {
            Semantics::new(anchor).tooltip(self.message.clone()).into()
        };

        let style = self
            .text_style
            .clone()
            .unwrap_or_else(|| theme.typography.small.clone())
            .color(theme.colors.accent_foreground);
        let popup: Widget = Container::with_child(Text::new(self.message.clone()).style(style))
            .padding(self.padding)
            .margin(self.margin)
            .decoration(
                BoxDecoration::new()
                    .color(self.decoration_color.unwrap_or(theme.colors.surface_active))
                    .border_radius(BorderRadius::circular(theme.tooltip.radius)),
            )
            .into();
        let popup =
            Widget::drop_shadow(Offset::new(0.0, 2.0), 3.0, Color::rgba(0, 0, 0, 70), popup);
        let popup = if self.prefer_below {
            Align::new(
                Alignment::TOP_CENTER,
                Padding::new(EdgeInsets::only(0.0, self.vertical_offset, 0.0, 0.0), popup),
            )
        } else {
            Align::new(
                Alignment::BOTTOM_CENTER,
                Padding::new(EdgeInsets::only(0.0, 0.0, 0.0, self.vertical_offset), popup),
            )
        };
        let anchor = if self.trigger_mode == TooltipTriggerMode::Focus {
            Focus::new(anchor).node(self.focus_node.clone()).into()
        } else {
            anchor
        };
        OverlayPortal::new(anchor)
            .overlay_child(popup)
            .show(self.enabled && self.controller.is_visible())
            .into()
    }
}

impl From<Tooltip> for Widget {
    fn from(value: Tooltip) -> Self {
        let value = Rc::new(value);
        Widget::stateful_layout_builder(value.controller.revision.clone(), move |_| {
            value.build(&current_control_theme())
        })
    }
}

/// Stroke cap options used by [`ProgressIndicatorThemeData`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProgressIndicatorStrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Theme data shared by Material linear and circular progress indicators.
///
/// The existing `LinearProgressIndicator` and `CircularProgressIndicator`
/// descriptors consume their own defaults; this record provides the complete
/// retained configuration surface so a parent/theme integration can resolve
/// those defaults without introducing renderer-specific state.
#[derive(Clone, Copy, Debug, Default, PartialEq, TypedBuilder)]
pub struct ProgressIndicatorThemeData {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub linear_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub circular_track_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub refresh_background_color: Option<Color>,
    #[builder(default, setter(strip_option))]
    pub stop_indicator_color: Option<Color>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub stop_indicator_radius: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub stroke_width: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub stroke_cap: Option<ProgressIndicatorStrokeCap>,
    #[builder(default)]
    pub stroke_align: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub track_gap: Option<f32>,
    #[builder(default, setter(transform = |value: f32| Some(finite_non_negative(value))))]
    pub linear_track_height: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub border_radius: Option<BorderRadius>,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub constraints: Option<Size>,
}

impl ProgressIndicatorThemeData {
    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn linear_track_color(mut self, value: Color) -> Self {
        self.linear_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn circular_track_color(mut self, value: Color) -> Self {
        self.circular_track_color = Some(value);
        self
    }

    #[must_use]
    pub fn refresh_background_color(mut self, value: Color) -> Self {
        self.refresh_background_color = Some(value);
        self
    }

    #[must_use]
    pub fn stop_indicator_color(mut self, value: Color) -> Self {
        self.stop_indicator_color = Some(value);
        self
    }

    #[must_use]
    pub fn stop_indicator_radius(mut self, value: f32) -> Self {
        self.stop_indicator_radius = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn stroke_width(mut self, value: f32) -> Self {
        self.stroke_width = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn stroke_cap(mut self, value: ProgressIndicatorStrokeCap) -> Self {
        self.stroke_cap = Some(value);
        self
    }

    #[must_use]
    pub fn stroke_align(mut self, value: f32) -> Self {
        self.stroke_align = Some(value);
        self
    }

    #[must_use]
    pub fn track_gap(mut self, value: f32) -> Self {
        self.track_gap = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn linear_track_height(mut self, value: f32) -> Self {
        self.linear_track_height = Some(finite_non_negative(value));
        self
    }

    #[must_use]
    pub fn border_radius(mut self, value: BorderRadius) -> Self {
        self.border_radius = Some(value);
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn constraints(mut self, value: Size) -> Self {
        self.constraints = Some(value);
        self
    }

    /// Fills unset fields from `fallback`, matching Flutter theme-data merge
    /// semantics while retaining explicit values from `self`.
    #[must_use]
    pub fn merge(self, fallback: Self) -> Self {
        Self {
            color: self.color.or(fallback.color),
            linear_track_color: self.linear_track_color.or(fallback.linear_track_color),
            circular_track_color: self.circular_track_color.or(fallback.circular_track_color),
            refresh_background_color: self
                .refresh_background_color
                .or(fallback.refresh_background_color),
            stop_indicator_color: self.stop_indicator_color.or(fallback.stop_indicator_color),
            stop_indicator_radius: self
                .stop_indicator_radius
                .or(fallback.stop_indicator_radius),
            stroke_width: self.stroke_width.or(fallback.stroke_width),
            stroke_cap: self.stroke_cap.or(fallback.stroke_cap),
            stroke_align: self.stroke_align.or(fallback.stroke_align),
            track_gap: self.track_gap.or(fallback.track_gap),
            linear_track_height: self.linear_track_height.or(fallback.linear_track_height),
            border_radius: self.border_radius.or(fallback.border_radius),
            padding: self.padding.or(fallback.padding),
            constraints: self.constraints.or(fallback.constraints),
        }
    }
}

/// Places progress-indicator theme data in the retained build environment.
#[derive(Clone, TypedBuilder)]
pub struct ProgressIndicatorTheme {
    #[builder(setter(into))]
    data: ProgressIndicatorThemeData,
    #[builder(setter(into))]
    child: Widget,
}

impl ProgressIndicatorTheme {
    #[must_use]
    pub fn new(data: ProgressIndicatorThemeData, child: impl Into<Widget>) -> Self {
        Self {
            data,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn data(mut self, data: ProgressIndicatorThemeData) -> Self {
        self.data = data;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn data_value(&self) -> ProgressIndicatorThemeData {
        self.data
    }
}

impl From<ProgressIndicatorTheme> for Widget {
    fn from(value: ProgressIndicatorTheme) -> Self {
        Widget::environment_scope(value.data, value.child)
    }
}

/// Reads the nearest retained [`ProgressIndicatorThemeData`].
#[must_use]
pub fn current_progress_indicator_theme() -> Option<ProgressIndicatorThemeData> {
    incular_widgets::internal::current_build_environment::<ProgressIndicatorThemeData>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_handle_tracks_open_state_and_result() {
        let handle = show_dialog(Dialog::new(Text::new("hello")));
        assert!(!handle.is_open());
        handle.open();
        assert!(handle.is_open());
        handle.close();
        assert!(!handle.is_open());

        let result = show_dialog_result::<u32>(Dialog::empty());
        result.open();
        result.complete(42);
        assert!(!result.is_open());
        assert_eq!(result.take_result(), Some(42));
        assert_eq!(result.take_result(), None);
    }

    #[test]
    fn progress_indicator_theme_merges_only_unset_fields() {
        let base = ProgressIndicatorThemeData::default()
            .color(Color::WHITE)
            .stroke_width(3.0);
        let fallback = ProgressIndicatorThemeData::default()
            .color(Color::BLACK)
            .linear_track_height(4.0);
        let merged = base.merge(fallback);
        assert_eq!(merged.color, Some(Color::WHITE));
        assert_eq!(merged.stroke_width, Some(3.0));
        assert_eq!(merged.linear_track_height, Some(4.0));
    }

    #[test]
    fn feedback_descriptors_convert_to_widgets() {
        let snackbar: Widget = SnackBar::text("saved")
            .action(SnackBarAction::new("Undo", || {}))
            .into();
        let simple: Widget = SimpleDialog::new()
            .title_text("Choose")
            .option(SimpleDialogOption::text("A").on_pressed(|| {}))
            .into();
        let tooltip: Widget = Tooltip::new("help", Text::new("?"))
            .trigger_mode(TooltipTriggerMode::Manual)
            .into();
        let _ = (snackbar, simple, tooltip);
    }
}
