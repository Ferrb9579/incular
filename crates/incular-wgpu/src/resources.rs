use super::prelude::*;
use super::*;
use std::hash::Hash;

/// Stable, process-local identity for an immutable GPU resource shared by all
/// windows attached to one [`SharedGpuContext`]. It intentionally exposes no
/// native or `wgpu` handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedGpuResourceId(u64);
impl SharedGpuResourceId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A headless record of shared-resource identity. The live GPU context uses
/// this same registry when it creates image, glyph, and pipeline resources;
/// keeping it independent of an adapter makes ownership tests display-server
/// free.
#[derive(Debug, Default)]
pub struct SharedGpuResourceRegistry {
    next_identity: u64,
    images: HashMap<ImageId, SharedGpuResourceId>,
    glyphs: HashMap<GlyphCacheKey, SharedGpuResourceId>,
}
impl SharedGpuResourceRegistry {
    fn allocate(&mut self) -> SharedGpuResourceId {
        self.next_identity = self.next_identity.saturating_add(1).max(1);
        SharedGpuResourceId(self.next_identity)
    }
    /// Returns the one context-local identity allocated for `image`.
    pub fn image_identity(&mut self, image: ImageId) -> SharedGpuResourceId {
        if let Some(identity) = self.images.get(&image) {
            return *identity;
        }
        let identity = self.allocate();
        self.images.insert(image, identity);
        identity
    }
    /// Returns the one context-local identity allocated for this DPI-specific
    /// glyph raster key.
    pub fn glyph_identity(&mut self, glyph: GlyphCacheKey) -> SharedGpuResourceId {
        if let Some(identity) = self.glyphs.get(&glyph) {
            return *identity;
        }
        let identity = self.allocate();
        self.glyphs.insert(glyph, identity);
        identity
    }
    #[must_use]
    pub fn image_count(&self) -> usize {
        self.images.len()
    }
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
    /// Drops the context-local image identity retained for an evicted
    /// texture. A later re-upload allocates a fresh identity; identity
    /// values are never reused, so a stale handle can never alias a new
    /// texture. Glyph identities are untouched by image eviction.
    /// Drops the context-local glyph identity retained for an evicted
    /// placement. Re-resolution re-registers the same key idempotently; a
    /// fresh identity is only allocated when the key was actually absent.
    pub fn remove_glyph(&mut self, glyph: GlyphCacheKey) -> bool {
        self.glyphs.remove(&glyph).is_some()
    }
    pub fn remove_image(&mut self, image: ImageId) -> bool {
        self.images.remove(&image).is_some()
    }
}

/// Budget for one device-owned shared texture cache.
///
/// `max_bytes` counts nominal texel bytes per entry, computed with checked
/// arithmetic (per-extent `width × height × 4` for the single-mip
/// `Rgba8UnormSrgb` image textures; a fixed LUT constant for gradients).
/// It is an accounting estimate, not physical GPU memory: row-pitch
/// padding, driver overhead, samplers, and bind groups are not counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedTextureBudget {
    pub max_entries: usize,
    pub max_bytes: u64,
}

impl Default for SharedTextureBudget {
    fn default() -> Self {
        Self {
            max_entries: 256,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

impl SharedTextureBudget {
    #[must_use]
    pub const fn new(max_entries: usize, max_bytes: u64) -> Self {
        Self {
            max_entries,
            max_bytes,
        }
    }
}

/// Nominal texel bytes for one `Rgba8UnormSrgb` texture extent. `None` on
/// arithmetic overflow — callers reject the upload instead of admitting an
/// unaccounted entry.
#[must_use]
pub fn shared_image_texture_bytes(width: u32, height: u32) -> Option<u64> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
}

/// One retained entry: nominal bytes, the last-use tick, and the texture
/// generation the entry was admitted with (the context-local identity
/// value for images; a per-family sequence otherwise). Ticks come from
/// the cache's own monotonic counter, so interleaved renderers share one
/// recency order without any frame clock. The generation binds recency
/// updates to one upload: a touch carrying a superseded generation
/// refreshes nothing, so a stale local entry can never keep a different
/// upload alive in the order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedTexture {
    pub bytes: u64,
    pub last_use: u64,
    pub generation: u64,
}

/// One eviction: the dropped entry plus whether its texture stayed alive in
/// renderer bindings, active frames, or submitted work at drop time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedTextureEviction<K> {
    pub id: K,
    pub bytes: u64,
    pub live_elsewhere: bool,
}

/// Counters for one device-owned texture cache. Cumulative since
/// construction except where noted; gauges are current values. Together
/// they separate shared-cache retention (`shared_hits`, gauges below) from
/// outstanding ownership elsewhere (`evicted_live`, `stale_touches`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedTextureCounters {
    /// Textures uploaded and admitted to the shared map.
    pub admissions: u64,
    /// Shared-map hits across all renderers (each reuses one upload).
    pub shared_hits: u64,
    /// Map entries dropped by budget eviction or limit tightening.
    pub evictions: u64,
    /// Nominal bytes of dropped entries (not freed GPU memory).
    pub evicted_bytes: u64,
    /// Dropped entries still referenced elsewhere at drop time.
    pub evicted_live: u64,
    /// Uploads served without admission (over budget by size).
    pub unadmitted_uploads: u64,
    /// Touches refused because the caller named a superseded generation.
    pub stale_touches: u64,
}

/// Device-owned admission/eviction/accounting for shared image textures.
/// Holds metadata only — no wgpu types — so the policy is deterministic
/// without a display server. The single instance lives on
/// [`SharedGpuContext`]; every renderer admits and touches through it, so
/// one window's use keeps another window's textures alive and churn in one
/// window evicts oldest-first across both.
///
/// Ownership contract for one retained texture: exactly one cache entry
/// exists per retained texture, keyed by content identity (an [`ImageId`]
/// for image textures, the full gradient description for gradients). The
/// texture stays alive while any of these hold it: the device-owned map
/// (one `Arc` each), a renderer's local map (bounded by that window's
/// unused-frame eviction and released on window disposal), or transient
/// frame locals and in-flight submissions (wgpu keeps submitted work valid
/// after the `Arc` drops, so eviction needs no frame delay). Bind groups
/// reference the texture view at the wgpu level without holding the `Arc`;
/// they keep GPU memory alive invisibly to these counters and die with
/// their owning renderer's local entry. Evicting a map entry therefore
/// drops one reference, never a texture: counters report dropped entries
/// and their nominal bytes, plus how many were still referenced elsewhere.
/// They never claim freed GPU memory.
#[derive(Debug)]
pub struct SharedTextureCache<K> {
    limits: SharedTextureBudget,
    entries: HashMap<K, CachedTexture>,
    /// Least-recently-used order; front is evicted first. Touch moves to
    /// the back. Length always equals entries length, so metadata scales
    /// with retained entries — themselves capped by the budget.
    lru: VecDeque<K>,
    tick: u64,
    retained_bytes: u64,
    /// Bumped once per dropped entry. Host event loops compare this
    /// against their last serviced value to run per-renderer reclamation
    /// only on passes where something actually evicted.
    eviction_revision: u64,
    counters: SharedTextureCounters,
}

