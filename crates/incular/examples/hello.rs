//! Standalone example included in the published crate.
use incular::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Application::new(|_cx| {
        Container::builder()
            .padding(EdgeInsets::all(24.0))
            .alignment(Alignment::CENTER)
            .child(Text::new("Hello, Incular!"))
            .build()
            .into()
    })?;
    incular::run(app)?;
    Ok(())
}
