//! Device-owned shared image-texture cache policy.
//!
//! These tests exercise admission, cross-client reuse, LRU eviction, budgets,
//! and accounting through the public policy API without a display server —
//! the policy holds metadata only, no wgpu types. The GPU upload path
//! (`SharedGpuContext::image_resource`) requires a native window target and
//! is not covered here; that is recorded as unverified native behavior,
//! consistent with the repository's native-test policy. Texture lifetime
//! after eviction rests on wgpu's drop-after-submit contract plus ordinary
//! `Arc` ownership, mirrored by the client-close test below with real
//! reference counts.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use incular_image::ImageHandle;
use incular_wgpu::{
    LocalImageRetention, RendererImageCache, SharedImageTextureBudget, SharedImageTextureCache,
    shared_image_texture_bytes,
};

/// Distinct content identities from CPU-only handles (no GPU involved).
fn image_id(seed: u8) -> incular_image::ImageId {
    let pixels = [seed, seed.wrapping_mul(3), seed.wrapping_add(7), 255];
    ImageHandle::from_rgba8(1, 1, Arc::<[u8]>::from(pixels))
        .expect("test handle")
        .id()
}

const DEAD: fn(incular_image::ImageId) -> bool = |_| false;

#[test]
fn shared_reuse_across_two_clients_counts_one_admission() {
    // One shared instance, two simulated renderer clients interleaving use:
    // the second client's touch is a hit, not a second upload.
    let mut cache = SharedImageTextureCache::new();
    let id = image_id(1);
    assert!(cache.admit(id, 64, 1, &DEAD).is_empty());
    assert!(cache.touch(id, 1));
    let counters = cache.counters();
    assert_eq!(counters.admissions, 1);
    assert_eq!(counters.shared_hits, 1);
    assert_eq!(cache.retained_entries(), 1);
    assert_eq!(cache.retained_bytes(), 64);
}

#[test]
fn eviction_order_is_lru_across_clients() {
    let mut cache =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(2, u64::MAX));
    let (x, y, z) = (image_id(11), image_id(12), image_id(13));
    cache.admit(x, 100, 1, &DEAD);
    cache.admit(y, 100, 1, &DEAD);
    // Client B touches X, making Y the victim even though Y is newer.
    assert!(cache.touch(x, 1));
    let evicted = cache.admit(z, 100, 1, &DEAD);
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, y);
    assert_eq!(evicted[0].bytes, 100);
    assert!(!evicted[0].live_elsewhere);
    assert_eq!(cache.retained_entries(), 2);
    assert_eq!(cache.retained_bytes(), 200);
    let counters = cache.counters();
    assert_eq!(counters.admissions, 3);
    assert_eq!(counters.evictions, 1);
    assert_eq!(counters.evicted_bytes, 100);
    assert_eq!(counters.evicted_live, 0);
    // The survivor still hits; the victim re-admits as new work.
    assert!(cache.touch(x, 1));
    assert!(!cache.touch(y, 1));
}

#[test]
fn live_references_are_reported_not_freed() {
    let mut cache =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let (held, cold) = (image_id(21), image_id(22));
    cache.admit(held, 100, 1, &DEAD);
    // Selective liveness: only `held` is referenced elsewhere at eviction.
    let evicted = cache.admit(cold, 100, 2, &|id| id == held);
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, held);
    assert!(evicted[0].live_elsewhere);
    assert_eq!(cache.counters().evicted_live, 1);
    assert_eq!(cache.counters().evicted_bytes, 100);
}

#[test]
fn byte_budget_bounds_residency_exactly() {
    let mut cache = SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(100, 300));
    let (a, b, c) = (image_id(31), image_id(32), image_id(33));
    assert!(cache.admit(a, 100, 1, &DEAD).is_empty());
    assert!(cache.admit(b, 200, 1, &DEAD).is_empty());
    assert_eq!(cache.retained_bytes(), 300);
    // 350 > 300: oldest-first evicts exactly A, leaving B + C at 250.
    let evicted = cache.admit(c, 50, 1, &DEAD);
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, a);
    assert_eq!(cache.retained_bytes(), 250);
    assert_eq!(cache.retained_entries(), 2);
}