impl<K> Default for SharedTextureCache<K> {
    fn default() -> Self {
        Self {
            limits: SharedTextureBudget::default(),
            entries: HashMap::new(),
            lru: VecDeque::new(),
            tick: 0,
            retained_bytes: 0,
            eviction_revision: 0,
            counters: SharedTextureCounters::default(),
        }
    }
}

impl<K> SharedTextureCache<K> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_limits(limits: SharedTextureBudget) -> Self {
        Self {
            limits,
            entries: HashMap::new(),
            lru: VecDeque::new(),
            tick: 0,
            retained_bytes: 0,
            eviction_revision: 0,
            counters: SharedTextureCounters::default(),
        }
    }

    /// Eviction revision: advances once per dropped entry. Hosts compare
    /// this across maintenance passes so passes with no evictions touch no
    /// renderer at all.
    #[must_use]
    pub const fn eviction_revision(&self) -> u64 {
        self.eviction_revision
    }

    #[must_use]
    pub const fn limits(&self) -> SharedTextureBudget {
        self.limits
    }

    #[must_use]
    pub const fn counters(&self) -> SharedTextureCounters {
        self.counters
    }

    #[must_use]
    pub fn retained_entries(&self) -> usize {
        self.entries.len()
    }

    /// Counts one upload served without admission (over budget by size).
    pub fn note_unadmitted_upload(&mut self) {
        self.counters.unadmitted_uploads += 1;
    }
}

impl<K: Copy + Eq + Hash> SharedTextureCache<K> {
    /// Nominal texel bytes currently retained. An accounting estimate, not
    /// physical GPU memory (see [`SharedTextureBudget`]).
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    /// Whether `bytes` may be admitted under the current limits. Zero
    /// limits, or an entry larger than the whole byte budget, never admit.
    #[must_use]
    pub fn fits(&self, bytes: u64) -> bool {
        self.limits.max_entries > 0 && bytes <= self.limits.max_bytes
    }

    /// Whether the shared map currently retains `id`.
    #[must_use]
    pub fn contains(&self, id: &K) -> bool
    where
        K: Eq + Hash,
    {
        self.entries.contains_key(id)
    }

    /// Records a shared-map hit for `id` carrying the caller's `generation`.
    /// Refreshes recency only when the generation matches the admitted
    /// entry; a superseded generation counts a stale touch and refreshes
    /// nothing, so a stale local entry can never keep a different upload
    /// alive. Returns false for unknown ids (uncounted).
    pub fn touch(&mut self, id: K, generation: u64) -> bool {
        let matched = self
            .entries
            .get(&id)
            .is_some_and(|entry| entry.generation == generation);
        if !matched {
            if self.entries.contains_key(&id) {
                self.counters.stale_touches += 1;
            }
            return false;
        }
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.last_use = tick;
        }
        if let Some(position) = self.lru.iter().position(|candidate| *candidate == id)
            && let Some(key) = self.lru.remove(position)
        {
            self.lru.push_back(key);
        }
        self.counters.shared_hits += 1;
        true
    }

    /// Admitted generation for `id`, if the shared map retains an entry.
    /// Renderer-local caches compare this against the generation recorded
    /// at admission to detect superseded entries without refreshing them.
    #[must_use]
    pub fn generation(&self, id: K) -> Option<u64> {
        self.entries.get(&id).map(|entry| entry.generation)
    }

    /// Admits `id` with nominal `bytes` under `generation`, evicting
    /// least-recently-used entries first while over budget. `is_live`
    /// observes external references (renderer-held `Arc`s) so each eviction
    /// reports whether its texture survives the drop. An already-present id
    /// is treated as a generation-checked touch; an over-budget entry is
    /// refused without effect, so the shared map (whose keys always equal
    /// these entries) never holds what the policy cannot account for.
    pub fn admit(
        &mut self,
        id: K,
        bytes: u64,
        generation: u64,
        is_live: &dyn Fn(K) -> bool,
    ) -> Vec<SharedTextureEviction<K>> {
        if self.entries.contains_key(&id) {
            self.touch(id, generation);
            return Vec::new();
        }
        if !self.fits(bytes) {
            return Vec::new();
        }
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        self.entries.insert(
            id,
            CachedTexture {
                bytes,
                last_use: tick,
                generation,
            },
        );
        self.lru.push_back(id);
        self.retained_bytes = self.retained_bytes.saturating_add(bytes);
        self.counters.admissions += 1;
        self.evict_excess(is_live)
    }

    /// Replaces the budget and immediately trims oldest-first to it.
    pub fn set_limits(
        &mut self,
        limits: SharedTextureBudget,
        is_live: &dyn Fn(K) -> bool,
    ) -> Vec<SharedTextureEviction<K>> {
        self.limits = limits;
        self.evict_excess(is_live)
    }

    /// Evicts least-recently-used entries until both limits hold. The
    /// just-admitted entry sits at the back, so a fitting admission is
    /// never its own victim.
    fn evict_excess(&mut self, is_live: &dyn Fn(K) -> bool) -> Vec<SharedTextureEviction<K>> {
        let mut evicted = Vec::new();
        while self.entries.len() > self.limits.max_entries
            || self.retained_bytes > self.limits.max_bytes
        {
            let Some(oldest) = self.lru.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.retained_bytes = self.retained_bytes.saturating_sub(entry.bytes);
                self.eviction_revision = self.eviction_revision.saturating_add(1);
                self.counters.evictions += 1;
                self.counters.evicted_bytes =
                    self.counters.evicted_bytes.saturating_add(entry.bytes);
                let live_elsewhere = is_live(oldest);
                if live_elsewhere {
                    self.counters.evicted_live += 1;
                }
                evicted.push(SharedTextureEviction {
                    id: oldest,
                    bytes: entry.bytes,
                    live_elsewhere,
                });
            }
        }
        debug_assert_eq!(self.lru.len(), self.entries.len());
        evicted
    }
}

