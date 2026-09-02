//! Integration coverage migrated from the control source modules.

mod alert_dialog_tests {
    use incular_controls::alert_dialog::Root;
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_preserves_alert_dialog_defaults_and_accepts_widgets() {
        let default = Root::default();
        let built = Root::builder().build();

        assert_eq!(default.is_open(), built.is_open());
        assert!(!default.is_open());

        let root = Root::builder()
            .child(Text::new("Alert"))
            .open(true)
            .on_open_change(|_| {})
            .build();
        assert!(root.is_open());
        let _: Widget = root.into();
    }
}

mod autocomplete_tests {
    use incular_config::Constraints;
    use incular_controls::{ControlTheme, autocomplete::Root};
    use incular_core::Size;
    use incular_widgets::{Text, internal::WidgetTree};

    #[test]
    fn builder_preserves_autocomplete_defaults_and_accepts_widgets() {
        let default = Root::default();
        let built = Root::builder().build();

        let mut tree = WidgetTree::new();
        let root = Root::builder()
            .query("ap")
            .child(Text::new("Search"))
            .on_query_change(|_| {})
            .build();
        tree.mount(root.build(&ControlTheme::light()))
            .expect("mount autocomplete");
        tree.layout(Constraints::loose(Size::new(240.0, 80.0)))
            .expect("layout");
        tree.update_semantics();
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.value.as_deref() == Some("ap"))
        );

        let _: incular_widgets::Widget = default.into();
        let _: incular_widgets::Widget = built.into();
    }
}

mod avatar_tests {
    use incular_config::Constraints;
    use incular_controls::{ControlTheme, avatar::Root};
    use incular_core::Size;
    use incular_widgets::{Text, internal::WidgetTree};

    #[test]
    fn builder_defaults_match_new() {
        let new = Root::new();
        let built = Root::builder().build();
        let theme = ControlTheme::light();

        let mut tree = WidgetTree::new();
        tree.mount(new.build(&theme)).expect("mount avatar");
        tree.mount(built.build(&theme))
            .expect("mount avatar builder");
        tree.layout(Constraints::loose(Size::new(160.0, 80.0)))
            .expect("layout");
    }

    #[test]
    fn builder_accepts_widget_parts_and_preserves_size_normalization() {
        let avatar = Root::builder()
            .image(Text::new("image"))
            .fallback(Text::new("fallback"))
            .label("Ada Lovelace")
            .size(0.)
            .child(Text::new("custom"))
            .build();

        let mut tree = WidgetTree::new();
        tree.mount(avatar.build(&ControlTheme::light()))
            .expect("mount avatar");
        tree.layout(Constraints::loose(Size::new(160.0, 80.0)))
            .expect("layout");
        tree.update_semantics();
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("Ada Lovelace"))
        );

        let _: incular_widgets::Widget = Root::with_child(Text::new("child")).into();
    }
}

mod button_tests {
    use incular_config::Constraints;
    use incular_controls::{
        Button, ButtonStyle, ControlTheme, GhostButton, IconButton, PrimaryButton,
    };
    use incular_core::Size;
    use incular_rendering::PaintCommand;
    use incular_widgets::{Text, Widget, internal::WidgetTree};

    #[test]
    fn button_builder_defaults_and_accepts_generic_children() {
        let default = Button::default();
        let built = Button::builder().build();
        let _: Widget = default.into();
        let _: Widget = built.into();

        let button = Button::builder()
            .child(Text::new("Save"))
            .enabled(false)
            .on_click(|| ())
            .build();
        let _: Widget = button.into();
    }

    #[test]
    fn specialized_button_builders_keep_variant_presets() {
        let primary = PrimaryButton::builder()
            .child(Text::new("Save"))
            .on_click(|| ())
            .build();
        let ghost = GhostButton::builder().label("Cancel").build();
        let icon = IconButton::builder().build();

        let _: Widget = primary.into();
        let _: Widget = ghost.into();
        let _: Widget = icon.into();
        let _: Widget = PrimaryButton::builder().label("Save").build().into();
        let _: Widget = GhostButton::builder().label("Cancel").build().into();
        let _: Widget = IconButton::builder().label("Close").build().into();
    }

