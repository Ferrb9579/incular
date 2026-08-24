//! Widget-independent scroll state, physics, and scrollbar geometry.

use std::{cell::RefCell, rc::Rc};

use incular_core::{Color, Offset, Rect, RestorationKey, RestorationScope, Size};

/// A shared, chunked prefix index for lazily measured item extents.
///
/// Rows begin at `estimated_extent` and can be corrected independently once a
/// viewport has laid them out. Offset/index conversion is logarithmic in the
/// number of chunks, so seeking to a distant row never requires measuring the
/// rows before it. Structural mutations retain measurements where possible and
/// invalidate only the affected logical mapping.
#[derive(Clone)]
pub struct MeasuredExtentIndex {
    state: Rc<RefCell<MeasuredExtentState>>,
    metrics: Rc<ExtentIndexCounters>,
}

impl std::fmt::Debug for MeasuredExtentIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeasuredExtentIndex")
            .field("len", &self.len())
            .field("estimated_extent", &self.estimated_extent())
            .field("total_extent", &self.total_extent())
            .field("measured_count", &self.measured_count())
            .finish()
    }
}

impl PartialEq for MeasuredExtentIndex {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

const EXTENT_CHUNK_SIZE: usize = 256;

#[derive(Clone)]
struct ExtentChunk {
    /// `None` means the row still uses the index-wide estimate.
    values: Vec<Option<f32>>,
    extent_sum: f64,
    measured: usize,
}

impl ExtentChunk {
    fn unmeasured(count: usize, estimate: f32) -> Self {
        Self {
            values: vec![None; count],
            extent_sum: f64::from(estimate) * count as f64,
            measured: 0,
        }
    }

    fn rebuild(&mut self, estimate: f32) {
        self.extent_sum = self
            .values
            .iter()
            .map(|value| f64::from(value.unwrap_or(estimate)))
            .sum();
        self.measured = self.values.iter().filter(|value| value.is_some()).count();
    }
}

/// Compact Fenwick tree over chunks. It deliberately indexes chunks rather
/// than items, keeping 1M-row indexes small while preserving logarithmic
/// lookup for mature measurements.
#[derive(Clone, Default)]
struct Fenwick(Vec<f64>);

impl Fenwick {
    fn from_values(values: impl IntoIterator<Item = f64>) -> Self {
        let values = values.into_iter().collect::<Vec<_>>();
        let mut tree = Self(vec![0.; values.len()]);
        for (index, value) in values.into_iter().enumerate() {
            let cursor = index + 1;
            tree.0[cursor - 1] += value;
            let parent = cursor + (cursor & cursor.wrapping_neg());
            if parent <= tree.0.len() {
                tree.0[parent - 1] += tree.0[cursor - 1];
            }
        }
        tree
    }

    fn prefix(&self, mut end: usize) -> f64 {
        let mut sum = 0.;
        end = end.min(self.0.len());
        while end > 0 {
            sum += self.0[end - 1];
            end &= end - 1;
        }
        sum
    }

    fn add(&mut self, mut index: usize, delta: f64) {
        index += 1;
        while index <= self.0.len() {
            self.0[index - 1] += delta;
            index += index & index.wrapping_neg();
        }
    }

    /// First zero-based slot whose inclusive prefix is strictly greater than
    /// `target`; `len` is returned when every prefix is at most the target.
    fn upper_bound(&self, target: f64) -> usize {
        let mut index = 0usize;
        let mut accumulated = 0.;
        let mut bit = self.0.len().next_power_of_two() >> 1;
        while bit != 0 {
            let next = index + bit;
            if next <= self.0.len() && accumulated + self.0[next - 1] <= target {
                index = next;
                accumulated += self.0[next - 1];
            }
            bit >>= 1;
        }
        index
    }
}

#[derive(Clone)]
struct MeasuredExtentState {
    estimate: f32,
    chunks: Vec<ExtentChunk>,
    extent_tree: Fenwick,
    count_tree: Fenwick,
    revision: u64,
    structure_revision: u64,
}

/// Structural operation counts for one variable-extent index. These prove
/// lookup/materialization complexity contracts (O(visible + overscan + log N))
/// without relying on wall-clock timings. Increments are relaxed atomics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtentIndexMetrics {
    /// `offset_for_index` calls (index → offset direction).
    pub index_to_offset_queries: u64,
    /// `index_at_offset` calls (offset → index direction).
    pub offset_to_index_queries: u64,
    /// Viewport range computations (no building, pure arithmetic).
    pub viewport_queries: u64,
    /// Exact post-layout extent updates applied to the tree.
    pub extent_updates: u64,
}

/// Interior shared counters; `Rc`-shared across clones of the index so every
/// handle reports the same structural history.
#[derive(Default)]
struct ExtentIndexCounters {
    index_to_offset: std::cell::Cell<u64>,
    offset_to_index: std::cell::Cell<u64>,
    viewport_queries: std::cell::Cell<u64>,
    extent_updates: std::cell::Cell<u64>,
}
impl ExtentIndexCounters {
    #[must_use]
    fn snapshot(&self) -> ExtentIndexMetrics {
        ExtentIndexMetrics {
            index_to_offset_queries: self.index_to_offset.get(),
            offset_to_index_queries: self.offset_to_index.get(),
            viewport_queries: self.viewport_queries.get(),
            extent_updates: self.extent_updates.get(),
        }
    }
}

impl MeasuredExtentState {
    fn rebuild_trees(&mut self) {
        self.extent_tree = Fenwick::from_values(self.chunks.iter().map(|chunk| chunk.extent_sum));
        self.count_tree =
            Fenwick::from_values(self.chunks.iter().map(|chunk| chunk.values.len() as f64));
    }

    fn len(&self) -> usize {
        self.count_tree.prefix(self.chunks.len()) as usize
    }