/// How one renderer-local entry is retained. Shared-backed entries name
/// the texture generation the shared owner admitted, so later coordination
/// can tell "same upload, still current" from "a different upload that
/// reused this identity". Bypassed entries (oversized-for-budget uploads)
/// have no shared entry and no usable generation: shared-generation pruning
/// never applies to them, and only the local age bound releases them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalImageRetention {
    /// Retained in the shared map under this texture generation.
    Shared { generation: u64 },
    /// Served without shared admission; retained locally only. Bounded by
    /// the local age rule below — never by shared generations — with one
    /// honest limit: the age rule counts presented frames, not elapsed
    /// idle time, so a renderer that stops presenting keeps its bypassed
    /// entries until it resumes, reclaims, or is dropped. Idle retention
    /// of bypassed textures is bounded by disposal, not by this variant.
    Bypassed,
}

/// One renderer-local entry: the retained resource plus how it is
/// retained (see [`LocalImageRetention`]).
#[derive(Clone, Debug)]
pub struct RendererImageEntry<R> {
    pub resource: R,
    pub retention: LocalImageRetention,
}

/// One window's local texture retention, coordinated with the shared
/// owner. Generic over the cache key and the retained resource so the
/// coordination logic — frame-use batching, generation-checked refresh,
/// stale pruning, age eviction — is the same code production renderers and
/// headless tests run: production instantiates it with its GPU entry
/// types, tests with plain reference-counted stand-ins.
///
/// A local entry is retention, not proof of use: only ids drained through
/// [`Self::drain_frame_use`] count as used, and only
/// [`Self::sync_with_shared`] refreshes shared recency — at most one shared
/// lock per frame no matter how many draws referenced the textures.
#[derive(Clone, Debug)]
pub struct RendererImageCache<K, R> {
    entries: HashMap<K, (RendererImageEntry<R>, LocalImageUse)>,
}

#[derive(Clone, Copy, Debug, Default)]
struct LocalImageUse {
    last_used_frame: u64,
    used_this_frame: bool,
}

impl<K, R> Default for RendererImageCache<K, R> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<K, R> RendererImageCache<K, R> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: Copy + Eq + Hash, R> RendererImageCache<K, R> {
    #[must_use]
    pub fn contains(&self, id: &K) -> bool {
        self.entries.contains_key(id)
    }

    #[must_use]
    pub fn get(&self, id: &K) -> Option<&RendererImageEntry<R>> {
        self.entries.get(id).map(|(entry, _)| entry)
    }

    #[must_use]
    pub fn get_mut(&mut self, id: &K) -> Option<&mut RendererImageEntry<R>> {
        self.entries.get_mut(id).map(|(entry, _)| entry)
    }

    /// Records use of a locally retained entry for `frame`. Returns false
    /// when absent. Takes no shared lock; use is flushed in batch by
    /// [`Self::sync_with_shared`].
    pub fn record_use(&mut self, id: K, frame: u64) -> bool {
        if let Some((_, state)) = self.entries.get_mut(&id) {
            state.last_used_frame = frame;
            state.used_this_frame = true;
            true
        } else {
            false
        }
    }

    /// Inserts (or replaces) a locally retained entry, recording use for
    /// `frame`. Shared entries must carry the identity value current at
    /// admission, so later syncs can prove the entry still names it;
    /// bypassed entries carry [`LocalImageRetention::Bypassed`].
    pub fn insert(&mut self, id: K, resource: R, retention: LocalImageRetention, frame: u64) {
        self.entries.insert(
            id,
            (
                RendererImageEntry {
                    resource,
                    retention,
                },
                LocalImageUse {
                    last_used_frame: frame,
                    used_this_frame: true,
                },
            ),
        );
    }

    /// Takes this frame's used `(id, retention)` pairs, clearing use flags.
    /// Deduplicated by construction: an id drawn many times appears once.
    pub fn drain_frame_use(&mut self) -> Vec<(K, LocalImageRetention)> {
        let mut used = Vec::new();
        for (id, (entry, state)) in self.entries.iter_mut() {
            if state.used_this_frame {
                state.used_this_frame = false;
                used.push((*id, entry.retention));
            }
        }
        used
    }

    /// Drops shared-backed entries whose generation differs or is gone,
    /// returning the dropped `(id, resource)` pairs for the caller to
    /// release. Bypassed entries are never generation-pruned — they have no
    /// shared generation — and are bounded by age eviction instead.
    /// Dropping is always safe: submitted GPU work outlives the handles by
    /// the wgpu lifetime contract, and the next use re-resolves to the
    /// current shared generation.
    pub fn prune_stale(&mut self, current_generation: &dyn Fn(K) -> Option<u64>) -> Vec<(K, R)> {
        let stale: Vec<K> = self
            .entries
            .iter()
            .filter(|(id, (entry, _))| match entry.retention {
                LocalImageRetention::Shared { generation } => {
                    current_generation(**id) != Some(generation)
                }
                LocalImageRetention::Bypassed => false,
            })
            .map(|(id, _)| *id)
            .collect();
        let mut dropped = Vec::with_capacity(stale.len());
        for id in stale {
            if let Some((entry, _)) = self.entries.remove(&id) {
                dropped.push((id, entry.resource));
            }
        }
        dropped
    }

    /// Reclaims entries the shared owner no longer retains, without needing
    /// frame activity: drops locally stale entries against the live shared
    /// generations and returns the dropped `(id, resource)` pairs. This is
    /// the production idle-reclamation orchestration — it reads generations
    /// from the shared policy itself rather than taking an arbitrary
    /// comparison closure — for renderers that present no frames while
    /// shared churn moves on. Safe on idle clients for the same reason as
    /// [`Self::prune_stale`].
    pub fn reclaim_stale(&mut self, shared: &SharedTextureCache<K>) -> Vec<(K, R)> {
        self.prune_stale(&|id| shared.generation(id))
    }

    /// Drops entries unused for more than `max_unused_frames`, returning
    /// how many went. This is the age half of local retention; generation
    /// staleness is handled by [`Self::prune_stale`]. The bound counts
    /// presented frames, not wall-clock idle time: a renderer presenting
    /// no frames retains everything until it resumes, reclaims, or drops.
    /// Do not read this as an idle-retention bound.
    pub fn evict_unused(&mut self, frame: u64, max_unused_frames: u64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, (_, state)| {
            frame.saturating_sub(state.last_used_frame) <= max_unused_frames
        });
        before - self.entries.len()
    }

    /// Coordinates one frame with the shared owner: flushes this frame's
    /// batched use as generation-checked touches (shared-backed entries
    /// only; bypassed entries never contact shared state), then drops
    /// locally stale entries. Returns `(refreshed, pruned_ids)`. Holds no
    /// lock itself; the caller passes the (already locked) shared policy
    /// once, so a frame costs at most one shared acquisition no matter how
    /// many draws ran.
    ///
    /// Coherence granularity is one frame: protection lands when the sync
    /// runs, so a churn admission earlier in the same frame can still evict
    /// an entry whose touch has not landed yet. That entry's owner
    /// re-resolves on its next use (a shared hit on the replacement, or a
    /// counted re-upload), so the system converges instead of sticking.
    pub fn sync_with_shared(&mut self, shared: &mut SharedTextureCache<K>) -> (usize, Vec<K>) {
        let mut refreshed = 0;
        for (id, retention) in self.drain_frame_use() {
            if let LocalImageRetention::Shared { generation } = retention
                && shared.touch(id, generation)
            {
                refreshed += 1;
            }
        }
        let pruned: Vec<K> = self
            .prune_stale(&|id| shared.generation(id))
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        (refreshed, pruned)
    }
}