    #[test]
    fn building_a_button_does_not_invoke_its_click_callback() {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let observed = calls.clone();
        let button = Button::new("Save").on_click(move || {
            observed.set(observed.get() + 1);
        });

        let _ = button.build(&ControlTheme::default());

        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn button_defaults_shrink_wrap_and_fixed_size_is_honored() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Button::new("OK").into()).unwrap();
        tree.layout(Constraints::loose(Size::new(400.0, 300.0)))
            .expect("layout");
        let size = tree.render_size(tree.render_id(root).unwrap()).unwrap();
        assert!(size.width > 0.0 && size.width < 400.0);
        assert_eq!(size.height, 32.0);

        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                Button::new("OK")
                    .style(ButtonStyle::new().fixed_size(Size::new(180.0, 48.0)))
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::loose(Size::new(400.0, 300.0)))
            .expect("layout");
        let size = tree.render_size(tree.render_id(root).unwrap()).unwrap();
        assert_eq!(size, Size::new(180.0, 48.0));
    }

    #[test]
    fn button_semantics_preserve_the_public_label() {
        let mut tree = WidgetTree::new();
        tree.mount(Button::new("Increment").on_click(|| ()).into())
            .unwrap();
        tree.layout(Constraints::loose(Size::new(400.0, 300.0)))
            .expect("layout");
        tree.update_semantics();

        let semantic = tree
            .semantics()
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("Increment"))
            .map(|(_, node)| node)
            .expect("button semantics");
        assert_eq!(semantic.label.as_deref(), Some("Increment"));
    }

    #[test]
    fn primary_button_has_no_decorative_default_border() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(PrimaryButton::new("Save").into()).unwrap();
        tree.layout(Constraints::loose(Size::new(240.0, 80.0)))
            .expect("layout");
        let _ = tree.render_id(root).unwrap();
        let list = tree.paint();

        assert!(
            !list
                .commands()
                .iter()
                .any(|command| matches!(command, PaintCommand::Border { .. }))
        );
    }
}

mod checkbox_tests {
    use incular_controls::checkbox::{CheckedState, Group, Indicator, Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn anatomy_builders_keep_checkbox_defaults_and_generic_children() {
        let root = Root::builder().child(Text::new("Accept")).build();
        assert_eq!(root.state_value(), CheckedState::Unchecked);
        assert!(!root.is_required());
        let _: Widget = root.into();

        let indicator = Indicator::builder().child(Text::new("check")).build();
        assert_eq!(
            Widget::from(indicator).text_if_any().as_deref(),
            Some("check")
        );

        let group = Group::builder().values(["one", "two"]).build();
        assert_eq!(group.selected(), vec!["one", "two"]);
        let _: Widget = group.into();
    }

    #[test]
    fn anatomy_compatibility_constructors_use_builder_defaults() {
        assert_eq!(Root::new().state_value(), Root::default().state_value());
        let _: Widget = Indicator::new().into();
        assert!(Group::new().selected().is_empty());
    }
}

mod collapsible_tests {
    use incular_controls::collapsible::{Accordion, Panel, Root, Trigger};
    use incular_widgets::{Text, Widget};

    #[test]
    fn root_builder_preserves_defaults_and_accepts_generic_children() {
        let default = Root::default();
        let built = Root::builder().child(Text::new("Content")).build();

        assert!(!default.is_open());
        assert!(!built.is_open());
        let _: Widget = built.into();

        let root = Root::builder().open(true).build();
        let root = root.disabled(true).on_open_change(|_| {});
        assert!(root.is_open());
        let _: Widget = root.into();
    }

    #[test]
    fn required_parts_keep_required_children_and_explicit_accordion_defaults() {
        let trigger = Trigger::builder().child(Text::new("Trigger")).build();
        let panel = Panel::builder().child(Text::new("Panel")).build();
        let accordion = Accordion::builder().child(Text::new("Items")).build();

        assert_eq!(
            Widget::from(trigger).text_if_any().as_deref(),
            Some("Trigger")
        );
        assert_eq!(Widget::from(panel).text_if_any().as_deref(), Some("Panel"));
        let _: Widget = accordion.into();

        let _: Widget = Trigger::new(Text::new("Trigger")).into();
        let _: Widget = Panel::new(Text::new("Panel")).into();
        let _: Widget = Accordion::new(Text::new("Items")).into();
    }
}

mod combobox_tests {
    use incular_controls::combobox::Root;
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_uses_combobox_defaults() {
        let root = Root::builder().build();
        let _: Widget = root.into();
    }

    #[test]
    fn builder_accepts_widget_child_and_callback() {
        let root = Root::builder()
            .child(Text::new("Search"))
            .query("ap")
            .on_query_change(|_| {})
            .build();
        let _: Widget = root.into();
    }
}

mod containers_tests {
    use incular_config::Constraints;
    use incular_controls::{Card, CardStyle, ControlTheme, Divider, DividerStyle};
    use incular_core::Size;
    use incular_widgets::{Text, Widget, internal::WidgetTree};

    #[test]
    fn card_builder_accepts_widgets_and_preserves_child_compatibility() {
        let card = Card::builder().child(Text::new("Card")).build();
        let _: Widget = card.into();

        let nested = Card::builder()
            .child(Card::new(Text::new("Nested")))
            .build();
        let _: Widget = nested.into();

        let _: Card = Card::new(Text::new("New"));
        let _: Card = Card::with_child(Text::new("With child"));
        let _: Widget = Card::default().content(Text::new("Content")).into();
    }