    fn locate(&self, index: usize) -> Option<(usize, usize)> {
        if index >= self.len() {
            return None;
        }
        let chunk = self.count_tree.upper_bound(index as f64);
        let before = self.count_tree.prefix(chunk) as usize;
        Some((chunk, index - before))
    }

    fn offset_for_index(&self, index: usize) -> f64 {
        let index = index.min(self.len());
        if index == self.len() {
            return self.extent_tree.prefix(self.chunks.len());
        }
        let Some((chunk_index, local)) = self.locate(index) else {
            return 0.;
        };
        self.extent_tree.prefix(chunk_index)
            + self.chunks[chunk_index].values[..local]
                .iter()
                .map(|value| f64::from(value.unwrap_or(self.estimate)))
                .sum::<f64>()
    }
}

impl MeasuredExtentIndex {
    /// Creates `item_count` unmeasured rows using `estimated_extent`.
    #[must_use]
    pub fn new(item_count: usize, estimated_extent: f32) -> Self {
        let metrics = Rc::new(ExtentIndexCounters::default());
        let _ = metrics.clone();
        assert!(
            estimated_extent.is_finite() && estimated_extent > 0.,
            "estimated extent must be positive and finite"
        );
        let chunks = (0..item_count)
            .step_by(EXTENT_CHUNK_SIZE)
            .map(|start| {
                ExtentChunk::unmeasured(
                    (item_count - start).min(EXTENT_CHUNK_SIZE),
                    estimated_extent,
                )
            })
            .collect::<Vec<_>>();
        let mut state = MeasuredExtentState {
            estimate: estimated_extent,
            chunks,
            extent_tree: Fenwick::default(),
            count_tree: Fenwick::default(),
            revision: 0,
            structure_revision: 0,
        };
        state.rebuild_trees();
        Self {
            state: Rc::new(RefCell::new(state)),
            metrics,
        }
    }

