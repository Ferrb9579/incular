//! Integration coverage migrated from the material source modules.

mod library_tests {
    use incular_core::Color;
    use incular_material::*;
    use incular_text::TextStyle;
    use incular_widgets::{Widget, internal::TextEditingController};
    use std::time::Duration;

    #[test]
    fn autocomplete_filters_and_selects_options() {
        let mut autocomplete = Autocomplete::strings(["Ada", "Grace", "Alan"]);
        assert_eq!(autocomplete.matching_indices(), vec![0, 1, 2]);
        autocomplete.set_query("al");
        assert_eq!(autocomplete.matching_indices(), vec![2]);
        assert_eq!(autocomplete.select(2).map(String::as_str), Some("Alan"));
        assert_eq!(autocomplete.selected().map(String::as_str), Some("Alan"));
    }

    #[test]
    fn material_text_field_uses_the_core_editable_text_primitive() {
        let controller = TextEditingController::with_text("hello");
        let _widget: Widget = TextField::new(controller)
            .max_lines(None)
            .style(TextStyle::default().font_size(16.0))
            .into();
    }

    #[test]
    fn material_components_convert_to_widgets() {
        let _button: Widget = ElevatedButton::new("Save").into();
        let _dialog: Widget = AlertDialog::new().into();
        let _tabs: Widget = TabBar::new([]).into();
    }

    #[test]
    fn elevated_button_can_layout_and_paint() {
        let mut tree = incular_widgets::internal::WidgetTree::new();
        let button: Widget = ElevatedButton::new("Save").into();
        tree.mount(button).unwrap();
        tree.layout(incular_config::Constraints::tight(incular_core::Size::new(
            240.0, 64.0,
        )))
        .expect("layout");
        let _ = tree.paint();
    }

    #[test]
    fn raw_material_button_is_exposed_only_from_the_material_layer() {
        let _button: Widget = RawMaterialButton::new("Low-level action")
            .on_press(|| {})
            .into();
    }

    #[test]
    fn material_constants_and_state_enums_are_stable() {
        assert!(ThemeMode::System.is_system());
        assert!(!ThemeMode::Dark.is_system());
        assert_eq!(Colors::BLUE, Color::rgba(33, 150, 243, 255));
        assert_eq!(Durations::SHORT_4, Duration::from_millis(200));
        assert_eq!(SliderInteraction::default(), SliderInteraction::TapAndSlide);
        assert_eq!(TimePickerEntryMode::default(), TimePickerEntryMode::Dial);
        let swatch = MaterialColor::new(Colors::BLUE, [Colors::BLUE; 10]);
        assert_eq!(swatch.shade(500), Colors::BLUE);
    }

    #[test]
    fn vertical_divider_is_a_material_component() {
        let _: Widget = VerticalDivider::new().thickness(2.0).indent(4.0).into();
    }
}

mod p0_controls_tests {
    use incular_material::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn controlled_material_controls_lower_to_widgets() {
        let checkbox = Checkbox::new(false).tristate(true).value(None);
        assert_eq!(checkbox.is_checked(), None);
        let _: Widget = checkbox.into();
        let _: Widget = Radio::new("a").group_value(Some("b")).into();
        let _: Widget = Switch::new(true).enabled(false).into();
        let _: Widget = Slider::new(0.5).range(0.0, 10.0).divisions(Some(10)).into();
        let _: Widget = RangeSlider::new(RangeValues::new(0.2, 0.8)).into();
    }

    #[test]
    fn tabs_share_a_retained_controller_and_page_view() {
        let controller = TabController::new(2);
        controller.animate_to(1);
        assert_eq!(controller.index(), 1);
        let _: Widget = TabBar::new([Tab::text("One"), Tab::text("Two")])
            .controller(controller.clone())
            .into();
        let _: Widget = TabBarView::new([Text::new("One"), Text::new("Two")])
            .controller(controller)
            .into();
    }