    #[test]
    fn card_defaults_and_style_overrides_are_stable() {
        let style = CardStyle {
            padding: Some(incular_config::EdgeInsets::all(8.0)),
            ..CardStyle::default()
        };
        let card = Card::builder()
            .child(Text::new("Styled"))
            .style(style)
            .build();

        let mut tree = WidgetTree::new();
        let root = tree.mount(card.into()).unwrap();
        tree.layout(Constraints::loose(Size::new(240.0, 120.0)))
            .expect("layout");
        assert!(
            tree.render_size(tree.render_id(root).unwrap())
                .unwrap()
                .width
                > 0.0
        );
        let _: Widget = Card::default().build(&ControlTheme::light());
    }

    #[test]
    fn divider_builder_matches_new_and_default() {
        let new = Divider::new();
        let default = Divider::default();
        let built = Divider::builder().build();
        let theme = ControlTheme::light();

        let _: Widget = new.build(&theme);
        let _: Widget = default.build(&theme);
        let _: Widget = built.build(&theme);
        let _: Widget = built.into();
        let _ = DividerStyle::default();
    }
}

mod dialog_tests {
    use incular_controls::dialog::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn facade_aliases_preserve_dialog_composition_and_conversion() {
        let _: Widget = Root::new().open(true).child(Text::new("dialog")).into();
        let _: Widget = Backdrop::new(Text::new("backdrop")).open(true).into();
        let _: Widget = Viewport::new(Text::new("viewport")).into();
        let _: Widget = Trigger::new(Text::new("trigger")).into();
        let _: Widget = Close::new(Text::new("close")).into();
        let _: Widget = AlertRoot::new().open(true).child(Text::new("alert")).into();
    }
}

mod drawer_tests {
    use incular_controls::{drawer::Root, overlay::Side};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_uses_explicit_defaults_and_generic_children() {
        let default = Root::builder().build();
        let _: Widget = default.into();

        let drawer = Root::builder()
            .open(true)
            .side(Side::Left)
            .width(0.)
            .child(Text::new("page"))
            .panel(Text::new("drawer"))
            .modal(false)
            .build()
            .on_open_change(|_| {});
        let _: Widget = drawer.into();
        let _: Widget = Root::new().child(Text::new("legacy")).into();
    }
}

mod field_tests {
    use incular_controls::field::{Control, Description, Error, Label, Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn root_builder_matches_semantic_defaults_and_composes_widgets() {
        let default = Root::default();
        assert_eq!(Root::new().is_disabled(), default.is_disabled());
        assert_eq!(Root::new().is_invalid(), default.is_invalid());

        let root = Root::builder()
            .child(Text::new("control"))
            .label("Name")
            .description("Your display name")
            .error("Required")
            .disabled(true)
            .build();

        assert!(root.is_disabled());
        assert!(root.is_invalid());
        let _: Widget = root.into();
    }

    #[test]
    fn explicit_invalid_override_remains_stronger_than_error_inference() {
        assert!(Root::new().error("invalid").is_invalid());
        assert!(!Root::new().error("invalid").invalid(false).is_invalid());
        assert!(
            !Root::builder()
                .error("invalid")
                .invalid(false)
                .build()
                .is_invalid()
        );
    }

    #[test]
    fn required_child_descriptors_accept_arbitrary_widgets() {
        let label = Label::builder().child(Text::new("Label")).build();
        let description = Description::builder()
            .child(Text::new("Description"))
            .build();
        let control = Control::builder().child(Text::new("Control")).build();
        let error = Error::builder().child(Text::new("Error")).build();

        assert_eq!(Widget::from(label).text_if_any().as_deref(), Some("Label"));
        assert_eq!(
            Widget::from(description).text_if_any().as_deref(),
            Some("Description")
        );
        assert_eq!(
            Widget::from(control).text_if_any().as_deref(),
            Some("Control")
        );
        assert_eq!(Widget::from(error).text_if_any().as_deref(), Some("Error"));
    }

    #[test]
    fn legacy_constructors_and_widget_conversions_remain_available() {
        let _: Widget = Label::new("Label").into();
        let _: Widget = Label::child(Text::new("Label")).into();
        let _: Widget = Description::new("Description").into();
        let _: Widget = Control::new(Text::new("Control")).into();
        let _: Widget = Error::new("Error").into();
    }
}

mod fieldset_tests {
    use incular_controls::fieldset::{Legend, Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builders_preserve_fieldset_defaults_and_accept_generic_children() {
        let default = Root::default();
        let built = Root::builder().child(Text::new("Fields")).build();
        let _: Widget = default.into();
        let _: Widget = built.into();

        let legend = Legend::builder().child(Text::new("Name")).build();
        assert_eq!(Widget::from(legend).text_if_any().as_deref(), Some("Name"));
        let _: Widget = Root::new().disabled(true).into();
        let _: Widget = Legend::new("Name").into();
    }
}

mod form_tests {
    use incular_controls::form::Form;
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_preserves_form_defaults_and_accepts_widgets_and_callbacks() {
        let default = Form::default();
        let built = Form::builder().build();
        let _: Widget = default.into();
        let _: Widget = built.into();

        let form = Form::builder()
            .child(Text::new("Fields"))
            .disabled(true)
            .on_submit(|| {})
            .build();
        let _: Widget = form.into();
    }
}

mod menu_tests {
    use incular_controls::menu::{ContextMenu, Group, Item, Menubar, Root, Separator};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builders_preserve_defaults_and_accept_arbitrary_widgets() {
        let default = Root::default();
        let built = Root::builder().build();
        let _: Widget = default.into();
        let _: Widget = built.into();

        let root = Root::builder().child(Text::new("menu")).modal(true).build();
        assert_eq!(Widget::from(root).text_if_any().as_deref(), Some("menu"));

        let item = Item::builder()
            .child(Text::new("item"))
            .disabled(true)
            .build();
        assert_eq!(Widget::from(item).text_if_any().as_deref(), Some("item"));

        let separator = Separator::builder().child(Text::new("separator")).build();
        assert_eq!(
            Widget::from(separator).text_if_any().as_deref(),
            Some("separator")
        );

        let group = Group::builder().child(Text::new("group")).build();
        assert_eq!(Widget::from(group).text_if_any().as_deref(), Some("group"));

        let _: Widget = ContextMenu::builder()
            .child(Text::new("context"))
            .build()
            .into();
        let _: Widget = Menubar::builder()
            .child(Text::new("menubar"))
            .build()
            .into();
    }
}

mod meter_tests {
    use incular_controls::{ControlTheme, meter::Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_defaults_match_new() {
        let new = Root::new();
        let built = Root::builder().build();

        assert_eq!(new.normalized_value(), built.normalized_value());
        assert_eq!(new.normalized_value(), 0.);
        let theme = ControlTheme::light();
        let _: Widget = new.build(&theme);
        let _: Widget = built.build(&theme);
    }

    #[test]
    fn builder_accepts_child_and_preserves_numeric_setter_behavior() {
        let meter = Root::builder()
            .value(0.5)
            .min(0.)
            .max(2.)
            .low(0.25)
            .high(1.5)
            .optimum(1.)
            .width(-10.)
            .height(-4.)
            .label("Load")
            .child(Text::new("meter"))
            .build();

        assert_eq!(meter.normalized_value(), 0.25);
        let _: Widget = meter.build(&ControlTheme::light());
        let _: Widget = Root::with_child(Text::new("child")).into();
    }
}

mod number_field_tests {
    use incular_controls::number_field::{Decrement, Group, Increment, Input, Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builders_use_explicit_defaults_and_widget_children() {
        let default = Root::builder().build();
        let _: Widget = default.into();

        let root = Root::builder()
            .value(4.)
            .min(1.)
            .max(8.)
            .step(-2.)
            .child(Text::new("number"))
            .build()
            .on_value_change(|_| {});
        let _: Widget = root.into();

        let _: Widget = Group::builder().child(Text::new("group")).build().into();
        let _: Widget = Input::builder().child(Text::new("input")).build().into();
        let _: Widget = Increment::builder()
            .child(Text::new("increment"))
            .build()
            .into();
        let _: Widget = Decrement::builder()
            .child(Text::new("decrement"))
            .build()
            .into();
    }
}

mod otp_field_tests {
    use incular_controls::otp_field::Root;
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_uses_explicit_defaults_and_normalizes_length() {
        let default = Root::builder().build();
        assert_eq!(default.length(), 6);
        assert!(!default.is_masked());

        let root = Root::builder()
            .length(0)
            .masked(true)
            .child(Text::new("otp"))
            .build();
        assert_eq!(root.length(), 1);
        assert!(root.is_masked());
        let _: Widget = root.into();

        let _: Widget = Root::new(6).child(Text::new("legacy")).into();
    }
}

mod popover_tests {
    use incular_controls::popover::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn facade_parts_preserve_generic_content_and_widget_lowering() {
        let _: Widget = Root::new().open(true).child(Text::new("content")).into();
        let _: Widget = Trigger::new(Text::new("trigger")).into();
        let _: Widget = Portal::new(Text::new("anchor"))
            .overlay(Popup::new(Text::new("overlay")))
            .open(true)
            .into();
        let _: Widget = Positioner::new(Text::new("positioned")).into();
        let _: Widget = Arrow::new().child(Text::new("arrow")).into();
        let _: Widget = Title::new("title").into();
        let _: Widget = Description::new("description").into();
        let _: Widget = Close::new(Text::new("close")).into();
    }
}

mod popup_tests {
    use incular_config::Constraints;
    use incular_controls::{
        overlay::{Align, Side},
        popup::*,
    };
    use incular_core::{Offset, Size};
    use incular_widgets::{SizedBox, Text, TransientSide, Widget, internal::WidgetTree};

