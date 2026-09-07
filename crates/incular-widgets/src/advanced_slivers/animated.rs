use super::*;

const DEFAULT_ANIMATION_DURATION: Duration = Duration::from_millis(300);
const DEFAULT_ITEM_EXTENT: f32 = 48.0;

/// The lifecycle phase of one retained animated collection item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimatedItemPhase {
    Stable,
    Incoming,
    Outgoing,
}

/// A frame-stable snapshot passed to animated collection builders.
///
/// `id` remains stable across insertion/removal index shifts.  `index` is the
/// current logical index for live items and the original logical index for an
/// outgoing item, which lets a removed-item builder keep rendering its old
/// content while the collection is already reporting its shorter length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimatedItem {
    pub id: u64,
    pub index: usize,
    pub value: f32,
    pub phase: AnimatedItemPhase,
}

impl AnimatedItem {
    #[must_use]
    pub fn is_incoming(self) -> bool {
        self.phase == AnimatedItemPhase::Incoming
    }

    #[must_use]
    pub fn is_outgoing(self) -> bool {
        self.phase == AnimatedItemPhase::Outgoing
    }

    #[must_use]
    pub fn is_stable(self) -> bool {
        self.phase == AnimatedItemPhase::Stable
    }
}

pub type AnimatedItemBuilder = Rc<dyn Fn(usize, AnimatedItem) -> Widget>;
pub type AnimatedRemovedItemBuilder = Rc<dyn Fn(AnimatedItem) -> Widget>;

struct AnimatedEntry {
    id: u64,
    animation: AnimationController,
    phase: AnimatedItemPhase,
    original_index: usize,
    removed_builder: Option<AnimatedRemovedItemBuilder>,
}

impl AnimatedEntry {
    fn new(id: u64, duration: Duration) -> Self {
        let animation = AnimationController::new(duration);
        animation.set_value(0.0);
        Self {
            id,
            animation,
            phase: AnimatedItemPhase::Incoming,
            original_index: 0,
            removed_builder: None,
        }
    }
}

struct AnimatedCollectionState {
    entries: Vec<AnimatedEntry>,
    next_id: u64,
    duration: Duration,
    revision: u64,
    structure_revision: u64,
}

impl AnimatedCollectionState {
    fn new(item_count: usize, duration: Duration) -> Self {
        let mut next_id: u64 = 1;
        let entries = (0..item_count)
            .map(|_| {
                let id = next_id;
                next_id = next_id.saturating_add(1).max(1);
                let animation = AnimationController::new(duration);
                animation.set_value(1.0);
                AnimatedEntry {
                    id,
                    animation,
                    phase: AnimatedItemPhase::Stable,
                    original_index: 0,
                    removed_builder: None,
                }
            })
            .collect();
        Self {
            entries,
            next_id,
            duration,
            revision: 1,
            structure_revision: 1,
        }
    }

    fn logical_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.phase != AnimatedItemPhase::Outgoing)
            .count()
    }

    fn physical_index_for_logical(&self, logical_index: usize) -> Option<usize> {
        let mut live_index = 0;
        for (physical_index, entry) in self.entries.iter().enumerate() {
            if entry.phase == AnimatedItemPhase::Outgoing {
                continue;
            }
            if live_index == logical_index {
                return Some(physical_index);
            }
            live_index += 1;
        }
        (logical_index == live_index).then_some(self.entries.len())
    }

    fn snapshots(&self) -> Vec<AnimatedItem> {
        let mut logical_index = 0;
        self.entries
            .iter()
            .map(|entry| {
                let index = if entry.phase == AnimatedItemPhase::Outgoing {
                    entry.original_index
                } else {
                    let index = logical_index;
                    logical_index += 1;
                    index
                };
                AnimatedItem {
                    id: entry.id,
                    index,
                    value: entry.animation.value().clamp(0.0, 1.0),
                    phase: entry.phase,
                }
            })
            .collect()
    }
}

/// Shared insertion/removal state for `AnimatedList`, `AnimatedGrid`, and
/// `SliverAnimatedGrid`.
#[derive(Clone)]
pub struct AnimatedCollectionController {
    state: Rc<RefCell<AnimatedCollectionState>>,
}

