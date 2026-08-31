use incular::material::prelude::*;
use incular::material::{ControlState, ControlTheme, SplashFactory, StateTable, StateValue};
use incular::prelude::{
    Code, Color, Column, Constraints, EdgeInsets, Form, KeyboardEvent, KeyboardKey, NamedKey, Size,
    Text, TextEditingController, Widget,
};
use incular::widgets::internal::WidgetTree;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

#[test]
fn material_surface_exposes_state_theme_and_component_families() {
    let states = WidgetStates::new()
        .with(WidgetState::Hovered)
        .with(WidgetState::Pressed);
    let color = StateProperty::from_map([
        (WidgetState::Hovered, Color::rgba(1, 2, 3, 255)),
        (WidgetState::Pressed, Color::rgba(4, 5, 6, 255)),
    ]);
    assert_eq!(color.resolve(states), Color::rgba(4, 5, 6, 255));
    assert_eq!(
        WidgetStates::from(ControlState::DISABLED),
        WidgetStates::new().with(WidgetState::Disabled)
    );

    let theme = ThemeData::from_seed(Color::rgba(103, 80, 164, 255))
        .with_use_material3(true)
        .with_splash_factory(SplashFactory::Ripple)
        .with_page_transitions_theme(PageTransitionsTheme {
            duration: Duration::from_millis(200),
            reduced_motion: false,
        });
    assert!(theme.core().use_material3);
    assert_eq!(
        theme.core().material_tap_target_size,
        MaterialTapTargetSize::Padded
    );

    let child: Widget = Text::new("Material").into();
    let _: Widget = Theme::new(
        theme,
        Column::new([
            Widget::from(Material::new(child.clone()).elevation(2.0)),
            Widget::from(Card::new(child.clone()).padding(EdgeInsets::all(8.0))),
            Widget::from(Divider::new().thickness(1.0)),
            Widget::from(Badge::new("3").child(child.clone())),
            Widget::from(LinearProgressIndicator::new().value(0.5)),
            Widget::from(CircularProgressIndicator::new().value(0.5)),
            Widget::from(AppBar::new(Text::new("Material")).action(TextButton::new("Help"))),
            Widget::from(
                BottomNavigationBar::new([
                    BottomNavigationBarItem::new(Text::new("A"), "Home"),
                    BottomNavigationBarItem::new(Text::new("B"), "Settings"),
                ])
                .current_index(1),
            ),
            Widget::from(
                NavigationBar::new([
                    NavigationDestination::new(Text::new("A"), "Home"),
                    NavigationDestination::new(Text::new("B"), "Settings"),
                ])
                .selected_index(1),
            ),
            Widget::from(CircleAvatar::new().child(Text::new("A"))),
            Widget::from(ListTile::new(child.clone())),
            Widget::from(CheckboxListTile::new(true, child.clone())),
            Widget::from(RadioListTile::new(1_u8, Some(1_u8), child.clone())),
            Widget::from(SwitchListTile::new(false, child)),
        ]),
    )
    .into();
    let _: Widget = Scaffold::new(Text::new("Body"))
        .app_bar(AppBar::new(Text::new("Title")))
        .floating_action_button(FloatingActionButton::extended("Add"))
        .into();
    let _: Size = MaterialTapTargetSize::Padded.minimum_size();
}

#[test]
fn material_button_style_is_sparse_and_state_aware() {
    let style = ElevatedButton::style_from(
        ButtonStyleConfig::new()
            .elevation(StateValue::states(StateTable::new(1.0).pressed(4.0)))
            .minimum_size(Size::new(96.0, 48.0)),
    );
    assert_eq!(style.minimum_size, Some(Size::new(96.0, 48.0)));
    assert_eq!(
        style.resolve_elevation(ControlState::PRESSED, &ControlTheme::light()),
        4.0
    );
    assert!(ButtonStyle::new().background.is_none());
    assert!(ButtonStyle::new().merge(&style).minimum_size.is_some());
}

#[test]
fn material_seed_and_retained_controller_defaults_match_p0_contracts() {
    // `from_seed` defaults to Flutter's light ColorScheme; dark is an
    // explicit ThemeData/brightness choice rather than a seed-luminance
    // heuristic.
    assert_eq!(
        ColorScheme::from_seed(Color::rgba(103, 80, 164, 255)).surface,
        Color::rgba(255, 251, 255, 255)
    );

    let tabs = TabController::new(2);
    assert_eq!(tabs.index(), 0);
    tabs.set_index(1);
    assert_eq!(tabs.index(), 1);
    let _: Widget = TabBar::new([Tab::text("First"), Tab::text("Second")])
        .controller(tabs.clone())
        .into();

    let _: Widget = NavigationBar::new([
        NavigationDestination::new(Text::new("A"), "Enabled"),
        NavigationDestination::new(Text::new("B"), "Disabled").disabled(true),
    ])
    .into();
    let _: Widget = Slider::new(0.5)
        .interaction(SliderInteraction::TapOnly)
        .label("50%")
        .into();
}

#[test]
fn material_slider_uses_retained_keyboard_focus_and_range_constraints() {
    let observed = Rc::new(Cell::new(0.0_f32));
    let callback_value = observed.clone();
    let slider = Slider::new(0.5)
        .range(0.0, 1.0)
        .divisions(Some(10))
        .autofocus(true)
        .on_changed(move |value| callback_value.set(value));
    let mut tree = WidgetTree::new();
    tree.mount(slider.into()).expect("mount material slider");
    tree.layout(Constraints::loose(Size::new(320.0, 80.0)))
        .expect("layout");
    let focused = tree.autofocus_element().expect("slider autofocus node");
    tree.set_keyboard_focus(focused, true);
    assert!(tree.dispatch_keyboard(
        Some(focused),
        KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::ArrowRight), Code::ArrowRight,),
    ));
    assert!((observed.get() - 0.6).abs() < f32::EPSILON);
}

#[test]
fn material_text_form_field_registers_validation_and_save_callbacks() {
    let controller = TextEditingController::with_text("Ada");
    let form = Form::new();
    let saved = Rc::new(Cell::new(String::new()));
    let saved_value = saved.clone();
    let registration = TextFormField::new()
        .controller(controller)
        .validator(|value| value.trim().is_empty().then(|| "Required".to_owned()))
        .on_saved(move |value| saved_value.set(value))
        .register_with_form(&form);

    assert_eq!(form.field_count(), 1);
    assert!(form.save());
    assert_eq!(saved.take(), "Ada");
    drop(registration);
    assert_eq!(form.field_count(), 0);
}