    /// Structural operation counters since creation. Shared across clones.
    #[must_use]
    pub fn metrics(&self) -> ExtentIndexMetrics {
        self.metrics.snapshot()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.state.borrow().len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[must_use]
    pub fn estimated_extent(&self) -> f32 {
        self.state.borrow().estimate
    }
    #[must_use]
    pub fn total_extent(&self) -> f32 {
        self.state
            .borrow()
            .extent_tree
            .prefix(self.state.borrow().chunks.len())
            .min(f64::from(f32::MAX)) as f32
    }
    #[must_use]
    pub fn measured_count(&self) -> usize {
        self.state
            .borrow()
            .chunks
            .iter()
            .map(|chunk| chunk.measured)
            .sum()
    }
    /// Changes whenever an extent or the logical structure changes.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }
    /// Changes only for insertions, removals, moves, and resizes.
    #[must_use]
    pub fn structure_revision(&self) -> u64 {
        self.state.borrow().structure_revision
    }

    /// Returns the estimated or measured leading offset of `index`.
    #[must_use]
    pub fn offset_for_index(&self, index: usize) -> f32 {
        self.metrics
            .index_to_offset
            .set(self.metrics.index_to_offset.get() + 1);
        self.state
            .borrow()
            .offset_for_index(index)
            .min(f64::from(f32::MAX)) as f32
    }

    /// Returns the row containing `offset`, clamped to the final row.
    #[must_use]
    pub fn index_at_offset(&self, offset: f32) -> Option<usize> {
        self.metrics
            .offset_to_index
            .set(self.metrics.offset_to_index.get() + 1);
        let state = self.state.borrow();
        if state.len() == 0 {
            return None;
        }
        let target = f64::from(offset.max(0.));
        let chunk = state
            .extent_tree
            .upper_bound(target)
            .min(state.chunks.len() - 1);
        let before_extent = state.extent_tree.prefix(chunk);
        let before_count = state.count_tree.prefix(chunk) as usize;
        let mut cursor = before_extent;
        for (local, value) in state.chunks[chunk].values.iter().enumerate() {
            cursor += f64::from(value.unwrap_or(state.estimate));
            if cursor > target {
                return Some(before_count + local);
            }
        }
        Some(state.len() - 1)
    }

    /// Computes the bounded logical range that intersects a viewport plus
    /// `cache_extent`. This does no row building or measuring.
    #[must_use]
    pub fn materialized_range(
        &self,
        scroll_offset: f32,
        viewport_extent: f32,
        cache_extent: f32,
    ) -> std::ops::Range<usize> {
        let state = self.state.borrow();
        let count = state.len();
        drop(state);
        if count == 0 {
            return 0..0;
        }
        self.metrics
            .viewport_queries
            .set(self.metrics.viewport_queries.get() + 1);
        let start_offset = (scroll_offset - cache_extent.max(0.)).max(0.);
        let end_offset = (scroll_offset.max(0.) + viewport_extent.max(0.) + cache_extent.max(0.))
            .max(start_offset);
        let start = self.index_at_offset(start_offset).unwrap_or(0);
        let end = self
            .index_at_offset(end_offset)
            .map_or(count, |index| index.saturating_add(1).min(count));
        start..end.max(start)
    }

    /// Records an exact post-layout extent. Returns whether it changed.
    pub fn set_measured_extent(&self, index: usize, extent: f32) -> bool {
        if !extent.is_finite() || extent < 0. {
            return false;
        }
        let mut state = self.state.borrow_mut();
        let Some((chunk_index, local)) = state.locate(index) else {
            return false;
        };
        let estimate = state.estimate;
        let chunk = &mut state.chunks[chunk_index];
        let previous = chunk.values[local].unwrap_or(estimate);
        if previous == extent && chunk.values[local].is_some() {
            return false;
        }
        if chunk.values[local].is_none() {
            chunk.measured += 1;
        }
        chunk.values[local] = Some(extent);
        let delta = f64::from(extent - previous);
        chunk.extent_sum += delta;
        state.extent_tree.add(chunk_index, delta);
        state.revision += 1;
        drop(state);
        self.metrics
            .extent_updates
            .set(self.metrics.extent_updates.get() + 1);
        true
    }

    /// Makes one row use the current estimate again.
    pub fn invalidate_extent(&self, index: usize) -> bool {
        let mut state = self.state.borrow_mut();
        let Some((chunk_index, local)) = state.locate(index) else {
            return false;
        };
        let estimate = state.estimate;
        let chunk = &mut state.chunks[chunk_index];
        let Some(previous) = chunk.values[local].take() else {
            return false;
        };
        chunk.measured -= 1;
        let delta = f64::from(estimate - previous);
        chunk.extent_sum += delta;
        state.extent_tree.add(chunk_index, delta);
        state.revision += 1;
        true
    }

    /// Resizes the logical list, preserving measurements in the retained
    /// prefix and creating unmeasured rows for growth.
    pub fn set_len(&self, len: usize) {
        let current = self.len();
        if current == len {
            return;
        }
        if len > current {
            self.insert(current, len - current);
        } else {
            self.remove(len..current);
        }
    }

    /// Inserts unmeasured rows. Existing measured rows after `index` retain
    /// their measured extents at their new logical indices.
    pub fn insert(&self, index: usize, count: usize) {
        if count == 0 {
            return;
        }
        let mut state = self.state.borrow_mut();
        let estimate = state.estimate;
        let index = index.min(state.len());
        let (chunk_index, local) = state.locate(index).unwrap_or((state.chunks.len(), 0));
        let mut remaining = count;
        if let Some(chunk) = state.chunks.get_mut(chunk_index) {
            let take = remaining.min(EXTENT_CHUNK_SIZE);
            chunk
                .values
                .splice(local..local, std::iter::repeat_n(None, take));
            chunk.rebuild(estimate);
            remaining -= take;
        }
        let mut insert_at = if chunk_index < state.chunks.len() {
            chunk_index + 1
        } else {
            chunk_index
        };
        while remaining > 0 {
            let take = remaining.min(EXTENT_CHUNK_SIZE);
            state
                .chunks
                .insert(insert_at, ExtentChunk::unmeasured(take, estimate));
            insert_at += 1;
            remaining -= take;
        }
        // Split only touched/oversize chunks; all other chunk work remains bounded.
        let mut cursor = chunk_index.saturating_sub(1);
        while cursor < state.chunks.len() {
            if state.chunks[cursor].values.len() > EXTENT_CHUNK_SIZE * 2 {
                let tail = state.chunks[cursor].values.split_off(EXTENT_CHUNK_SIZE);
                state.chunks[cursor].rebuild(estimate);
                let mut next = ExtentChunk {
                    values: tail,
                    extent_sum: 0.,
                    measured: 0,
                };
                next.rebuild(estimate);
                state.chunks.insert(cursor + 1, next);
            }
            if cursor > chunk_index + 1 {
                break;
            }
            cursor += 1;
        }
        state.rebuild_trees();
        state.revision += 1;
        state.structure_revision += 1;
    }

    /// Removes a logical range and its measurements.
    pub fn remove(&self, range: std::ops::Range<usize>) {
        let mut state = self.state.borrow_mut();
        let start = range.start.min(state.len());
        let end = range.end.min(state.len()).max(start);
        if start == end {
            return;
        }
        let mut flattened = state
            .chunks
            .iter()
            .flat_map(|chunk| chunk.values.iter().copied())
            .collect::<Vec<_>>();
        flattened.drain(start..end);
        state.chunks = flattened
            .chunks(EXTENT_CHUNK_SIZE)
            .map(|values| {
                let mut chunk = ExtentChunk {
                    values: values.to_vec(),
                    extent_sum: 0.,
                    measured: 0,
                };
                chunk.rebuild(state.estimate);
                chunk
            })
            .collect();
        state.rebuild_trees();
        state.revision += 1;
        state.structure_revision += 1;
    }

    /// Moves one row, retaining its measurement. `to` is the destination index
    /// after removing `from`, matching `Vec::remove` then `Vec::insert`.
    pub fn move_item(&self, from: usize, to: usize) -> bool {
        let mut state = self.state.borrow_mut();
        let len = state.len();
        if from >= len || from == to || to >= len {
            return false;
        }
        let mut flattened = state
            .chunks
            .iter()
            .flat_map(|chunk| chunk.values.iter().copied())
            .collect::<Vec<_>>();
        let value = flattened.remove(from);
        flattened.insert(to, value);
        state.chunks = flattened
            .chunks(EXTENT_CHUNK_SIZE)
            .map(|values| {
                let mut chunk = ExtentChunk {
                    values: values.to_vec(),
                    extent_sum: 0.,
                    measured: 0,
                };
                chunk.rebuild(state.estimate);
                chunk
            })
            .collect();
        state.rebuild_trees();
        state.revision += 1;
        state.structure_revision += 1;
        true
    }
}

/// Cloneable state for a logical vertical scroll position.
#[derive(Clone, Default)]
pub struct ScrollController {
    state: Rc<RefCell<ScrollState>>,
}
impl std::fmt::Debug for ScrollController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollController")
            .field("offset", &self.offset())
            .field("max_offset", &self.max_offset())
            .finish()
    }
}
impl PartialEq for ScrollController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
#[derive(Clone, Default)]
struct ScrollState {
    offset: f32,
    max_offset: f32,
    content_extent: f32,
    viewport_extent: f32,
    revision: u64,
    restoration: Option<ScrollRestoration>,
    pending_restored_offset: Option<f32>,
}
#[derive(Clone)]
struct ScrollRestoration {
    scope: RestorationScope,
    key: RestorationKey,
}
impl ScrollController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Creates a scroll controller that restores its position, if present.
    ///
    /// The saved logical offset is applied after the first layout establishes
    /// content bounds. The widgets crate's `PageController` alias gains the
    /// same opt-in behavior.
    #[must_use]
    pub fn restored(scope: RestorationScope, key: RestorationKey) -> Self {
        let controller = Self::new();
        controller.bind_restoration(scope, key);
        controller
    }
    /// Binds this controller's logical offset to a stable restoration value.
    ///
    /// Binding a second scope replaces the previous binding. No value is
    /// written until a position actually changes, so defaults remain defaults
    /// when an application never scrolls the viewport.
    pub fn bind_restoration(&self, scope: RestorationScope, key: RestorationKey) {
        let pending_restored_offset = scope.get_json(&key).and_then(restored_scroll_offset);
        let mut state = self.state.borrow_mut();
        state.restoration = Some(ScrollRestoration { scope, key });
        state.pending_restored_offset = pending_restored_offset;
    }
    /// Stops persisting subsequent position changes without removing the
    /// already-stored restoration value.
    pub fn unbind_restoration(&self) {
        let mut state = self.state.borrow_mut();
        state.restoration = None;
        state.pending_restored_offset = None;
    }
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.state.borrow().offset
    }
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        self.state.borrow().max_offset
    }
    #[must_use]
    pub fn content_extent(&self) -> f32 {
        self.state.borrow().content_extent
    }
    #[must_use]
    pub fn viewport_extent(&self) -> f32 {
        self.state.borrow().viewport_extent
    }
    /// Moves the offset after clamping it to the current content bounds.
    pub fn jump_to(&self, offset: f32) -> bool {
        let (restoration, value) = {
            let mut state = self.state.borrow_mut();
            if !offset.is_finite() {
                return false;
            }
            let value = offset.clamp(0., state.max_offset);
            if value == state.offset {
                return false;
            }
            state.offset = value;
            state.pending_restored_offset = None;
            state.revision += 1;
            (state.restoration.clone(), value)
        };
        persist_scroll_offset(restoration, value);
        true
    }
    pub fn scroll_by(&self, delta: f32) -> bool {
        self.jump_to(self.offset() + delta)
    }
    /// Applies one unified scroll policy step. Clamping is persisted normally;
    /// transient bounce positions stay in memory and return through
    /// [`ScrollPhysics::spring_step`] rather than being restored as a logical
    /// position.
    pub fn apply_physics(&self, physics: ScrollPhysics, delta: f32) -> ScrollDelta {
        let result = physics.apply_delta(self.offset(), delta, 0., self.max_offset());
        if result.position == self.offset() {
            return result;
        }
        let persistence = {
            let mut state = self.state.borrow_mut();
            state.offset = result.position;
            state.pending_restored_offset = None;
            state.revision += 1;
            matches!(physics.boundary, BoundaryPhysics::Clamping)
                .then(|| (state.restoration.clone(), result.position))
        };
        if let Some((restoration, offset)) = persistence {
            persist_scroll_offset(restoration, offset);
        }
        result
    }

    /// Stores a spring result produced with a monotonic frame duration.
    pub fn apply_spring_step(&self, step: ScrollSpringStep) -> bool {
        if !step.position.is_finite() || step.position == self.offset() {
            return false;
        }
        let mut state = self.state.borrow_mut();
        state.offset = step.position;
        state.revision += 1;
        true
    }
    /// Updates content and viewport extents after a viewport layout pass.
    ///
    /// The method is public so independent viewport implementations can share
    /// a controller; applications normally use `jump_to` or `scroll_by`.
    pub fn update_extents(&self, content: f32, viewport: f32) {
        let persistence = {
            let mut state = self.state.borrow_mut();
            state.content_extent = content.max(0.);
            state.viewport_extent = viewport.max(0.);
            state.max_offset = (state.content_extent - state.viewport_extent).max(0.);
            if let Some(restored) = state.pending_restored_offset {
                let next = restored.min(state.max_offset);
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                }
                // Keep a larger requested position pending while asynchronous
                // content grows, instead of overwriting the only snapshot
                // with a temporary short-content clamp.
                if restored <= state.max_offset {
                    state.pending_restored_offset = None;
                }
                None
            } else {
                let next = state.offset.min(state.max_offset);
                if next != state.offset {
                    state.offset = next;
                    state.revision += 1;
                    Some((state.restoration.clone(), next))
                } else {
                    None
                }
            }
        };
        if let Some((restoration, offset)) = persistence {
            persist_scroll_offset(restoration, offset);
        }
    }
}

