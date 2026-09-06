use incular::prelude::*;
use incular::reactive::{Action, Effect, Memo};
use incular_controls::Button;

#[path = "../tests/support/mod.rs"]
pub(crate) mod example_support;
#[path = "tests/simulations.rs"]
pub(crate) mod simulations;

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

    let query = incular::text::TextEditingController::with_text("rust");
    // A deterministic async service keeps this example runnable offline.
    let search = Action::<String, Vec<String>, ()>::new(|query| async move {
        let query = query.to_lowercase();
        Ok(["Rust signals", "Rust widgets", "Layout", "Painting"]
            .into_iter()
            .filter(|title| title.to_lowercase().contains(&query))
            .map(str::to_owned)
            .collect())
    });
    let app = Application::new(move |_| {
        logger.mount();
        let count_value = count.get();
        let request_state = request.state();
        Widget::from(Column::new(Vec::<Widget>::from([
            Text::new(format!("count: {count_value}")).into(),
            Text::new(format!("memo: {}", doubled.get())).into(),
            Text::new(format!("request: {request_state:?}")).into(),
            incular::material::TextField::new(query.clone())
                .placeholder("Search topics")
                .into(),
            Button::new("Search")
                .on_click({
                    let query = query.clone();
                    let search = search.clone();
                    move || {
                        let _ = search.dispatch(query.text());
                    }
                })
                .into(),
            Text::new(format!("Search results: {:?}", search.state())).into(),
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
        ])))
    })
    .expect("valid reactive application");
    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native reactive application");
}
