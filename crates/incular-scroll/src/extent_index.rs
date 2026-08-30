use std::{cell::RefCell, rc::Rc};

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

    /// Creates an index with per-item initial estimates. Estimates are used
    /// for seeking and virtualization immediately, then can be replaced by
    /// exact post-layout measurements without changing child identity.
    #[must_use]
    pub fn with_estimates(
        item_count: usize,
        fallback_extent: f32,
        estimate: impl Fn(usize) -> f32,
    ) -> Self {
        let index = Self::new(item_count, fallback_extent);
        for item in 0..item_count {
            let extent = estimate(item);
            if extent.is_finite() && extent >= 0. {
                index.set_measured_extent(item, extent);
            }
        }
        index
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
