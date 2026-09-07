use super::*;

const DEFAULT_TREE_ROW_EXTENT: f32 = 40.0;
const DEFAULT_TREE_INDENTATION: f32 = 10.0;
const DEFAULT_TREE_ANIMATION_DURATION: Duration = Duration::from_millis(150);

/// Stable identity for a node in a `TreeSliver`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TreeSliverNodeId(pub u64);

/// A node in the owned tree model. Node IDs, rather than flattened indices,
/// are used for retained child identity and remain stable when ancestors are
/// expanded, collapsed, inserted, or removed from the model.
#[derive(Clone)]
pub struct TreeSliverNode<T> {
    id: TreeSliverNodeId,
    content: T,
    children: Vec<Self>,
    expanded: bool,
    depth: usize,
    parent_id: Option<TreeSliverNodeId>,
}

impl<T> TreeSliverNode<T> {
    #[must_use]
    pub fn new(content: T) -> Self {
        static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: TreeSliverNodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed).max(1)),
            content,
            children: Vec::new(),
            expanded: false,
            depth: 0,
            parent_id: None,
        }
    }

    #[must_use]
    pub fn with_children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded && !self.children.is_empty();
        self
    }

    pub fn add_child(&mut self, child: Self) {
        self.children.push(child);
    }

    #[must_use]
    pub fn id(&self) -> TreeSliverNodeId {
        self.id
    }

    #[must_use]
    pub fn content(&self) -> &T {
        &self.content
    }

    pub fn content_mut(&mut self) -> &mut T {
        &mut self.content
    }

    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Self> {
        &mut self.children
    }

    #[must_use]
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded && !self.children.is_empty();
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<TreeSliverNodeId> {
        self.parent_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeRowAnimation {
    pub value: f32,
    pub expanding: bool,
}

impl TreeRowAnimation {
    #[must_use]
    pub fn collapsing(self) -> bool {
        !self.expanding
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TreeSliverIndentation {
    None,
    #[default]
    Standard,
    Fixed(f32),
}

impl TreeSliverIndentation {
    #[must_use]
    pub fn extent(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Standard => DEFAULT_TREE_INDENTATION,
            Self::Fixed(value) => value.max(0.0),
        }
    }
}

struct TreeAnimationState {
    controller: AnimationController,
    expanding: bool,
}

#[derive(Clone, Copy)]
struct TreeActiveRow {
    id: TreeSliverNodeId,
    depth: usize,
    parent_id: Option<TreeSliverNodeId>,
}

#[derive(Clone)]
struct TreeAnimationSnapshot {
    parent_id: TreeSliverNodeId,
    value: f32,
    expanding: bool,
    direct_child_ids: Vec<TreeSliverNodeId>,
}

struct TreeState<T> {
    roots: Vec<TreeSliverNode<T>>,
    duration: Duration,
    active: Vec<TreeActiveRow>,
    animations: HashMap<TreeSliverNodeId, TreeAnimationState>,
    revision: u64,
    on_toggle: Option<TreeToggleCallback>,
}

type TreeToggleCallback = Rc<dyn Fn(TreeSliverNodeId)>;
type TreeRowBuilder<T> = Rc<dyn Fn(&TreeSliverNode<T>, TreeRowAnimation) -> Widget>;
type TreeRowExtentBuilder<T> = Rc<dyn Fn(&TreeSliverNode<T>) -> f32>;

impl<T> TreeState<T> {
    fn new(roots: Vec<TreeSliverNode<T>>, duration: Duration) -> Self {
        let mut state = Self {
            roots,
            duration,
            active: Vec::new(),
            animations: HashMap::new(),
            revision: 1,
            on_toggle: None,
        };
        state.rebuild_active();
        state
    }

    fn rebuild_active(&mut self) {
        self.active.clear();
        flatten_nodes(&mut self.roots, 0, None, &self.animations, &mut self.active);
    }

    fn active_ids(&self) -> Vec<TreeSliverNodeId> {
        self.active.iter().map(|row| row.id).collect()
    }

    fn find_node(&self, id: TreeSliverNodeId) -> Option<&TreeSliverNode<T>> {
        find_node_in(&self.roots, id)
    }

    fn find_node_mut(&mut self, id: TreeSliverNodeId) -> Option<&mut TreeSliverNode<T>> {
        find_node_in_mut(&mut self.roots, id)
    }

    fn toggle_at(
        &mut self,
        id: TreeSliverNodeId,
        now: Instant,
    ) -> (bool, Option<TreeToggleCallback>) {
        let Some(node) = self.find_node(id) else {
            return (false, None);
        };
        if node.children.is_empty() {
            return (false, None);
        }
        let expanded = !node.expanded;
        let current_value = self
            .animations
            .get(&id)
            .map(|animation| animation.controller.value());
        if let Some(node) = self.find_node_mut(id) {
            node.expanded = expanded;
        }
        if self.duration.is_zero() {
            self.animations.remove(&id);
        } else {
            let animation = self.animations.entry(id).or_insert_with(|| {
                let controller = AnimationController::new(self.duration);
                controller.set_value(if expanded { 0.0 } else { 1.0 });
                TreeAnimationState {
                    controller,
                    expanding: expanded,
                }
            });
            animation.expanding = expanded;
            if let Some(value) = current_value {
                animation.controller.set_value(value);
            }
            if expanded {
                animation.controller.forward(now);
            } else {
                animation.controller.reverse(now);
            }
        }
        self.rebuild_active();
        self.revision = self.revision.wrapping_add(1);
        (true, self.on_toggle.clone())
    }

    fn tick(&mut self, now: Instant) -> bool {
        let mut changed = false;
        let mut finished = Vec::new();
        for (id, animation) in &mut self.animations {
            changed |= animation.controller.tick(now);
            if !animation.controller.is_active() {
                finished.push((*id, animation.expanding));
            }
        }
        for (id, expanding) in finished {
            self.animations.remove(&id);
            if expanding {
                if let Some(node) = self.find_node_mut(id) {
                    node.expanded = true;
                }
            } else if let Some(node) = self.find_node_mut(id) {
                node.expanded = false;
            }
            changed = true;
        }
        if changed {
            self.rebuild_active();
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }

    fn animation_snapshots(&self) -> Vec<TreeAnimationSnapshot> {
        self.animations
            .iter()
            .filter_map(|(parent_id, animation)| {
                let node = self.find_node(*parent_id)?;
                Some(TreeAnimationSnapshot {
                    parent_id: *parent_id,
                    value: animation.controller.value().clamp(0.0, 1.0),
                    expanding: animation.expanding,
                    direct_child_ids: node.children.iter().map(|child| child.id).collect(),
                })
            })
            .collect()
    }

    fn expand_or_collapse_all(&mut self, expanded: bool) {
        set_expanded_recursive(&mut self.roots, expanded);
        self.animations.clear();
        self.rebuild_active();
        self.revision = self.revision.wrapping_add(1);
    }
}

fn flatten_nodes<T>(
    nodes: &mut [TreeSliverNode<T>],
    depth: usize,
    parent_id: Option<TreeSliverNodeId>,
    animations: &HashMap<TreeSliverNodeId, TreeAnimationState>,
    active: &mut Vec<TreeActiveRow>,
) {
    for node in nodes {
        node.depth = depth;
        node.parent_id = parent_id;
        active.push(TreeActiveRow {
            id: node.id,
            depth,
            parent_id,
        });
        if (node.expanded || animations.contains_key(&node.id)) && !node.children.is_empty() {
            flatten_nodes(
                &mut node.children,
                depth + 1,
                Some(node.id),
                animations,
                active,
            );
        }
    }
}

fn find_node_in<T>(
    nodes: &[TreeSliverNode<T>],
    id: TreeSliverNodeId,
) -> Option<&TreeSliverNode<T>> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        if let Some(found) = find_node_in(&node.children, id) {
            return Some(found);
        }
    }
    None
}

fn find_node_in_mut<T>(
    nodes: &mut [TreeSliverNode<T>],
    id: TreeSliverNodeId,
) -> Option<&mut TreeSliverNode<T>> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        if let Some(found) = find_node_in_mut(&mut node.children, id) {
            return Some(found);
        }
    }
    None
}

