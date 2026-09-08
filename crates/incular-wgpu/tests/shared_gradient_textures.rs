//! Device-owned shared gradient-texture cache policy and coordination.
//!
//! Same scope rules as the image-texture suite: the policy and the
//! renderer-local coordinator hold metadata only, so everything here runs
//! without a display server. The GPU upload path
//! (`SharedGpuContext::gradient_resource`) requires a native window target
//! and is recorded as unverified native behavior. Gradient lookup textures
//! are fixed-size (256 samples × RGBA8), so the byte budget behaves as a
//! scaled entry budget; oversized/bypass behavior is exercised through tiny
//! custom budgets rather than real uploads.

use std::collections::HashMap;
use std::sync::Arc;

use incular_core::Color;
use incular_rendering::{GradientStop, GradientStops};
use incular_wgpu::{
    GradientResourceKey, LocalImageRetention, RendererImageCache, SHARED_GRADIENT_TEXEL_BYTES,
    SharedTextureAcquisition, SharedTextureBudget, SharedTextureCache,
};

/// Mirrors `ensure_gradient`'s resolve step through the production
/// conversion: acquisition → local parts → local insert. The test never
/// touches the conversion internals; identity preservation below proves
/// the outer handle moves over unchanged.
fn resolve_gradient(
    local: &mut RendererImageCache<GradientResourceKey, Arc<TestOuter>>,
    key: GradientResourceKey,
    acquisition: SharedTextureAcquisition<Arc<TestOuter>>,
    frame: u64,
) {
    let (resource, retention) = acquisition.into_local_parts();
    local.insert(key, resource, retention, frame);
}

/// One gradient description with `count` stops spread across 0..=1.
fn stops(seed: u8, count: usize) -> GradientStops {
    GradientStops::new(
        (0..count)
            .map(|index| GradientStop {
                offset: index as f32 / count.max(1) as f32,
                color: Color::rgba(seed, index as u8, 255 - seed, 255),
            })
            .collect(),
    )
}

fn key(stops: &GradientStops) -> GradientResourceKey {
    GradientResourceKey {
        gradient: stops.id(),
        format: wgpu::TextureFormat::Rgba8Unorm,
    }
}

#[derive(Clone, Debug)]
struct TestTexture;

fn test_texture() -> Arc<TestTexture> {
    Arc::new(TestTexture)
}

/// Two-level stand-in mirroring `SharedGpuGradient { resource: GpuGradient }`:
/// the shared wrapper owns an inner renderer-facing handle. A conversion
/// that re-wrapped the inner handle would mint a new outer allocation even
/// with identical content.
#[derive(Clone, Debug)]
struct TestInner;

#[derive(Debug)]
struct TestOuter {
    #[allow(dead_code)]
    inner: TestInner,
}

const DEAD: fn(GradientResourceKey) -> bool = |_| false;

#[test]
fn distinct_descriptions_never_alias() {
    // Separately constructed stops — even byte-identical content — mint
    // distinct identities and occupy distinct shared entries. Only clones
    // intentionally share an entry.
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(10, u64::MAX));
    let first = stops(7, 2);
    let second = stops(7, 2);
    assert_ne!(first.id(), second.id());
    assert!(
        shared
            .admit(key(&first), SHARED_GRADIENT_TEXEL_BYTES, 1, &DEAD)
            .is_empty()
    );
    assert!(
        shared
            .admit(key(&second), SHARED_GRADIENT_TEXEL_BYTES, 2, &DEAD)
            .is_empty()
    );
    assert_eq!(shared.retained_entries(), 2);
    let clone = first.clone();
    assert_eq!(clone.id(), first.id());
    assert!(shared.touch(key(&clone), 1));
    assert_eq!(shared.counters().admissions, 2);
    assert_eq!(shared.counters().shared_hits, 1);
}

#[test]
fn shared_reuse_counts_one_admission() {
    let mut shared = SharedTextureCache::new();
    let stops = stops(3, 3);
    let id = key(&stops);
    assert!(
        shared
            .admit(id, SHARED_GRADIENT_TEXEL_BYTES, 1, &DEAD)
            .is_empty()
    );
    assert!(shared.touch(id, 1));
    assert_eq!(shared.counters().admissions, 1);
    assert_eq!(shared.counters().shared_hits, 1);
    assert_eq!(shared.retained_bytes(), SHARED_GRADIENT_TEXEL_BYTES);
}

