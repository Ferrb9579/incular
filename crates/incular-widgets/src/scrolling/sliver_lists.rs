use super::*;

/// Retained state for a reorderable sliver. The order stores logical item
/// identities rather than visible slots, so moving an item preserves its
/// element state and measured extent.
#[derive(Clone)]
pub struct SliverReorderController {
    state: Rc<RefCell<SliverReorderState>>,
}

struct SliverReorderState {
    order: Vec<usize>,
    next_item: usize,
    revision: u64,
}

impl SliverReorderController {
    #[must_use]
    pub fn new(item_count: usize) -> Self {
        Self {
            state: Rc::new(RefCell::new(SliverReorderState {
                order: (0..item_count).collect(),
                next_item: item_count,
                revision: 0,
            })),
        }
    }

    #[must_use]
    pub fn item_count(&self) -> usize {
        self.state.borrow().order.len()
    }

    /// Returns the logical item IDs in their current visual order.
    #[must_use]
    pub fn order(&self) -> Vec<usize> {
        self.state.borrow().order.clone()
    }

    #[must_use]
    pub fn item_at(&self, position: usize) -> Option<usize> {
        self.state.borrow().order.get(position).copied()
    }

    #[must_use]
    pub fn position_of(&self, item: usize) -> Option<usize> {
        self.state
            .borrow()
            .order
            .iter()
            .position(|candidate| *candidate == item)
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    /// Moves `from` to `to`, where `to` is the final position after removal.
    /// This is the same operation used by the retained sliver drop target.
    pub fn move_item(&self, from: usize, to: usize) -> bool {
        let mut state = self.state.borrow_mut();
        if from >= state.order.len() || to >= state.order.len() || from == to {
            return false;
        }
        let item = state.order.remove(from);
        state.order.insert(to, item);
        state.revision = state.revision.wrapping_add(1);
        true
    }

    /// Applies Flutter's `onReorder(oldIndex, newIndex)` convention. Flutter
    /// reports `newIndex` before the old item is removed, so destinations after
    /// the source are shifted back by one.
    pub fn reorder(&self, old_index: usize, new_index: usize) -> bool {
        let len = self.item_count();
        if old_index >= len || new_index > len {
            return false;
        }
        let destination = if old_index < new_index {
            new_index.saturating_sub(1)
        } else {
            new_index
        };
        self.move_item(old_index, destination.min(len.saturating_sub(1)))
    }

    /// Changes the number of logical items. Newly appended items receive
    /// fresh identities; existing identities and their retained state remain.
    pub fn set_item_count(&self, item_count: usize) {
        let mut state = self.state.borrow_mut();
        if item_count == state.order.len() {
            return;
        }
        if item_count < state.order.len() {
            state.order.truncate(item_count);
        } else {
            while state.order.len() < item_count {
                let item = state.next_item;
                state.next_item = state.next_item.wrapping_add(1);
                state.order.push(item);
            }
        }
        state.revision = state.revision.wrapping_add(1);
    }
}

struct ReorderableRenderSliver {
    index: MeasuredExtentIndex,
    builder: Rc<dyn Fn(usize) -> Widget>,
    controller: SliverReorderController,
    drag_context: DragDropContext<usize>,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    widgets: HashMap<usize, Widget>,
    order: Vec<usize>,
    controller_revision: u64,
}

impl ReorderableRenderSliver {
    fn new(
        controller: SliverReorderController,
        builder: Rc<dyn Fn(usize) -> Widget>,
        drag_context: DragDropContext<usize>,
        on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
    ) -> Self {
        let order = controller.order();
        Self {
            index: MeasuredExtentIndex::new(order.len(), DEFAULT_LAZY_ITEM_EXTENT),
            builder,
            controller_revision: controller.revision(),
            controller,
            drag_context,
            on_reorder,
            widgets: HashMap::new(),
            order,
        }
    }

    fn sync_controller(&mut self) {
        let revision = self.controller.revision();
        if revision == self.controller_revision {
            return;
        }
        let next_order = self.controller.order();
        if self.order.len() == next_order.len() {
            let mut current = self.order.clone();
            for (destination, item) in next_order.iter().copied().enumerate() {
                let Some(source) = current.iter().position(|candidate| *candidate == item) else {
                    continue;
                };
                if source != destination {
                    let _ = self.index.move_item(source, destination);
                    let moved = current.remove(source);
                    current.insert(destination, moved);
                }
            }
        } else {
            self.index.set_len(next_order.len());
        }
        // A cached target captures its old destination position. Recreate the
        // lightweight drag wrappers after an order mutation while retaining
        // the underlying element by its stable logical item ID.
        if self.order != next_order {
            self.widgets.clear();
        }
        self.widgets
            .retain(|item, _| next_order.iter().any(|candidate| candidate == item));
        self.order = next_order;
        self.controller_revision = revision;
    }