#[test]
fn oversized_entries_do_not_admit() {
    let mut cache = SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(100, 300));
    let big = image_id(41);
    assert!(!cache.fits(301));
    assert!(cache.fits(300));
    // Refused without effect: no entry, no eviction of others.
    let small = image_id(42);
    cache.admit(small, 100, 1, &DEAD);
    assert!(cache.admit(big, 301, 1, &DEAD).is_empty());
    assert_eq!(cache.retained_entries(), 1);
    assert_eq!(cache.retained_bytes(), 100);
    cache.note_unadmitted_upload();
    assert_eq!(cache.counters().unadmitted_uploads, 1);
    assert_eq!(cache.counters().admissions, 1);
}

#[test]
fn zero_limits_admit_nothing() {
    let mut cache = SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(0, 0));
    assert!(!cache.fits(1));
    assert!(cache.admit(image_id(51), 64, 1, &DEAD).is_empty());
    assert_eq!(cache.retained_entries(), 0);
    assert_eq!(cache.counters().admissions, 0);
}

#[test]
fn set_limits_trims_oldest_first() {
    let mut cache = SharedImageTextureCache::new();
    let ids = [image_id(61), image_id(62), image_id(63)];
    for id in ids {
        cache.admit(id, 100, 1, &DEAD);
    }
    let evicted = cache.set_limits(SharedImageTextureBudget::new(1, u64::MAX), &DEAD);
    assert_eq!(evicted.len(), 2);
    assert_eq!([evicted[0].id, evicted[1].id], [ids[0], ids[1]]);
    assert_eq!(cache.retained_entries(), 1);
    assert!(cache.touch(ids[2], 1));
}

#[test]
fn re_admission_after_eviction_works() {
    let mut cache =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let (a, b) = (image_id(71), image_id(72));
    cache.admit(a, 100, 1, &DEAD);
    cache.admit(b, 100, 1, &DEAD);
    assert!(!cache.touch(a, 1));
    let re_evicted = cache.admit(a, 100, 2, &DEAD);
    assert_eq!(re_evicted.len(), 1);
    assert_eq!(re_evicted[0].id, b);
    assert!(cache.touch(a, 2));
    assert_eq!(cache.counters().admissions, 3);
}

#[test]
fn checked_texture_bytes_rejects_overflow() {
    assert_eq!(shared_image_texture_bytes(4, 4), Some(64));
    assert_eq!(shared_image_texture_bytes(0, 5), Some(0));
    assert_eq!(
        shared_image_texture_bytes(16_384, 16_384),
        Some(1_073_741_824)
    );
    assert_eq!(shared_image_texture_bytes(u32::MAX, u32::MAX), None);
}

#[test]
fn touch_missing_identity_reports_false() {
    let mut cache = SharedImageTextureCache::new();
    assert!(!cache.touch(image_id(81), 1));
    assert_eq!(cache.counters().shared_hits, 0);
}

#[test]
fn client_close_reclaims_evicted_ownership() {
    // Mirrors the wiring contract with real reference counts: the shared
    // map holds one `Arc`, a renderer client holds another, and eviction
    // drops only the map's reference. Releasing the client then frees the
    // resource fully — reclamation needs no GPU, just ownership.
    let mut cache =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let (pinned, next) = (image_id(91), image_id(92));
    let mut map: HashMap<incular_image::ImageId, Arc<()>> = HashMap::new();
    map.insert(pinned, Arc::new(()));
    cache.admit(pinned, 100, 1, &|id| {
        map.get(&id).is_some_and(|arc| Arc::strong_count(arc) > 1)
    });
    // The "renderer client" clones the handle out of the map.
    let client: Arc<()> = Arc::clone(&map[&pinned]);
    let probe: Weak<()> = Arc::downgrade(&client);
    let evicted = cache.admit(next, 100, 2, &|id| {
        map.get(&id).is_some_and(|arc| Arc::strong_count(arc) > 1)
    });
    assert_eq!(evicted.len(), 1);
    assert!(evicted[0].live_elsewhere);
    // Wiring drops the map entry; the client's clone keeps it alive.
    map.remove(&pinned);
    assert!(probe.upgrade().is_some());
    // The client closing releases the last reference: fully reclaimed.
    drop(client);
    assert!(probe.upgrade().is_none());
}