#[test]
fn churn_evicts_least_recently_used_first() {
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(2, u64::MAX));
    let (a, b, c) = (stops(11, 2), stops(12, 2), stops(13, 2));
    shared.admit(key(&a), SHARED_GRADIENT_TEXEL_BYTES, 1, &DEAD);
    shared.admit(key(&b), SHARED_GRADIENT_TEXEL_BYTES, 2, &DEAD);
    assert!(shared.touch(key(&a), 1));
    let evicted = shared.admit(key(&c), SHARED_GRADIENT_TEXEL_BYTES, 3, &DEAD);
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, key(&b));
    assert_eq!(evicted[0].bytes, SHARED_GRADIENT_TEXEL_BYTES);
    assert!(!evicted[0].live_elsewhere);
    assert_eq!(shared.generation(key(&a)), Some(1));
    assert_eq!(shared.generation(key(&b)), None);
}

#[test]
fn stale_touch_cannot_refresh_a_replacement() {
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(1, u64::MAX));
    let mut local = RendererImageCache::new();
    let old = stops(21, 2);
    shared.admit(key(&old), SHARED_GRADIENT_TEXEL_BYTES, 7, &DEAD);
    local.insert(
        key(&old),
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        1,
    );
    // Churn evicts the entry and the same description re-uploads under a
    // new generation while this client is not syncing.
    let next = stops(22, 2);
    shared.admit(key(&next), SHARED_GRADIENT_TEXEL_BYTES, 8, &DEAD);
    shared.admit(key(&old), SHARED_GRADIENT_TEXEL_BYTES, 9, &DEAD);
    assert!(local.record_use(key(&old), 2));
    let (touched, pruned) = local.sync_with_shared(&mut shared);
    assert_eq!(touched, 0);
    assert_eq!(pruned, vec![key(&old)]);
    assert_eq!(shared.counters().stale_touches, 1);
    assert_eq!(shared.generation(key(&old)), Some(9));
}

#[test]
fn bypassed_gradients_reuse_locally_with_bounded_retention() {
    // A sub-kilobyte custom budget refuses every real lookup texture, so
    // the bypass path is exercisable without any GPU upload: drawing
    // reuses the local resource with zero shared contact, and the age rule
    // releases it once unused.
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(100, 100));
    let mut local = RendererImageCache::new();
    let stops = stops(31, 2);
    assert!(!shared.fits(SHARED_GRADIENT_TEXEL_BYTES));
    assert!(
        shared
            .admit(key(&stops), SHARED_GRADIENT_TEXEL_BYTES, 1, &DEAD)
            .is_empty()
    );
    assert_eq!(shared.retained_entries(), 0);
    shared.note_unadmitted_upload();
    local.insert(
        key(&stops),
        test_texture(),
        LocalImageRetention::Bypassed,
        1,
    );
    for frame in 2..=4u64 {
        assert!(local.record_use(key(&stops), frame));
        let (touched, pruned) = local.sync_with_shared(&mut shared);
        assert_eq!(touched, 0);
        assert!(pruned.is_empty());
    }
    assert_eq!(shared.counters().shared_hits, 0);
    assert_eq!(shared.counters().admissions, 0);
    assert_eq!(shared.counters().unadmitted_uploads, 1);
    assert!(local.contains(&key(&stops)));
    assert_eq!(local.evict_unused(605, 600), 1);
    assert!(!local.contains(&key(&stops)));
}

#[test]
fn idle_reclaim_releases_shared_entries_but_never_bypassed_ones() {
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(1, u64::MAX));
    let mut idle = RendererImageCache::new();
    let (old, big) = (stops(41, 2), stops(42, 2));
    shared.admit(key(&old), SHARED_GRADIENT_TEXEL_BYTES, 7, &DEAD);
    idle.insert(
        key(&old),
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        1,
    );
    idle.insert(key(&big), test_texture(), LocalImageRetention::Bypassed, 1);
    // Churn evicts the shared entry while the client presents nothing.
    let next = stops(43, 2);
    shared.admit(key(&next), SHARED_GRADIENT_TEXEL_BYTES, 8, &DEAD);
    // Production idle orchestration: only the shared-backed entry goes.
    let dropped = idle.reclaim_stale(&shared);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, key(&old));
    assert!(idle.contains(&key(&big)));
    assert_eq!(idle.len(), 1);
}