/// Renderer capability driven by host event-loop maintenance: release
/// locally retained textures the shared owner dropped, returning how many
/// went. This is the dispatch interface hosts program against; fakes
/// implement it behind the same dispatch in tests.
pub trait ReclaimStaleTextures {
    fn reclaim_stale_textures(&mut self) -> usize;
}

/// Host-side maintenance dispatch for shared image retention. Tracks the
/// shared eviction revision across passes so passes with no evictions
/// touch no renderer: `maintain` returns 0 without calling any client.
/// Otherwise it drives reclamation on every client and returns the total
/// released. The revision is read before any client runs and no shared
/// lock is held while clients run, so an eviction landing mid-pass is
/// picked up — with a fresh comparison — on the next pass instead of
/// being missed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedImageMaintenance {
    last_seen_eviction_revision: u64,
}

impl SharedImageMaintenance {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs one maintenance pass against `eviction_revision` (read from
    /// [`SharedGpuContext::image_eviction_revision`]). Returns textures
    /// released across all clients, or 0 without contacting any client
    /// when nothing evicted since the last pass.
    pub fn maintain<'a>(
        &mut self,
        eviction_revision: u64,
        clients: impl IntoIterator<Item = &'a mut dyn ReclaimStaleTextures>,
    ) -> usize {
        if eviction_revision == self.last_seen_eviction_revision {
            return 0;
        }
        self.last_seen_eviction_revision = eviction_revision;
        clients
            .into_iter()
            .map(|client| client.reclaim_stale_textures())
            .sum()
    }
}

/// Read-only shared GPU ownership diagnostics. Counts describe one
/// application/device, rather than any individual presentation surface.
/// Image-texture counters come from the device-owned admission policy:
/// `shared_image_*` describe retained map entries and dropped entries,
/// never freed GPU memory — an evicted entry still referenced elsewhere
/// (renderer bindings, active frames, submitted work) frees nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SharedGpuDiagnostics {
    pub device_generation: u64,
    pub pipeline_variants: usize,
    pub pipeline_count: usize,
    pub shared_image_resources: usize,
    pub shared_glyph_resources: usize,
    pub shared_gradient_resources: usize,
    pub glyph_atlas_pages: usize,
    /// Nominal texel bytes retained in the shared image map. An accounting
    /// estimate, not physical GPU memory (see [`SharedTextureBudget`]).
    pub shared_image_texel_bytes: u64,
    /// Map entries dropped by image-texture budget eviction.
    pub shared_image_evictions: u64,
    /// Dropped entries still referenced elsewhere at drop time.
    pub shared_image_evicted_live: u64,
    /// Uploads served without shared admission (over budget by size).
    pub shared_image_unadmitted: u64,
    /// Touches refused because the caller named a superseded generation.
    pub shared_image_stale_touches: u64,
    /// Nominal texel bytes retained in the shared gradient map. An
    /// accounting estimate, not physical GPU memory.
    pub shared_gradient_texel_bytes: u64,
    /// Map entries dropped by gradient budget eviction.
    pub shared_gradient_evictions: u64,
    /// Dropped gradient entries still referenced elsewhere at drop time.
    pub shared_gradient_evicted_live: u64,
    /// Gradient uploads served without shared admission.
    pub shared_gradient_unadmitted: u64,
    /// Gradient touches refused on superseded generations.
    pub shared_gradient_stale_touches: u64,
}

/// Per-window presentation state that is independent of the shared GPU
/// device. It deliberately remains useful in headless tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowGpuPresentation {
    pub physical_size: PhysicalSize,
    pub surface_generation: u64,
    pub configured: bool,
    pub presented_frames: u64,
    pub skipped_frames: u64,
}
impl WindowGpuPresentation {
    #[must_use]
    pub const fn new(physical_size: PhysicalSize) -> Self {
        let configured = !physical_size.is_zero();
        Self {
            physical_size,
            surface_generation: if configured { 1 } else { 0 },
            configured,
            presented_frames: 0,
            skipped_frames: 0,
        }
    }
    /// Updates only this window's physical presentation state. A zero-sized
    /// surface is intentionally left unconfigured until it becomes valid.
    pub fn resize(&mut self, physical_size: PhysicalSize) -> bool {
        self.physical_size = physical_size;
        self.configured = !physical_size.is_zero();
        if self.configured {
            self.surface_generation = self.surface_generation.saturating_add(1);
        }
        self.configured
    }
    /// Records a recoverable surface loss. Device loss belongs to the shared
    /// context; this method cannot affect another window's surface state.
    pub fn surface_lost(&mut self) {
        if self.configured {
            self.surface_generation = self.surface_generation.saturating_add(1);
        }
    }
    pub fn record_present(&mut self) {
        self.presented_frames = self.presented_frames.saturating_add(1);
    }
    pub fn record_skipped(&mut self) {
        self.skipped_frames = self.skipped_frames.saturating_add(1);
    }
}

/// Surface and compositor ownership for exactly one native window. The owned
/// target keeps the native window alive through rendering and surface recovery.
pub struct WindowGpuState {
    pub(crate) target: WindowSurfaceTarget,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) transparency_mode: TransparencyMode,
    pub(crate) background_color: Color,
    pub(crate) alpha_plan: SurfaceAlphaPlan,
    pub(crate) stencil_texture: wgpu::Texture,
    pub(crate) stencil_view: wgpu::TextureView,
    pub(crate) presentation: WindowGpuPresentation,
}
impl WindowGpuState {
    #[must_use]
    pub const fn presentation(&self) -> WindowGpuPresentation {
        self.presentation
    }

    /// Concrete native-compositor alpha contract selected for this surface.
    #[must_use]
    pub const fn alpha_plan(&self) -> SurfaceAlphaPlan {
        self.alpha_plan
    }

    #[must_use]
    pub const fn transparency_mode(&self) -> TransparencyMode {
        self.transparency_mode
    }

    #[must_use]
    pub const fn background_color(&self) -> Color {
        self.background_color
    }
}