fn find_node_id_by_content<T: PartialEq>(
    nodes: &[TreeSliverNode<T>],
    content: &T,
) -> Option<TreeSliverNodeId> {
    for node in nodes {
        if &node.content == content {
            return Some(node.id);
        }
        if let Some(found) = find_node_id_by_content(&node.children, content) {
            return Some(found);
        }
    }
    None
}

fn set_expanded_recursive<T>(nodes: &mut [TreeSliverNode<T>], expanded: bool) {
    for node in nodes {
        node.expanded = expanded && !node.children.is_empty();
        set_expanded_recursive(&mut node.children, expanded);
    }
}

struct TreeControllerCore<T> {
    state: Option<Rc<RefCell<TreeState<T>>>>,
}

/// Controller for a `TreeSliver`.
pub struct TreeSliverController<T> {
    core: Rc<RefCell<TreeControllerCore<T>>>,
}

impl<T> Clone for TreeSliverController<T> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
        }
    }
}

impl<T> TreeSliverController<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: Rc::new(RefCell::new(TreeControllerCore { state: None })),
        }
    }

    fn attach(&self, state: Rc<RefCell<TreeState<T>>>) {
        self.core.borrow_mut().state = Some(state);
    }

    fn with_state<R>(&self, callback: impl FnOnce(&TreeState<T>) -> R) -> Option<R> {
        let state = self.core.borrow().state.clone()?;
        Some(callback(&state.borrow()))
    }

    fn with_state_mut<R>(&self, callback: impl FnOnce(&mut TreeState<T>) -> R) -> Option<R> {
        let state = self.core.borrow().state.clone()?;
        Some(callback(&mut state.borrow_mut()))
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.with_state(|state| state.revision).unwrap_or(0)
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.with_state(|state| state.active.len()).unwrap_or(0)
    }

    #[must_use]
    pub fn active_nodes(&self) -> Vec<TreeSliverNodeId> {
        self.with_state(TreeState::active_ids).unwrap_or_default()
    }

    #[must_use]
    pub fn is_active(&self, node: TreeSliverNodeId) -> bool {
        self.with_state(|state| state.active.iter().any(|row| row.id == node))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn active_index_for(&self, node: TreeSliverNodeId) -> Option<usize> {
        self.with_state(|state| state.active.iter().position(|row| row.id == node))
            .flatten()
    }

    pub fn get_active_index_for(&self, node: TreeSliverNodeId) -> Option<usize> {
        self.active_index_for(node)
    }

    #[must_use]
    pub fn is_expanded(&self, node: TreeSliverNodeId) -> bool {
        self.with_state(|state| state.find_node(node).is_some_and(|node| node.expanded))
            .unwrap_or(false)
    }

    pub fn toggle_node(&self, node: TreeSliverNodeId) -> bool {
        self.toggle_node_at(node, Instant::now())
    }

    pub fn toggle_node_at(&self, node: TreeSliverNodeId, now: Instant) -> bool {
        let result = self.with_state_mut(|state| state.toggle_at(node, now));
        let Some((changed, callback)) = result else {
            return false;
        };
        if changed && let Some(callback) = callback {
            callback(node);
        }
        changed
    }

    pub fn expand_node(&self, node: TreeSliverNodeId) -> bool {
        self.expand_node_at(node, Instant::now())
    }

    pub fn expand_node_at(&self, node: TreeSliverNodeId, now: Instant) -> bool {
        if self.is_expanded(node) {
            return false;
        }
        self.toggle_node_at(node, now)
    }

    pub fn collapse_node(&self, node: TreeSliverNodeId) -> bool {
        self.collapse_node_at(node, Instant::now())
    }

    pub fn collapse_node_at(&self, node: TreeSliverNodeId, now: Instant) -> bool {
        if !self.is_expanded(node) {
            return false;
        }
        self.toggle_node_at(node, now)
    }

    pub fn expand_all(&self) {
        self.expand_all_at(Instant::now());
    }

    pub fn expand_all_at(&self, _now: Instant) {
        let _ = self.with_state_mut(|state| state.expand_or_collapse_all(true));
    }

    pub fn collapse_all(&self) {
        self.collapse_all_at(Instant::now());
    }

    pub fn collapse_all_at(&self, _now: Instant) {
        let _ = self.with_state_mut(|state| state.expand_or_collapse_all(false));
    }

    pub fn tick(&self, now: Instant) -> bool {
        self.with_state_mut(|state| state.tick(now))
            .unwrap_or(false)
    }
}