    #[test]
    fn typed_builders_keep_material_defaults_and_accept_widgets() {
        let checkbox = Checkbox::builder().build();
        assert_eq!(checkbox.is_checked(), None);
        let _: Widget = checkbox.into();

        let switch = Switch::builder().value(true).build();
        let _: Widget = switch.into();

        let radio = Radio::builder()
            .value("option")
            .group_value("option")
            .build();
        let _: Widget = radio.into();

        let range_values = RangeValues::builder().start(0.8_f32).end(0.2_f32).build();
        assert_eq!(range_values.normalized(), RangeValues::new(0.2, 0.8));
        let range = RangeSlider::builder()
            .values(range_values)
            .minimum_separation(-1.0)
            .build();
        let _: Widget = range.into();

        let slider_theme = SliderThemeData::builder()
            .track_height(-1.0)
            .thumb_size(0.0)
            .build();
        assert_eq!(slider_theme.track_height, Some(0.0));
        assert_eq!(slider_theme.thumb_size, Some(1.0));

        let tab = Tab::builder().child(Text::new("First")).build();
        let _: Widget = TabBar::builder().tabs([tab]).build().into();
        let _: Widget = TabBarView::builder()
            .children([Text::new("First"), Text::new("Second")])
            .build()
            .into();
    }
}

mod button_tests {
    use incular_config::Constraints;
    use incular_controls::{ButtonVariant, ControlState, StateTable, StateValue};
    use incular_core::{Color, Size};
    use incular_material::*;
    use incular_widgets::{SizedBox, Widget, internal::WidgetTree};

    #[test]
    fn style_factory_preserves_sparse_state_properties() {
        let style = ElevatedButton::style_from(
            ButtonStyleConfig::new()
                .background_color(Color::rgba(1, 2, 3, 255))
                .elevation(StateValue::states(StateTable::new(1.0).pressed(4.0)))
                .minimum_size(Size::new(80.0, 40.0)),
        );
        assert_eq!(style.background, Some(Color::rgba(1, 2, 3, 255)));
        assert_eq!(style.minimum_size, Some(Size::new(80.0, 40.0)));
        assert_eq!(
            style
                .elevation
                .as_ref()
                .expect("elevation")
                .resolve(ControlState::PRESSED),
            4.0
        );
        assert_eq!(style.variant, ButtonVariant::Primary);
    }

    #[test]
    fn button_families_have_widget_conversions() {
        let _: Widget = ElevatedButton::new("Elevated").into();
        let _: Widget = FilledButton::tonal("Tonal").into();
        let _: Widget = OutlinedButton::new("Outlined").into();
        let _: Widget = TextButton::new("Text").into();
        let _: Widget = IconButton::new("⚙").into();
        let _: Widget = IconButton::icon(SizedBox::square(16.0))
            .tooltip("Settings")
            .into();
        let _: Widget = FloatingActionButton::small(SizedBox::square(20.0)).into();
        let _: Widget = FloatingActionButton::extended("Create").into();
    }

    #[test]
    fn material_default_constraints_follow_tap_target_policy() {
        let theme = ThemeData::light().with_tap_target_size(MaterialTapTargetSize::Padded);
        assert_eq!(
            theme.core().material_tap_target_size,
            MaterialTapTargetSize::Padded
        );
        assert_eq!(
            MaterialTapTargetSize::Padded.minimum_size(),
            Size::new(48.0, 48.0)
        );

        let mut tree = WidgetTree::new();
        let root = tree.mount(TextButton::new("Text").into()).unwrap();
        tree.layout(Constraints::loose(Size::new(240.0, 120.0)))
            .expect("layout");
        assert!(
            tree.render_size(tree.render_id(root).unwrap())
                .unwrap()
                .height
                >= 36.0
        );
    }
}

mod foundation_tests {
    use incular_config::Constraints;
    use incular_core::{Color, Size};
    use incular_material::*;

    #[test]
    fn state_property_prefers_more_specific_state() {
        let property =
            StateProperty::from_map([(WidgetState::Hovered, 1_u32), (WidgetState::Pressed, 2_u32)]);
        let states = WidgetStates::new()
            .with(WidgetState::Hovered)
            .with(WidgetState::Pressed);
        assert_eq!(property.resolve(states), 2);
    }

