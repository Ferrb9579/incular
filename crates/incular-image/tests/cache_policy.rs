//! Owned CPU image-cache policy: budgets, eviction, live handles, errors.
//!
//! Payloads are generated through the public encoder so every test decodes
//! real PNG bytes through `ImageCache::load_bytes`, exactly like production.

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use incular_image::{ImageCache, ImageCacheLimits, ImageError};

/// Encodes a `width` x `height` RGBA PNG whose pixels vary with `seed`, so
/// different seeds yield different encoded payloads of equal decoded size.
fn png(width: u32, height: u32, seed: u8) -> Vec<u8> {
    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|i| {
            let i = i as u8;
            [
                seed.wrapping_add(i),
                seed.wrapping_mul(3).wrapping_add(i / 7),
                seed.wrapping_add(255 - i),
                255,
            ]
        })
        .collect();
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encode test png");
    encoded
}

#[test]
fn repeated_hits_decode_once_and_share_identity() {
    let bytes = png(4, 4, 7);
    let mut cache = ImageCache::new();
    let first = cache.load_bytes(&bytes).unwrap();
    let second = cache.load_bytes(bytes.clone()).unwrap();
    let third = cache.load_bytes(&bytes).unwrap();
    assert_eq!(first, second);
    assert_eq!(second, third);
    assert_eq!(cache.len(), 1);
    let diagnostics = cache.diagnostics();
    assert_eq!(diagnostics.loads_requested, 3);
    assert_eq!(diagnostics.image_decodes, 1);
    assert_eq!(diagnostics.load_cache_hits, 2);
    assert_eq!(diagnostics.evictions, 0);
}

#[test]
fn entry_limit_evicts_least_recently_used_first() {
    let (a, b, c) = (png(4, 4, 1), png(4, 4, 2), png(4, 4, 3));
    assert!(a != b && b != c && a != c);
    let mut cache = ImageCache::with_limits(ImageCacheLimits::new(2, usize::MAX));
    let a_first = cache.load_bytes(&a).unwrap();
    let b_first = cache.load_bytes(&b).unwrap();
    // Touch A so B becomes the eviction victim even though B is newer.
    assert_eq!(cache.load_bytes(&a).unwrap(), a_first);
    cache.load_bytes(&c).unwrap();
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.diagnostics().evictions, 1);
    // A survived via the touch; B must decode again with a fresh identity.
    assert_eq!(cache.load_bytes(&a).unwrap(), a_first);
    let b_second = cache.load_bytes(&b).unwrap();
    assert_ne!(b_second, b_first);
    assert_eq!(cache.diagnostics().image_decodes, 4);
}

#[test]
fn byte_accounting_is_exact_and_bounds_residency() {
    let bytes = png(4, 4, 9);
    let mut cache = ImageCache::new();
    let image = cache.load_bytes(&bytes).unwrap();
    assert_eq!(
        cache.resident_bytes(),
        bytes.len() + image.decoded().byte_len()
    );
    assert_eq!(image.decoded().byte_len(), 4 * 4 * 4);

    // Size the budget at exactly two entries, then admit a smaller third:
    // precisely the oldest entry goes, and residency never exceeds the cap.
    let big_a = png(8, 8, 11);
    let big_b = png(8, 8, 12);
    let small_c = png(2, 2, 13);
    let mut cache = ImageCache::new();
    let a_first = cache.load_bytes(&big_a).unwrap();
    let b_first = cache.load_bytes(&big_b).unwrap();
    let cap = cache.resident_bytes();
    cache.set_limits(ImageCacheLimits::new(usize::MAX, cap));
    assert_eq!(cache.len(), 2);
    cache.load_bytes(&small_c).unwrap();
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.diagnostics().evictions, 1);
    assert!(cache.resident_bytes() <= cap);
    assert_eq!(cache.load_bytes(&big_b).unwrap(), b_first);
    assert_ne!(cache.load_bytes(&big_a).unwrap(), a_first);
}

#[test]
fn oversized_entries_serve_fresh_without_admission() {
    let bytes = png(4, 4, 21);
    let entry_bytes = bytes.len() + 4 * 4 * 4;
    let mut cache = ImageCache::with_limits(ImageCacheLimits::new(100, entry_bytes - 1));
    let first = cache.load_bytes(&bytes).unwrap();
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.resident_bytes(), 0);
    // Served fresh every time: no hit, no eviction, still a valid image.
    let second = cache.load_bytes(&bytes).unwrap();
    assert_ne!(second, first);
    assert_eq!(second.decoded().width(), 4);
    let diagnostics = cache.diagnostics();
    assert_eq!(diagnostics.image_decodes, 2);
    assert_eq!(diagnostics.load_cache_hits, 0);
    assert_eq!(diagnostics.evictions, 0);
}

#[test]
fn zero_limits_disable_admission() {
    let bytes = png(4, 4, 23);
    let mut cache = ImageCache::with_limits(ImageCacheLimits::new(0, 0));
    let first = cache.load_bytes(&bytes).unwrap();
    let second = cache.load_bytes(&bytes).unwrap();
    assert_ne!(first, second);
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.diagnostics().image_decodes, 2);
}