impl<T> Default for TreeSliverController<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq> TreeSliverController<T> {
    #[must_use]
    pub fn get_node_for(&self, content: &T) -> Option<TreeSliverNodeId> {
        self.with_state(|state| find_node_id_by_content(&state.roots, content))
            .flatten()
    }
}

pub struct TreeSliver<T> {
    state: Rc<RefCell<TreeState<T>>>,
    controller: TreeSliverController<T>,
    builder: TreeRowBuilder<T>,
    row_extent_builder: TreeRowExtentBuilder<T>,
    indentation: TreeSliverIndentation,
    semantic_index_offset: usize,
    add_semantic_indexes: bool,
    semantic_index_callback: Option<Rc<dyn Fn(usize) -> Option<usize>>>,
}

impl<T> Clone for TreeSliver<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            controller: self.controller.clone(),
            builder: self.builder.clone(),
            row_extent_builder: self.row_extent_builder.clone(),
            indentation: self.indentation,
            semantic_index_offset: self.semantic_index_offset,
            add_semantic_indexes: self.add_semantic_indexes,
            semantic_index_callback: self.semantic_index_callback.clone(),
        }
    }
}

impl<T: 'static> TreeSliver<T> {
    #[must_use]
    pub fn new<W, F>(roots: Vec<TreeSliverNode<T>>, builder: F) -> Self
    where
        W: Into<Widget> + 'static,
        F: Fn(&TreeSliverNode<T>, TreeRowAnimation) -> W + 'static,
    {
        let state = Rc::new(RefCell::new(TreeState::new(
            roots,
            DEFAULT_TREE_ANIMATION_DURATION,
        )));
        let controller = TreeSliverController::new();
        controller.attach(state.clone());
        Self {
            state,
            controller,
            builder: Rc::new(move |node, animation| builder(node, animation).into()),
            row_extent_builder: Rc::new(|_| DEFAULT_TREE_ROW_EXTENT),
            indentation: TreeSliverIndentation::default(),
            semantic_index_offset: 0,
            add_semantic_indexes: true,
            semantic_index_callback: None,
        }
    }

    #[must_use]
    pub fn controller(&self) -> TreeSliverController<T> {
        self.controller.clone()
    }

    #[must_use]
    pub fn with_controller(mut self, controller: TreeSliverController<T>) -> Self {
        controller.attach(self.state.clone());
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn row_extent_builder<F>(mut self, builder: F) -> Self
    where
        F: Fn(&TreeSliverNode<T>) -> f32 + 'static,
    {
        self.row_extent_builder = Rc::new(builder);
        self
    }

    #[must_use]
    pub fn row_extent(self, extent: f32) -> Self {
        self.row_extent_builder(move |_| extent.max(0.0))
    }

    #[must_use]
    pub fn indentation(mut self, indentation: TreeSliverIndentation) -> Self {
        self.indentation = indentation;
        self
    }

    #[must_use]
    pub fn indent(self, extent: f32) -> Self {
        self.indentation(TreeSliverIndentation::Fixed(extent))
    }

    #[must_use]
    pub fn duration(self, duration: Duration) -> Self {
        self.state.borrow_mut().duration = duration;
        self
    }

    #[must_use]
    pub fn on_node_toggle<F>(self, callback: F) -> Self
    where
        F: Fn(TreeSliverNodeId) + 'static,
    {
        self.state.borrow_mut().on_toggle = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn semantic_index_offset(mut self, offset: usize) -> Self {
        self.semantic_index_offset = offset;
        self
    }

    #[must_use]
    pub fn add_semantic_indexes(mut self, add: bool) -> Self {
        self.add_semantic_indexes = add;
        self
    }

    #[must_use]
    pub fn semantic_index_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize) -> Option<usize> + 'static,
    {
        self.semantic_index_callback = Some(Rc::new(callback));
        self
    }

    fn render(&self) -> TreeRenderSliver<T> {
        TreeRenderSliver {
            state: self.state.clone(),
            builder: self.builder.clone(),
            row_extent_builder: self.row_extent_builder.clone(),
            indentation: self.indentation.extent(),
            semantic_index_offset: self.semantic_index_offset,
            add_semantic_indexes: self.add_semantic_indexes,
            semantic_index_callback: self.semantic_index_callback.clone(),
            active: Vec::new(),
            index: MeasuredExtentIndex::new(0, DEFAULT_TREE_ROW_EXTENT),
            extents: HashMap::new(),
            animation_distances: HashMap::new(),
            estimated_extent: DEFAULT_TREE_ROW_EXTENT,
            structure_revision: 0,
        }
    }
}