    #[test]
    fn root_builder_uses_explicit_defaults_and_generic_child() {
        let default = Root::builder().build();
        let _: Widget = default.into();

        let root = Root::builder().open(true).child(Text::new("popup")).build();
        let _: Widget = root.into();
    }

    #[test]
    fn compound_parts_expose_typed_builders_and_widget_lowering() {
        let portal = Portal::builder()
            .child(Text::new("anchor"))
            .overlay(Popup::new(Text::new("overlay")))
            .open(true)
            .build();
        let _: Widget = portal.into();

        let positioner = Positioner::builder()
            .child(Text::new("positioned"))
            .side(Side::Top)
            .align(Align::End)
            .side_offset(8.)
            .anchor(Offset::new(12., 16.))
            .build();
        let _: Widget = positioner.into();

        let _: Widget = Trigger::builder()
            .child(Text::new("trigger"))
            .build()
            .into();
        let _: Widget = Popup::builder().child(Text::new("popup")).build().into();
        let _: Widget = Arrow::builder().child(Text::new("arrow")).build().into();
        let _: Widget = Title::builder().child(Text::new("title")).build().into();
        let _: Widget = Description::builder()
            .child(Text::new("description"))
            .build()
            .into();
        let _: Widget = Close::builder().child(Text::new("close")).build().into();
        let _: Widget = Title::with_child(Text::new("legacy")).into();
    }