impl AnimatedCollectionController {
    #[must_use]
    pub fn new(item_count: usize) -> Self {
        Self::with_duration(item_count, DEFAULT_ANIMATION_DURATION)
    }

    #[must_use]
    pub fn with_duration(item_count: usize, duration: Duration) -> Self {
        Self {
            state: Rc::new(RefCell::new(AnimatedCollectionState::new(
                item_count, duration,
            ))),
        }
    }

    #[must_use]
    pub fn item_count(&self) -> usize {
        self.state.borrow().logical_count()
    }

    #[must_use]
    pub fn physical_item_count(&self) -> usize {
        self.state.borrow().entries.len()
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.state.borrow().duration
    }

    pub fn set_duration(&self, duration: Duration) {
        let mut state = self.state.borrow_mut();
        state.duration = duration;
        for entry in &state.entries {
            entry.animation.set_duration(duration);
        }
        state.revision = state.revision.wrapping_add(1);
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    #[must_use]
    pub fn structure_revision(&self) -> u64 {
        self.state.borrow().structure_revision
    }

    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.state
            .borrow()
            .entries
            .iter()
            .any(|entry| entry.animation.is_active())
    }

    /// Returns snapshots in physical paint order. Outgoing snapshots remain
    /// present until their reverse animation reaches zero.
    #[must_use]
    pub fn entries(&self) -> Vec<AnimatedItem> {
        self.state.borrow().snapshots()
    }

    #[must_use]
    pub fn item_at(&self, index: usize) -> Option<AnimatedItem> {
        self.state
            .borrow()
            .snapshots()
            .into_iter()
            .filter(|entry| !entry.is_outgoing())
            .nth(index)
    }