#[test]
fn client_closure_and_metadata_reclamation() {
    // Eviction drops map, policy, and (for images) registry state together;
    // gradients carry no registry identity, so reclaiming one is policy +
    // map only — and re-admission works with a fresh generation.
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(1, u64::MAX));
    let (a, b) = (stops(51, 2), stops(52, 2));
    shared.admit(key(&a), SHARED_GRADIENT_TEXEL_BYTES, 7, &DEAD);
    let evicted = shared.admit(key(&b), SHARED_GRADIENT_TEXEL_BYTES, 8, &DEAD);
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, key(&a));
    assert_eq!(shared.retained_entries(), 1);
    assert_eq!(shared.retained_bytes(), SHARED_GRADIENT_TEXEL_BYTES);
    // Re-admitting the evicted description works and mints no alias with
    // the entry that replaced it.
    let re_evicted = shared.admit(key(&a), SHARED_GRADIENT_TEXEL_BYTES, 9, &DEAD);
    assert_eq!(re_evicted.len(), 1);
    assert_eq!(re_evicted[0].id, key(&b));
    assert_eq!(shared.generation(key(&a)), Some(9));
    assert_eq!(shared.counters().admissions, 3);
    assert_eq!(shared.counters().evictions, 2);
}

#[test]
fn cross_client_local_hits_protect_shared_entries() {
    // Two local coordinators, one shared owner: client A draws one
    // gradient every frame while client B churns. The budget fits the
    // steady working set (the working entry plus two churn slots), so A's
    // batched touches protect its entry while each older churn retires.
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(3, u64::MAX));
    let mut client_a = RendererImageCache::new();
    let mut client_b = RendererImageCache::new();
    let work = stops(61, 2);
    assert!(
        shared
            .admit(key(&work), SHARED_GRADIENT_TEXEL_BYTES, 7, &DEAD)
            .is_empty()
    );
    client_a.insert(
        key(&work),
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        0,
    );
    let churn: Vec<GradientStops> = (62..67u8).map(|seed| stops(seed, 2)).collect();
    for (index, stops) in churn.iter().enumerate() {
        let frame = index as u64 + 1;
        assert!(client_a.record_use(key(&work), frame));
        let evicted = shared.admit(key(stops), SHARED_GRADIENT_TEXEL_BYTES, 10 + frame, &DEAD);
        assert!(
            evicted.iter().all(|entry| entry.id != key(&work)),
            "A's batched use must protect the working entry"
        );
        if frame >= 3 {
            // Each frame retires the churn entry admitted two frames ago.
            assert_eq!(evicted.len(), 1);
            assert_eq!(evicted[0].id, key(&churn[(frame - 3) as usize]));
        }
        client_b.insert(
            key(stops),
            test_texture(),
            LocalImageRetention::Shared {
                generation: 10 + frame,
            },
            frame,
        );
        let (touched, pruned) = client_a.sync_with_shared(&mut shared);
        assert_eq!(touched, 1);
        assert!(pruned.is_empty());
        let _ = client_b.sync_with_shared(&mut shared);
    }
    assert!(client_a.contains(&key(&work)));
    assert_eq!(shared.generation(key(&work)), Some(7));
}

#[test]
fn acquisition_conversion_preserves_outer_arc_identity() {
    // The production conversion moves the shared outer handle into local
    // retention unchanged. `Arc::ptr_eq` distinguishes that from
    // re-wrapping the inner handle, which would mint a new allocation even
    // with identical content — the exact bug class this locks out.
    let outer = Arc::new(TestOuter { inner: TestInner });
    let acquisition = SharedTextureAcquisition::new(Arc::clone(&outer), true, Some(11));
    // Admission is derived, not stored: no second flag can contradict the
    // generation in any build.
    assert!(acquisition.admitted());
    let (resource, retention) = acquisition.into_local_parts();
    assert!(Arc::ptr_eq(&resource, &outer));
    assert_eq!(retention, LocalImageRetention::Shared { generation: 11 });
    // Bypassed acquisitions resolve to local-only retention with the same
    // move semantics.
    let acquisition = SharedTextureAcquisition::new(Arc::clone(&outer), true, None);
    assert!(!acquisition.admitted());
    let (resource, retention) = acquisition.into_local_parts();
    assert!(Arc::ptr_eq(&resource, &outer));
    assert_eq!(retention, LocalImageRetention::Bypassed);
}

