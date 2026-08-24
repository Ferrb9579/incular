//! Retained-memory probe: measures live heap bytes per mounted widget by
//! keeping several sized trees alive simultaneously.

use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::{Key, Text, Widget, WidgetTree};

/// Live-allocation accounting to distinguish leaks from page retention.
static LIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Counting;
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        unsafe {
            let ptr = std::alloc::System.alloc(layout);
            if !ptr.is_null() {
                LIVE.fetch_add(layout.size(), std::sync::atomic::Ordering::Relaxed);
            }
            ptr
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe {
            LIVE.fetch_sub(layout.size(), std::sync::atomic::Ordering::Relaxed);
            std::alloc::System.dealloc(ptr, layout)
        }
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;

#[allow(dead_code)]
fn live_mb() -> usize {
    live_kb() / 1024
}
fn live_kb() -> usize {
    LIVE.load(std::sync::atomic::Ordering::Relaxed) / 1024
}

fn rss_mb() -> usize {
    let stat = std::fs::read_to_string("/proc/self/status").unwrap();
    stat.split('\n')
        .find(|line| line.starts_with("VmRSS"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|kb| kb.parse::<usize>().ok())
        .unwrap_or(0)
        / 1024
}

fn row(items: usize) -> Widget {
    let children = (0..items)
        .map(|index| {
            Widget::from(Text::new(format!("row {index}"))).with_key(Key::Value(index as u64))
        })
        .collect::<Vec<_>>();
    Widget::column(children)
}

fn main() {
    let constraints = Constraints::tight(Size::new(600., 6000.));
    let mut trees = Vec::new();
    let mut previous_live = live_kb();
    for items in [1_000usize, 2_000, 4_000, 8_000] {
        let mut tree = WidgetTree::default();
        tree.mount(row(items)).expect("mount");
        tree.layout(constraints);
        // Force paint so display-list caches exist like a real frame.
        let _ = tree.paint();
        trees.push(tree);
        let live = live_kb();
        println!(
            "n={items:6}: live={} MB (delta {} KB, {} B/widget) rss={} MB",
            live / 1024,
            live - previous_live,
            (live - previous_live) * 1024 / items.max(1),
            rss_mb()
        );
        previous_live = live;
    }
}