    #[must_use]
    pub fn animation_for_id(&self, id: u64) -> Option<AnimationController> {
        self.state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.animation.clone())
    }

    #[must_use]
    pub fn animation_for_index(&self, index: usize) -> Option<AnimationController> {
        self.item_at(index)
            .and_then(|entry| self.animation_for_id(entry.id))
    }

    fn removed_builder_for(&self, id: u64) -> Option<AnimatedRemovedItemBuilder> {
        self.state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.removed_builder.clone())
    }

    /// Inserts a new item before the live logical item at `index`.
    pub fn insert(&self, index: usize, now: Instant) -> u64 {
        self.insert_with_duration(index, now, self.duration())
    }

    pub fn insert_with_duration(&self, index: usize, now: Instant, duration: Duration) -> u64 {
        let mut state = self.state.borrow_mut();
        let logical_index = index.min(state.logical_count());
        let physical_index = state
            .physical_index_for_logical(logical_index)
            .unwrap_or(state.entries.len());
        for entry in &mut state.entries {
            if entry.phase == AnimatedItemPhase::Outgoing && entry.original_index >= logical_index {
                entry.original_index = entry.original_index.saturating_add(1);
            }
        }
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1).max(1);
        let mut entry = AnimatedEntry::new(id, duration);
        entry.original_index = logical_index;
        entry.animation.forward(now);
        state.entries.insert(physical_index, entry);
        state.revision = state.revision.wrapping_add(1);
        state.structure_revision = state.structure_revision.wrapping_add(1);
        id
    }

    pub fn insert_at(&self, index: usize, now: Instant) -> u64 {
        self.insert(index, now)
    }

    pub fn insert_all(&self, index: usize, count: usize, now: Instant) -> Vec<u64> {
        let mut inserted = Vec::new();
        let mut target = index;
        for _ in 0..count {
            inserted.push(self.insert(target, now));
            target = target.saturating_add(1);
        }
        inserted
    }

    pub fn insert_item(&self, index: usize) -> u64 {
        self.insert(index, Instant::now())
    }

    /// Starts an outgoing animation for the live item at `index`. Its slot
    /// remains in the physical list until `tick` observes completion.
    pub fn remove(&self, index: usize, now: Instant) -> Option<u64> {
        self.remove_with_builder(index, now, None)
    }

    pub fn remove_at(&self, index: usize, now: Instant) -> Option<u64> {
        self.remove(index, now)
    }

    pub fn remove_item(&self, index: usize) -> Option<u64> {
        self.remove(index, Instant::now())
    }

    pub fn remove_with_builder(
        &self,
        index: usize,
        now: Instant,
        removed_builder: Option<AnimatedRemovedItemBuilder>,
    ) -> Option<u64> {
        let mut state = self.state.borrow_mut();
        let physical_index = state.physical_index_for_logical(index)?;
        let duration = state.duration;
        let id = {
            for existing in &mut state.entries {
                if existing.phase == AnimatedItemPhase::Outgoing && existing.original_index >= index
                {
                    existing.original_index = existing.original_index.saturating_sub(1);
                }
            }
            let entry = state.entries.get_mut(physical_index)?;
            if entry.phase == AnimatedItemPhase::Outgoing {
                return None;
            }
            entry.original_index = index;
            entry.removed_builder = removed_builder;
            entry.phase = AnimatedItemPhase::Outgoing;
            entry.animation.set_duration(duration);
            entry.animation.reverse(now);
            entry.id
        };
        state.revision = state.revision.wrapping_add(1);
        Some(id)
    }

    pub fn remove_at_with_builder<W, F>(
        &self,
        index: usize,
        now: Instant,
        removed_builder: F,
    ) -> Option<u64>
    where
        W: Into<Widget> + 'static,
        F: Fn(AnimatedItem) -> W + 'static,
    {
        self.remove_with_builder(
            index,
            now,
            Some(Rc::new(move |item| removed_builder(item).into())),
        )
    }

    pub fn remove_all(&self, now: Instant) -> Vec<u64> {
        let count = self.item_count();
        (0..count)
            .rev()
            .filter_map(|index| self.remove(index, now))
            .collect()
    }

    pub fn remove_all_with_builder<W, F>(&self, now: Instant, removed_builder: F) -> Vec<u64>
    where
        W: Into<Widget> + 'static,
        F: Fn(AnimatedItem) -> W + Clone + 'static,
    {
        let count = self.item_count();
        (0..count)
            .rev()
            .filter_map(|index| {
                let builder = removed_builder.clone();
                self.remove_at_with_builder(index, now, builder)
            })
            .collect()
    }

    /// Advances every retained item and removes outgoing slots only after
    /// their reverse animation has reached zero.
    pub fn tick(&self, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        let mut changed = false;
        for entry in &mut state.entries {
            changed |= entry.animation.tick(now);
            if entry.phase == AnimatedItemPhase::Incoming
                && !entry.animation.is_active()
                && entry.animation.value() >= 1.0
            {
                entry.phase = AnimatedItemPhase::Stable;
                changed = true;
            }
        }
        let before = state.entries.len();
        state.entries.retain(|entry| {
            !(entry.phase == AnimatedItemPhase::Outgoing
                && !entry.animation.is_active()
                && entry.animation.value() <= 0.0)
        });
        if state.entries.len() != before {
            state.structure_revision = state.structure_revision.wrapping_add(1);
            changed = true;
        }
        if changed {
            state.revision = state.revision.wrapping_add(1);
        }
        changed
    }
}

pub type AnimatedListController = AnimatedCollectionController;
pub type AnimatedGridController = AnimatedCollectionController;
pub type SliverAnimatedGridController = AnimatedCollectionController;

#[derive(Clone)]
enum CollectionLayout {
    List,
    Grid(SliverGridDelegate),
}

#[derive(Clone)]
struct AnimatedCollectionSliver {
    controller: AnimatedCollectionController,
    builder: AnimatedItemBuilder,
    removed_builder: Option<AnimatedRemovedItemBuilder>,
    layout: CollectionLayout,
}

impl AnimatedCollectionSliver {
    fn list(
        controller: AnimatedCollectionController,
        builder: AnimatedItemBuilder,
        removed_builder: Option<AnimatedRemovedItemBuilder>,
    ) -> Self {
        Self {
            controller,
            builder,
            removed_builder,
            layout: CollectionLayout::List,
        }
    }

