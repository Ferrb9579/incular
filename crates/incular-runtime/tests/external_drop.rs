use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_platform::{
    DataTransfer, ExternalDragEvent, ExternalDragPhase, PlatformEvent, TransferOperation,
    TransferOperations, WindowEvent,
};
use incular_runtime::Application;
use incular_widgets::Widget;
use incular_widgets::extensions::ExternalDropTarget;
use std::{cell::Cell, path::PathBuf, rc::Rc, time::Instant};

fn event(phase: ExternalDragPhase) -> ExternalDragEvent {
    ExternalDragEvent {
        phase,
        transfer: DataTransfer::files([PathBuf::from("input.txt")]),
        position: Offset::new(20.0, 20.0),
        allowed_operations: TransferOperations::COPY,
    }
}

#[test]
fn application_routes_external_transfer_to_exact_window_and_reports_operation() {
    let drops = Rc::new(Cell::new(0));
    let drops_for_build = drops.clone();
    let mut application = Application::new(move |_| {
        ExternalDropTarget::new(Widget::box_(Size::new(80.0, 60.0), Color::BLACK))
            .on_drop({
                let drops = drops_for_build.clone();
                move |_, operation| {
                    assert_eq!(operation, TransferOperation::Copy);
                    drops.set(drops.get() + 1);
                }
            })
            .into()
    })
    .expect("application");
    let window = application.primary_window();
    application
        .run_window_frame_at(
            window,
            Constraints::tight(Size::new(80.0, 60.0)),
            Instant::now(),
        )
        .expect("initial frame");

    let enter = application.handle_external_drag_event(window, event(ExternalDragPhase::Enter));
    assert_eq!(enter.requested_operation, Some(TransferOperation::Copy));
    application.handle_window_event(WindowEvent::platform(
        window,
        PlatformEvent::ExternalDrag(event(ExternalDragPhase::Drop)),
    ));
    assert_eq!(drops.get(), 1);
    assert_eq!(
        application
            .window_diagnostics(window)
            .expect("window diagnostics")
            .input_events,
        2
    );
}