    #[test]
    fn positioner_metadata_delegates_to_the_retained_transient_policy() {
        let popup = Positioner::builder()
            .child(SizedBox::new().width(80.0).height(40.0))
            .side(Side::Top)
            .align(Align::Start)
            .side_offset(6.0)
            .anchor(Offset::new(120.0, 80.0))
            .build();
        let root: Widget = Portal::new(SizedBox::new().width(20.0).height(20.0))
            .overlay(popup)
            .open(true)
            .into();
        let mut tree = WidgetTree::new();
        tree.mount(root).expect("mount positioned popup");
        tree.layout(Constraints::tight(Size::new(300.0, 200.0)))
            .expect("layout positioned popup");

        let snapshot = tree
            .transient_surfaces()
            .into_iter()
            .next()
            .expect("positioned transient surface");
        assert_eq!(snapshot.anchor_rect.origin, Offset::new(120.0, 80.0));
        assert_eq!(snapshot.anchor_rect.size, Size::ZERO);
        assert_eq!(snapshot.desired_size, Size::new(80.0, 40.0));
        assert_eq!(snapshot.placement_result.side, TransientSide::Top);
        assert_eq!(snapshot.content_rect.origin, Offset::new(120.0, 34.0));
    }
}

mod progress_tests {
    use incular_controls::{ControlTheme, progress::Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_defaults_match_new() {
        let new = Root::new();
        let built = Root::builder().build();

        assert_eq!(new.normalized_value(), Some(0.));
        assert_eq!(new.normalized_value(), built.normalized_value());
        assert!(!new.is_indeterminate());
        let _: Widget = new.build(&ControlTheme::light());
        let _: Widget = built.build(&ControlTheme::light());
    }

    #[test]
    fn builder_accepts_widget_child_and_preserves_numeric_setter_behavior() {
        let progress = Root::builder()
            .value(0.5)
            .min(0.)
            .max(2.)
            .width(-10.)
            .height(-4.)
            .label("Loading")
            .child(Text::new("indicator"))
            .build();

        assert_eq!(progress.normalized_value(), Some(0.25));
        assert!(!progress.is_indeterminate());
        let _: Widget = progress.build(&ControlTheme::light());
        let _: Widget = Root::with_child(Text::new("child")).into();
    }
}

mod radio_tests {
    use incular_controls::radio::{Group, Indicator, Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn anatomy_builders_keep_radio_defaults_and_generic_children() {
        let group = Group::<u8>::builder()
            .selected(2)
            .child(Text::new("options"))
            .build();
        let _: Widget = group.into();

        let root = Root::<u8>::builder()
            .value(2)
            .selected(2)
            .child(Text::new("choice"))
            .on_change(|_| {})
            .build();
        let _: Widget = root.into();

        let indicator = Indicator::builder().child(Text::new("dot")).build();
        assert_eq!(
            Widget::from(indicator).text_if_any().as_deref(),
            Some("dot")
        );
    }

    #[test]
    fn group_and_indicator_compatibility_constructors_keep_defaults() {
        let _: Widget = Group::<u8>::new().into();
        let _: Widget = Indicator::new().into();
    }
}

mod scroll_area_tests {
    use incular_controls::scroll_area::{Corner, Root, Thumb, Viewport};
    use incular_widgets::{Text, Widget};

    #[test]
    fn root_builder_preserves_scroll_defaults_and_controller_wiring() {
        let default = Root::default();
        let built = Root::builder().build();
        let _: Widget = default.into();
        let _: Widget = built.into();

        let controller = incular_scroll::ScrollController::new();
        let root = Root::builder()
            .child(Text::new("scroll content"))
            .controller(controller)
            .show_scrollbar(false)
            .build();
        let _: Widget = root.into();
        let _: Widget = Root::new().child(Text::new("legacy")).into();
    }