    #[test]
    fn visual_density_adjusts_both_axes() {
        let constraints = Constraints::tight(Size::new(48.0, 48.0));
        let compact = VisualDensity::COMPACT.effective_constraints(constraints);
        assert_eq!(compact.min_width(), 40.0);
        assert_eq!(compact.min_height(), 40.0);
    }

    #[test]
    fn seeded_scheme_has_complete_surface_roles() {
        let scheme = ColorScheme::from_seed(Color::rgba(0x67, 0x50, 0xa4, 255));
        assert_eq!(scheme.background, scheme.surface);
        assert_eq!(scheme.on_background, scheme.on_surface);
        assert_ne!(scheme.primary, Color::TRANSPARENT);
        assert_ne!(scheme.surface_container_highest, Color::TRANSPARENT);
    }

    #[test]
    fn theme_precedence_is_sparse() {
        let theme = ThemeData::light().copy_with(ThemeDataPatch {
            use_material3: Some(false),
            ..ThemeDataPatch::default()
        });
        assert!(!theme.core().use_material3);
        assert_eq!(theme.core().color_scheme, ColorScheme::light());
    }
}

mod feedback_tests {
    use incular_core::{Color, Size};
    use incular_material::*;
    use incular_widgets::{Text, Widget};

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

    #[test]
    fn snackbar_action_is_semantically_exposed() {
        let snackbar: Widget = SnackBar::text("Saved")
            .action(SnackBarAction::new("Dismiss", || {}))
            .into();
        let mut tree = incular_widgets::internal::WidgetTree::new();
        tree.mount(snackbar).expect("mount snackbar");
        tree.layout(incular_config::Constraints::tight(Size::new(400.0, 100.0)))
            .expect("layout");
        tree.update_semantics();

        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("Dismiss"))
        );
    }
}

mod menu_tests {
    use std::{cell::Cell, rc::Rc};

    use incular_config::{Constraints, EdgeInsets};
    use incular_core::{Color, Size};
    use incular_material::*;
    use incular_semantics::{
        Role as SemanticRole, SemanticActionKind, SemanticNodeId, SemanticsTree,
    };
    use incular_widgets::{Column, Container, ListView, Text, Widget, internal::WidgetTree};

    #[test]
    fn menu_style_merge_keeps_explicit_values() {
        let base = MenuStyle::new()
            .elevation(4.0)
            .padding(EdgeInsets::all(3.0));
        let override_style = MenuStyle::new()
            .elevation(9.0)
            .background_color(Color::WHITE);
        let merged = base.clone().merge(&override_style);
        assert_eq!(merged.elevation, base.elevation);
        assert_eq!(merged.padding, base.padding);
        assert_eq!(merged.background_color, override_style.background_color);
    }

    #[test]
    fn menu_controller_notifies_retained_state_on_transitions() {
        let controller = MenuController::new();
        controller.open();
        assert!(controller.is_open());
        controller.open();
        assert!(controller.is_open());
        controller.close();
        assert!(!controller.is_open());
    }