impl<T: 'static> Sliver for TreeSliver<T> {
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

struct TreeRenderSliver<T> {
    state: Rc<RefCell<TreeState<T>>>,
    builder: TreeRowBuilder<T>,
    row_extent_builder: TreeRowExtentBuilder<T>,
    indentation: f32,
    semantic_index_offset: usize,
    add_semantic_indexes: bool,
    semantic_index_callback: Option<Rc<dyn Fn(usize) -> Option<usize>>>,
    active: Vec<TreeActiveRow>,
    index: MeasuredExtentIndex,
    extents: HashMap<TreeSliverNodeId, f32>,
    animation_distances: HashMap<TreeSliverNodeId, f32>,
    estimated_extent: f32,
    structure_revision: u64,
}

impl<T> TreeRenderSliver<T> {
    fn sync_active(&mut self) {
        let next_active = self.state.borrow().active.clone();
        let changed = self.active.len() != next_active.len()
            || self
                .active
                .iter()
                .zip(&next_active)
                .any(|(left, right)| left.id != right.id || left.depth != right.depth);
        if changed {
            let ids = next_active.iter().map(|row| row.id).collect::<HashSet<_>>();
            self.extents.retain(|id, _| ids.contains(id));
            self.animation_distances.retain(|id, _| ids.contains(id));
            self.index = MeasuredExtentIndex::new(next_active.len(), self.estimated_extent);
            for (position, row) in next_active.iter().enumerate() {
                if let Some(extent) = self.extents.get(&row.id).copied() {
                    let _ = self.index.set_measured_extent(position, extent);
                }
            }
            self.active = next_active;
            self.structure_revision = self.structure_revision.wrapping_add(1);
        }
    }