    fn child_widget(&mut self, position: usize, item: usize) -> Widget {
        if let Some(widget) = self.widgets.get(&item) {
            return widget.clone();
        }
        let child = (self.builder)(item);
        let context = self.drag_context.clone();
        let controller = self.controller.clone();
        let on_reorder = self.on_reorder.clone();
        let draggable: Widget = Draggable::new(context.clone(), item, child).into();
        let target = DragTarget::new(context, draggable).on_drop(move |source_item| {
            let Some(source_position) = controller.position_of(source_item) else {
                return;
            };
            if controller.move_item(source_position, position)
                && let Some(callback) = &on_reorder
            {
                callback(source_position, position);
            }
        });
        let widget: Widget = target.into();
        self.widgets.insert(item, widget.clone());
        widget
    }
}

impl RenderSliver for ReorderableRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.sync_controller();
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let range =
            self.index
                .materialized_range(cache_start, (cache_end - cache_start).max(0.), 0.);
        self.widgets
            .retain(|item, _| self.order.iter().any(|candidate| candidate == item));
        let order = self.order.clone();
        let children = range
            .map(|position| {
                let item = order[position];
                let offset = self.index.offset_for_index(position);
                let extent = self.index.offset_for_index(position + 1) - offset;
                SliverChildLayout {
                    id: SliverChildId::list_item(item),
                    widget: self.child_widget(position, item),
                    offset,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(extent),
                    ),
                    extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, self.index.total_extent()),
            children,
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let Some(item) = child
            .0
            .checked_sub(1)
            .and_then(|id| usize::try_from(id).ok())
        else {
            return false;
        };
        self.order
            .iter()
            .position(|candidate| *candidate == item)
            .is_some_and(|position| self.index.set_measured_extent(position, extent))
    }

    fn revision(&self) -> u64 {
        self.controller
            .revision()
            .wrapping_add(self.index.revision())
    }
}

/// Reorderable sliver list. Items are wrapped in retained drag sources and
/// targets, so a normal pointer drag performs the same logical operation as a
/// Flutter reorderable list without OS-level input injection.
pub struct SliverReorderableList {
    controller: SliverReorderController,
    builder: Rc<dyn Fn(usize) -> Widget>,
    drag_context: DragDropContext<usize>,
    on_reorder: Option<Rc<dyn Fn(usize, usize)>>,
}

impl SliverReorderableList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            controller: SliverReorderController::new(item_count),
            builder: Rc::new(move |i| builder(i).into()),
            drag_context: DragDropContext::new(),
            on_reorder: None,
        }
    }

    /// Replaces the retained order controller.
    #[must_use]
    pub fn controller(mut self, controller: SliverReorderController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn reorder_controller(&self) -> SliverReorderController {
        self.controller.clone()
    }

    /// Receives `(old_position, new_position)` after a successful drop.
    #[must_use]
    pub fn on_reorder(mut self, callback: impl Fn(usize, usize) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(callback));
        self
    }
}

impl Sliver for SliverReorderableList {
    fn build(&self, controller: &ScrollController) -> Widget {
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(ReorderableRenderSliver::new(
            self.controller.clone(),
            self.builder.clone(),
            self.drag_context.clone(),
            self.on_reorder.clone(),
        ))
    }
}

struct AnimatedListEntry {
    id: u64,
    animation: AnimationController,
    removing: bool,
}

#[derive(Clone, Copy)]
struct AnimatedListEntrySnapshot {
    id: u64,
    value: f32,
    removing: bool,
}

struct SliverAnimatedListState {
    entries: Vec<AnimatedListEntry>,
    next_id: u64,
    duration: Duration,
    revision: u64,
    structure_revision: u64,
}

/// Retained insertion/removal state for [`SliverAnimatedList`]. Calling
/// `insert` or `remove` changes only this small state object; the viewport
/// then drives the size transition from its normal frame clock.
#[derive(Clone)]
pub struct SliverAnimatedListController {
    state: Rc<RefCell<SliverAnimatedListState>>,
}

impl SliverAnimatedListController {
    #[must_use]
    pub fn new(item_count: usize) -> Self {
        Self::with_duration(item_count, Duration::from_millis(250))
    }

