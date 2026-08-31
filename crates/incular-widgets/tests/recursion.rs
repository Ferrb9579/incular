use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};

use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::{
    Widget,
    internal::{FramePhase, WidgetTree},
};

fn recursive_builder() -> Widget {
    type Builder = Rc<dyn Fn(Constraints) -> Widget>;

    let slot: Rc<RefCell<Option<Builder>>> = Rc::new(RefCell::new(None));
    let slot_for_builder = Rc::clone(&slot);
    let builder: Builder = Rc::new(move |_| {
        let next = slot_for_builder
            .borrow()
            .as_ref()
            .expect("recursive builder installed")
            .clone();
        Widget::layout_builder(move |_, constraints| next(constraints))
    });
    *slot.borrow_mut() = Some(Rc::clone(&builder));

    Widget::layout_builder(move |_, constraints| builder(constraints))
}

fn report_directory(test_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("incular-{test_name}-{}", std::process::id()))
}

#[test]
fn recursive_layout_is_reported_before_stack_exhaustion() {
    let directory = report_directory("recursion");
    let mut tree = WidgetTree::new();
    tree.set_recursion_limit(16);
    tree.set_recursion_report_directory(&directory);
    tree.set_diagnostic_trigger("test recursive layout");
    tree.mount(recursive_builder())
        .expect("mount recursive builder");

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tree.layout(Constraints::tight(Size::new(100., 100.)))
            .expect("layout");
    }));

    assert!(panic.is_err());
    let report = tree.last_recursion_report().expect("recursion report");
    assert_eq!(report.phase, FramePhase::Build);
    assert!(!report.reentered);
    assert_eq!(report.trigger.as_deref(), Some("test recursive layout"));
    assert!(report.reason.contains("exceeded the configured limit"));
    assert!(!report.widget_path.is_empty());
    assert!(report.log_path.as_ref().is_some_and(|path| path.exists()));

    let _ = fs::remove_dir_all(directory);
}

#[test]
fn recursion_report_display_includes_public_diagnostics() {
    let directory = report_directory("recursion-display");
    let mut tree = WidgetTree::new();
    tree.set_recursion_limit(16);
    tree.set_recursion_report_directory(&directory);
    tree.set_diagnostic_trigger("open recursive fixture");
    tree.mount(recursive_builder())
        .expect("mount recursive builder");

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tree.layout(Constraints::tight(Size::new(80., 80.)))
            .expect("layout");
    }));

    let report = tree.last_recursion_report().expect("recursion report");
    let rendered = report.to_string();
    assert!(rendered.contains("Build"));
    assert!(rendered.contains("open recursive fixture"));
    assert!(rendered.contains("configured limit"));

    let _ = fs::remove_dir_all(directory);
}
