//! Low-overhead performance profiling model.
//!
//! Cheap counters always exist; timing history is opt-in via
//! [`ProfilerMode`]. The hot path records into fixed-size `Copy` structs — no
//! dynamic allocation, no string formatting, no locks on the UI thread.

use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

/// How much profiling state the framework retains per frame.
///
/// [`ProfilerMode::Normal`] keeps only the latest frame record plus cumulative
/// counters, so ordinary applications pay one struct copy per frame.
/// History modes additionally retain a bounded ring of recent frames for
/// percentile reporting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProfilerMode {
    /// Counters and latest frame only. Default production mode.
    #[default]
    Normal,
    /// Retains bounded frame history for diagnostics tooling.
    Diagnostic,
    /// History plus fine-grained spans; intended for local investigation.
    Profiling,
}

/// Monotonic per-phase CPU durations for exactly one frame, in microseconds.
///
/// Phases do not nest: each duration covers only work attributed to that
/// phase, so the sum equals the measured CPU frame cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FrameTimings {
    /// Input dispatch before the frame began.
    pub event_processing: u32,
    /// Tokio/UI message drain and async completions.
    pub runtime_messages: u32,
    /// BUILD phase: widget updates and reactive rebuilds.
    pub build: u32,
    /// LAYOUT phase.
    pub layout: u32,
    /// COMPOSITE phase (retained layer transforms/opacity/effects).
    pub composite: u32,
    /// SEMANTICS phase.
    pub semantics: u32,
    /// PAINT phase including display-list flattening.
    pub paint: u32,
    /// Sum of all phases above.
    pub cpu_total: u32,
}
impl FrameTimings {
    #[must_use]
    pub fn total(&self) -> Duration {
        Duration::from_micros(u64::from(self.cpu_total))
    }
}

/// Structural work executed during one frame (deltas, not totals).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FrameWork {
    pub updated_elements: u64,
    pub rebuilt_elements: u64,
    pub laid_out_render_objects: u64,
    pub repainted_render_objects: u64,
    pub composited_layers: u64,
    pub active_animations: u64,
    pub display_list_commands: usize,
    pub requested_another_frame: bool,
}

/// One observed frame: identity, timings, work counters, budget verdict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FrameRecord {
    pub frame: u64,
    pub timings: FrameTimings,
    pub work: FrameWork,
    /// True when this frame exceeded the configured refresh budget.
    pub over_budget: bool,
}

/// Bounded statistics over retained frame history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct FrameStatistics {
    pub samples: u64,
    pub latest_us: f64,
    pub mean_us: f64,
    pub min_us: f64,
    pub max_us: f64,
    pub p50_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
}

/// Fixed-capacity ring of recent frames. Capacity is clamped to
/// `[300, 1000]`; insertion never allocates beyond the initial buffer.
#[derive(Debug)]
pub struct FrameHistory {
    records: std::collections::VecDeque<FrameRecord>,
    capacity: usize,
}
impl FrameHistory {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            records: std::collections::VecDeque::with_capacity(capacity.clamp(300, 1000)),
            capacity: capacity.clamp(300, 1000),
        }
    }
    pub fn push(&mut self, record: FrameRecord) {
        if self.records.len() == self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }
    /// Maximum retained samples.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.capacity
    }
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        false
    }
    #[must_use]
    pub fn recorded(&self) -> usize {
        self.records.len()
    }
    #[must_use]
    pub fn latest(&self) -> Option<FrameRecord> {
        self.records.back().copied()
    }
    pub fn iter(&self) -> impl Iterator<Item = FrameRecord> + '_ {
        self.records.iter().copied()
    }
    /// Percentile summary of CPU frame time. Sorting happens only when called;
    /// the hot path never invokes this.
    #[must_use]
    pub fn statistics(&self) -> FrameStatistics {
        let mut samples: Vec<u32> = self.records.iter().map(|f| f.timings.cpu_total).collect();
        let count = samples.len() as u64;
        if count == 0 {
            return FrameStatistics::default();
        }
        samples.sort_unstable();
        let pick = |percentile: f64| -> f64 {
            let index = ((count as f64 - 1.) * percentile).round() as usize;
            f64::from(samples[index.min(samples.len() - 1)])
        };
        let sum: u64 = samples.iter().map(|s| u64::from(*s)).sum();
        FrameStatistics {
            samples: count,
            latest_us: f64::from(*samples.last().expect("non-empty")),
            mean_us: sum as f64 / count as f64,
            min_us: f64::from(samples[0]),
            max_us: f64::from(samples[samples.len() - 1]),
            p50_us: pick(0.50),
            p95_us: pick(0.95),
            p99_us: pick(0.99),
        }
    }
}