    #[test]
    fn menu_item_selection_closes_its_owning_menu_chain() {
        let controller = MenuController::new();
        let hits = Rc::new(Cell::new(0));
        let observed = hits.clone();
        let widget: Widget =
            MenuAnchor::new([MenuItemButton::label("Select")
                .on_pressed(move || observed.set(observed.get() + 1))])
            .controller(controller.clone())
            .child(Text::new("Menu"))
            .into();
        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount menu");
        tree.layout(Constraints::tight(Size::new(320.0, 240.0)))
            .expect("initial layout");
        controller.open();
        tree.layout(Constraints::tight(Size::new(320.0, 240.0)))
            .expect("open layout");
        tree.update_semantics();
        let semantic = tree
            .semantics()
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("Select"))
            .map(|(id, _)| id)
            .expect("menu item semantics");
        let element = tree
            .element_for_semantic_node(semantic)
            .expect("menu item element");
        let action = tree.action_for_element(element).expect("menu item action");
        let callback = tree
            .take_pending_handlers()
            .into_iter()
            .find(|(id, _)| *id == action)
            .map(|(_, callback)| callback)
            .expect("menu item callback");
        callback();
        assert_eq!(hits.get(), 1);
        assert!(!controller.is_open());
    }

    #[test]
    fn submenu_item_selection_closes_every_controller_in_the_chain() {
        let outer = MenuController::new();
        let inner = MenuController::new();
        let hits = Rc::new(Cell::new(0));
        let observed = hits.clone();
        let submenu = SubmenuButton::new(
            Text::new("More"),
            [MenuItemButton::label("Leaf").on_pressed(move || observed.set(observed.get() + 1))],
        )
        .controller(inner.clone());
        let widget: Widget = MenuAnchor::new([submenu])
            .controller(outer.clone())
            .child(Text::new("Menu"))
            .into();
        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount nested menu");
        tree.layout(Constraints::tight(Size::new(480.0, 320.0)))
            .expect("initial layout");
        outer.open();
        inner.open();
        tree.layout(Constraints::tight(Size::new(480.0, 320.0)))
            .expect("open nested layout");
        tree.update_semantics();
        let semantic = tree
            .semantics()
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("Leaf"))
            .map(|(id, _)| id)
            .expect("submenu leaf semantics");
        let element = tree
            .element_for_semantic_node(semantic)
            .expect("submenu leaf element");
        let action = tree
            .action_for_element(element)
            .expect("submenu leaf action");
        let callback = tree
            .take_pending_handlers()
            .into_iter()
            .find(|(id, _)| *id == action)
            .map(|(_, callback)| callback)
            .expect("submenu leaf callback");

        callback();

        assert_eq!(hits.get(), 1);
        assert!(!inner.is_open());
        assert!(!outer.is_open());
    }

    #[test]
    fn open_menu_exposes_ordered_menu_and_menu_item_semantics() {
        fn descendant_order(
            semantics: &SemanticsTree,
            root: SemanticNodeId,
        ) -> Vec<SemanticNodeId> {
            let mut ordered = Vec::new();
            let mut stack = semantics
                .node(root)
                .map(|node| node.children.iter().rev().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            while let Some(id) = stack.pop() {
                ordered.push(id);
                if let Some(node) = semantics.node(id) {
                    stack.extend(node.children.iter().rev().copied());
                }
            }
            ordered
        }

        let outer = MenuController::new();
        let inner = MenuController::new();
        let widget: Widget = MenuAnchor::new([
            Widget::from(MenuItemButton::label("First")),
            SubmenuButton::new(Text::new("More"), [MenuItemButton::label("Nested")])
                .controller(inner.clone())
                .into(),
            Widget::from(MenuItemButton::label("Disabled").enabled(false)),
        ])
        .controller(outer.clone())
        .child(Text::new("Menu"))
        .into();
        let mut tree = WidgetTree::new();
        tree.mount(widget).expect("mount semantic menu");
        tree.layout(Constraints::tight(Size::new(480.0, 320.0)))
            .expect("initial semantic layout");
        outer.open();
        inner.open();
        tree.layout(Constraints::tight(Size::new(480.0, 320.0)))
            .expect("open semantic layout");
        tree.update_semantics();

        let semantics = tree.semantics();
        let menu_nodes = semantics
            .iter()
            .filter(|(_, node)| node.role == SemanticRole::Menu)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        assert_eq!(menu_nodes.len(), 2, "outer and nested menu surfaces");

        let (first_id, first) = semantics
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("First"))
            .expect("first item semantics");
        assert_eq!(first.role, SemanticRole::MenuItem);
        assert!(first.actions.contains(&SemanticActionKind::Focus));
        assert!(first.actions.contains(&SemanticActionKind::Activate));

        let (submenu_id, submenu) = semantics
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("More"))
            .expect("submenu anchor semantics");
        assert_eq!(submenu.role, SemanticRole::MenuItem);
        assert_eq!(submenu.state.expanded, Some(true));

        let (nested_id, nested) = semantics
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("Nested"))
            .expect("nested item semantics");
        assert_eq!(nested.role, SemanticRole::MenuItem);

        let (disabled_id, disabled) = semantics
            .iter()
            .find(|(_, node)| node.label.as_deref() == Some("Disabled"))
            .expect("disabled item semantics");
        assert_eq!(disabled.role, SemanticRole::MenuItem);
        assert!(!disabled.state.enabled);
        assert!(!disabled.actions.contains(&SemanticActionKind::Activate));

        let outer_menu = menu_nodes
            .iter()
            .copied()
            .find(|menu| {
                let descendants = descendant_order(semantics, *menu);
                descendants.contains(&first_id)
                    && descendants.contains(&submenu_id)
                    && descendants.contains(&disabled_id)
            })
            .expect("outer menu semantic ownership");
        let nested_menu = menu_nodes
            .iter()
            .copied()
            .find(|menu| {
                *menu != outer_menu && descendant_order(semantics, *menu).contains(&nested_id)
            })
            .expect("nested menu semantic ownership");
        assert_ne!(outer_menu, nested_menu);

        let top_level_order = descendant_order(semantics, outer_menu)
            .into_iter()
            .filter(|id| [first_id, submenu_id, disabled_id].contains(id))
            .collect::<Vec<_>>();
        assert_eq!(top_level_order, [first_id, submenu_id, disabled_id]);
        assert!(descendant_order(semantics, nested_menu).contains(&nested_id));
    }

    #[test]
    fn dropdown_entry_preserves_value_and_disabled_state() {
        let entry = DropdownMenuEntry::new(7_u32, "Seven").disabled(true);
        assert_eq!(entry.value, 7);
        assert_eq!(entry.label, "Seven");
        assert!(!entry.enabled);
    }

    #[test]
    fn menu_bar_builds_a_retained_material_surface() {
        let children: Vec<Widget> = vec![
            MenuItemButton::label("File").into(),
            SubmenuButton::new(Text::new("Edit"), [MenuItemButton::label("Undo")]).into(),
        ];
        let _: Widget = MenuBar::new(children).spacing(4.0).into();
    }

    #[test]
    fn menu_anchor_opens_in_a_retained_tree() {
        let controller = MenuController::new();
        let widget: Widget = MenuAnchor::new([MenuItemButton::label("New")])
            .controller(controller.clone())
            .child(FilledButton::tonal("Menu"))
            .into();
        let mut tree = WidgetTree::new();
        tree.mount(ListView::new([widget]).into())
            .expect("mount menu anchor");
        tree.layout(Constraints::tight(Size::new(320.0, 240.0)))
            .expect("layout");
        controller.open();
        tree.layout(Constraints::tight(Size::new(320.0, 240.0)))
            .expect("layout");
        tree.update_semantics();
        let anchor_nodes = tree
            .semantics()
            .iter()
            .filter(|(_, node)| node.label.as_deref() == Some("Menu"))
            .map(|(_, node)| node)
            .collect::<Vec<_>>();
        assert_eq!(
            anchor_nodes.len(),
            1,
            "button-styled anchor visuals must not duplicate trigger semantics"
        );
        assert_eq!(anchor_nodes[0].role, SemanticRole::Button);
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("New"))
        );
    }

    #[test]
    fn menu_anchor_opens_inside_material_scaffold() {
        let controller = MenuController::new();
        let body: Widget = Container::new()
            .color(Color::WHITE)
            .padding(EdgeInsets::all(24.0))
            .child(ListView::new([Card::new(
                Column::new([
                    Widget::from(Text::new("Button families")),
                    MenuAnchor::new([MenuItemButton::label("New document")])
                        .controller(controller.clone())
                        .child(FilledButton::tonal("Menu"))
                        .into(),
                ])
                .spacing(12.0),
            )]))
            .into();
        let root: Widget = MaterialApp::new(ScaffoldMessenger::new(
            Scaffold::new(body)
                .app_bar(AppBar::new(Text::new("Material Workbench")))
                .floating_action_button(FloatingActionButton::extended("Create")),
        ))
        .into();
        let mut tree = WidgetTree::new();
        tree.mount(root).expect("mount material scaffold");
        tree.layout(Constraints::tight(Size::new(1180.0, 820.0)))
            .expect("layout");
        controller.open();
        tree.layout(Constraints::tight(Size::new(1180.0, 820.0)))
            .expect("layout");
        tree.layout(Constraints::tight(Size::new(1180.0, 820.0)))
            .expect("layout");
        let _ = tree.paint();
        tree.update_semantics();
        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("New document"))
        );
    }

    #[test]
    fn typed_builders_keep_menu_defaults_and_widget_composition() {
        let style = MenuStyle::builder()
            .background_color(Color::WHITE)
            .padding(EdgeInsets::all(4.0))
            .build();
        assert_eq!(style.padding, Some(EdgeInsets::all(4.0)));

        let item = MenuItemButton::builder()
            .child(Text::new("Open"))
            .leading_icon(Text::new("+"))
            .enabled(false)
            .build();
        let mut item_tree = WidgetTree::new();
        item_tree.mount(item.into()).expect("mount menu item");
        item_tree
            .layout(Constraints::loose(Size::new(240.0, 80.0)))
            .expect("layout");
        item_tree.update_semantics();
        assert!(
            item_tree
                .semantics()
                .iter()
                .any(|(_, node)| node.role == incular_semantics::Role::MenuItem
                    && !node.state.enabled)
        );

        let submenu = SubmenuButton::builder()
            .child(Text::new("Edit"))
            .menu_children([MenuItemButton::label("Undo")])
            .build();
        let _: Widget = submenu.into();

        let anchor = MenuAnchor::typed_builder()
            .menu_children([MenuItemButton::label("Close")])
            .child(Text::new("Menu"))
            .build();
        let _: Widget = anchor.into();

        let popup_item = PopupMenuItem::<u32>::builder()
            .child(Text::new("One"))
            .value(1)
            .height(-1.0)
            .build();
        assert_eq!(popup_item.item_value(), Some(&1));
        assert_eq!(
            popup_item.item_child().text_if_any().as_deref(),
            Some("One")
        );
        assert!(popup_item.is_enabled());
        let checked = CheckedPopupMenuItem::<u32>::builder()
            .child(Text::new("Checked"))
            .checked(true)
            .value(2)
            .build();
        let _: Widget = checked.into();

        let popup = PopupMenuButton::<u32>::builder()
            .item_builder(|| vec![PopupMenuItem::label("One").value(1)])
            .child(Text::new("More"))
            .build();
        let _: Widget = popup.into();

        let dropdown = DropdownButton::<u32>::builder()
            .items([DropdownMenuItem::label("One").value(1)])
            .hint(Text::new("Choose"))
            .build();
        let _: Widget = dropdown.into();

        let entry = DropdownMenuEntry::<u32>::builder()
            .value(1_u32)
            .label("One")
            .label_widget(Text::new("Custom"))
            .build();
        let menu = DropdownMenu::builder()
            .entries([entry])
            .leading_icon(Text::new("+"))
            .build();
        let _: Widget = menu.into();
    }
}