    fn extent_at(&self, position: usize, row: &TreeActiveRow) -> f32 {
        self.extents
            .get(&row.id)
            .copied()
            .unwrap_or_else(|| {
                self.state
                    .borrow()
                    .find_node(row.id)
                    .map(|node| (self.row_extent_builder)(node))
                    .unwrap_or(self.estimated_extent)
            })
            .max(0.0)
            .max(if position < self.index.len() {
                let next = self.index.offset_for_index(position.saturating_add(1));
                let current = self.index.offset_for_index(position);
                (next - current).max(0.0)
            } else {
                0.0
            })
    }

    fn row_animation(
        &self,
        row: &TreeActiveRow,
        animations: &[TreeAnimationSnapshot],
    ) -> TreeRowAnimation {
        row.parent_id
            .and_then(|parent| {
                animations
                    .iter()
                    .find(|animation| animation.parent_id == parent)
            })
            .map_or(
                TreeRowAnimation {
                    value: 1.0,
                    expanding: true,
                },
                |animation| TreeRowAnimation {
                    value: animation.value,
                    expanding: animation.expanding,
                },
            )
    }

    fn animation_shift(
        &self,
        position: usize,
        animations: &[TreeAnimationSnapshot],
        distances: &HashMap<TreeSliverNodeId, f32>,
    ) -> f32 {
        animations
            .iter()
            .filter_map(|animation| {
                let parent_position = self
                    .active
                    .iter()
                    .position(|row| row.id == animation.parent_id)?;
                (parent_position < position).then_some(
                    distances.get(&animation.parent_id).copied().unwrap_or(0.0)
                        * (1.0 - animation.value),
                )
            })
            .sum()
    }
}