/// Aggregated counter snapshot for scheduler/reactivity behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SchedulerCounters {
    /// Number of signal reads.
    pub signal_reads: u64,
    /// Number of state-changing signal writes.
    pub signal_writes: u64,
    /// Number of unique dependent queue entries created after deduplication.
    pub dependents_enqueued: u64,
    pub runtime_wakes: u64,
    pub redraw_requests: u64,
    pub frames_started: u64,
    pub frames_presented: u64,
    pub frames_skipped: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct BudgetStatistics {
    /// Refresh rate used to derive the per-frame budget, if known.
    pub refresh_rate_hz: Option<f32>,
    /// Derived budget in microseconds; `None` without a known rate.
    pub budget_us: Option<f64>,
    pub frames_over_budget: u64,
    pub longest_frame_us: f64,
}

/// Per-application profiling coordinator. Lives off the hot path except for
/// one `record` call per frame.
#[derive(Debug)]
pub struct PerformanceProfiler {
    mode: ProfilerMode,
    history: FrameHistory,
    budget: BudgetStatistics,
    next_frame: u64,
    /// Bounded presentation-timestamp ring for true FPS (wall clock).
    present_times: std::collections::VecDeque<Instant>,
}
impl PerformanceProfiler {
    const FPS_WINDOW: usize = 120;

    /// Frames per second over the recent presentation window, using actual
    /// present timestamps rather than CPU frame cost.
    #[must_use]
    pub fn frames_per_second(&self) -> Option<f32> {
        let first = *self.present_times.front()?;
        let last = *self.present_times.back()?;
        let elapsed = last.duration_since(first).as_secs_f32();
        (elapsed > 0.).then(|| (self.present_times.len().saturating_sub(1)) as f32 / elapsed)
    }
    pub(crate) fn note_present(&mut self, presented: bool) {
        if !presented {
            return;
        }
        if self.present_times.len() == Self::FPS_WINDOW {
            self.present_times.pop_front();
        }
        self.present_times.push_back(Instant::now());
    }
}
impl PerformanceProfiler {
    #[must_use]
    pub fn new(mode: ProfilerMode) -> Self {
        Self {
            mode,
            history: FrameHistory::new(512),
            budget: BudgetStatistics::default(),
            next_frame: 1,
            present_times: std::collections::VecDeque::with_capacity(Self::FPS_WINDOW),
        }
    }
    #[must_use]
    pub const fn mode(&self) -> ProfilerMode {
        self.mode
    }
    pub fn set_mode(&mut self, mode: ProfilerMode) {
        self.mode = mode;
    }
    #[must_use]
    pub const fn history(&self) -> &FrameHistory {
        &self.history
    }
    #[must_use]
    pub const fn budget(&self) -> &BudgetStatistics {
        &self.budget
    }
    /// Derives the frame budget from an actual refresh rate. `None` clears it.
    pub fn set_refresh_rate_hz(&mut self, hz: Option<f32>) {
        self.budget.refresh_rate_hz = hz.filter(|rate| *rate > 0.);
        self.budget.budget_us = self
            .budget
            .refresh_rate_hz
            .map(|rate| 1_000_000. / f64::from(rate));
    }
    #[must_use]
    pub fn next_frame_id(&mut self) -> u64 {
        let id = self.next_frame;
        self.next_frame += 1;
        id
    }
    /// Records one finished frame. In [`ProfilerMode::Normal`] only budget and
    /// longest-frame accounting runs; history retention requires a higher mode.
    pub fn record(&mut self, mut record: FrameRecord) {
        if let Some(budget_us) = self.budget.budget_us {
            record.over_budget = f64::from(record.timings.cpu_total) > budget_us;
            if record.over_budget {
                self.budget.frames_over_budget += 1;
            }
        }
        let total = f64::from(record.timings.cpu_total);
        if total > self.budget.longest_frame_us {
            self.budget.longest_frame_us = total;
        }
        if self.mode != ProfilerMode::Normal {
            self.history.push(record);
        }
    }
}