fn restored_scroll_offset(value: serde_json::Value) -> Option<f32> {
    let offset = value.get("offset")?.as_f64()? as f32;
    (offset.is_finite() && offset >= 0.).then_some(offset)
}

fn persist_scroll_offset(restoration: Option<ScrollRestoration>, offset: f32) {
    if let Some(restoration) = restoration
        && offset.is_finite()
        && offset >= 0.
    {
        restoration
            .scope
            .set_json(&restoration.key, serde_json::json!({ "offset": offset }));
    }
}

/// Visual configuration for a logical vertical overlay scrollbar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarStyle {
    pub width: f32,
    pub min_thumb_extent: f32,
    pub track_color: Color,
    pub thumb_color: Color,
}
impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            width: 10.,
            min_thumb_extent: 24.,
            track_color: Color::rgba(20, 22, 30, 120),
            thumb_color: Color::rgba(170, 180, 205, 190),
        }
    }
}

/// Deterministic geometry for an overlay scrollbar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarGeometry {
    pub visible: bool,
    pub track: Rect,
    pub thumb: Rect,
    pub max_scroll_extent: f32,
    pub thumb_travel: f32,
}
impl ScrollbarGeometry {
    #[must_use]
    pub fn thumb_top_for_offset(self, offset: f32) -> f32 {
        let normalized = if self.max_scroll_extent > 0. && offset.is_finite() {
            (offset / self.max_scroll_extent).clamp(0., 1.)
        } else {
            0.
        };
        self.track.origin.y + normalized * self.thumb_travel
    }
    #[must_use]
    pub fn offset_for_thumb_top(self, thumb_top: f32) -> f32 {
        if self.thumb_travel <= 0. || self.max_scroll_extent <= 0. || !thumb_top.is_finite() {
            return 0.;
        }
        ((thumb_top - self.track.origin.y).clamp(0., self.thumb_travel) / self.thumb_travel)
            * self.max_scroll_extent
    }
}