/// Stand-in retained resource for the renderer-local coordinator: the
/// coordination logic (batching, generations, pruning) never inspects it,
/// exactly as production's logic never inspects GPU entry contents.
#[derive(Clone, Debug)]
struct TestTexture;

fn test_texture() -> Arc<TestTexture> {
    Arc::new(TestTexture)
}

#[test]
fn local_hits_keep_shared_entry_alive_across_churn() {
    // One shared owner, two renderer-local coordinators. Client A draws one
    // image every frame; client B churns a fresh image per frame. The budget
    // fits the steady working set (the working image plus two churn slots),
    // so A's once-per-frame batched touch must protect its entry across the
    // whole run while each older churn image falls out in turn.
    let mut shared =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(3, u64::MAX));
    let mut client_a = RendererImageCache::new();
    let mut client_b = RendererImageCache::new();
    let work = image_id(101);
    // Content identities mint once: precompute the churn ids up front, since
    // every `image_id` call mints a fresh identity.
    let churn_ids: Vec<incular_image::ImageId> = (111..=115u8).map(image_id).collect();
    assert!(shared.admit(work, 100, 7, &DEAD).is_empty());
    client_a.insert(
        work,
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        0,
    );
    for frame in 1..=5u64 {
        assert!(client_a.record_use(work, frame));
        let churn = churn_ids[(frame - 1) as usize];
        let evicted = shared.admit(churn, 100, 100 + frame, &DEAD);
        assert!(
            evicted.iter().all(|entry| entry.id != work),
            "A's batched use must protect the working entry"
        );
        if frame >= 3 {
            // Each frame retires the churn image admitted two frames ago;
            // the churn client prunes its stale local copy on sync.
            assert_eq!(
                evicted.iter().map(|entry| entry.id).collect::<Vec<_>>(),
                vec![churn_ids[(frame - 3) as usize]],
            );
        }
        client_b.insert(
            churn,
            test_texture(),
            LocalImageRetention::Shared {
                generation: 100 + frame,
            },
            frame,
        );
        let (touched, pruned) = client_a.sync_with_shared(&mut shared);
        assert_eq!(touched, 1);
        assert!(pruned.is_empty());
        let (_, pruned_b) = client_b.sync_with_shared(&mut shared);
        if frame >= 3 {
            assert_eq!(pruned_b.len(), 1);
        }
    }
    assert!(client_a.contains(&work));
    assert_eq!(shared.generation(work), Some(7));
    // A goes quiet: churn evicts the working entry, and A's next sync
    // prunes its stale local entry instead of refreshing anything.
    for frame in 6..=7u64 {
        let churn = image_id(120 + frame as u8);
        shared.admit(churn, 100, 200 + frame, &DEAD);
    }
    assert_eq!(shared.generation(work), None);
    let (touched, pruned) = client_a.sync_with_shared(&mut shared);
    assert_eq!(touched, 0);
    assert_eq!(pruned, vec![work]);
    assert!(!client_a.contains(&work));
}