/// Device-wide GPU ownership. Cloning this value never creates another
/// `Instance`, `Adapter`, `Device`, or `Queue`; it merely gives another window
/// access to the same application-owned GPU context.
#[derive(Clone)]
pub struct SharedGpuContext {
    pub(crate) inner: Arc<SharedGpuContextInner>,
}
pub(crate) struct SharedGpuContextInner {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) device_generation: u64,
    /// Shared pipeline sets by surface format: one entry per configured
    /// format, created once on first renderer init and never evicted.
    /// Retention is bounded by construction, so there is deliberately no
    /// budget or eviction machinery here:
    /// - Keys are only surface-advertised `TextureFormat`s, written solely
    ///   by renderer initialization (`register_pipeline_resources`) and
    ///   read solely by renderer initialization (`pipeline_resources`) —
    ///   never per frame, draw, or resource. Retention is bounded by the
    ///   supported format-key space: entries only arise for formats the
    ///   surface stack actually configures, not one per adapter, window,
    ///   or frame.
    /// - Each entry holds the closed `pipeline_contracts()` registry (11
    ///   fixed classes + 11 Porter-Duff blends + 4 clip-mask directions =
    ///   26 contracts), each built exactly once per format.
    /// - Renderers clone the wgpu handles (internally reference-counted
    ///   handles to the same GPU objects, never duplicate allocations),
    ///   so dropping shared-cache ownership cannot invalidate an active
    ///   renderer or submitted work; dropping every renderer and the
    ///   context releases everything. No removal path exists.
    /// - Racing first initializations for one format may each build a set,
    ///   but registration keeps the first (`or_insert_with`) and drops the
    ///   loser unretained: duplicate retention is impossible, at most
    ///   transient duplicate creation work.
    ///
    /// Pipeline memory is driver-opaque: diagnostics report entry counts
    /// (`pipeline_variants`, `pipeline_count`) and tracked ownership,
    /// never invented GPU-byte estimates.
    pub(crate) pipelines: Mutex<HashMap<wgpu::TextureFormat, Arc<SharedPipelineResources>>>,
    pub(crate) resources: Mutex<SharedGpuResources>,
    /// Bytes written for retained device-level textures (images, gradient
    /// LUTs). These uploads happen once per resource rather than per frame.
    pub(crate) texture_upload_bytes: std::sync::atomic::AtomicU64,
}
pub(crate) struct SharedGpuResources {
    pub(crate) registry: SharedGpuResourceRegistry,
    pub(crate) images: HashMap<ImageId, Arc<SharedGpuImage>>,
    /// Device-owned admission/eviction/accounting for `images`. The map
    /// keys always equal the policy entries: every admission and eviction
    /// updates both together, and oversized uploads bypass both.
    pub(crate) image_textures: SharedTextureCache<ImageId>,
    pub(crate) gradients: HashMap<GradientResourceKey, Arc<SharedGpuGradient>>,
    /// Device-owned admission/eviction/accounting for `gradients`, under
    /// the same map-keys-equal-policy-entries invariant.
    pub(crate) gradient_textures: SharedTextureCache<GradientResourceKey>,
    /// Per-family admission sequence for gradients, which carry no registry
    /// identity. Bumped once per gradient admission; unlike registry
    /// identities it is scoped to this cache and only needs uniqueness
    /// within one key's re-upload history.
    pub(crate) gradient_generation: u64,
    pub(crate) glyph_atlas: GlyphAtlas,
    pub(crate) glyph_pages: Vec<Option<SharedGpuAtlasPage>>,
    /// Atlas eviction revision at the last shared texture-slot prune, so
    /// resolves without intervening retirements skip the scan.
    pub(crate) glyph_prune_revision: u64,
}
pub(crate) struct SharedGpuImage {
    pub(crate) identity: SharedGpuResourceId,
    pub(crate) _texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
/// What one renderer acquisition of a shared texture established: the
/// renderer-integration handoff between shared upload and local retention
/// (this crate's API class is backend, covering custom rendering
/// integrations that acquire from the shared context the same way).
/// `uploaded` reports a fresh GPU upload versus shared reuse — an
/// independent fact, kept separate. Admission has one authoritative
/// representation, the `generation`: `Some` names the admitted upload for
/// local retention bookkeeping (registry identity for images, admission
/// sequence for gradients), `None` means the upload bypassed admission.
/// A contradictory state is unrepresentable in every build, not merely
/// debug-asserted.
pub struct SharedTextureAcquisition<R> {
    pub(crate) resource: R,
    pub(crate) uploaded: bool,
    pub(crate) generation: Option<u64>,
}

impl<R> SharedTextureAcquisition<R> {
    /// Builds an acquisition. Admission follows from `generation` alone:
    /// `Some` admits, `None` bypasses. There is no separate admission flag
    /// to contradict it.
    pub fn new(resource: R, uploaded: bool, generation: Option<u64>) -> Self {
        Self {
            resource,
            uploaded,
            generation,
        }
    }

    /// Whether the upload was admitted to shared retention. Derived from
    /// the generation; see the type documentation.
    #[must_use]
    pub fn admitted(&self) -> bool {
        self.generation.is_some()
    }

    /// Splits an acquisition into the `(resource, retention)` pair the
    /// renderer inserts locally. The shared handle moves over unchanged —
    /// generic over `R`, this conversion cannot name (and therefore cannot
    /// re-wrap or clone) any inner handle — so liveness checks observe
    /// renderer ownership, and retention follows admission exactly.
    pub fn into_local_parts(self) -> (R, LocalImageRetention) {
        let retention = self
            .generation
            .map_or(LocalImageRetention::Bypassed, |generation| {
                LocalImageRetention::Shared { generation }
            });
        (self.resource, retention)
    }
}

/// Image-texture acquisition: admitted uploads additionally carry a
/// registry identity; bypassed uploads hold a fresh per-upload identity
/// with no registry entry.
pub(crate) type SharedImageAcquisition = SharedTextureAcquisition<Arc<SharedGpuImage>>;

/// Gradient-texture acquisition. Gradients carry no registry identity at
/// all; generations come from the per-family admission sequence.
pub(crate) type SharedGradientAcquisition = SharedTextureAcquisition<Arc<SharedGpuGradient>>;
pub(crate) struct SharedGpuGradient {
    pub(crate) resource: GpuGradient,
}
/// Shared-cache key for one gradient lookup texture: the normalized stops
/// identity plus the target surface format. Stops identities are minted
/// per construction (clones intentionally share), so distinct descriptions
/// never alias — even byte-identical stops built separately upload
/// separately — while geometry (kind, endpoints) flows to shaders through
/// instance parameters, never through this key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GradientResourceKey {
    pub gradient: GradientId,
    /// A bind group is pipeline-layout compatible only within this explicit
    /// target-format pipeline variant.
    pub format: wgpu::TextureFormat,
}