/// Derives display geometry without creating renderer state.
#[must_use]
pub fn scrollbar_geometry(
    size: Size,
    controller: &ScrollController,
    style: ScrollbarStyle,
) -> ScrollbarGeometry {
    let viewport = controller.viewport_extent();
    let content = controller.content_extent();
    if !viewport.is_finite()
        || !content.is_finite()
        || content <= viewport
        || viewport <= 0.
        || !size.height.is_finite()
        || size.height <= 0.
    {
        return ScrollbarGeometry::default();
    }
    let width = style.width.min(size.width).max(0.);
    let track = Rect::from_origin_size(
        Offset::new(size.width - width, 0.),
        Size::new(width, size.height),
    );
    let extent = (size.height * (viewport / content))
        .clamp(style.min_thumb_extent.min(size.height), size.height);
    let mut geometry = ScrollbarGeometry {
        visible: true,
        track,
        thumb: Rect::from_origin_size(Offset::new(track.origin.x, 0.), Size::new(width, extent)),
        max_scroll_extent: controller.max_offset().max(0.),
        thumb_travel: (size.height - extent).max(0.),
    };
    geometry.thumb.origin.y = geometry.thumb_top_for_offset(controller.offset());
    geometry
}

/// The default clamping policy used by native desktop/mobile views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClampingScrollPhysics;
impl ClampingScrollPhysics {
    #[must_use]
    pub fn apply(self, current: f32, delta: f32, min: f32, max: f32) -> f32 {
        (current + delta).clamp(min.min(max), max.max(min))
    }
}

/// Whether a policy participates in a gesture when there is no scroll range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scrollability {
    /// Accept a gesture only when a position can move.
    #[default]
    WhenScrollable,
    /// Accept a gesture even for a zero-length range (useful for pull effects).
    Always,
    /// Never consume a gesture.
    Never,
}

/// Boundary behavior for the unified [`ScrollPhysics`] policy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BoundaryPhysics {
    /// Clamp at the content bounds and return the unused delta to a parent
    /// coordinator.
    #[default]
    Clamping,
    /// Permit a finite, resistant visual overscroll. Call
    /// [`ScrollPhysics::spring_step`] with monotonic frame deltas to return it
    /// to the nearest content bound.
    Bouncing {
        resistance: f32,
        max_overscroll: f32,
        spring: f32,
        damping: f32,
    },
}

/// Optional deterministic settle target after wheel/drag momentum ends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SnapPhysics {
    #[default]
    None,
    Page {
        extent: f32,
    },
    FixedExtent {
        extent: f32,
    },
}

/// Result of one policy application. `unconsumed` is deliberately explicit so
/// nested viewports transfer a delta once, rather than applying it to every
/// ancestor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollDelta {
    pub position: f32,
    pub consumed: f32,
    pub unconsumed: f32,
    pub overscroll: f32,
    pub accepted: bool,
}

/// One monotonic-frame spring integration result for bouncing scroll.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollSpringStep {
    pub position: f32,
    pub velocity: f32,
    pub settled: bool,
}

/// Rust-native composable scroll policy. It replaces a hierarchy of widget
/// subclasses with independent scrollability, boundary, and snap choices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollPhysics {
    pub scrollability: Scrollability,
    pub boundary: BoundaryPhysics,
    pub snap: SnapPhysics,
}

impl Default for ScrollPhysics {
    fn default() -> Self {
        Self::clamping()
    }
}

impl ScrollPhysics {
    #[must_use]
    pub const fn clamping() -> Self {
        Self {
            scrollability: Scrollability::WhenScrollable,
            boundary: BoundaryPhysics::Clamping,
            snap: SnapPhysics::None,
        }
    }

    #[must_use]
    pub const fn always_scrollable(mut self) -> Self {
        self.scrollability = Scrollability::Always;
        self
    }

    #[must_use]
    pub const fn never_scrollable(mut self) -> Self {
        self.scrollability = Scrollability::Never;
        self
    }

    #[must_use]
    pub const fn bouncing(mut self) -> Self {
        self.boundary = BoundaryPhysics::Bouncing {
            resistance: 0.5,
            max_overscroll: 160.,
            spring: 220.,
            damping: 28.,
        };
        self
    }

    #[must_use]
    pub const fn page_snapping(mut self, extent: f32) -> Self {
        self.snap = SnapPhysics::Page { extent };
        self
    }

    #[must_use]
    pub const fn fixed_extent_snapping(mut self, extent: f32) -> Self {
        self.snap = SnapPhysics::FixedExtent { extent };
        self
    }