#[test]
fn stale_touch_cannot_refresh_a_replacement() {
    let mut shared =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let mut client = RendererImageCache::new();
    let x = image_id(121);
    shared.admit(x, 100, 7, &DEAD);
    client.insert(
        x,
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        1,
    );
    // Another client evicts x and the same identity is re-uploaded under a
    // new generation while this client is not syncing.
    let y = image_id(122);
    shared.admit(y, 100, 8, &DEAD);
    shared.admit(x, 100, 9, &DEAD);
    // The first client draws its stale entry and syncs: the touch names
    // generation 7 against a generation-9 entry, so nothing refreshes and
    // the stale touch is counted.
    assert!(client.record_use(x, 2));
    let (touched, pruned) = client.sync_with_shared(&mut shared);
    assert_eq!(touched, 0);
    assert_eq!(pruned, vec![x]);
    assert_eq!(shared.counters().stale_touches, 1);
    assert_eq!(shared.counters().shared_hits, 0);
    assert_eq!(shared.generation(x), Some(9));
    // Re-resolving converges onto the single current generation.
    client.insert(
        x,
        test_texture(),
        LocalImageRetention::Shared { generation: 9 },
        3,
    );
    let (touched, pruned) = client.sync_with_shared(&mut shared);
    assert_eq!(touched, 1);
    assert!(pruned.is_empty());
}

#[test]
fn idle_client_stale_ownership_is_reclaimable() {
    let mut shared =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(2, u64::MAX));
    let mut idle = RendererImageCache::new();
    let (a, b) = (image_id(131), image_id(132));
    shared.admit(a, 100, 7, &DEAD);
    idle.insert(
        a,
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        1,
    );
    shared.admit(b, 100, 8, &DEAD);
    idle.insert(
        b,
        test_texture(),
        LocalImageRetention::Shared { generation: 8 },
        1,
    );
    // The client presents no further frames while churn evicts both entries
    // from shared ownership.
    for (index, seed) in (133..135u8).enumerate() {
        shared.admit(image_id(seed), 100, 20 + index as u64, &DEAD);
    }
    assert_eq!(shared.generation(a), None);
    assert_eq!(shared.generation(b), None);
    // Reclaim through the production idle orchestration — not the raw
    // pruning helper: release against the live shared generations with no
    // frame activity. One clone stands in for submitted work still
    // referencing the texture.
    let inflight = Arc::clone(&idle.get(&a).expect("local entry").resource);
    let probe: Weak<TestTexture> = Arc::downgrade(&inflight);
    let dropped = idle.reclaim_stale(&shared);
    assert_eq!(dropped.len(), 2);
    assert!(idle.is_empty());
    assert!(probe.upgrade().is_some());
    drop(dropped);
    drop(inflight);
    assert!(probe.upgrade().is_none());
}

#[test]
fn shared_gauges_count_retention_not_outstanding_holders() {
    // Mirrors the wiring ops: a simulated shared map plus client clones,
    // with liveness observed from real reference counts.
    let mut shared =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(2, u64::MAX));
    let mut map: HashMap<incular_image::ImageId, Arc<TestTexture>> = HashMap::new();
    let (x, y, z) = (image_id(141), image_id(142), image_id(143));
    for (id, generation) in [(x, 1), (y, 2)] {
        assert!(shared.admit(id, 100, generation, &DEAD).is_empty());
        map.insert(id, test_texture());
    }
    let client = Arc::clone(&map[&x]);
    let liveness =
        |id: incular_image::ImageId| map.get(&id).is_some_and(|arc| Arc::strong_count(arc) > 1);
    let evicted = shared.admit(z, 100, 3, &liveness);
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].id, x);
    assert!(evicted[0].live_elsewhere);
    map.remove(&x);
    // Gauges describe retention only: two entries, 200 nominal bytes. The
    // outstanding client holder is invisible here and counted instead.
    assert_eq!(shared.retained_entries(), 2);
    assert_eq!(shared.retained_bytes(), 200);
    assert_eq!(shared.counters().evictions, 1);
    assert_eq!(shared.counters().evicted_live, 1);
    assert_eq!(shared.counters().evicted_bytes, 100);
    assert!(Arc::strong_count(&client) >= 1);
    drop(client);
}

