use incular_config::{Brightness, Constraints, Locale, RuntimeEnvironment, TextDirection};
use incular_core::Size;
use incular_material::{MaterialApp, Theme, ThemeData, ThemeMode};
use incular_runtime::Runtime;
use incular_widgets::{LayoutBuilder, SizedBox, Widget};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
struct ObservedEnvironment {
    brightness: Brightness,
    reduced_motion: bool,
    locale: Option<String>,
    direction: TextDirection,
}

#[test]
fn material_system_theme_locale_direction_and_motion_follow_runtime_environment() {
    let observations = Rc::new(RefCell::new(Vec::<ObservedEnvironment>::new()));
    let captured = observations.clone();
    let home: Widget = LayoutBuilder::new(move |context, _| {
        let theme = Theme::of(context).expect("MaterialApp supplies Theme");
        captured.borrow_mut().push(ObservedEnvironment {
            brightness: theme.core().brightness,
            reduced_motion: theme.control_theme().motion.reduced_motion,
            locale: context
                .depend_on::<Locale>()
                .map(|locale| locale.to_string()),
            direction: context
                .depend_on::<TextDirection>()
                .expect("MaterialApp supplies Directionality"),
        });
        SizedBox::shrink().into()
    })
    .into();

    let app: Widget = MaterialApp::new(home)
        .dark_theme(ThemeData::dark())
        .theme_mode(ThemeMode::System)
        .supported_locales([
            "en".parse::<Locale>().expect("locale"),
            "ar".parse::<Locale>().expect("locale"),
        ])
        .into();
    let mut runtime = Runtime::new(app).expect("runtime");
    let constraints = Constraints::tight(Size::new(200.0, 100.0));

    runtime.run_frame(constraints).expect("initial frame");
    assert_eq!(
        observations.borrow().last(),
        Some(&ObservedEnvironment {
            brightness: Brightness::Light,
            reduced_motion: false,
            locale: Some("en".into()),
            direction: TextDirection::Ltr,
        })
    );

    assert!(runtime.set_environment(RuntimeEnvironment {
        brightness: Brightness::Dark,
        reduced_motion: true,
        locales: vec!["ar-EG".parse().expect("locale")],
        text_direction: TextDirection::Rtl,
        ..runtime.environment()
    }));
    runtime.run_frame(constraints).expect("environment frame");
    assert_eq!(
        observations.borrow().last(),
        Some(&ObservedEnvironment {
            brightness: Brightness::Dark,
            reduced_motion: true,
            locale: Some("ar".into()),
            direction: TextDirection::Rtl,
        })
    );
}
