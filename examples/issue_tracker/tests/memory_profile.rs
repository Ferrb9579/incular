//! Diagnostic-only heap accounting around the unchanged benchmark workload.
//! This executable is never used for published process-memory measurements.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct CountingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn allocated(size: usize) {
    let live = LIVE.fetch_add(size, Relaxed) + size;
    PEAK.fetch_max(live, Relaxed);
}

// Each operation forwards the original pointer/layout to System. Counters do
// not allocate; failed allocations leave the live total unchanged.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            allocated(size);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[allow(unused_attributes)]
#[path = "../main.rs"]
mod workload;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(stage) = std::env::var("INCULAR_HEAP_STAGE") {
        if !matches!(stage.as_str(), "empty" | "text") {
            return Err("INCULAR_HEAP_STAGE must be empty or text".into());
        }
        let mut text = (stage == "text").then(incular::text::TextEngine::new);
        if let Some(text) = text.as_mut() {
            let style = incular::text::TextStyle::new()
                .font_family("Arial")
                .font_size(14.);
            std::hint::black_box(text.layout(
                "The quick brown fox",
                &style,
                None,
                incular::text::TextAlign::Start,
            ));
        }
        eprintln!(
            "RUST_HEAP stage={stage} live={} peak={}",
            LIVE.load(Relaxed),
            PEAK.load(Relaxed)
        );
        if let Some(path) = std::env::var_os("INCULAR_BENCH_READY") {
            std::fs::write(path, "ready")?;
        }
        std::thread::sleep(std::time::Duration::from_secs(30));
        std::hint::black_box(text);
        return Ok(());
    }
    std::thread::spawn(|| {
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_secs(2));
            eprintln!(
                "RUST_HEAP live={} peak={}",
                LIVE.load(Relaxed),
                PEAK.load(Relaxed)
            );
        }
        std::process::exit(0);
    });
    workload::main()
}