    fn grid(
        controller: AnimatedCollectionController,
        delegate: SliverGridDelegate,
        builder: AnimatedItemBuilder,
        removed_builder: Option<AnimatedRemovedItemBuilder>,
    ) -> Self {
        Self {
            controller,
            builder,
            removed_builder,
            layout: CollectionLayout::Grid(delegate),
        }
    }

    fn render(&self) -> AnimatedCollectionRenderSliver {
        AnimatedCollectionRenderSliver::new(
            self.controller.clone(),
            self.builder.clone(),
            self.removed_builder.clone(),
            self.layout.clone(),
        )
    }
}

impl Sliver for AnimatedCollectionSliver {
    fn build(&self, controller: &ScrollController) -> Widget {
        CustomScrollView::new(vec![Box::new(self.clone()) as Box<dyn Sliver>])
            .controller(controller.clone())
            .into()
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(self.render())
    }
}

struct AnimatedCollectionRenderSliver {
    controller: AnimatedCollectionController,
    builder: AnimatedItemBuilder,
    removed_builder: Option<AnimatedRemovedItemBuilder>,
    layout: CollectionLayout,
    entries: Vec<AnimatedItem>,
    index: MeasuredExtentIndex,
    measured_extents: HashMap<u64, f32>,
    widgets: HashMap<u64, Widget>,
    structure_revision: u64,
    estimated_extent: f32,
}

impl AnimatedCollectionRenderSliver {
    fn new(
        controller: AnimatedCollectionController,
        builder: AnimatedItemBuilder,
        removed_builder: Option<AnimatedRemovedItemBuilder>,
        layout: CollectionLayout,
    ) -> Self {
        let count = controller.physical_item_count();
        Self {
            controller,
            builder,
            removed_builder,
            layout,
            entries: Vec::new(),
            index: MeasuredExtentIndex::new(count, DEFAULT_ITEM_EXTENT),
            measured_extents: HashMap::new(),
            widgets: HashMap::new(),
            structure_revision: 0,
            estimated_extent: DEFAULT_ITEM_EXTENT,
        }
    }

    fn sync_entries(&mut self) {
        let next_entries = self.controller.entries();
        let next_ids = next_entries
            .iter()
            .map(|entry| entry.id)
            .collect::<HashSet<_>>();
        self.widgets.retain(|id, _| next_ids.contains(id));
        self.measured_extents.retain(|id, _| next_ids.contains(id));
        if self
            .entries
            .iter()
            .map(|entry| entry.id)
            .ne(next_entries.iter().map(|entry| entry.id))
        {
            self.index = MeasuredExtentIndex::new(next_entries.len(), self.estimated_extent);
            for (position, entry) in next_entries.iter().enumerate() {
                if let Some(extent) = self.measured_extents.get(&entry.id).copied() {
                    let _ = self.index.set_measured_extent(position, extent);
                }
            }
            self.structure_revision = self.structure_revision.wrapping_add(1);
        }
        self.entries = next_entries;
    }

    fn widget_for(&mut self, entry: AnimatedItem) -> Widget {
        let widget = if entry.is_outgoing() {
            self.controller
                .removed_builder_for(entry.id)
                .or_else(|| self.removed_builder.clone())
                .as_ref()
                .map_or_else(
                    || (self.builder)(entry.index, entry),
                    |builder| builder(entry),
                )
        } else {
            (self.builder)(entry.index, entry)
        };
        self.widgets.insert(entry.id, widget.clone());
        widget
    }

    fn list_entry_extent(&self, position: usize) -> f32 {
        let entry = self.entries[position];
        let base_extent = self
            .measured_extents
            .get(&entry.id)
            .copied()
            .unwrap_or(self.estimated_extent)
            .max(0.0);
        if entry.is_outgoing() || entry.is_incoming() {
            base_extent * entry.value
        } else {
            base_extent
        }
    }

    fn list_offset_for(&self, position: usize) -> f32 {
        (0..position)
            .map(|index| self.list_entry_extent(index))
            .sum()
    }

