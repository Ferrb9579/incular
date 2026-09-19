use incular::config::Constraints;
use incular::core::{Color, InputEvent, Offset, PointerPhase, Size};
use incular::material::{CheckboxThemeData, ElevatedButton, Theme, ThemeData, ThemeDataPatch};
use incular::runtime::{Application, Runtime, Signal};
use std::{cell::Cell, rc::Rc};

fn pointer(runtime: &mut Runtime, phase: PointerPhase, point: Offset) {
    let _ = runtime.handle_input(InputEvent::Pointer {
        phase,
        position: point,
    });
}

fn click(runtime: &mut Runtime, point: Offset) {
    pointer(runtime, PointerPhase::Down, point);
    pointer(runtime, PointerPhase::Up, point);
}

#[test]
fn padded_button_region_is_really_clickable() {
    let calls = Rc::new(Cell::new(0_u32));
    let observed = calls.clone();
    let root = Theme::new(
        ThemeData::light(),
        ElevatedButton::new("Tap").on_click(move || observed.set(observed.get() + 1)),
    );
    let mut runtime = Runtime::new(root.into()).expect("button runtime");
    runtime
        .run_frame(Constraints::loose(Size::new(160.0, 80.0)))
        .expect("initial frame");

    // Material's padded tap target is at least 48px high. This point sits at
    // the extreme bottom edge of that target, outside the ordinary 36px-ish
    // painted button content.
    click(&mut runtime, Offset::new(8.0, 47.0));
    assert_eq!(calls.get(), 1);
}

#[test]
fn theme_rebuild_during_press_preserves_one_activation() {
    let theme_revision = Signal::new(false);
    let app_revision = theme_revision.clone();
    let calls = Rc::new(Cell::new(0_u32));
    let observed = calls.clone();
    let app = Application::new(move |_| {
        let mut theme = ThemeData::light();
        if app_revision.get() {
            theme = theme.copy_with(ThemeDataPatch {
                checkbox_theme: Some(
                    CheckboxThemeData::new().fill_color(Color::rgba(220, 20, 80, 255)),
                ),
                ..ThemeDataPatch::default()
            });
        }
        Theme::new(
            theme,
            ElevatedButton::new("Press").on_click({
                let observed = observed.clone();
                move || observed.set(observed.get() + 1)
            }),
        )
        .into()
    })
    .expect("application");
    let mut runtime = app.into_runtime();
    let constraints = Constraints::loose(Size::new(160.0, 80.0));
    runtime.run_frame(constraints).expect("initial frame");

    let point = Offset::new(12.0, 20.0);
    pointer(&mut runtime, PointerPhase::Down, point);
    assert!(theme_revision.set(true));
    runtime
        .run_frame(constraints)
        .expect("frame while pointer remains pressed");
    pointer(&mut runtime, PointerPhase::Up, point);

    assert_eq!(
        calls.get(),
        1,
        "press tenure must survive theme reconciliation"
    );
}