mod app_shell_tests {
    use incular_material::*;
    use incular_widgets::{ScrollPhysics, SizedBox, Text};

    #[test]
    fn scroll_behavior_preserves_physics_and_devices() {
        let behavior = MaterialScrollBehavior::new()
            .physics(ScrollPhysics::clamping().always_scrollable())
            .scrollbars(false)
            .drag_devices([MaterialPointerDevice::Mouse]);
        assert!(!behavior.get_scrollbars());
        assert_eq!(behavior.get_drag_devices(), &[MaterialPointerDevice::Mouse]);
        assert_ne!(behavior.get_physics(), ScrollPhysics::clamping());
    }

    #[test]
    fn messenger_controller_queues_and_removes_current_snackbar() {
        let controller = ScaffoldMessengerController::new();
        assert!(!controller.is_showing());
        controller.show_snack_bar(SnackBar::text("first"));
        assert!(controller.is_showing());
        controller.show_snack_bar(SnackBar::text("second"));
        assert!(controller.current_snack_bar().is_some());
        assert_eq!(controller.queued_count(), 1);
        assert!(controller.hide_current_snack_bar().is_some());
        assert!(controller.is_showing());
        assert!(controller.hide_current_snack_bar().is_some());
        assert!(!controller.is_showing());
    }