    #[must_use]
    pub fn with_duration(item_count: usize, duration: Duration) -> Self {
        let entries = (0..item_count)
            .map(|id| {
                let animation = AnimationController::new(duration);
                animation.set_value(1.);
                AnimatedListEntry {
                    id: id as u64,
                    animation,
                    removing: false,
                }
            })
            .collect();
        Self {
            state: Rc::new(RefCell::new(SliverAnimatedListState {
                entries,
                next_id: item_count as u64,
                duration,
                revision: 0,
                structure_revision: 0,
            })),
        }
    }

    #[must_use]
    pub fn item_count(&self) -> usize {
        self.state.borrow().entries.len()
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.state.borrow().duration
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.borrow().revision
    }

    /// Inserts one logical item and starts its expansion from zero extent.
    pub fn insert(&self, index: usize) -> bool {
        self.insert_at(index, Instant::now())
    }

    /// Deterministic-clock form of [`Self::insert`], useful for tests.
    pub fn insert_at(&self, index: usize, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        let duration = state.duration;
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        let animation = AnimationController::new(duration);
        animation.forward(now);
        let index = index.min(state.entries.len());
        state.entries.insert(
            index,
            AnimatedListEntry {
                id,
                animation,
                removing: false,
            },
        );
        state.revision = state.revision.wrapping_add(1);
        state.structure_revision = state.structure_revision.wrapping_add(1);
        true
    }

    /// Starts shrinking the item at `index`. It remains in the sliver until
    /// the reverse animation reaches zero, so the retained element and drag
    /// state are not destroyed halfway through the transition.
    pub fn remove(&self, index: usize) -> bool {
        self.remove_at(index, Instant::now())
    }

    /// Deterministic-clock form of [`Self::remove`], useful for tests.
    pub fn remove_at(&self, index: usize, now: Instant) -> bool {
        let state = self.state.borrow_mut();
        let Some(entry) = state.entries.get(index) else {
            return false;
        };
        if entry.removing {
            return false;
        }
        entry.animation.reverse(now);
        drop(state);
        let mut state = self.state.borrow_mut();
        if let Some(entry) = state.entries.get_mut(index) {
            entry.removing = true;
            state.revision = state.revision.wrapping_add(1);
            state.structure_revision = state.structure_revision.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// Advances all active insert/remove transitions and removes entries that
    /// have completed their reverse animation.
    pub fn tick(&self, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        let mut changed = false;
        for entry in &state.entries {
            changed |= entry.animation.tick(now);
        }
        let before = state.entries.len();
        state.entries.retain(|entry| {
            !(entry.removing && !entry.animation.is_active() && entry.animation.value() <= 0.)
        });
        if state.entries.len() != before {
            changed = true;
            state.structure_revision = state.structure_revision.wrapping_add(1);
        }
        if changed {
            state.revision = state.revision.wrapping_add(1);
        }
        changed
    }

    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.state
            .borrow()
            .entries
            .iter()
            .any(|entry| entry.animation.is_active())
    }

    fn structure_revision(&self) -> u64 {
        self.state.borrow().structure_revision
    }

    fn snapshot(&self) -> Vec<AnimatedListEntrySnapshot> {
        self.state
            .borrow()
            .entries
            .iter()
            .map(|entry| AnimatedListEntrySnapshot {
                id: entry.id,
                value: entry.animation.value().clamp(0., 1.),
                removing: entry.removing,
            })
            .collect()
    }
}

struct AnimatedExtentRenderSliver {
    controller: SliverAnimatedListController,
    builder: Rc<dyn Fn(usize) -> Widget>,
    index: MeasuredExtentIndex,
    entries: Vec<AnimatedListEntrySnapshot>,
    base_extents: HashMap<u64, f32>,
    widgets: HashMap<u64, Widget>,
    structure_revision: u64,
    estimated_extent: f32,
}

impl AnimatedExtentRenderSliver {
    fn new(
        controller: SliverAnimatedListController,
        builder: Rc<dyn Fn(usize) -> Widget>,
        estimated_extent: f32,
    ) -> Self {
        let entries = controller.snapshot();
        Self {
            index: MeasuredExtentIndex::new(entries.len(), estimated_extent),
            controller,
            builder,
            entries,
            base_extents: HashMap::new(),
            widgets: HashMap::new(),
            structure_revision: u64::MAX,
            estimated_extent: estimated_extent.max(1.),
        }
    }

