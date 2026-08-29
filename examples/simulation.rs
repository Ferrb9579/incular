//! A live in-process simulator: semantic mouse input, keyboard input, and a
//! rendered application-frame capture, without using the host mouse/keyboard.

use incular::material::TextField;
use incular::prelude::*;
use incular::testing::Simulation;
use incular_controls::Button;
use std::fs;
use std::path::Path;

fn main() {
    let count = Signal::new(0_u32);
    let input = TextEditingController::new();

    let app_count = count.clone();
    let app_input = input.clone();
    let app = Application::new(move |_| {
        let value = app_count.get();

        let callback_count = app_count.clone();
        let increment = Button::new("Increment")
            .on_click(move || {
                callback_count.update(|count| *count += 1);
                println!("application callback: increment");
            })
            .into();

        Container::builder()
            .padding(EdgeInsets::all(24.0))
            .child(Widget::column(vec![
                Text::new("Live simulation demo")
                    .style(TextStyle::new().font_size(20.0).color(Color::WHITE))
                    .into(),
                Text::new("The commands below run in another Rust thread.").into(),
                Text::new(format!("Count: {value}")).into(),
                Widget::from(
                    TextField::new(app_input.clone())
                        .placeholder("Simulation input")
                        .size(Size::new(360.0, 42.0)),
                )
                .accessibility_label("Simulation input"),
                increment,
            ]))
            .build()
            .into()
    })
    .expect("valid simulation example application");

    let simulation = app.simulation();
    std::thread::spawn(move || run_simulation(simulation));

    println!("live simulation running; use your real mouse/keyboard independently");
    incular::run(app).expect("native simulation example application");
}

fn run_simulation(simulation: Simulation) {
    if let Err(error) = simulation.wait_for_frame() {
        eprintln!("simulation: initial frame failed: {error}");
        return;
    }
    println!("simulation: initial frame presented");

    if let Err(error) = simulation.click("Simulation input") {
        eprintln!("simulation: semantic text-field click failed: {error}");
        return;
    }
    if let Err(error) = simulation.type_text("Typed by simulation") {
        eprintln!("simulation: keyboard text failed: {error}");
        return;
    }
    if let Err(error) = simulation.press(Code::Backspace) {
        eprintln!("simulation: keyboard press failed: {error}");
        return;
    }
    if let Err(error) = simulation.wait_for_frame() {
        eprintln!("simulation: keyboard frame failed: {error}");
        return;
    }
    println!("simulation: semantic field click and keyboard input complete");

    for step in 1..=3 {
        if let Err(error) = simulation.click("Increment") {
            eprintln!("simulation: semantic click failed: {error}");
            return;
        }
        if let Err(error) = simulation.wait_for_frame() {
            eprintln!("simulation: frame {step} failed: {error}");
            return;
        }
        println!("simulation: semantic click {step} complete");
    }

    match simulation.capture() {
        Ok(screenshot) => {
            let path = Path::new("target/simulation-example.ppm");
            if let Err(error) = write_ppm(path, &screenshot) {
                eprintln!(
                    "simulation: capture succeeded but could not save {:?}: {error}",
                    path
                );
            } else {
                println!(
                    "simulation: captured {}x{} RGBA8 frame to {:?}",
                    screenshot.width(),
                    screenshot.height(),
                    path
                );
            }
        }
        Err(error) => eprintln!("simulation: capture failed: {error}"),
    }
}

fn write_ppm(path: &Path, screenshot: &incular::testing::Screenshot) -> std::io::Result<()> {
    let mut bytes =
        format!("P6\n{} {}\n255\n", screenshot.width(), screenshot.height()).into_bytes();
    for pixel in screenshot.pixels().chunks_exact(4) {
        bytes.extend_from_slice(&pixel[..3]);
    }
    fs::write(path, bytes)
}