#[test]
fn clear_keeps_live_handles_valid() {
    let (a, b) = (png(4, 4, 31), png(4, 4, 32));
    let mut cache = ImageCache::new();
    let a_live = cache.load_bytes(&a).unwrap();
    let b_live = cache.load_bytes(&b).unwrap();
    let requested_before = cache.diagnostics().loads_requested;
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    assert_eq!(cache.resident_bytes(), 0);
    // Live handles keep their pixels after the cache drops its references.
    assert_eq!(a_live.decoded().width(), 4);
    assert_eq!(a_live.decoded().byte_len(), 64);
    assert_eq!(b_live.decoded().height(), 4);
    // Reloading re-decodes with a fresh identity; counters stay cumulative.
    let a_again = cache.load_bytes(&a).unwrap();
    assert_ne!(a_again, a_live);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.diagnostics().loads_requested, requested_before + 1);
}

#[test]
fn set_limits_trims_oldest_first() {
    let payloads = [png(4, 4, 41), png(4, 4, 42), png(4, 4, 43)];
    let mut cache = ImageCache::new();
    let mut first_ids = Vec::new();
    for payload in &payloads {
        first_ids.push(cache.load_bytes(payload).unwrap());
    }
    assert_eq!(cache.len(), 3);
    cache.set_limits(ImageCacheLimits::new(1, usize::MAX));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.diagnostics().evictions, 2);
    // Only the newest survives; the rest decode again on demand.
    assert_eq!(cache.load_bytes(&payloads[2]).unwrap(), first_ids[2]);
    assert_ne!(cache.load_bytes(&payloads[0]).unwrap(), first_ids[0]);
    assert_ne!(cache.load_bytes(&payloads[1]).unwrap(), first_ids[1]);
}

#[test]
fn failures_are_never_cached() {
    let garbage = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
    // Truncated PNG: intact header, severed data.
    let mut truncated = png(4, 4, 51);
    truncated.truncate(truncated.len() / 2);
    let mut cache = ImageCache::new();
    for bad in [&garbage, &truncated, &Vec::new()] {
        assert!(cache.load_bytes(bad).is_err());
    }
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.resident_bytes(), 0);
    let diagnostics = cache.diagnostics();
    assert_eq!(diagnostics.loads_requested, 3);
    assert_eq!(diagnostics.load_failures, 3);
    assert_eq!(diagnostics.image_decodes, 0);
    // Retries decode again instead of replaying a stored error.
    assert!(cache.load_bytes(&garbage).is_err());
    assert_eq!(cache.diagnostics().load_failures, 4);
}

#[test]
fn distinct_payloads_never_alias_and_equal_payloads_share() {
    let (a, b) = (png(4, 4, 61), png(4, 4, 62));
    assert_eq!(a.len(), b.len());
    assert_ne!(a, b);
    let mut cache = ImageCache::new();
    let a_handle = cache.load_bytes(&a).unwrap();
    let b_handle = cache.load_bytes(&b).unwrap();
    // Same length, different bytes: full-byte equality keeps them apart.
    assert_ne!(a_handle, b_handle);
    // Separately allocated but byte-equal payloads resolve to one handle.
    let a_copy = a.clone();
    assert_ne!(a.as_ptr(), a_copy.as_ptr());
    assert_eq!(cache.load_bytes(a_copy).unwrap(), a_handle);
    assert_eq!(cache.len(), 2);
}

/// Minimal CRC-32 (IEEE) over a few header bytes, so the hostile-dimension
/// PNG below carries a valid IHDR checksum without a codec dependency.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[test]
fn hostile_dimensions_fail_before_any_decoding_allocation() {
    // IHDR claiming 2^31 x 2^31 pixels: 2^64 pixel bytes, far beyond any
    // allocator. Header-time limits (the decoder's own defaults plus the
    // checked pre-read) must reject it cheaply with a clean error — never
    // an allocation attempt, an admission, or a counted decode.
    let mut png_bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = vec![0, 0, 0, 13, 73, 72, 68, 82];
    ihdr.extend_from_slice(&[0x80, 0, 0, 0, 0x80, 0, 0, 0, 8, 6, 0, 0, 0]);
    let checksum = crc32(&ihdr[4..]);
    png_bytes.extend_from_slice(&ihdr);
    png_bytes.extend_from_slice(&checksum.to_be_bytes());
    let mut cache = ImageCache::new();
    let error = cache
        .load_bytes(&png_bytes)
        .expect_err("hostile dimensions must fail");
    assert!(
        matches!(error, ImageError::InvalidDimensions | ImageError::Decode(_)),
        "unexpected rejection: {error:?}"
    );
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.resident_bytes(), 0);
    assert_eq!(cache.diagnostics().image_decodes, 0);
    assert_eq!(cache.diagnostics().load_failures, 1);
}