/// Measures a phase boundary into microseconds on a fixed-width field.
#[derive(Debug)]
pub struct PhaseSpan {
    started: Instant,
}
impl PhaseSpan {
    #[must_use]
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }
    #[must_use]
    pub fn elapsed_us(&self) -> u32 {
        self.started
            .elapsed()
            .as_micros()
            .try_into()
            .unwrap_or(u32::MAX)
    }
}

/// Renderer-reported metrics for exactly one presented or skipped frame.
/// Platform adapters convert backend-specific statistics into this compact
/// snapshot so the runtime never depends on a particular GPU backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RenderFrameMetrics {
    pub frame: u64,
    pub draw_calls: u32,
    pub instances: u32,
    pub render_passes: u32,
    /// Path triangles submitted (three vertices each).
    pub path_triangles: u32,
    /// Instance-buffer bytes written this frame.
    pub upload_bytes: u64,
    /// Texture pixel bytes written this frame.
    pub texture_upload_bytes: u64,
    /// Pipeline creations this frame. The steady-state invariant is zero;
    /// nonzero values occur only at initialization or genuine format change.
    pub pipelines_created: u32,
    /// Queue submissions this frame (main plus offscreen effect work).
    pub queue_submissions: u32,
    pub prepare_us: u32,
    pub encode_us: u32,
    pub submit_us: u32,
}

/// Shared and per-window GPU residency snapshot reported by the adapter
/// without rendering. Plain integers only — like [`RenderFrameMetrics`],
/// platform adapters convert backend-specific statistics into this compact
/// snapshot so the runtime never depends on a particular GPU backend.
/// Shared counts describe the device-owned caches; `local_*` counts
/// describe the queried window's retained bindings. Cumulative rasterized
/// and eviction counters sit alongside current residency so tests and
/// hosts can separate what is retained now from what has happened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GpuResourceSummary {
    /// Shared image textures currently retained.
    pub shared_image_entries: u64,
    /// Shared image entries dropped by budget eviction (cumulative).
    pub shared_image_evictions: u64,
    /// Shared gradient textures currently retained.
    pub shared_gradient_entries: u64,
    /// Shared gradient entries dropped by budget eviction (cumulative).
    pub shared_gradient_evictions: u64,
    /// Live glyph atlas pages.
    pub glyph_live_pages: u64,
    /// Glyph atlas pages retired by eviction or tightening (cumulative).
    pub glyph_page_evictions: u64,
    /// Glyph bitmaps rasterized across all windows (cumulative, shared).
    pub glyphs_rasterized: u64,
    /// Image bindings retained by the queried window.
    pub local_image_entries: u64,
    /// Gradient bindings retained by the queried window.
    pub local_gradient_entries: u64,
    /// Glyph page bindings retained by the queried window.
    pub local_glyph_pages: u64,
}

/// Optional non-blocking GPU timing sample forwarded by the adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct GpuSample {
    pub supported: bool,
    pub frame: u64,
    pub main_pass_us: f64,
}

/// Read-only performance view of one retained window.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct WindowPerformance {
    pub requested_frames: u64,
    pub presented_frames: u64,
    pub skipped_frames: u64,
    /// Most recent frame's CPU phase timings (microseconds).
    pub latest: Option<FrameRecord>,
    /// Most recent renderer-reported metrics.
    pub render: RenderFrameMetrics,
    /// Latest non-blocking GPU timing sample when the adapter supports them.
    pub gpu: Option<GpuSample>,
}

