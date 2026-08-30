use incular::prelude::*;
use incular::reactive::{Action, Effect, Memo};
use incular_controls::Button;

#[path = "../support/mod.rs"]
mod example_support;
#[cfg(test)]
#[path = "tests.rs"]
mod example_tests;
mod simulations;

fn main() {
    let count = Signal::new(0_u32);
    let doubled = Memo::new({
        let count = count.clone();
        move || count.get() * 2
    });
    let logger = Effect::new({
        let count = count.clone();
        move || println!("count changed: {}", count.get())
    });
    let request =
        Action::<u32, String, ()>::new(
            |value| async move { Ok(format!("async result for {value}")) },
        );

    let app = Application::new(move |_| {
        logger.mount();
        let count_value = count.get();
        let request_state = request.state();
        Widget::column(vec![
            Text::new(format!("count: {count_value}")).into(),
            Text::new(format!("memo: {}", doubled.get())).into(),
            Text::new(format!("request: {request_state:?}")).into(),
            Button::new("Increment")
                .on_click({
                    let count = count.clone();
                    move || count.update(|value| *value += 1)
                })
                .into(),
            Button::new("Run action")
                .on_click({
                    let request = request.clone();
                    let count = count.clone();
                    move || {
                        let _ = request.dispatch(count.get());
                    }
                })
                .into(),
        ])
    })
    .expect("valid reactive application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native reactive application");
}