impl<T: 'static> RenderSliver for TreeRenderSliver<T> {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        self.sync_active();
        if self.active.is_empty() {
            return SliverLayout {
                geometry: SliverGeometry::from_scroll_extent(constraints, 0.0),
                children: Vec::new(),
                absorbed_overlap: 0.0,
            };
        }
        let animations = self.state.borrow().animation_snapshots();
        let animation_ids = animations
            .iter()
            .map(|animation| animation.parent_id)
            .collect::<HashSet<_>>();
        self.animation_distances
            .retain(|id, _| animation_ids.contains(id));
        for animation in &animations {
            if !self.animation_distances.contains_key(&animation.parent_id) {
                let distance =
                    animation
                        .direct_child_ids
                        .iter()
                        .filter_map(|id| {
                            let row = self.active.iter().find(|row| row.id == *id)?;
                            Some(self.extent_at(
                                self.active.iter().position(|item| item.id == row.id)?,
                                row,
                            ))
                        })
                        .sum::<f32>();
                self.animation_distances
                    .insert(animation.parent_id, distance);
            }
        }
        let range = self.index.materialized_range(
            constraints.scroll_offset,
            constraints.remaining_paint_extent,
            constraints.remaining_cache_extent,
        );
        let mut positions = range.collect::<Vec<_>>();
        for animation in &animations {
            for child_id in &animation.direct_child_ids {
                if let Some(position) = self.active.iter().position(|row| row.id == *child_id) {
                    positions.push(position);
                }
            }
        }
        positions.sort_unstable();
        positions.dedup();
        let state = self.state.borrow();
        let mut children = Vec::with_capacity(positions.len());
        for position in positions {
            let row = self.active[position];
            let node = state.find_node(row.id);
            let Some(node) = node else {
                continue;
            };
            let extent = self.extent_at(position, &row);
            let offset = (self.index.offset_for_index(position)
                - self.animation_shift(position, &animations, &self.animation_distances))
            .max(0.0);
            let animation = self.row_animation(&row, &animations);
            let logical_index = position.saturating_add(self.semantic_index_offset);
            let semantic_index = if !self.add_semantic_indexes {
                None
            } else if let Some(callback) = &self.semantic_index_callback {
                callback(logical_index)
            } else {
                Some(logical_index)
            };
            let cross_offset = row.depth as f32 * self.indentation;
            children.push(SliverChildLayout {
                id: SliverChildId(row.id.0),
                widget: (self.builder)(node, animation),
                semantic_index,
                offset,
                cross_offset,
                constraints: box_constraints(
                    constraints.axis,
                    (constraints.cross_axis_extent - cross_offset).max(0.0),
                    extent,
                ),
                extent,
                placement: crate::scrolling::SliverChildPlacement::Flow,
            });
        }
        let total_shift = animations
            .iter()
            .map(|animation| {
                self.animation_distances
                    .get(&animation.parent_id)
                    .copied()
                    .unwrap_or(0.0)
                    * (1.0 - animation.value)
            })
            .sum::<f32>();
        let scroll_extent = (self.index.total_extent() - total_shift).max(0.0);
        drop(state);
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, scroll_extent),
            children,
            absorbed_overlap: 0.0,
        }
    }

    fn child_count(&self) -> Option<usize> {
        Some(self.state.borrow().active.len())
    }

    fn set_child_extent(&mut self, child: SliverChildId, extent: f32) -> bool {
        if !extent.is_finite() || extent < 0.0 {
            return false;
        }
        let id = TreeSliverNodeId(child.0);
        let Some(position) = self.active.iter().position(|row| row.id == id) else {
            return false;
        };
        self.estimated_extent = if self.extents.is_empty() {
            extent.max(f32::EPSILON)
        } else {
            self.estimated_extent
        };
        let changed = self.extents.insert(id, extent) != Some(extent);
        changed | self.index.set_measured_extent(position, extent)
    }

    fn revision(&self) -> u64 {
        self.state
            .borrow()
            .revision
            .wrapping_add(self.structure_revision)
            .wrapping_add(self.index.revision())
    }

    fn tick(&mut self, now: Instant) -> bool {
        self.state.borrow_mut().tick(now)
    }

    fn is_animating(&self) -> bool {
        !self.state.borrow().animations.is_empty()
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