/// Machine-readable aggregate of everything the framework measured. Produced
/// only on demand; building one allocates and is never done in the hot path.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PerformanceSnapshot {
    pub scheduler: SchedulerCounters,
    pub budget: BudgetStatistics,
    /// Present-timestamp FPS over the recent window, when presenting.
    pub fps: Option<f32>,
    pub windows: Vec<WindowPerformance>,
    /// Bounded CPU frame-time statistics over retained history, if enabled.
    pub frame_statistics: FrameStatistics,
    /// Retained-tree work totals for the primary window's tree.
    pub widgets: WidgetWorkSnapshot,
    /// Text layout/shaping cache behavior.
    pub text: TextCacheSnapshot,
    /// Accessibility projection behavior.
    pub accessibility: AccessibilitySnapshot,
}

/// Subset of retained-tree counters relevant to invalidation contracts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct WidgetWorkSnapshot {
    pub mounts: u64,
    pub unmounts: u64,
    pub rebuilds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub composites: u64,
    pub animation_ticks: u64,
    pub scroll_offset_updates: u64,
    pub reconciliation_fast_paths: u64,
    pub layout_cache_hits: u64,
    pub display_lists_reused: u64,
    pub compositor_only_updates: u64,
    pub lazy_layouts: u64,
    pub items_built: u64,
    pub items_reused: u64,
    // Task 15 reconciliation breakdown.
    pub child_list_scans: u64,
    pub identical_child_bailouts: u64,
    pub elements_created: u64,
    pub elements_removed: u64,
    pub elements_moved: u64,
    pub dirty_requests: u64,
    pub dirty_queue_deduplicated: u64,
    pub elements_total: usize,
    pub render_objects_total: usize,
    pub layers_total: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TextCacheSnapshot {
    pub layouts_requested: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    /// Paragraphs shaped fresh (document path misses).
    pub paragraphs_reshaped: u64,
    /// Paragraph layouts served entirely from cache.
    pub parley_layouts_reused: u64,
    /// Multi-paragraph documents composed from retained paragraphs.
    pub documents_composed: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AccessibilitySnapshot {
    pub nodes_published: u64,
    pub updates_skipped_unchanged: u64,
}

/// Shared, observable view of the latest [`PerformanceSnapshot`].
///
/// The application publishes throttled snapshots (default at most ~5 Hz and
/// only when the profiler mode is [`ProfilerMode::Diagnostic`] or higher, so
/// production frames never build one). Overlay builders read
/// [`PerformanceHub::version`] during their build; publishing bumps that
/// version through a `Signal`-compatible counter so only overlay elements
/// rebuild.
#[derive(Clone)]
pub struct PerformanceHub {
    inner: std::rc::Rc<HubInner>,
}
struct HubInner {
    snapshot: RefCell<PerformanceSnapshot>,
    /// Bumped on every publish; overlay builders read it during their build
    /// so publishing invalidates exactly the observing elements.
    version: super::Signal<u64>,
    /// Set by the facade when an overlay element subscribes; publishing is
    /// skipped entirely while nobody observes, keeping idle cost at zero.
    observed: Cell<bool>,
    last_publish: Cell<Option<Instant>>,
}
impl Default for PerformanceHub {
    fn default() -> Self {
        Self {
            inner: std::rc::Rc::new(HubInner {
                snapshot: RefCell::new(PerformanceSnapshot::default()),
                version: super::Signal::new(0),
                observed: Cell::new(false),
                last_publish: Cell::new(None),
            }),
        }
    }
}
impl PerformanceHub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Reads the current publish counter. Calling this from a builder
    /// subscribes that element to future publications.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.inner.version.get()
    }
    #[must_use]
    pub fn observed(&self) -> bool {
        self.inner.observed.get()
    }
    pub fn set_observed(&self, observed: bool) {
        self.inner.observed.set(observed);
    }
    #[must_use]
    pub fn snapshot(&self) -> PerformanceSnapshot {
        self.inner.snapshot.borrow().clone()
    }
    /// True when enough time has elapsed since the previous publish.
    #[must_use]
    pub fn publish_due(&self, min_interval: Duration) -> bool {
        match self.inner.last_publish.get() {
            Some(previous) => previous.elapsed() >= min_interval,
            None => true,
        }
    }
    pub fn publish(&self, snapshot: PerformanceSnapshot) {
        *self.inner.snapshot.borrow_mut() = snapshot;
        let next = self.inner.version.get().wrapping_add(1);
        self.inner.version.set(next);
        self.inner.last_publish.set(Some(Instant::now()));
    }
}