    #[test]
    fn required_parts_use_widget_children_and_keep_from_conversions() {
        let viewport = Viewport::builder().child(Text::new("viewport")).build();
        let thumb = Thumb::builder().child(Text::new("thumb")).build();
        let corner = Corner::builder().child(Text::new("corner")).build();

        assert_eq!(
            Widget::from(viewport).text_if_any().as_deref(),
            Some("viewport")
        );
        assert_eq!(Widget::from(thumb).text_if_any().as_deref(), Some("thumb"));
        assert_eq!(
            Widget::from(corner).text_if_any().as_deref(),
            Some("corner")
        );
    }
}

mod scrollbar_tests {
    use incular_config::Constraints;
    use incular_controls::{ControlTheme, scrollbar::Scrollbar};
    use incular_core::{Color, Offset, PointerPhase, Size};
    use incular_rendering::{Brush, PaintCommand};
    use incular_scroll::ScrollbarStyle as RawScrollbarStyle;
    use incular_widgets::{SingleChildScrollView, Text, Widget, internal::WidgetTree};

    #[test]
    fn builder_keeps_controller_private_and_uses_explicit_defaults() {
        let default: Widget = Scrollbar::builder()
            .child(Text::new("content"))
            .build()
            .into();
        assert_eq!(default.debug_type_name(), "LayoutBuilder");

        let style = RawScrollbarStyle {
            width: 10.,
            min_thumb_extent: 24.,
            track_color: Color::TRANSPARENT,
            thumb_color: Color::WHITE,
        };
        let configured: Widget = Scrollbar::builder()
            .child(Text::new("content"))
            .style(style)
            .thumb_visibility(true)
            .build()
            .into();
        assert_eq!(configured.debug_type_name(), "LayoutBuilder");
    }

    #[test]
    fn plain_content_becomes_a_retained_scroll_view() {
        let controller = incular_scroll::ScrollController::new();
        let widget = Scrollbar::new(Widget::box_(Size::new(120., 1_000.), Color::WHITE))
            .controller(controller.clone())
            .thumb_visibility(true)
            .build(&ControlTheme::dark());

        assert_eq!(widget.debug_type_name(), "ScrollView");

        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount scrollbar");
        tree.layout(Constraints::tight(Size::new(120., 100.)))
            .expect("layout");

        let geometry = tree
            .scrollbar_diagnostics()
            .pop()
            .expect("retained scrollbar geometry");
        assert!(geometry.visible);
        assert_eq!(geometry.track.size.width, 8.);

        assert!(tree.scrollbar_pointer(PointerPhase::Down, Offset::new(115., 10.)));
        assert!(tree.scrollbar_pointer(PointerPhase::Move, Offset::new(115., 65.)));
        assert!(tree.scrollbar_pointer(PointerPhase::Up, Offset::new(115., 65.)));
        assert!(controller.offset() > 0.);
    }

    #[test]
    fn themed_style_reaches_retained_track_and_thumb_paint() {
        let controller = incular_scroll::ScrollController::new();
        let style = RawScrollbarStyle {
            width: 12.,
            min_thumb_extent: 30.,
            track_color: Color::rgba(10, 20, 30, 120),
            thumb_color: Color::rgba(200, 210, 220, 230),
        };
        let widget = Scrollbar::new(Widget::box_(Size::new(100., 800.), Color::WHITE))
            .controller(controller)
            .style(style)
            .thumb_visibility(true);
        let mut tree = WidgetTree::new();
        tree.mount(widget.into()).expect("mount scrollbar");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");

        let commands = tree.paint();
        assert!(commands.commands().iter().any(|command| {
            matches!(command, PaintCommand::RRect { rrect, brush } if rrect.rect.size.width == 12. && brush == &Brush::Solid(style.track_color))
        }));
        assert!(commands.commands().iter().any(|command| {
            matches!(command, PaintCommand::RRect { rrect, brush } if rrect.rect.size.width == 12. && brush == &Brush::Solid(style.thumb_color))
        }));
    }

    #[test]
    fn existing_scroll_view_reuses_its_controller_without_nesting() {
        let controller = incular_scroll::ScrollController::new();
        let existing: Widget =
            SingleChildScrollView::new(Widget::box_(Size::new(100., 800.), Color::WHITE))
                .controller(controller.clone())
                .into();
        let widget = Scrollbar::new(existing).build(&ControlTheme::light());
        assert_eq!(widget.debug_type_name(), "ScrollView");

        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount scrollbar");
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
        assert!(tree.scroll_at(Offset::new(10., 50.), Offset::new(0., 100.)));
        assert!(controller.offset() > 0.);
    }
}

mod select_tests {
    use incular_controls::select::{Item, List, Root};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builders_use_select_defaults() {
        let root = Root::builder().build();
        let _: Widget = root.into();

        let item = Item::builder().value("one").child(Text::new("One")).build();
        assert_eq!(item.value(), "one");
        assert!(!item.is_disabled());
    }