    /// Applies a raw logical delta. Bounds are normalized, non-finite input is
    /// rejected, and clamping returns exactly the unused portion for nesting.
    #[must_use]
    pub fn apply_delta(self, current: f32, delta: f32, min: f32, max: f32) -> ScrollDelta {
        if !current.is_finite() || !delta.is_finite() {
            return ScrollDelta {
                position: current,
                unconsumed: delta,
                ..ScrollDelta::default()
            };
        }
        if self.scrollability == Scrollability::Never {
            return ScrollDelta {
                position: current,
                unconsumed: delta,
                ..ScrollDelta::default()
            };
        }
        let (min, max) = (min.min(max), max.max(min));
        if min == max && self.scrollability == Scrollability::WhenScrollable {
            return ScrollDelta {
                position: current.clamp(min, max),
                unconsumed: delta,
                ..ScrollDelta::default()
            };
        }
        match self.boundary {
            BoundaryPhysics::Clamping => {
                let position = (current + delta).clamp(min, max);
                let consumed = position - current;
                ScrollDelta {
                    position,
                    consumed,
                    unconsumed: delta - consumed,
                    accepted: consumed != 0. || self.scrollability == Scrollability::Always,
                    ..ScrollDelta::default()
                }
            }
            BoundaryPhysics::Bouncing {
                resistance,
                max_overscroll,
                ..
            } => {
                let resistance = resistance.clamp(0., 1.);
                let limit = max_overscroll.max(0.);
                let raw = current + delta;
                let position = if raw < min {
                    (min + (raw - min) * resistance).max(min - limit)
                } else if raw > max {
                    (max + (raw - max) * resistance).min(max + limit)
                } else {
                    raw
                };
                let consumed = position - current;
                ScrollDelta {
                    position,
                    consumed,
                    // A resistant boundary owns a sequence (including its
                    // finite visual limit); callers return it with the spring
                    // rather than duplicating the same delta into a parent.
                    unconsumed: 0.,
                    overscroll: if position < min {
                        position - min
                    } else {
                        (position - max).max(0.)
                    },
                    accepted: true,
                }
            }
        }
    }

    /// Selects the deterministic page/fixed-item settle target from current
    /// position and velocity. A positive velocity advances, a negative one
    /// retreats; low velocity picks the nearest item.
    #[must_use]
    pub fn snap_target(self, position: f32, velocity: f32, min: f32, max: f32) -> f32 {
        let extent = match self.snap {
            SnapPhysics::None => return position.clamp(min.min(max), max.max(min)),
            SnapPhysics::Page { extent } | SnapPhysics::FixedExtent { extent } => extent,
        };
        if !extent.is_finite() || extent <= 0. {
            return position.clamp(min.min(max), max.max(min));
        }
        let unit = position / extent;
        let index = if velocity > 120. {
            unit.ceil()
        } else if velocity < -120. {
            unit.floor()
        } else {
            unit.round()
        };
        (index * extent).clamp(min.min(max), max.max(min))
    }

    /// Advances a bounce spring using a monotonic elapsed frame duration in
    /// seconds. Runtime schedulers own when to call this; no sleep or executor
    /// timing is hidden in the policy.
    #[must_use]
    pub fn spring_step(
        self,
        position: f32,
        velocity: f32,
        elapsed_seconds: f32,
        min: f32,
        max: f32,
    ) -> ScrollSpringStep {
        let BoundaryPhysics::Bouncing {
            spring, damping, ..
        } = self.boundary
        else {
            return ScrollSpringStep {
                position: position.clamp(min.min(max), max.max(min)),
                velocity: 0.,
                settled: true,
            };
        };
        let target = position.clamp(min.min(max), max.max(min));
        let dt = elapsed_seconds.clamp(0., 0.05);
        let next_velocity = (velocity + (target - position) * spring * dt) * (-damping * dt).exp();
        let next_position = position + next_velocity * dt;
        let settled = (next_position - target).abs() < 0.01 && next_velocity.abs() < 0.01;
        ScrollSpringStep {
            position: if settled { target } else { next_position },
            velocity: if settled { 0. } else { next_velocity },
            settled,
        }
    }

    #[must_use]
    pub fn parent(mut self, parent: Self) -> Self {
        if self.scrollability == Scrollability::WhenScrollable {
            self.scrollability = parent.scrollability;
        }
        if self.boundary == BoundaryPhysics::Clamping {
            self.boundary = parent.boundary;
        }
        if self.snap == SnapPhysics::None {
            self.snap = parent.snap;
        }
        self
    }

    #[must_use]
    pub fn then(self, next: Self) -> Self {
        next.parent(self)
    }

    #[must_use]
    pub const fn range_maintaining(self) -> Self {
        self
    }

    #[must_use]
    pub const fn carousel(mut self, item_extent: f32) -> Self {
        self.snap = SnapPhysics::FixedExtent {
            extent: item_extent,
        };
        self
    }

    #[must_use]
    pub const fn min_fling_velocity(&self) -> f32 {
        50.0
    }

    #[must_use]
    pub const fn max_fling_velocity(&self) -> f32 {
        8000.0
    }

    #[must_use]
    pub const fn drag_start_distance_motion_threshold(&self) -> f32 {
        3.5
    }
}

/// A read-only snapshot of current scroll geometry and extents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollMetrics {
    pub pixels: f32,
    pub min_scroll_extent: f32,
    pub max_scroll_extent: f32,
    pub viewport_dimension: f32,
    pub axis: incular_config::Axis,
    pub axis_direction: incular_config::AxisDirection,
    pub device_pixel_ratio: f32,
}

impl ScrollMetrics {
    #[must_use]
    pub fn extent_before(&self) -> f32 {
        (self.pixels - self.min_scroll_extent).max(0.0)
    }

    #[must_use]
    pub fn extent_inside(&self) -> f32 {
        self.viewport_dimension
    }

    #[must_use]
    pub fn extent_after(&self) -> f32 {
        (self.max_scroll_extent - self.pixels).max(0.0)
    }

    #[must_use]
    pub fn extent_total(&self) -> f32 {
        self.max_scroll_extent - self.min_scroll_extent + self.viewport_dimension
    }