impl PerformanceSnapshot {
    /// Formats this snapshot into the standard debug-overlay lines.
    #[must_use]
    pub fn overlay_lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(16);
        lines.push(format!(
            "FPS {}",
            self.fps
                .map(|fps| format!("{fps:.0}"))
                .unwrap_or_else(|| "idle".into())
        ));
        let latest = self.windows.iter().find_map(|window| window.latest);
        if let Some(record) = latest {
            let timings = &record.timings;
            let ms = |micros: u32| f64::from(micros) / 1000.;
            lines.push(format!("CPU {:>6.2} ms", ms(timings.cpu_total)));
            match self
                .windows
                .iter()
                .filter_map(|window| window.gpu)
                .find(|gpu| gpu.supported && gpu.main_pass_us.is_finite())
            {
                Some(gpu) => lines.push(format!("GPU {:>6.2} ms", gpu.main_pass_us / 1000.)),
                None => lines.push("GPU  timing unavailable".into()),
            }
            lines.push(format!("Build      {:>7.2}", ms(timings.build)));
            lines.push(format!("Layout     {:>7.2}", ms(timings.layout)));
            lines.push(format!("Paint      {:>7.2}", ms(timings.paint)));
            lines.push(format!("Composite  {:>7.2}", ms(timings.composite)));
            let render = self
                .windows
                .iter()
                .map(|window| window.render)
                .max_by_key(|render| render.draw_calls)
                .unwrap_or_default();
            lines.push(format!(
                "Render     {:>7.2}",
                (f64::from(render.prepare_us) + f64::from(render.encode_us + render.submit_us))
                    / 1000.
            ));
            lines.push(format!("Draws      {:>7}", render.draw_calls));
            lines.push(format!("Instances  {:>7}", render.instances));
            lines.push(format!("Passes     {:>7}", render.render_passes));
            if render.pipelines_created > 0 {
                lines.push(format!(
                    "Pipelines  {:>7} (unexpected after init)",
                    render.pipelines_created
                ));
            }
            if let Some(budget) = self.budget.budget_us {
                lines.push(format!(
                    "Budget     {:>6.2} ms ({} over)",
                    budget / 1000.,
                    self.budget.frames_over_budget
                ));
            }
        }
        let widgets = &self.widgets;
        lines.push(format!("Elements   {:>7}", widgets.elements_total));
        lines.push(format!(
            "Built      {:>7}",
            latest.map_or(0, |record| record.work.updated_elements)
        ));
        lines.push(format!(
            "Layout     {:>7}",
            latest.map_or(0, |record| record.work.laid_out_render_objects)
        ));
        lines.push(format!(
            "Paint      {:>7}",
            latest.map_or(0, |record| record.work.repainted_render_objects)
        ));
        let requests = self.text.cache_hits + self.text.cache_misses;
        if requests > 0 {
            let hit_rate = self.text.cache_hits as f64 / requests as f64 * 100.;
            lines.push(format!("Text hit   {:>6.1}%", hit_rate));
        }
        if self.widgets.child_list_scans > 0 {
            lines.push(format!(
                "Scan/Bail  {:>4}/{}",
                self.widgets.child_list_scans, self.widgets.identical_child_bailouts
            ));
        }
        if self.text.documents_composed > 0 {
            lines.push(format!(
                "Para resh  {:>4} ({} docs)",
                self.text.paragraphs_reshaped, self.text.documents_composed
            ));
        }
        if self.accessibility.updates_skipped_unchanged > 0 {
            lines.push(format!(
                "A11y skips {:>7}",
                self.accessibility.updates_skipped_unchanged
            ));
        }
        lines
    }
}
