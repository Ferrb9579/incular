//! Optional readiness/capture automation; dormant in an ordinary app launch.
use incular::testing::Simulation;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct Observation {
    pub records: usize,
    pub rows: usize,
    pub page: usize,
    pub matches: usize,
    pub selected: usize,
    pub status: String,
    pub notes: String,
    pub list_scroll: f32,
    pub details_scroll: f32,
}

#[derive(Clone, Default)]
pub struct Probe(Option<Arc<Mutex<Observation>>>);

impl Probe {
    pub fn new() -> Self {
        Self(std::env::var_os("INCULAR_BENCH_SMOKE").map(|_| Arc::default()))
    }

    pub fn observe(&self, observation: impl FnOnce() -> Observation) {
        if let Some(probe) = &self.0 {
            *probe.lock().unwrap() = observation();
        }
    }

    pub fn observe_scroll(&self, details: bool, offset: f32) {
        if let Some(probe) = &self.0 {
            let mut observation = probe.lock().unwrap();
            if details {
                observation.details_scroll = offset;
            } else {
                observation.list_scroll = offset;
            }
        }
    }

    fn read(&self) -> Observation {
        self.0.as_ref().unwrap().lock().unwrap().clone()
    }
}

pub fn start(simulation: Simulation, probe: Probe) {
    let ready = std::env::var_os("INCULAR_BENCH_READY");
    let capture_path = std::env::var_os("INCULAR_BENCH_CAPTURE");
    let profile = std::env::var_os("INCULAR_BENCH_PROFILE").is_some();
    if ready.is_none() && capture_path.is_none() && !profile {
        return;
    }
    std::thread::spawn(move || {
        let run = || -> Result<(), Box<dyn std::error::Error>> {
            for _ in 0..3 {
                simulation.wait_for_frame()?;
            }
            if let Some(path) = capture_path {
                simulation.move_mouse_to(incular::prelude::Offset::new(-1.0, -1.0))?;
                simulation.wait_for_frame()?;
                capture(&simulation, std::path::Path::new(&path))?;
            }
            if probe.0.is_some() {
                smoke(&simulation, &probe)?;
            }
            if let Some(path) = ready {
                std::fs::write(
                    path,
                    "three frames presented; 1000 records; page size 100; first issue selected",
                )?;
            }
            if profile {
                for _ in 0..4 {
                    eprintln!("SCHEDULER {:?}", incular::runtime::scheduler_counters());
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                eprintln!("GPU {:?}", simulation.query_gpu_resources()?);
                std::process::exit(0);
            }
            Ok(())
        };
        if let Err(error) = run() {
            eprintln!("Benchmark readiness failed: {error}");
            std::process::exit(1);
        }
    });
}

fn smoke(simulation: &Simulation, probe: &Probe) -> Result<(), Box<dyn std::error::Error>> {
    use incular::prelude::Offset;
    use keyboard_types::Code;
    let settle = || -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..3 {
            simulation.wait_for_frame()?;
        }
        Ok(())
    };
    let click = |label| -> Result<(), Box<dyn std::error::Error>> {
        simulation.click(label)?;
        settle()
    };
    let initial = probe.read();
    assert_eq!(
        (
            initial.records,
            initial.rows,
            initial.matches,
            initial.selected
        ),
        (1000, 100, 1000, 0)
    );
    simulation.click_at(Offset::new(742.0, 654.0))?;
    settle()?;
    assert!(
        probe.read().list_scroll > 0.0,
        "scrollbar arrow must scroll the list"
    );
    simulation.mouse().down(Offset::new(742.0, 180.0))?;
    simulation.mouse().move_to(Offset::new(742.0, 220.0))?;
    simulation.mouse().move_to(Offset::new(742.0, 260.0))?;
    simulation.mouse().up(Offset::new(742.0, 260.0))?;
    settle()?;
    assert!(
        probe.read().list_scroll > 100.0,
        "scrollbar thumb must drag"
    );
    click("Next")?;
    assert_eq!(probe.read().page, 1);
    assert_eq!(probe.read().list_scroll, 0.0);
    click("Previous")?;
    assert_eq!(probe.read().page, 0);
    click("Completed")?;
    assert_eq!(probe.read().matches, 200);
    capture_state(simulation, "completed")?;
    click("Open")?;
    assert_eq!(probe.read().matches, 800);
    click("All issues")?;
    // The Electron detail pane scrolls independently; completion is below the fold.
    simulation.mouse().move_to(Offset::new(950.0, 600.0))?;
    simulation.scroll(Offset::new(0.0, 800.0))?;
    settle()?;
    assert!(probe.read().details_scroll > 0.0);
    click("Mark complete")?;
    assert_eq!(probe.read().status, "Done");
    capture_state(simulation, "complete")?;
    click("Reopen issue")?;
    assert_eq!(probe.read().status, "Open");
    simulation.click_at(Offset::new(850.0, 530.0))?;
    simulation.key_down(Code::ControlLeft)?;
    simulation.press(Code::KeyA)?;
    simulation.key_up(Code::ControlLeft)?;
    simulation.type_text("Persisted benchmark note")?;
    settle()?;
    simulation.click_at(Offset::new(450.0, 240.0))?;
    settle()?;
    assert_eq!(probe.read().selected, 1);
    simulation.click_at(Offset::new(450.0, 165.0))?;
    settle()?;
    assert_eq!(probe.read().notes, "Persisted benchmark note");
    simulation.click_at(Offset::new(900.0, 47.0))?;
    simulation.type_text("  APP-2000  ")?;
    settle()?;
    assert_eq!((probe.read().matches, probe.read().rows), (1, 1));
    simulation.click_at(Offset::new(450.0, 165.0))?;
    settle()?;
    assert_eq!(probe.read().selected, 999);
    capture_state(simulation, "search")?;
    simulation.click_at(Offset::new(900.0, 47.0))?;
    simulation.key_down(Code::ControlLeft)?;
    simulation.press(Code::KeyA)?;
    simulation.key_up(Code::ControlLeft)?;
    simulation.type_text("no-matching-issue")?;
    settle()?;
    assert_eq!((probe.read().matches, probe.read().rows), (0, 0));
    capture_state(simulation, "empty")?;
    eprintln!(
        "SMOKE PASSED: dataset, retained rows, pagination, filters, completion, selection, note persistence, trimmed search, empty state, wheel, scrollbar arrows and drag"
    );
    Ok(())
}

fn capture(
    simulation: &Simulation,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let screenshot = simulation.capture()?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(
        file,
        "P6\n{} {}\n255\n",
        screenshot.width(),
        screenshot.height()
    )?;
    for pixel in screenshot.pixels().as_chunks::<4>().0 {
        file.write_all(&pixel[..3])?;
    }
    file.flush()?;
    Ok(())
}

fn capture_state(simulation: &Simulation, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(directory) = std::env::var_os("INCULAR_BENCH_STATE_CAPTURES") {
        simulation.move_mouse_to(incular::prelude::Offset::new(-1.0, -1.0))?;
        simulation.wait_for_frame()?;
        capture(
            simulation,
            &std::path::Path::new(&directory).join(format!("{name}.ppm")),
        )?;
    }
    Ok(())
}