    #[test]
    fn messenger_mount_exposes_the_current_snackbar_action() {
        let controller = ScaffoldMessengerController::new();
        let mut tree = incular_widgets::internal::WidgetTree::new();
        tree.mount(
            ScaffoldMessenger::with_controller(controller.clone(), SizedBox::shrink()).into(),
        )
        .expect("mount messenger");
        tree.layout(incular_config::Constraints::tight(incular_core::Size::new(
            400.0, 100.0,
        )))
        .expect("layout");

        controller
            .show_snack_bar(SnackBar::text("Saved").action(SnackBarAction::new("Dismiss", || {})));
        tree.layout(incular_config::Constraints::tight(incular_core::Size::new(
            400.0, 100.0,
        )))
        .expect("layout");
        tree.update_semantics();

        assert!(
            tree.semantics()
                .iter()
                .any(|(_, node)| node.label.as_deref() == Some("Dismiss"))
        );
    }

    #[test]
    fn app_uses_named_initial_route_before_home() {
        let app = MaterialApp::new(Text::new("home"))
            .route("/settings", Text::new("settings"))
            .initial_route("/settings")
            .title("sample");
        assert_eq!(app.get_title(), Some("sample"));
        assert_eq!(app.get_theme_mode(), ThemeMode::System);
    }
}