    fn list_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let physical_count = self.entries.len();
        if physical_count == 0 {
            return SliverLayout {
                geometry: SliverGeometry::from_scroll_extent(constraints, 0.0),
                children: Vec::new(),
                absorbed_overlap: 0.0,
            };
        }
        let range = self.index.materialized_range(
            constraints.scroll_offset,
            constraints.remaining_paint_extent,
            constraints.remaining_cache_extent,
        );
        let mut positions = range.collect::<Vec<_>>();
        positions.extend(
            self.entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| entry.is_incoming().then_some(index)),
        );
        positions.sort_unstable();
        positions.dedup();

        let children = positions
            .into_iter()
            .filter_map(|position| {
                let entry = *self.entries.get(position)?;
                let extent = self.list_entry_extent(position);
                let offset = self.list_offset_for(position);
                Some(SliverChildLayout {
                    id: SliverChildId(entry.id),
                    widget: self.widget_for(entry),
                    semantic_index: Some(entry.index),
                    offset,
                    cross_offset: 0.0,
                    constraints: box_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        extent,
                    ),
                    extent,
                    placement: crate::scrolling::SliverChildPlacement::Flow,
                })
            })
            .collect::<Vec<_>>();

        let scroll_extent = (0..self.entries.len())
            .map(|position| self.list_entry_extent(position))
            .sum::<f32>()
            .max(0.0);
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, scroll_extent),
            children,
            absorbed_overlap: 0.0,
        }
    }

    fn grid_layout(
        &mut self,
        constraints: SliverConstraints,
        delegate: &SliverGridDelegate,
    ) -> SliverLayout {
        let metrics = grid_metrics(delegate, constraints.cross_axis_extent);
        let Some((columns, cell_cross_extent, row_extent, cross_spacing, main_spacing)) = metrics
        else {
            return SliverLayout {
                geometry: SliverGeometry::from_scroll_extent(constraints, 0.0),
                children: Vec::new(),
                absorbed_overlap: 0.0,
            };
        };
        let count = self.entries.len();
        if count == 0 {
            return SliverLayout {
                geometry: SliverGeometry::from_scroll_extent(constraints, 0.0),
                children: Vec::new(),
                absorbed_overlap: 0.0,
            };
        }
        let row_step = row_extent + main_spacing;
        let row_count = count.div_ceil(columns);
        let scroll_extent = (row_count as f32 * row_extent
            + row_count.saturating_sub(1) as f32 * main_spacing)
            .max(0.0);
        let start_row = ((constraints.scroll_offset - constraints.remaining_cache_extent.max(0.0))
            / row_step.max(f32::EPSILON))
        .floor()
        .max(0.0) as usize;
        let end_row = ((constraints.scroll_offset
            + constraints.remaining_paint_extent
            + constraints.remaining_cache_extent.max(0.0))
            / row_step.max(f32::EPSILON))
        .ceil()
        .min(row_count as f32) as usize;
        let mut positions = Vec::new();
        for row in start_row.min(row_count)..end_row.max(start_row.min(row_count)).min(row_count) {
            for column in 0..columns {
                let position = row * columns + column;
                if position < count {
                    positions.push(position);
                }
            }
        }
        positions.extend(
            self.entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| entry.is_incoming().then_some(index)),
        );
        positions.sort_unstable();
        positions.dedup();
        let children = positions
            .into_iter()
            .filter_map(|position| {
                let entry = *self.entries.get(position)?;
                let row = position / columns;
                let column = position % columns;
                let child = self.widget_for(entry);
                let widget = match constraints.axis {
                    Axis::Vertical => SizedBox::new()
                        .width(cell_cross_extent)
                        .height(row_extent)
                        .child(child)
                        .into(),
                    Axis::Horizontal => SizedBox::new()
                        .width(row_extent)
                        .height(cell_cross_extent)
                        .child(child)
                        .into(),
                };
                Some(SliverChildLayout {
                    id: SliverChildId(entry.id),
                    widget,
                    semantic_index: Some(entry.index),
                    offset: row as f32 * row_step,
                    cross_offset: column as f32 * (cell_cross_extent + cross_spacing),
                    constraints: box_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        row_extent,
                    ),
                    extent: row_extent,
                    placement: crate::scrolling::SliverChildPlacement::Flow,
                })
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, scroll_extent),
            children,
            absorbed_overlap: 0.0,
        }
    }
}