    #[test]
    fn builders_accept_widget_children_and_callbacks() {
        let root = Root::builder()
            .value("one")
            .child(Text::new("Choose"))
            .on_change(|_| {})
            .build();
        let _: Widget = root.into();

        let list = List::builder().child(Text::new("Options")).build();
        let _: Widget = list.into();
    }
}

mod selection_tests {
    use incular_config::Constraints;
    use incular_controls::{
        ControlTheme,
        checkbox::CheckedState,
        selection::{Checkbox, Radio, Switch},
    };
    use incular_core::Size;
    use incular_widgets::{Widget, internal::WidgetTree};

    #[test]
    fn checkbox_builder_preserves_semantic_defaults_and_initial_state() {
        let default = Checkbox::default();
        assert_eq!(default.checked_state(), CheckedState::Unchecked);
        let _: Widget = default.clone().build(&ControlTheme::light());

        let checked = Checkbox::builder()
            .value(true)
            .label("Accept")
            .on_changed(|_| {})
            .build();
        assert_eq!(checked.checked_state(), CheckedState::Checked);
        let _: Widget = checked.into();
    }

    #[test]
    fn radio_builder_keeps_required_value_and_explicit_defaults() {
        let radio = Radio::<u8>::builder()
            .value(2)
            .group_value(2)
            .label("Two")
            .on_changed(|_| {})
            .build();
        let _: Widget = radio.build(&ControlTheme::light());
        let _: Widget = radio.into();
    }

    #[test]
    fn switch_builder_preserves_runtime_defaults_and_compatibility() {
        let default = Switch::default();
        let _: Widget = default.build(&ControlTheme::light());

        let enabled = Switch::builder().value(true).on_changed(|_| {}).build();
        let _: Widget = enabled.clone().into();
        let _: Widget = Switch::new(true).into();

        let mut tree = WidgetTree::new();
        tree.mount(enabled.into()).expect("mount switch");
        tree.layout(Constraints::loose(Size::new(240.0, 80.0)))
            .expect("layout");
        tree.update_semantics();
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.state.checked == Some(true))
        );
    }
}

mod separator_tests {
    use incular_controls::{
        ControlTheme,
        separator::{Orientation, Separator},
    };
    use incular_widgets::Widget;

    #[test]
    fn builder_defaults_match_new() {
        let new = Separator::new();
        let built = Separator::builder().build();
        let theme = ControlTheme::light();

        let _: Widget = new.build(&theme);
        let _: Widget = built.build(&theme);
        let _: Widget = new.into();
        let _: Widget = built.into();
    }

    #[test]
    fn builder_preserves_orientation_and_thickness_behavior() {
        let separator = Separator::builder()
            .orientation(Orientation::Vertical)
            .thickness(-2.)
            .decorative(true)
            .build();
        let _: Widget = separator.into();
    }
}

mod slider_tests {
    use incular_controls::ControlTheme;
    use incular_controls::slider::{Control, Indicator, Range, Root, Thumb, Track};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builders_use_slider_semantic_defaults() {
        let range = Range::builder().build();
        assert_eq!(range, Range::default());

        let root = Root::builder().build();
        let _: Widget = root.build(&ControlTheme::light());
        let _: Widget = root.into();
    }

    #[test]
    fn builder_accepts_generic_widgets_and_callback_configuration() {
        let root = Root::builder()
            .value(0.5)
            .range(Range::builder().max(2.0).step(0.5).build())
            .orientation(incular_config::Axis::Vertical)
            .child(Text::new("Slider"))
            .on_change(|_| {})
            .on_change_start(|_| {})
            .on_change_end(|_| {})
            .build();
        let _: Widget = root.into();

        let _: Widget = Control::builder()
            .child(Text::new("Control"))
            .build()
            .into();
        let _: Widget = Track::builder().child(Text::new("Track")).build().into();
        let _: Widget = Indicator::builder()
            .child(Text::new("Indicator"))
            .build()
            .into();
        let _: Widget = Thumb::builder().child(Text::new("Thumb")).build().into();
    }
}

mod switch_tests {
    use incular_controls::switch::{Root, Thumb};
    use incular_widgets::{Text, Widget};

    #[test]
    fn anatomy_builders_keep_switch_defaults_and_generic_children() {
        let root = Root::builder().child(Text::new("Toggle")).build();
        assert!(!root.is_checked());
        let _: Widget = root.into();

        let default_checked = Root::builder().default_checked(true).build();
        assert!(default_checked.is_checked());

        let thumb = Thumb::builder().child(Text::new("thumb")).build();
        assert_eq!(Widget::from(thumb).text_if_any().as_deref(), Some("thumb"));
    }

    #[test]
    fn anatomy_compatibility_constructors_use_builder_defaults() {
        assert!(Root::new().is_checked() == Root::default().is_checked());
        let _: Widget = Thumb::new().into();
    }
}

mod tabs_tests {
    use incular_controls::CompositeOrientation;
    use incular_controls::tabs::{Indicator, List, Panel, Root, Tab};
    use incular_widgets::{Text, Widget};

