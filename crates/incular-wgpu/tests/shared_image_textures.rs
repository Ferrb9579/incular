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
use incular_wgpu::{SharedImageTextureBudget, SharedImageTextureCache, shared_image_texture_bytes};

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
    assert!(cache.admit(id, 64, &DEAD).is_empty());
    assert!(cache.touch(id));
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
    cache.admit(x, 100, &DEAD);
    cache.admit(y, 100, &DEAD);
    // Client B touches X, making Y the victim even though Y is newer.
    assert!(cache.touch(x));
    let evicted = cache.admit(z, 100, &DEAD);
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
    assert!(cache.touch(x));
    assert!(!cache.touch(y));
}

#[test]
fn live_references_are_reported_not_freed() {
    let mut cache =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let (held, cold) = (image_id(21), image_id(22));
    cache.admit(held, 100, &DEAD);
    // Selective liveness: only `held` is referenced elsewhere at eviction.
    let evicted = cache.admit(cold, 100, &|id| id == held);
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
    assert!(cache.admit(a, 100, &DEAD).is_empty());
    assert!(cache.admit(b, 200, &DEAD).is_empty());
    assert_eq!(cache.retained_bytes(), 300);
    // 350 > 300: oldest-first evicts exactly A, leaving B + C at 250.
    let evicted = cache.admit(c, 50, &DEAD);
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
    cache.admit(small, 100, &DEAD);
    assert!(cache.admit(big, 301, &DEAD).is_empty());
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
    assert!(cache.admit(image_id(51), 64, &DEAD).is_empty());
    assert_eq!(cache.retained_entries(), 0);
    assert_eq!(cache.counters().admissions, 0);
}

#[test]
fn set_limits_trims_oldest_first() {
    let mut cache = SharedImageTextureCache::new();
    let ids = [image_id(61), image_id(62), image_id(63)];
    for id in ids {
        cache.admit(id, 100, &DEAD);
    }
    let evicted = cache.set_limits(SharedImageTextureBudget::new(1, u64::MAX), &DEAD);
    assert_eq!(evicted.len(), 2);
    assert_eq!([evicted[0].id, evicted[1].id], [ids[0], ids[1]]);
    assert_eq!(cache.retained_entries(), 1);
    assert!(cache.touch(ids[2]));
}

#[test]
fn re_admission_after_eviction_works() {
    let mut cache =
        SharedImageTextureCache::with_limits(SharedImageTextureBudget::new(1, u64::MAX));
    let (a, b) = (image_id(71), image_id(72));
    cache.admit(a, 100, &DEAD);
    cache.admit(b, 100, &DEAD);
    assert!(!cache.touch(a));
    let re_evicted = cache.admit(a, 100, &DEAD);
    assert_eq!(re_evicted.len(), 1);
    assert_eq!(re_evicted[0].id, b);
    assert!(cache.touch(a));
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
    assert!(!cache.touch(image_id(81)));
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
    cache.admit(pinned, 100, &|id| {
        map.get(&id).is_some_and(|arc| Arc::strong_count(arc) > 1)
    });
    // The "renderer client" clones the handle out of the map.
    let client: Arc<()> = Arc::clone(&map[&pinned]);
    let probe: Weak<()> = Arc::downgrade(&client);
    let evicted = cache.admit(next, 100, &|id| {
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