#[test]
fn frame_use_drains_once_per_id() {
    let mut local = RendererImageCache::new();
    let id = image_id(151);
    local.insert(
        id,
        test_texture(),
        LocalImageRetention::Shared { generation: 4 },
        1,
    );
    assert!(local.record_use(id, 2));
    assert!(local.record_use(id, 3));
    assert_eq!(
        local.drain_frame_use(),
        vec![(id, LocalImageRetention::Shared { generation: 4 })]
    );
    assert!(local.drain_frame_use().is_empty());
}

#[test]
fn unused_local_entries_evict_by_age() {
    let mut local = RendererImageCache::new();
    let (a, b) = (image_id(161), image_id(162));
    local.insert(
        a,
        test_texture(),
        LocalImageRetention::Shared { generation: 1 },
        10,
    );
    local.insert(
        b,
        test_texture(),
        LocalImageRetention::Shared { generation: 1 },
        10,
    );
    assert!(local.record_use(a, 15));
    // B unseen for 5 frames exceeds a 4-frame budget; A was just used.
    assert_eq!(local.evict_unused(15, 4), 1);
    assert!(local.contains(&a));
    assert!(!local.contains(&b));
}

#[test]
fn bypassed_images_reuse_locally_with_bounded_retention() {
    // An oversized-for-budget image has no shared entry: drawing it across
    // frames reuses the local resource with zero shared contact, and once
    // unused it is bounded by the same age rule instead of lingering.
    let mut shared = SharedImageTextureCache::new();
    let mut local = RendererImageCache::new();
    let big = image_id(171);
    local.insert(big, test_texture(), LocalImageRetention::Bypassed, 1);
    for frame in 2..=4u64 {
        assert!(local.record_use(big, frame));
        let (touched, pruned) = local.sync_with_shared(&mut shared);
        assert_eq!(touched, 0);
        assert!(pruned.is_empty());
    }
    assert_eq!(shared.counters().shared_hits, 0);
    assert_eq!(shared.counters().admissions, 0);
    assert!(local.contains(&big));
    // Unused for 601 frames past its last use at frame 4: released.
    assert_eq!(local.evict_unused(605, 600), 1);
    assert!(!local.contains(&big));
}

#[test]
fn generation_pruning_applies_only_to_shared_entries() {
    // A stale shared-backed entry and a bypassed entry side by side:
    // reclamation drops exactly the shared one. The bypassed entry has no
    // shared generation to compare, so pruning can never release it — only
    // the age bound can.
    let mut shared =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let mut local = RendererImageCache::new();
    let (old, big, next) = (image_id(181), image_id(182), image_id(183));
    shared.admit(old, 100, 7, &DEAD);
    local.insert(
        old,
        test_texture(),
        LocalImageRetention::Shared { generation: 7 },
        1,
    );
    local.insert(big, test_texture(), LocalImageRetention::Bypassed, 1);
    // Supersede the shared entry while the client is not syncing.
    shared.admit(next, 100, 8, &DEAD);
    let dropped = local.reclaim_stale(&shared);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].0, old);
    assert!(local.contains(&big));
    assert_eq!(local.len(), 1);
}

#[test]
fn cumulative_counters_do_not_measure_current_retention() {
    // Two histories with identical cumulative counters but different live
    // sets: counters alone cannot establish current retained bytes.
    let history = |first: u8, second: u8| {
        let mut shared =
            SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
        let a = image_id(first);
        let b = image_id(second);
        shared.admit(a, 100, 1, &DEAD);
        shared.admit(b, 100, 2, &DEAD);
        (shared.counters(), shared.retained_bytes(), b)
    };
    let (counters_one, bytes_one, live_one) = history(191, 192);
    let (counters_two, bytes_two, live_two) = history(193, 194);
    assert_eq!(counters_one, counters_two);
    assert_eq!(bytes_one, 100);
    assert_eq!(bytes_two, 100);
    assert_ne!(live_one, live_two);
}