    #[test]
    fn builders_use_tabs_defaults() {
        let root = Root::builder().build();
        assert_eq!(
            root.navigation().orientation(),
            CompositeOrientation::Horizontal
        );
        let _: Widget = root.into();

        let indicator = Indicator::builder().build();
        let _: Widget = indicator.into();
    }

    #[test]
    fn builders_preserve_required_parts_and_widget_composition() {
        let root = Root::builder()
            .value("overview")
            .orientation(CompositeOrientation::Vertical)
            .automatic(false)
            .child(Text::new("Tabs"))
            .on_change(|_| {})
            .build();
        assert_eq!(
            root.navigation().orientation(),
            CompositeOrientation::Vertical
        );
        let _: Widget = root.into();

        let list = List::builder().child(Text::new("List")).build();
        let tab = Tab::builder()
            .value("overview")
            .child(Text::new("Overview"))
            .build();
        let panel = Panel::builder().child(Text::new("Panel")).build();
        assert_eq!(tab.value(), "overview");
        assert!(!tab.is_disabled());
        let _: Widget = list.into();
        let _: Widget = tab.into();
        let _: Widget = panel.into();
    }
}

mod text_input_tests {
    use incular_controls::{ControlTheme, TextArea, TextField, TextFieldStyle};
    use incular_core::Size;
    use incular_widgets::{Widget, internal::TextEditingController};

    #[test]
    fn text_field_builder_has_explicit_empty_defaults() {
        let field = TextField::builder().build();
        let default = TextField::default();
        let theme = ControlTheme::light();
        let _: Widget = field.build(&theme);
        let _: Widget = default.build(&theme);
        let _: Widget = field.into();
    }

    #[test]
    fn text_field_builder_preserves_controller_and_legacy_setters() {
        let controller = TextEditingController::with_text("initial");
        let style = TextFieldStyle {
            padding: Some(incular_config::EdgeInsets::all(6.0)),
            ..TextFieldStyle::default()
        };
        let field = TextField::builder()
            .controller(controller.clone())
            .size(Size::new(240.0, 40.0))
            .placeholder("Search")
            .style(style)
            .on_submit(|_| {})
            .build();
        let _: Widget = field.into();

        let legacy = TextField::new(controller)
            .size(Size::new(120.0, 32.0))
            .placeholder("Legacy")
            .style(TextFieldStyle::default())
            .on_submit(|_| {});
        let _: Widget = legacy.into();
    }

    #[test]
    fn text_area_builder_has_defaults_and_preserves_composition() {
        let area = TextArea::builder()
            .controller(TextEditingController::with_text("notes"))
            .placeholder("Notes")
            .size(Size::new(300.0, 160.0))
            .build();

        let _: Widget = area.into();
        let _: Widget = TextArea::default().into();
        let _: Widget = TextArea::new(TextEditingController::new()).into();
    }
}

mod toggle_tests {
    use incular_controls::toggle::Group;
    use incular_widgets::{Text, Widget};

    #[test]
    fn group_builder_uses_explicit_defaults() {
        let group = Group::builder().build();
        let _: Widget = group.into();
        let _: Widget = Group::new().into();
    }

    #[test]
    fn group_builder_accepts_generic_widget_children() {
        let group = Group::builder()
            .multiple(true)
            .child(Text::new("toggles"))
            .build();
        let _: Widget = group.into();
        let _: Widget = Group::with_child(Text::new("child")).into();
    }
}

mod toolbar_tests {
    use incular_controls::toolbar::{Button, Group, Input, Link, Root, Separator};
    use incular_widgets::{Text, Widget};

    #[test]
    fn root_builder_accepts_a_generic_child() {
        let root = Root::builder().child(Text::new("toolbar")).build();
        let _: Widget = root.into();
        let _: Widget = Root::default().into();
    }

    #[test]
    fn toolbar_parts_expose_typed_builders_and_preserve_constructors() {
        let _: Widget = Button::builder().child(Text::new("button")).build().into();
        let _: Widget = Input::builder().child(Text::new("input")).build().into();
        let _: Widget = Link::builder().child(Text::new("link")).build().into();
        let _: Widget = Group::builder().child(Text::new("group")).build().into();
        let _: Widget = Separator::builder()
            .child(Text::new("separator"))
            .build()
            .into();
        let _: Widget = Button::new(Text::new("new button")).into();
    }
}

mod tooltip_tests {
    use incular_controls::tooltip::{PopupDescription, Provider, Trigger};
    use incular_widgets::{Text, Widget};

    #[test]
    fn provider_builder_matches_explicit_default_and_legacy_api() {
        let default = Provider::default();
        let built = Provider::builder().build();
        let _: Widget = default.into();
        let _: Widget = built.into();
        let _: Widget = Provider::builder().skip_delay(true).build().into();
        let _: Widget = Provider::new().into();
        let _: Widget = PopupDescription::new("description").into();
        let _: Widget = Trigger::new(Text::new("trigger")).into();
    }
}