mod chips_tests {
    use incular_controls::ControlTheme;
    use incular_material::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn raw_chip_defaults_match_a_neutral_material_chip() {
        let chip = RawChip::new("Inbox");
        assert_eq!(chip.label(), "Inbox");
        assert!(!chip.is_selected());
        assert!(chip.is_enabled());
        assert_eq!(chip.elevation_value(), 0.0);
        let _: Widget = chip.into();
    }

    #[test]
    fn raw_chip_sanitizes_elevation_and_retains_actions() {
        let deleted = std::rc::Rc::new(std::cell::Cell::new(false));
        let deleted_callback = deleted.clone();
        let chip = RawChip::new("Remove me")
            .selected(true)
            .show_checkmark(true)
            .elevation(f32::INFINITY)
            .on_deleted(move || deleted_callback.set(true));

        assert_eq!(chip.elevation_value(), 0.0);
        assert!(chip.is_selected());
        assert!(chip.is_enabled());
        assert!(!deleted.get());
        let _: Widget = chip.into();
    }

    #[test]
    fn named_chip_wrappers_expose_their_selection_modes() {
        let choice = ChoiceChip::new("One", true).on_selected(|_| {});
        let filter = FilterChip::new("Unread", false).on_deleted(|| {});
        let input = InputChip::new("Tag")
            .selected(true)
            .on_pressed(|| {})
            .on_deleted(|| {});

        let _: Widget = choice.into();
        let _: Widget = filter.into();
        let _: Widget = input.into();
    }

    #[test]
    fn chip_family_materializes_through_retained_widgets() {
        let theme = ControlTheme::light();
        let _: Widget = Chip::new("Base").build(&theme);
        let _: Widget = ActionChip::new("Run").on_pressed(|| {}).into();
        let _: Widget = ChoiceChip::new("One", true).into();
        let _: Widget = FilterChip::new("Unread", false)
            .on_selected(|_| {})
            .on_deleted(|| {})
            .into();
        let _: Widget = InputChip::new("Token").on_deleted(|| {}).into();
        let _: Widget = RawChip::new("Raw").delete_icon(Text::new("x")).into();
    }
}