    #[must_use]
    pub fn at_edge(&self) -> bool {
        self.pixels <= self.min_scroll_extent || self.pixels >= self.max_scroll_extent
    }

    #[must_use]
    pub fn out_of_range(&self) -> bool {
        self.pixels < self.min_scroll_extent || self.pixels > self.max_scroll_extent
    }
}

/// Cache extent strategy for pre-rendering lazy list items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollCacheExtent {
    Pixels(f32),
    Viewport(f32),
}

impl ScrollCacheExtent {
    #[must_use]
    pub fn to_pixels(self, viewport_dimension: f32) -> f32 {
        match self {
            Self::Pixels(px) => px.max(0.0),
            Self::Viewport(vp) => (vp * viewport_dimension).max(0.0),
        }
    }
}

/// How a scroll view dismisses the virtual keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollViewKeyboardDismissBehavior {
    #[default]
    Manual,
    OnDrag,
}

/// Determines when a drag gesture begins recognizing motion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DragStartBehavior {
    #[default]
    Start,
    Down,
}

/// Authoritative shared configuration for all scroll view widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollViewConfig {
    pub direction: incular_config::Axis,
    pub reverse: bool,
    pub primary: Option<bool>,
    pub physics: ScrollPhysics,
    pub shrink_wrap: bool,
    pub padding: Option<incular_config::EdgeInsets>,
    pub scroll_cache_extent: Option<ScrollCacheExtent>,
    pub semantic_child_count: Option<usize>,
    pub drag_start_behavior: DragStartBehavior,
    pub keyboard_dismiss_behavior: Option<ScrollViewKeyboardDismissBehavior>,
    pub restoration_id: Option<String>,
    pub clip_behavior: incular_config::Clip,
}

impl Default for ScrollViewConfig {
    fn default() -> Self {
        Self {
            direction: incular_config::Axis::Vertical,
            reverse: false,
            primary: None,
            physics: ScrollPhysics::clamping(),
            shrink_wrap: false,
            padding: None,
            scroll_cache_extent: None,
            semantic_child_count: None,
            drag_start_behavior: DragStartBehavior::Start,
            keyboard_dismiss_behavior: None,
            restoration_id: None,
            clip_behavior: incular_config::Clip::HardEdge,
        }
    }
}

/// General nested scrolling coordinator. Controllers are ordered innermost to
/// outermost. A clamped child consumes only what it can, and the precise
/// leftover propagates once to its parent.
#[derive(Clone, Debug, Default)]
pub struct NestedScrollCoordinator {
    controllers: Vec<ScrollController>,
    active: Option<usize>,
}

impl NestedScrollCoordinator {
    #[must_use]
    pub fn new(innermost_first: impl IntoIterator<Item = ScrollController>) -> Self {
        Self {
            controllers: innermost_first.into_iter().collect(),
            active: None,
        }
    }

    #[must_use]
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// Transfers a wheel delta without duplication.
    pub fn apply_delta(&mut self, delta: f32) -> ScrollDelta {
        let mut remaining = delta;
        let mut total = 0.;
        self.active = None;
        for (index, controller) in self.controllers.iter().enumerate() {
            let result = ScrollPhysics::clamping().apply_delta(
                controller.offset(),
                remaining,
                0.,
                controller.max_offset(),
            );
            if result.consumed != 0. {
                controller.jump_to(result.position);
                total += result.consumed;
                self.active = Some(index);
            }
            remaining = result.unconsumed;
            if remaining == 0. {
                break;
            }
        }
        ScrollDelta {
            position: total,
            consumed: total,
            unconsumed: remaining,
            accepted: total != 0.,
            ..ScrollDelta::default()
        }
    }

    /// Touch-drag entry point. The sign convention is the same logical
    /// content-offset convention as wheel input.
    pub fn apply_drag_delta(&mut self, delta: f32) -> ScrollDelta {
        self.apply_delta(delta)
    }

    /// Momentum entry point. The coordinator keeps the receiving viewport as
    /// `active_index` until the caller settles or cancels that sequence.
    pub fn apply_momentum_delta(&mut self, delta: f32) -> ScrollDelta {
        self.apply_delta(delta)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    use serde_json::{Value, json};

    use super::*;

    #[derive(Default)]
    struct MemoryRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, Value>>);

    impl incular_core::RestorationBackend for MemoryRestorationBackend {
        fn read_value(&self, path: &[RestorationKey]) -> Option<Value> {
            self.0.borrow().get(path).cloned()
        }

        fn write_value(&self, path: &[RestorationKey], value: Value) {
            self.0.borrow_mut().insert(path.to_vec(), value);
        }

        fn remove_value(&self, path: &[RestorationKey]) {
            self.0.borrow_mut().remove(path);
        }
    }

    fn restoration_key(value: &str) -> RestorationKey {
        RestorationKey::new(value).unwrap()
    }

    fn restoration_scope() -> RestorationScope {
        RestorationScope::root(Rc::new(MemoryRestorationBackend::default()))
            .child_unchecked(restoration_key("window"))
            .child_unchecked(restoration_key("main"))
    }

    #[test]
    fn extents_clamp_an_existing_offset() {
        let controller = ScrollController::new();
        controller.update_extents(100., 20.);
        assert!(controller.jump_to(80.));
        controller.update_extents(40., 20.);
        assert_eq!(controller.offset(), 20.);
    }
    #[test]
    fn scrollbar_round_trips_offset() {
        let controller = ScrollController::new();
        controller.update_extents(200., 100.);
        controller.jump_to(50.);
        let geometry = scrollbar_geometry(
            Size::new(100., 100.),
            &controller,
            ScrollbarStyle::default(),
        );
        assert!(geometry.visible);
        assert!((geometry.offset_for_thumb_top(geometry.thumb.origin.y) - 50.).abs() < 0.001);
    }