impl RenderSliver for AnimatedCollectionRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.sync_entries();
        match &self.layout.clone() {
            CollectionLayout::List => self.list_layout(constraints),
            CollectionLayout::Grid(delegate) => self.grid_layout(constraints, delegate),
        }
    }

    fn child_count(&self) -> Option<usize> {
        Some(self.controller.item_count())
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if !extent.is_finite() || extent < 0.0 {
            return false;
        }
        let Some(position) = self.entries.iter().position(|entry| entry.id == child.0) else {
            return false;
        };
        let entry = self.entries[position];
        let base_extent = if entry.value <= f32::EPSILON {
            self.measured_extents
                .get(&entry.id)
                .copied()
                .unwrap_or(self.estimated_extent)
                .max(extent)
        } else if entry.is_incoming() || entry.is_outgoing() {
            extent / entry.value.max(f32::EPSILON)
        } else {
            extent
        };
        if !base_extent.is_finite() || base_extent < 0.0 {
            return false;
        }
        self.estimated_extent = if self.measured_extents.is_empty() {
            base_extent.max(f32::EPSILON)
        } else {
            self.estimated_extent
        };
        let changed = self.measured_extents.insert(child.0, base_extent) != Some(base_extent);
        changed | self.index.set_measured_extent(position, base_extent)
    }

    fn revision(&self) -> u64 {
        self.controller
            .revision()
            .wrapping_add(self.structure_revision)
            .wrapping_add(self.index.revision())
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.controller.tick(now)
    }

    fn is_animating(&self) -> bool {
        self.controller.is_animating()
    }
}

fn box_constraints(axis: Axis, cross_axis_extent: f32, main_extent: f32) -> Constraints {
    let cross_axis_extent = cross_axis_extent.max(0.0);
    let main_extent = main_extent.max(0.0);
    match axis {
        Axis::Vertical => Constraints::new(0.0, cross_axis_extent, main_extent, main_extent),
        Axis::Horizontal => Constraints::new(main_extent, main_extent, 0.0, cross_axis_extent),
    }
}

fn grid_metrics(
    delegate: &SliverGridDelegate,
    available_cross_extent: f32,
) -> Option<(usize, f32, f32, f32, f32)> {
    let available_cross_extent = available_cross_extent.max(0.0);
    let (columns, cross_spacing, main_spacing, main_axis_extent, aspect_ratio) = match delegate {
        SliverGridDelegate::FixedCrossAxisCount {
            cross_axis_count,
            main_axis_spacing,
            cross_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        } => (
            (*cross_axis_count).max(1),
            (*cross_axis_spacing).max(0.0),
            (*main_axis_spacing).max(0.0),
            *main_axis_extent,
            (*child_aspect_ratio).max(f32::EPSILON),
        ),
        SliverGridDelegate::MaxCrossAxisExtent {
            max_cross_axis_extent,
            main_axis_spacing,
            cross_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        } => {
            let cross_spacing = (*cross_axis_spacing).max(0.0);
            let columns = ((available_cross_extent + cross_spacing)
                / ((*max_cross_axis_extent).max(f32::EPSILON) + cross_spacing))
                .floor()
                .max(1.0) as usize;
            (
                columns,
                cross_spacing,
                (*main_axis_spacing).max(0.0),
                *main_axis_extent,
                (*child_aspect_ratio).max(f32::EPSILON),
            )
        }
    };
    let usable_cross =
        (available_cross_extent - cross_spacing * columns.saturating_sub(1) as f32).max(0.0);
    let cell_cross_extent = (usable_cross / columns as f32).max(0.0);
    let row_extent = main_axis_extent
        .unwrap_or_else(|| (cell_cross_extent / aspect_ratio.max(f32::EPSILON)).max(f32::EPSILON));
    row_extent.is_finite().then_some((
        columns,
        cell_cross_extent,
        row_extent,
        cross_spacing,
        main_spacing,
    ))
}