    fn sync_entries(&mut self) {
        let next = self.controller.snapshot();
        let structure_revision = self.controller.structure_revision();
        if structure_revision != self.structure_revision {
            self.index = MeasuredExtentIndex::new(next.len(), self.estimated_extent);
            let live = next.iter().map(|entry| entry.id).collect::<Vec<_>>();
            self.base_extents
                .retain(|id, _| live.iter().any(|candidate| candidate == id));
            self.widgets
                .retain(|id, _| live.iter().any(|candidate| candidate == id));
            for (position, entry) in next.iter().enumerate() {
                let base = self
                    .base_extents
                    .get(&entry.id)
                    .copied()
                    .unwrap_or(self.estimated_extent);
                let _ = self.index.set_measured_extent(position, base * entry.value);
            }
            self.structure_revision = structure_revision;
        } else {
            for (position, (old, next)) in self.entries.iter().zip(&next).enumerate() {
                if (old.value - next.value).abs() > f32::EPSILON {
                    let base = self
                        .base_extents
                        .get(&next.id)
                        .copied()
                        .unwrap_or(self.estimated_extent);
                    let _ = self.index.set_measured_extent(position, base * next.value);
                }
            }
        }
        self.entries = next;
    }

    fn child_widget(&mut self, position: usize, id: u64) -> Widget {
        if let Some(widget) = self.widgets.get(&id) {
            return widget.clone();
        }
        let widget = (self.builder)(position);
        self.widgets.insert(id, widget.clone());
        widget
    }
}

impl RenderSliver for AnimatedExtentRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.sync_entries();
        let cache_start = (constraints.scroll_offset + constraints.cache_origin).max(0.);
        let cache_end = (cache_start + constraints.remaining_cache_extent).max(cache_start);
        let mut positions = self
            .index
            .materialized_range(cache_start, (cache_end - cache_start).max(0.), 0.)
            .collect::<Vec<_>>();
        // A freshly inserted zero-size entry does not intersect an offset
        // interval mathematically, but it must still be retained so its size
        // transition can paint on the next frame.
        positions.extend(
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.value < 1. || entry.removing)
                .map(|(position, _)| position),
        );
        positions.sort_unstable();
        positions.dedup();
        let children = positions
            .into_iter()
            .map(|position| {
                let entry = self.entries[position];
                let offset = self.index.offset_for_index(position);
                let extent = (self.index.offset_for_index(position + 1) - offset).max(0.);
                SliverChildLayout {
                    id: SliverChildId::list_item(entry.id as usize),
                    widget: self.child_widget(position, entry.id),
                    offset,
                    cross_offset: 0.,
                    constraints: sliver_child_constraints(
                        constraints.axis,
                        constraints.cross_axis_extent,
                        Some(extent),
                    ),
                    extent,
                    pinned: false,
                }
            })
            .collect();
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, self.index.total_extent()),
            children,
            absorbed_overlap: 0.,
        }
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        let Some(id) = child.0.checked_sub(1) else {
            return false;
        };
        let Some((position, entry)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.id == id)
        else {
            return false;
        };
        if entry.value <= f32::EPSILON || !extent.is_finite() || extent < 0. {
            return false;
        }
        let base = (extent / entry.value).max(0.);
        self.base_extents.insert(id, base);
        self.index.set_measured_extent(position, extent)
    }

    fn revision(&self) -> u64 {
        self.controller
            .revision()
            .wrapping_add(self.index.revision())
    }

    fn tick(&mut self, now: Instant) -> bool {
        let changed = self.controller.tick(now);
        if changed {
            self.sync_entries();
        }
        changed
    }

    fn is_animating(&self) -> bool {
        self.controller.is_animating()
    }
}

/// Animated list sliver with retained insertion/removal transitions.
pub struct SliverAnimatedList {
    builder: Rc<dyn Fn(usize) -> Widget>,
    controller: SliverAnimatedListController,
}

impl SliverAnimatedList {
    #[must_use]
    pub fn new<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Self
    where
        W: Into<Widget> + 'static,
    {
        Self {
            controller: SliverAnimatedListController::new(item_count),
            builder: Rc::new(move |i| builder(i).into()),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: SliverAnimatedListController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn animation_controller(&self) -> SliverAnimatedListController {
        self.controller.clone()
    }
}

impl Sliver for SliverAnimatedList {
    fn build(&self, controller: &ScrollController) -> Widget {
        single_sliver_viewport(
            controller.clone(),
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
            DEFAULT_SLIVER_CACHE_EXTENT,
            Clip::HardEdge,
            self.create_render_sliver(controller, Axis::Vertical, false),
        )
    }

    fn create_render_sliver(
        &self,
        _controller: &ScrollController,
        _axis: Axis,
        _reverse: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(AnimatedExtentRenderSliver::new(
            self.controller.clone(),
            self.builder.clone(),
            DEFAULT_LAZY_ITEM_EXTENT,
        ))
    }
}
