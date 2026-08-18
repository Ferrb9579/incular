//! Tokio runtime demo: component-scoped work, cancellation, a blocking job,
//! and UI-thread-only signal updates.
use std::{cell::Cell, rc::Rc, time::Duration};

use incular::prelude::*;

fn main() {
    let state = Signal::new(AsyncValue::<String>::Idle);
    let started = Rc::new(Cell::new(false));
    let app = Application::new(move |cx| {
        if !started.replace(true) {
            state.set(AsyncValue::Loading);
            let after_timer = state.clone();
            cx.spawn_into(
                async {
                    tokio::time::sleep(Duration::from_millis(650)).await;
                    "Tokio timer completed; UI update ran on the native owner".to_owned()
                },
                move |result, _| {
                    match result {
                        Ok(message) => after_timer.set(AsyncValue::Ready(message)),
                        Err(error) => after_timer.set(AsyncValue::Error(error)),
                    };
                },
            );

            // A scoped task is cancelled before it can update a stale owner.
            let scope = cx.task_scope();
            let cancelled = cx.spawn_in(&scope, async move {
                tokio::time::sleep(Duration::from_secs(30)).await;
            });
            cancelled.cancel();
            scope.cancel();

            let after_blocking = state.clone();
            cx.spawn_blocking(
                || (1_u64..=25_000).sum::<u64>(),
                move |result, _runtime| {
                    match result {
                        Ok(sum) => after_blocking
                            .set(AsyncValue::Ready(format!("background sum finished: {sum}"))),
                        Err(error) => after_blocking.set(AsyncValue::Error(error)),
                    };
                },
            );
        }

        let message = match state.get() {
            AsyncValue::Idle => "idle".into(),
            AsyncValue::Loading => "loading local work…".into(),
            AsyncValue::Ready(value) => value,
            AsyncValue::Error(error) => format!("task error: {error:?}"),
        };
        Padding::all(
            28.,
            Widget::column(vec![
                Text::new("Incular async runtime")
                    .style(TextStyle {
                        size: 28.,
                        color: Color::rgba(180, 220, 255, 255),
                        ..TextStyle::default()
                    })
                    .into(),
                Text::new("The window remains input responsive; work wakes Winit only when ready.")
                    .into(),
                Padding::all(16., Text::new(message)).into(),
            ]),
        )
        .into()
    })
    .expect("valid async application");
    // Advanced libraries can use Tokio directly. Direct tasks have Tokio/app
    // lifetime, not a declarative component lifetime.
    app.tokio_handle().spawn(async {});
    incular::run(app).expect("native async runtime application");
}