#[derive(Clone)]
pub struct AnimatedList {
    collection: AnimatedCollectionSliver,
    scroll_direction: Axis,
    reverse: bool,
    physics: Option<ScrollPhysics>,
    cache_extent: f32,
    clip_behavior: Clip,
}

impl AnimatedList {
    #[must_use]
    pub fn new<W, F>(item_count: usize, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize) -> W + 'static,
    {
        Self::with_animation_builder(item_count, move |index, _| builder(index))
    }

    #[must_use]
    pub fn animated<W, F>(item_count: usize, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        Self::with_animation_builder(item_count, builder)
    }

    #[must_use]
    pub fn with_animation_builder<W, F>(item_count: usize, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        let controller = AnimatedCollectionController::new(item_count);
        let builder = Rc::new(move |index, item| builder(index, item).into());
        Self {
            collection: AnimatedCollectionSliver::list(controller, builder, None),
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            physics: None,
            cache_extent: WidgetDefaults::DEFAULT.sliver_cache_extent,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: AnimatedListController) -> Self {
        self.collection.controller = controller;
        self
    }

    #[must_use]
    pub fn animation_builder<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        self.collection.builder = Rc::new(move |index, item| builder(index, item).into());
        self
    }

    #[must_use]
    pub fn removed_item_builder<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(AnimatedItem) -> W + 'static,
    {
        self.collection.removed_builder = Some(Rc::new(move |item| builder(item).into()));
        self
    }

    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    #[must_use]
    pub fn cache_extent(mut self, extent: f32) -> Self {
        self.cache_extent = extent.max(0.0);
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    #[must_use]
    pub fn animation_controller(&self) -> AnimatedListController {
        self.collection.controller.clone()
    }

    fn viewport(&self, controller: &ScrollController) -> Widget {
        let mut viewport = CustomScrollView::new(vec![Box::new(self.clone()) as Box<dyn Sliver>])
            .controller(controller.clone())
            .scroll_direction(self.scroll_direction)
            .reverse(self.reverse)
            .cache_extent(self.cache_extent)
            .clip_behavior(self.clip_behavior);
        if let Some(physics) = self.physics {
            viewport = viewport.physics(physics);
        }
        viewport.into()
    }
}

impl Sliver for AnimatedList {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.viewport(controller)
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(self.collection.render())
    }
}

impl From<AnimatedList> for Widget {
    fn from(list: AnimatedList) -> Self {
        let controller = ScrollController::new();
        list.viewport(&controller)
    }
}

#[derive(Clone)]
pub struct AnimatedGrid {
    collection: AnimatedCollectionSliver,
    delegate: SliverGridDelegate,
    scroll_direction: Axis,
    reverse: bool,
    physics: Option<ScrollPhysics>,
    cache_extent: f32,
    clip_behavior: Clip,
}

impl AnimatedGrid {
    #[must_use]
    pub fn new<W, F>(item_count: usize, delegate: SliverGridDelegate, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize) -> W + 'static,
    {
        Self::with_animation_builder(item_count, delegate, move |index, _| builder(index))
    }

    #[must_use]
    pub fn animated<W, F>(item_count: usize, delegate: SliverGridDelegate, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        Self::with_animation_builder(item_count, delegate, builder)
    }

    #[must_use]
    pub fn with_animation_builder<W, F>(
        item_count: usize,
        delegate: SliverGridDelegate,
        builder: F,
    ) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        let controller = AnimatedCollectionController::new(item_count);
        let builder = Rc::new(move |index, item| builder(index, item).into());
        Self {
            collection: AnimatedCollectionSliver::grid(controller, delegate.clone(), builder, None),
            delegate,
            scroll_direction: WidgetDefaults::DEFAULT.scroll_direction,
            reverse: false,
            physics: None,
            cache_extent: WidgetDefaults::DEFAULT.sliver_cache_extent,
            clip_behavior: WidgetDefaults::DEFAULT.scroll_clip_behavior,
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: AnimatedGridController) -> Self {
        self.collection.controller = controller;
        self
    }