/// Nominal texel bytes of one gradient lookup texture: a fixed
/// 256-sample single-row RGBA8 upload. Constant for every entry, so the
/// byte budget for gradients behaves as a scaled entry budget.
pub const SHARED_GRADIENT_TEXEL_BYTES: u64 = 256 * 4;
pub(crate) struct SharedGpuAtlasPage {
    pub(crate) texture: wgpu::Texture,
    pub(crate) generation: u64,
}
impl SharedGpuContext {
    /// Creates the one device context to be shared by every desktop window in
    /// an application. The owned target and transparency mode select an adapter
    /// that satisfies the first window's presentation contract. The temporary
    /// surface and target are released before this method returns; the shared
    /// context does not retain the first window.
    pub async fn new(
        target: WindowSurfaceTarget,
        transparency_mode: TransparencyMode,
    ) -> Result<Self, RendererError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = target
            .create_surface(&instance)
            .map_err(RendererError::Surface)?;
        let power_preference = wgpu::PowerPreference::from_env().unwrap_or_default();
        let preferred_adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference,
                ..Default::default()
            })
            .await
            .map_err(RendererError::Adapter)?;
        let adapter = if adapter_supports_window_transparency(
            &surface,
            &preferred_adapter,
            transparency_mode,
        ) {
            preferred_adapter
        } else {
            let candidates = instance
                .enumerate_adapters(wgpu::Backends::all())
                .await
                .into_iter()
                .filter(|adapter| adapter.is_surface_supported(&surface))
                .filter(|adapter| {
                    adapter_supports_window_transparency(&surface, adapter, transparency_mode)
                });
            select_fallback_adapter(candidates, power_preference).ok_or_else(|| {
                RendererError::SurfaceAlpha(match transparency_mode {
                    TransparencyMode::Transparent => {
                        crate::surface::SurfaceAlphaError::TransparentCompositingUnsupported
                    }
                    TransparencyMode::Opaque => {
                        crate::surface::SurfaceAlphaError::NoOpaqueCompositingMode
                    }
                })
            })?
        };
        // Timestamp support is additive and optional: adapters that expose it
        // get non-blocking GPU frame timing through wgpu-profiler, everyone
        // else reports `GPU timing unavailable` instead of failing
        // initialization.
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::default();
        if adapter_features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            required_features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("incular shared device"),
                required_features,
                ..Default::default()
            })
            .await
            .map_err(RendererError::Device)?;
        drop(surface);
        Ok(Self {
            inner: Arc::new(SharedGpuContextInner {
                instance,
                adapter,
                device,
                queue,
                device_generation: 1,
                pipelines: Mutex::new(HashMap::new()),
                resources: Mutex::new(SharedGpuResources {
                    registry: SharedGpuResourceRegistry::default(),
                    images: HashMap::new(),
                    image_textures: SharedTextureCache::new(),
                    gradients: HashMap::new(),
                    gradient_textures: SharedTextureCache::new(),
                    gradient_generation: 0,
                    glyph_atlas: GlyphAtlas::new(),
                    glyph_pages: Vec::new(),
                    glyph_prune_revision: 0,
                }),
                texture_upload_bytes: std::sync::atomic::AtomicU64::new(0),
            }),
        })
    }
    /// Creates a renderer for one additional native surface without creating a
    /// second device context.
    pub async fn create_renderer(
        &self,
        target: WindowSurfaceTarget,
        size: PhysicalSize,
        transparency_mode: TransparencyMode,
        background_color: Color,
    ) -> Result<WgpuRenderer, RendererError> {
        WgpuRenderer::new_with_shared(
            self.clone(),
            target,
            size,
            transparency_mode,
            background_color,
        )
        .await
    }
    #[must_use]
    pub fn diagnostics(&self) -> SharedGpuDiagnostics {
        let pipelines = self.inner.pipelines.lock().expect("shared pipeline lock");
        let resources = self.inner.resources.lock().expect("shared resource lock");
        // The shared image map keys always equal the policy entries:
        // admission and eviction update both together, and oversized
        // uploads bypass both.
        debug_assert_eq!(
            resources.registry.image_count(),
            resources.image_textures.retained_entries()
        );
        let image_counters = resources.image_textures.counters();
        let gradient_counters = resources.gradient_textures.counters();
        SharedGpuDiagnostics {
            device_generation: self.inner.device_generation,
            pipeline_variants: pipelines.len(),
            pipeline_count: pipelines.len().saturating_mul(pipeline_contracts().len()),
            shared_image_resources: resources.registry.image_count(),
            shared_glyph_resources: resources.registry.glyph_count(),
            shared_gradient_resources: resources.gradients.len(),
            glyph_atlas_pages: resources.glyph_atlas.live_page_count(),
            shared_image_texel_bytes: resources.image_textures.retained_bytes(),
            shared_image_evictions: image_counters.evictions,
            shared_image_evicted_live: image_counters.evicted_live,
            shared_image_unadmitted: image_counters.unadmitted_uploads,
            shared_image_stale_touches: image_counters.stale_touches,
            shared_gradient_texel_bytes: resources.gradient_textures.retained_bytes(),
            shared_gradient_evictions: gradient_counters.evictions,
            shared_gradient_evicted_live: gradient_counters.evicted_live,
            shared_gradient_unadmitted: gradient_counters.unadmitted_uploads,
            shared_gradient_stale_touches: gradient_counters.stale_touches,
        }
    }
    /// Context-local identity of the currently retained texture for
    /// `image`, if the budget admits one. Eviction drops the identity with
    /// the texture (a re-upload allocates a fresh one), and oversized
    /// uploads never register, so `None` means "not retained", never a
    /// missing allocation.
    ///
    /// Eviction revision of the device-owned image-texture cache: advances
    /// once per dropped entry. Host event loops service per-renderer
    /// reclamation only when this moves, so idle passes cost one integer
    /// comparison and no renderer visits.
    #[must_use]
    pub fn image_eviction_revision(&self) -> u64 {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .image_textures
            .eviction_revision()
    }

    /// Combined texture-eviction revision across the image, gradient, and
    /// glyph-page caches: the saturating sum of all three revisions. Any
    /// family's eviction strictly advances it (none ever decreases), so one
    /// comparison gates host maintenance for all — while no family evicts,
    /// no renderer is visited at all.
    #[must_use]
    pub fn texture_eviction_revision(&self) -> u64 {
        let resources = self.inner.resources.lock().expect("shared resource lock");
        resources
            .image_textures
            .eviction_revision()
            .saturating_add(resources.gradient_textures.eviction_revision())
            .saturating_add(resources.glyph_atlas.eviction_revision())
    }

    /// Whether the shared gradient map currently retains `key`. Used by
    /// renderer debug assertions to verify admitted acquisitions.
    pub(crate) fn gradient_textures_contains(&self, key: &GradientResourceKey) -> bool {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .gradient_textures
            .contains(key)
    }

    #[must_use]
    pub fn image_resource_identity(&self, image: ImageId) -> Option<SharedGpuResourceId> {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .registry
            .images
            .get(&image)
            .copied()
    }
    #[must_use]
    pub fn glyph_resource_identity(&self, glyph: GlyphCacheKey) -> Option<SharedGpuResourceId> {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .registry
            .glyphs
            .get(&glyph)
            .copied()
    }
    pub(crate) fn create_surface(
        &self,
        target: WindowSurfaceTarget,
    ) -> Result<wgpu::Surface<'static>, RendererError> {
        target
            .create_surface(&self.inner.instance)
            .map_err(RendererError::Surface)
    }
    /// Clones the retained pipeline set for `format`, if a renderer has
    /// already created and registered it. Shared reuse path: every later
    /// window on the same format binds these objects instead of building
    /// its own set.
    pub(crate) fn pipeline_resources(
        &self,
        format: wgpu::TextureFormat,
    ) -> Option<Arc<SharedPipelineResources>> {
        self.inner
            .pipelines
            .lock()
            .expect("shared pipeline lock")
            .get(&format)
            .cloned()
    }
    /// Retains one pipeline set per surface format, keeping the first set
    /// when racing initializations build two (the loser drops unretained).
    /// Entries are never removed: the format key space is bounded by
    /// configured surfaces (see the `pipelines` field), so no eviction
    /// budget applies.
    pub(crate) fn register_pipeline_resources(
        &self,
        format: wgpu::TextureFormat,
        resources: SharedPipelineResources,
    ) {
        self.inner
            .pipelines
            .lock()
            .expect("shared pipeline lock")
            .entry(format)
            .or_insert_with(|| Arc::new(resources));
    }
    pub(crate) fn image_resource(
        &self,
        image: &ImageHandle,
    ) -> Result<SharedImageAcquisition, RendererError> {
        let id = image.id();
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        // Split field borrows up front: the liveness closure below observes
        // the texture map while admission mutates the policy.
        let SharedGpuResources {
            images,
            image_textures,
            registry,
            ..
        } = &mut *resources;
        let hit = images.get(&id).cloned();
        if let Some(resource) = hit {
            // Cross-renderer reuse: one window's use refreshes the shared
            // recency order, keeping another window's textures alive. The
            // generation comes from the retained entry itself, so a hit
            // always names the current upload.
            if let Some(generation) = image_textures.generation(id) {
                image_textures.touch(id, generation);
            }
            return Ok(SharedImageAcquisition::new(
                resource,
                false,
                image_textures.generation(id),
            ));
        }
        let decoded = image.decoded();
        let limit = self.inner.device.limits().max_texture_dimension_2d;
        if decoded.width() > limit || decoded.height() > limit {
            return Err(RendererError::ImageTooLarge {
                width: decoded.width(),
                height: decoded.height(),
                limit,
            });
        }
        let bytes = shared_image_texture_bytes(decoded.width(), decoded.height()).ok_or(
            RendererError::ImageTooLarge {
                width: decoded.width(),
                height: decoded.height(),
                limit,
            },
        )?;
        // Oversized-for-budget textures still upload (the caller needs them
        // now and the renderer's local frame-evicted cache retains them),
        // but bypass shared admission: the shared map keys always equal the
        // policy entries, and one giant texture must not evict the working
        // set. Such uploads get a fresh identity without a registry entry.
        let admitted = image_textures.fits(bytes);
        let texture = self.inner.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("incular shared image texture"),
            size: wgpu::Extent3d {
                width: decoded.width(),
                height: decoded.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.inner.texture_upload_bytes.fetch_add(
            decoded.pixels().len() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.inner.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            decoded.pixels().as_ref(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(decoded.width() * 4),
                rows_per_image: Some(decoded.height()),
            },
            wgpu::Extent3d {
                width: decoded.width(),
                height: decoded.height(),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let resource = Arc::new(SharedGpuImage {
            identity: if admitted {
                registry.image_identity(id)
            } else {
                registry.allocate()
            },
            _texture: texture,
            view,
            width: decoded.width(),
            height: decoded.height(),
        });
        if admitted {
            // The map holds one reference; anything above one means a
            // renderer, an active frame, or submitted work still references
            // the texture. Evictions are reported as entry drops, never as
            // texture freeings.
            let generation = resource.identity.get();
            let evicted = image_textures.admit(id, bytes, generation, &|candidate| {
                images
                    .get(&candidate)
                    .is_some_and(|held| Arc::strong_count(held) > 1)
            });
            for eviction in &evicted {
                images.remove(&eviction.id);
                registry.remove_image(eviction.id);
            }
            images.insert(id, Arc::clone(&resource));
        } else {
            image_textures.note_unadmitted_upload();
        }
        let generation = admitted.then(|| resource.identity.get());
        Ok(SharedImageAcquisition::new(resource, true, generation))
    }
    pub(crate) fn rasterize_glyph(
        &self,
        run: &GlyphRun,
        glyph: u16,
        scale: f64,
        protected_pages: &std::collections::HashSet<u16>,
    ) -> Option<RasterizedGlyph> {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        let request = GlyphRasterRequest::new(run.font_size, scale);
        let key = GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: request.physical_size,
        };
        let raster = resources
            .glyph_atlas
            .lookup_or_rasterize(run, glyph, scale, protected_pages);
        // Split field borrows up front: the prune closure below observes
        // the atlas while retirement mutates the texture slots.
        let SharedGpuResources {
            registry,
            glyph_atlas,
            glyph_pages,
            glyph_prune_revision,
            ..
        } = &mut *resources;
        // Retire placement identities evicted while resolving above, then
        // (re-)register this key. Re-registration is idempotent, so the
        // stale-refresh path below neither leaks nor duplicates identities.
        retire_glyph_page_resources(glyph_atlas, registry, glyph_pages, glyph_prune_revision);
        if raster.is_some() {
            registry.glyph_identity(key);
        }
        raster
    }

    pub(crate) fn set_glyph_page_budget(
        &self,
        max_pages: usize,
        protected: &std::collections::HashSet<u16>,
    ) {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        resources.glyph_atlas.set_max_pages(max_pages, protected);
        resources.retire_glyph_resources();
    }

    pub(crate) fn release_glyph_frame_protection(&self) {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        resources.glyph_atlas.release_frame_protection();
        resources.retire_glyph_resources();
    }
    pub(crate) fn glyph_counters(&self) -> GpuCounters {
        self.inner
            .resources
            .lock()
            .expect("shared resource lock")
            .glyph_atlas
            .counters()
    }
    pub(crate) fn gradient_resource(
        &self,
        id: GradientId,
        format: wgpu::TextureFormat,
        pixels: &[[u8; 4]],
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> SharedGradientAcquisition {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        let key = GradientResourceKey {
            gradient: id,
            format,
        };
        // Split field borrows up front: the liveness closure below observes
        // the texture map while admission mutates the policy.
        let SharedGpuResources {
            gradients,
            gradient_textures,
            gradient_generation,
            ..
        } = &mut *resources;
        let hit = gradients.get(&key).cloned();
        if let Some(resource) = hit {
            // The generation comes from the retained entry itself, so a hit
            // always names the current upload.
            let generation = gradient_textures.generation(key);
            if let Some(generation) = generation {
                gradient_textures.touch(key, generation);
            }
            return SharedGradientAcquisition::new(resource, false, generation);
        }
        // Lookup textures are fixed-size: every entry costs the same nominal
        // bytes, so the byte budget behaves as a scaled entry budget. The
        // oversized/bypass path below exists for tiny custom budgets, not
        // for real uploads.
        debug_assert_eq!(
            pixels.len().max(1) * 4,
            SHARED_GRADIENT_TEXEL_BYTES as usize,
            "gradient lookup uploads must stay fixed-size",
        );
        let admitted = gradient_textures.fits(SHARED_GRADIENT_TEXEL_BYTES);
        self.inner.texture_upload_bytes.fetch_add(
            (pixels.len().max(1) * 4) as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        let resource = Arc::new(SharedGpuGradient {
            resource: create_gradient_resource(
                &self.inner.device,
                &self.inner.queue,
                layout,
                sampler,
                pixels,
            ),
        });
        let generation = if admitted {
            *gradient_generation = gradient_generation.saturating_add(1);
            Some(*gradient_generation)
        } else {
            None
        };
        if admitted {
            let evicted = gradient_textures.admit(
                key,
                SHARED_GRADIENT_TEXEL_BYTES,
                generation.expect("admitted uploads name a generation"),
                &|candidate| {
                    gradients
                        .get(&candidate)
                        .is_some_and(|held| Arc::strong_count(held) > 1)
                },
            );
            for eviction in &evicted {
                gradients.remove(&eviction.id);
            }
            gradients.insert(key, Arc::clone(&resource));
        } else {
            gradient_textures.note_unadmitted_upload();
        }
        SharedGradientAcquisition::new(resource, true, generation)
    }
}

/// Drops shared glyph-page slots with no live atlas page, returning how
/// many went. The production retirement path for shared page textures:
/// the shared-context resolve path runs this after draining retired
/// placement identities. Generic over the slot payload so the same path
/// runs with stand-in resources where GPU creation is unavailable.
/// Submitted work stays valid through the wgpu lifetime contract; only
/// vacant pages prune, and pages go vacant solely through budget
/// tightening or eviction turnover — never while frame-pinned.
pub fn prune_vacant_glyph_page_slots<T>(
    slots: &mut [Option<T>],
    page_generation: &dyn Fn(u16) -> Option<u64>,
) -> usize {
    let mut dropped = 0;
    for (index, slot) in slots.iter_mut().enumerate() {
        let vacant = u16::try_from(index)
            .ok()
            .and_then(page_generation)
            .is_none();
        if vacant && slot.is_some() {
            *slot = None;
            dropped += 1;
        }
    }
    dropped
}

/// Applies atlas retirement to its associated identities and shared resources.
/// Backend owners call this after budget changes, protection release, and
/// resolves (including failed resolves). Live frame clones remain valid;
/// vacant slots drop only the shared owner's references.
pub fn retire_glyph_page_resources<T>(
    atlas: &mut GlyphAtlas,
    registry: &mut SharedGpuResourceRegistry,
    slots: &mut [Option<T>],
    last_revision: &mut u64,
) -> usize {
    for key in atlas.take_retired_keys() {
        registry.remove_glyph(key);
    }
    if *last_revision == atlas.eviction_revision() {
        return 0;
    }
    *last_revision = atlas.eviction_revision();
    prune_vacant_glyph_page_slots(slots, &|page| atlas.page_generation(page))
}

impl SharedGpuResources {
    fn retire_glyph_resources(&mut self) {
        retire_glyph_page_resources(
            &mut self.glyph_atlas,
            &mut self.registry,
            &mut self.glyph_pages,
            &mut self.glyph_prune_revision,
        );
    }
}

impl SharedGpuContext {
    pub(crate) fn shared_glyph_texture(&self, page: u16, generation: u64) -> wgpu::Texture {
        let mut resources = self.inner.resources.lock().expect("shared resource lock");
        while resources.glyph_pages.len() <= usize::from(page) {
            resources.glyph_pages.push(None);
        }
        let replace = !matches!(
            &resources.glyph_pages[usize::from(page)],
            Some(slot) if slot.generation == generation
        );
        if replace {
            resources.glyph_pages[usize::from(page)] = Some(SharedGpuAtlasPage {
                texture: self.inner.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("incular shared glyph atlas page"),
                    size: wgpu::Extent3d {
                        width: u32::from(ATLAS_PAGE_SIZE),
                        height: u32::from(ATLAS_PAGE_SIZE),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                }),
                generation,
            });
        }
        resources.glyph_pages[usize::from(page)]
            .as_ref()
            .expect("glyph texture slot just ensured")
            .texture
            .clone()
    }
}

fn adapter_supports_window_transparency(
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
    transparency_mode: TransparencyMode,
) -> bool {
    SurfaceAlphaPlan::select(
        transparency_mode,
        &surface.get_capabilities(adapter).alpha_modes,
    )
    .is_ok()
}

fn select_fallback_adapter(
    mut candidates: impl Iterator<Item = wgpu::Adapter>,
    preference: wgpu::PowerPreference,
) -> Option<wgpu::Adapter> {
    match preference {
        wgpu::PowerPreference::None => candidates.next(),
        wgpu::PowerPreference::LowPower => {
            candidates.max_by_key(|adapter| low_power_rank(adapter.get_info().device_type))
        }
        wgpu::PowerPreference::HighPerformance => {
            candidates.max_by_key(|adapter| high_performance_rank(adapter.get_info().device_type))
        }
    }
}

const fn low_power_rank(device_type: wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::IntegratedGpu => 5,
        wgpu::DeviceType::DiscreteGpu => 4,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Other => 2,
        wgpu::DeviceType::Cpu => 1,
    }
}

const fn high_performance_rank(device_type: wgpu::DeviceType) -> u8 {
    match device_type {
        wgpu::DeviceType::DiscreteGpu => 5,
        wgpu::DeviceType::IntegratedGpu => 4,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Other => 2,
        wgpu::DeviceType::Cpu => 1,
    }
}