#[test]
fn gradient_eviction_observes_renderer_ownership() {
    // End-to-end through the production pieces: shared admission, the
    // production resolve step, churn eviction with a real strong-count
    // closure over the wiring-shaped map, and local pruning. The evicted
    // entry must report live while the local entry holds the same `Arc`
    // the shared map held.
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(1, u64::MAX));
    let mut map: HashMap<GradientResourceKey, Arc<TestOuter>> = HashMap::new();
    let mut local = RendererImageCache::new();
    let held = stops(71, 2);
    let held_key = key(&held);
    let outer = Arc::new(TestOuter { inner: TestInner });
    assert!(
        shared
            .admit(held_key, SHARED_GRADIENT_TEXEL_BYTES, 7, &DEAD)
            .is_empty()
    );
    map.insert(held_key, Arc::clone(&outer));
    // Same conversion `ensure_gradient` applies: the shared `Arc` moves
    // into local retention, so the map and the local entry alias one
    // allocation.
    resolve_gradient(
        &mut local,
        held_key,
        SharedTextureAcquisition::new(Arc::clone(&outer), false, Some(7)),
        1,
    );
    assert!(Arc::ptr_eq(
        &local.get(&held_key).expect("local entry").resource,
        &map[&held_key],
    ));
    // Churn evicts with liveness observed exactly as the wiring observes
    // it: map references beyond the map's own.
    let next = stops(72, 2);
    let evicted = shared.admit(key(&next), SHARED_GRADIENT_TEXEL_BYTES, 8, &|id| {
        map.get(&id).is_some_and(|arc| Arc::strong_count(arc) > 1)
    });
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, held_key);
    assert!(
        evicted[0].live_elsewhere,
        "the locally retained Arc must be observed"
    );
    // Wiring drops the map entry; the local entry keeps the texture alive
    // until the production reclaim path releases it.
    map.remove(&held_key);
    let probe = Arc::downgrade(&outer);
    assert!(probe.upgrade().is_some());
    let dropped = local.reclaim_stale(&shared);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, held_key);
    drop(dropped);
    drop(outer);
    assert!(probe.upgrade().is_none());
}

#[test]
fn superseded_format_entries_decay_without_immediate_retirement() {
    // The same stops under two surface formats are distinct shared
    // entries. Nothing retires the old format immediately on reconfigure:
    // reconfigured lookups simply stop naming it. This test locks in that
    // decay contract — locally unreachable entries age out, shared entries
    // linger until LRU pressure — instead of claiming eager retirement.
    let old_format = wgpu::TextureFormat::Rgba8Unorm;
    let new_format = wgpu::TextureFormat::Bgra8Unorm;
    let content = stops(81, 2);
    let old_key = GradientResourceKey {
        gradient: content.id(),
        format: old_format,
    };
    let new_key = GradientResourceKey {
        gradient: content.id(),
        format: new_format,
    };
    let mut shared = SharedTextureCache::with_limits(SharedTextureBudget::new(10, u64::MAX));
    let mut local = RendererImageCache::new();
    shared.admit(old_key, SHARED_GRADIENT_TEXEL_BYTES, 7, &DEAD);
    local.insert(
        old_key,
        Arc::new(TestOuter { inner: TestInner }),
        LocalImageRetention::Shared { generation: 7 },
        1,
    );
    // "Reconfigure": all subsequent resolves name the new format only.
    shared.admit(new_key, SHARED_GRADIENT_TEXEL_BYTES, 8, &DEAD);
    local.insert(
        new_key,
        Arc::new(TestOuter { inner: TestInner }),
        LocalImageRetention::Shared { generation: 8 },
        2,
    );
    assert!(local.record_use(new_key, 3));
    // No immediate retirement happened on either side.
    assert!(local.contains(&old_key));
    assert_eq!(shared.generation(old_key), Some(7));
    // The old-format local entry is unreachable now: with no further use
    // recorded, the age bound releases it while the new-format entry stays.
    assert_eq!(local.evict_unused(602, 600), 1);
    assert!(!local.contains(&old_key));
    assert!(local.contains(&new_key));
    // The old-format shared entry lingers without pressure — and goes
    // first once churn arrives, since nothing refreshes it.
    assert_eq!(shared.generation(old_key), Some(7));
    for index in 0..9u64 {
        let churn = stops(82 + index as u8, 2);
        shared.admit(
            GradientResourceKey {
                gradient: churn.id(),
                format: new_format,
            },
            SHARED_GRADIENT_TEXEL_BYTES,
            20 + index,
            &DEAD,
        );
    }
    assert_eq!(shared.generation(old_key), None);
    assert_eq!(shared.generation(new_key), Some(8));
}