    #[must_use]
    pub fn animation_builder<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        self.collection.builder = Rc::new(move |index, item| builder(index, item).into());
        self
    }

    #[must_use]
    pub fn removed_item_builder<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(AnimatedItem) -> W + 'static,
    {
        self.collection.removed_builder = Some(Rc::new(move |item| builder(item).into()));
        self
    }

    #[must_use]
    pub fn delegate(&self) -> &SliverGridDelegate {
        &self.delegate
    }

    #[must_use]
    pub fn scroll_direction(mut self, direction: Axis) -> Self {
        self.scroll_direction = direction;
        self
    }

    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    #[must_use]
    pub fn physics(mut self, physics: ScrollPhysics) -> Self {
        self.physics = Some(physics);
        self
    }

    #[must_use]
    pub fn cache_extent(mut self, extent: f32) -> Self {
        self.cache_extent = extent.max(0.0);
        self
    }

    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    #[must_use]
    pub fn animation_controller(&self) -> AnimatedGridController {
        self.collection.controller.clone()
    }

    fn viewport(&self, controller: &ScrollController) -> Widget {
        let mut viewport = CustomScrollView::new(vec![Box::new(self.clone()) as Box<dyn Sliver>])
            .controller(controller.clone())
            .scroll_direction(self.scroll_direction)
            .reverse(self.reverse)
            .cache_extent(self.cache_extent)
            .clip_behavior(self.clip_behavior);
        if let Some(physics) = self.physics {
            viewport = viewport.physics(physics);
        }
        viewport.into()
    }
}

impl Sliver for AnimatedGrid {
    fn build(&self, controller: &ScrollController) -> Widget {
        self.viewport(controller)
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(self.collection.render())
    }
}

impl From<AnimatedGrid> for Widget {
    fn from(grid: AnimatedGrid) -> Self {
        let controller = ScrollController::new();
        grid.viewport(&controller)
    }
}

/// A sliver-only animated grid descriptor, analogous to Flutter's
/// `SliverAnimatedGrid`.
#[derive(Clone)]
pub struct SliverAnimatedGrid {
    collection: AnimatedCollectionSliver,
    delegate: SliverGridDelegate,
}

impl SliverAnimatedGrid {
    #[must_use]
    pub fn new<W, F>(item_count: usize, delegate: SliverGridDelegate, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize) -> W + 'static,
    {
        Self::with_animation_builder(item_count, delegate, move |index, _| builder(index))
    }

    #[must_use]
    pub fn animated<W, F>(item_count: usize, delegate: SliverGridDelegate, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        Self::with_animation_builder(item_count, delegate, builder)
    }

    #[must_use]
    pub fn with_animation_builder<W, F>(
        item_count: usize,
        delegate: SliverGridDelegate,
        builder: F,
    ) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        let controller = AnimatedCollectionController::new(item_count);
        let builder = Rc::new(move |index, item| builder(index, item).into());
        Self {
            collection: AnimatedCollectionSliver::grid(controller, delegate.clone(), builder, None),
            delegate,
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: SliverAnimatedGridController) -> Self {
        self.collection.controller = controller;
        self
    }

    #[must_use]
    pub fn animation_builder<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(usize, AnimatedItem) -> W + 'static,
    {
        self.collection.builder = Rc::new(move |index, item| builder(index, item).into());
        self
    }

    #[must_use]
    pub fn removed_item_builder<W, F>(mut self, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(AnimatedItem) -> W + 'static,
    {
        self.collection.removed_builder = Some(Rc::new(move |item| builder(item).into()));
        self
    }

    #[must_use]
    pub fn delegate(&self) -> &SliverGridDelegate {
        &self.delegate
    }

    #[must_use]
    pub fn animation_controller(&self) -> SliverAnimatedGridController {
        self.collection.controller.clone()
    }
}

impl Sliver for SliverAnimatedGrid {
    fn build(&self, controller: &ScrollController) -> Widget {
        CustomScrollView::new(vec![Box::new(self.clone()) as Box<dyn Sliver>])
            .controller(controller.clone())
            .into()
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(self.collection.render())
    }
}
