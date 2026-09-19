use incular::config::Constraints;
use incular::core::{Color, Size};
use incular::material::{
    ButtonComponentThemes, CheckboxThemeData, ElevatedButton, Theme, ThemeData, ThemeDataPatch,
};
use incular::widgets::{LayoutBuilder, SizedBox, Widget, internal::WidgetTree};
use std::{cell::Cell, rc::Rc};

#[test]
fn checkbox_theme_does_not_recolor_or_rebuild_button_consumer() {
    let base = ThemeData::light();
    let original_accent = base.control_theme().colors.accent;
    let original_buttons = base.buttons() as *const ButtonComponentThemes;
    let patched = base.clone().copy_with(ThemeDataPatch {
        checkbox_theme: Some(
            CheckboxThemeData::new()
                .fill_color(Color::rgba(230, 20, 80, 255))
                .icon_size(31.0),
        ),
        ..ThemeDataPatch::default()
    });

    assert_eq!(patched.control_theme().colors.accent, original_accent);
    assert_eq!(
        patched.buttons() as *const ButtonComponentThemes,
        original_buttons,
        "checkbox-only patch must preserve button projection identity"
    );

    let builds = Rc::new(Cell::new(0_u32));
    let probe: Widget = {
        let builds = builds.clone();
        LayoutBuilder::new(move |context, _| {
            builds.set(builds.get() + 1);
            let _ = context
                .depend_on_shared::<ButtonComponentThemes>()
                .expect("button projection scope");
            ElevatedButton::new("Stable").into()
        })
        .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Theme::new(base, probe.clone()).into())
        .expect("mount themed button consumer");
    let constraints = Constraints::loose(Size::new(180.0, 80.0));
    tree.layout(constraints).expect("initial layout");
    assert_eq!(builds.get(), 1);

    tree.update(root, Theme::new(patched, probe).into())
        .expect("update checkbox-only theme");
    tree.layout(constraints).expect("layout after theme update");

    assert_eq!(
        builds.get(),
        1,
        "unchanged button projection must not rebuild its consumer"
    );
}

#[test]
fn whole_theme_api_still_subscribes_when_requested() {
    let builds = Rc::new(Cell::new(0_u32));
    let probe: Widget = {
        let builds = builds.clone();
        LayoutBuilder::new(move |context, _| {
            builds.set(builds.get() + 1);
            let _ = Theme::of(context).expect("whole theme");
            SizedBox::shrink().into()
        })
        .into()
    };
    let base = ThemeData::light();
    let changed = base.clone().copy_with(ThemeDataPatch {
        checkbox_theme: Some(CheckboxThemeData::new().icon_size(29.0)),
        ..ThemeDataPatch::default()
    });
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Theme::new(base, probe.clone()).into())
        .expect("mount whole-theme consumer");
    let constraints = Constraints::loose(Size::new(80.0, 40.0));
    tree.layout(constraints).expect("initial layout");
    tree.update(root, Theme::new(changed, probe).into())
        .expect("update whole theme");
    tree.layout(constraints).expect("updated layout");
    assert_eq!(builds.get(), 2);
}