    #[test]
    fn restored_offsets_wait_for_layout_and_survive_temporary_short_content() {
        let scope = restoration_scope();
        let key = restoration_key("sidebar");
        scope.set_json(&key, json!({ "offset": 80. }));

        let controller = ScrollController::restored(scope.clone(), key.clone());
        assert_eq!(controller.offset(), 0.);

        controller.update_extents(40., 20.);
        assert_eq!(controller.offset(), 20.);
        assert_eq!(scope.get_json(&key), Some(json!({ "offset": 80. })));

        controller.update_extents(120., 20.);
        assert_eq!(controller.offset(), 80.);
        assert!(controller.jump_to(50.));
        assert_eq!(scope.get_json(&key), Some(json!({ "offset": 50. })));
    }

    #[test]
    fn measured_extent_index_seeks_a_million_unmeasured_rows_without_prefix_work() {
        let index = MeasuredExtentIndex::new(1_000_000, 40.);
        assert_eq!(index.index_at_offset(36_000_012.), Some(900_000));
        assert_eq!(index.offset_for_index(900_000), 36_000_000.);
        assert_eq!(
            index.materialized_range(36_000_000., 600., 240.),
            899_994..900_022
        );
        // Only rows directly corrected by layout become measured; seeking did
        // not manufacture a prefix of 900k measurements.
        assert_eq!(index.measured_count(), 0);
    }

    #[test]
    fn measured_extent_index_updates_prefix_and_preserves_other_measurements() {
        let index = MeasuredExtentIndex::new(8, 10.);
        assert!(index.set_measured_extent(2, 25.));
        assert_eq!(index.offset_for_index(3), 45.);
        assert_eq!(index.index_at_offset(44.9), Some(2));
        assert_eq!(index.index_at_offset(45.), Some(3));
        assert!(index.invalidate_extent(2));
        assert_eq!(index.offset_for_index(3), 30.);
    }

    #[test]
    fn measured_extent_index_structural_mutations_retain_and_invalidate_mapping() {
        let index = MeasuredExtentIndex::new(4, 10.);
        index.set_measured_extent(1, 30.);
        index.insert(1, 2);
        assert_eq!(index.len(), 6);
        assert_eq!(index.offset_for_index(4), 60.);
        assert!(index.move_item(3, 0));
        assert_eq!(index.offset_for_index(1), 30.);
        index.remove(0..2);
        assert_eq!(index.len(), 4);
        assert_eq!(index.measured_count(), 0);
        assert!(index.structure_revision() >= 3);
    }

    #[test]
    fn clamping_and_scrollability_policies_return_precise_unused_delta() {
        let clamped = ScrollPhysics::clamping().apply_delta(95., 20., 0., 100.);
        assert_eq!(clamped.position, 100.);
        assert_eq!(clamped.consumed, 5.);
        assert_eq!(clamped.unconsumed, 15.);

        let never = ScrollPhysics::clamping()
            .never_scrollable()
            .apply_delta(0., 10., 0., 100.);
        assert!(!never.accepted);
        assert_eq!(never.unconsumed, 10.);

        let always = ScrollPhysics::clamping()
            .always_scrollable()
            .apply_delta(0., 10., 0., 0.);
        assert!(always.accepted);
        assert_eq!(always.unconsumed, 10.);
    }

    #[test]
    fn bouncing_is_resistant_bounded_and_returns_with_monotonic_spring_steps() {
        let physics = ScrollPhysics::clamping().bouncing();
        let bounced = physics.apply_delta(0., -1_000., 0., 100.);
        assert!(bounced.position < 0.);
        assert!(bounced.position >= -160.);
        assert_eq!(bounced.unconsumed, 0.);
        let mut position = bounced.position;
        let mut velocity = 0.;
        for _ in 0..240 {
            let step = physics.spring_step(position, velocity, 1. / 120., 0., 100.);
            position = step.position;
            velocity = step.velocity;
            if step.settled {
                break;
            }
        }
        assert!((position - 0.).abs() < 0.01);
        let controller = ScrollController::new();
        controller.update_extents(200., 100.);
        let result = controller.apply_physics(physics, -20.);
        assert!(result.position < 0.);
        assert!(controller.offset() < 0.);
    }

    #[test]
    fn page_and_fixed_extent_snap_targets_are_velocity_deterministic() {
        let page = ScrollPhysics::clamping().page_snapping(100.);
        assert_eq!(page.snap_target(149., 0., 0., 500.), 100.);
        assert_eq!(page.snap_target(149., 500., 0., 500.), 200.);
        let fixed = ScrollPhysics::clamping().fixed_extent_snapping(32.);
        assert_eq!(fixed.snap_target(70., -500., 0., 320.), 64.);
    }

    #[test]
    fn nested_coordinator_transfers_only_unconsumed_delta_to_the_outer_viewport() {
        let inner = ScrollController::new();
        let outer = ScrollController::new();
        inner.update_extents(300., 100.);
        outer.update_extents(1_000., 100.);
        inner.jump_to(0.);
        outer.jump_to(200.);
        let mut coordinator = NestedScrollCoordinator::new([inner.clone(), outer.clone()]);
        let result = coordinator.apply_drag_delta(-50.);
        assert_eq!(inner.offset(), 0.);
        assert_eq!(outer.offset(), 150.);
        assert_eq!(result.consumed, -50.);
        assert_eq!(result.unconsumed, 0.);
        assert_eq!(coordinator.active_index(), Some(1));

        let result = coordinator.apply_momentum_delta(250.);
        assert_eq!(inner.offset(), 200.);
        assert_eq!(outer.offset(), 200.);
        assert_eq!(result.unconsumed, 0.);
        assert_eq!(coordinator.active_index(), Some(1));
        coordinator.cancel();
        assert_eq!(coordinator.active_index(), None);
    }
}
