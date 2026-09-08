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

/// Appends one checksummed chunk to a PNG under construction.
fn push_chunk(png_bytes: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png_bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png_bytes.extend_from_slice(kind);
    png_bytes.extend_from_slice(data);
    let mut checksummed = kind.to_vec();
    checksummed.extend_from_slice(data);
    png_bytes.extend_from_slice(&crc32(&checksummed).to_be_bytes());
}

/// Minimal PNG carrying a signature, IHDR, an empty IDAT, and IEND. The
/// decoder's header parse requires the image-data stream to begin (it reads
/// past IHDR), so the empty zlib stored block stands in for pixel data that
/// decoding — which never starts here — would consume.
fn png_header_only(width: u32, height: u32) -> Vec<u8> {
    let mut png_bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_chunk(&mut png_bytes, b"IHDR", &ihdr);
    // Empty zlib stream: header, final stored empty block, empty Adler-32.
    push_chunk(
        &mut png_bytes,
        b"IDAT",
        &[
            0x78, 0x01, 0x01, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x01,
        ],
    );
    push_chunk(&mut png_bytes, b"IEND", &[]);
    png_bytes
}

#[test]
fn decode_policy_rejects_absurd_dimensions_with_reason() {
    // IHDR claiming 2^31 x 2^31 pixels. The PNG layer's own overflow guard
    // fires while parsing the header — before dimensions are reportable
    // through any decoder API — so the rejection carries the documented
    // zero-sized shape. What matters is structural: the failure surfaces
    // during the header probe, the pixel decode never starts, no output
    // buffer is requested, nothing is admitted, and no decode is counted.
    let png_bytes = png_header_only(0x8000_0000, 0x8000_0000);
    let mut cache = ImageCache::new();
    assert_eq!(
        cache.load_bytes(&png_bytes),
        Err(ImageError::DecodeTooLarge {
            width: 0,
            height: 0
        })
    );
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.resident_bytes(), 0);
    assert_eq!(cache.diagnostics().image_decodes, 0);
    assert_eq!(cache.diagnostics().load_failures, 1);
    assert_eq!(cache.diagnostics().evictions, 0);
}

#[test]
fn decode_policy_enforces_limits_during_header_construction() {
    use image::codecs::jpeg::JpegEncoder;

    // 20000 x 100 is a small file (tens of kilobytes) that would decode to
    // ~6-8 MiB — inexpensive either way, and far inside the output budget.
    // Only the per-side dimension cap rejects it. Both formats enforce it
    // inside decoder construction (`into_decoder`: PNG at `with_limits`
    // time, JPEG at `set_limits` time), so dimensions are never returned
    // and both rejections carry the zero-sized shape. That shape is the
    // evidence enforcement happened during header construction rather than
    // after dimensions came back: contrast the output-gate test below,
    // where dimensions survive into the rejection. Both reject before any
    // pixel buffer is requested and neither admits nor counts a decode.
    let wide_rgba: Vec<u8> = (0..20_000u32 * 100)
        .flat_map(|i| {
            let i = i as u8;
            [i, i.wrapping_mul(3), 255 - i, 255]
        })
        .collect();
    let mut wide_png = Vec::new();
    PngEncoder::new(&mut wide_png)
        .write_image(&wide_rgba, 20_000, 100, ExtendedColorType::Rgba8)
        .expect("encode wide test png");
    let mut wide_rgb = Vec::with_capacity(wide_rgba.len() / 4 * 3);
    for (index, byte) in wide_rgba.iter().enumerate() {
        if index % 4 != 3 {
            wide_rgb.push(*byte);
        }
    }
    let mut wide_jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut wide_jpeg, 80)
        .write_image(&wide_rgb, 20_000, 100, ExtendedColorType::Rgb8)
        .expect("encode wide test jpeg");

    let mut cache = ImageCache::new();
    assert_eq!(
        cache.load_bytes(&wide_png).unwrap_err(),
        ImageError::DecodeTooLarge {
            width: 0,
            height: 0
        }
    );
    assert_eq!(
        cache.load_bytes(&wide_jpeg).unwrap_err(),
        ImageError::DecodeTooLarge {
            width: 0,
            height: 0
        }
    );
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.resident_bytes(), 0);
    assert_eq!(cache.diagnostics().image_decodes, 0);
    assert_eq!(cache.diagnostics().load_failures, 2);
    assert_eq!(cache.diagnostics().evictions, 0);
}

#[test]
fn decode_policy_rejects_excessive_output_bytes() {
    // 16384 x 16384 passes the per-side dimension cap with plainly valid
    // arithmetic (2^30 output bytes, no overflow anywhere), yet the RGBA8
    // output would be 1 GiB against a 256 MiB policy: rejected by the
    // output gate with the exact boundary attached, before decoding starts.
    assert_eq!(
        16384u64 * 16384 * 4,
        4 * incular_image::MAX_DECODE_OUTPUT_BYTES
    );
    let png_bytes = png_header_only(16384, 16384);
    let mut cache = ImageCache::new();
    assert_eq!(
        cache.load_bytes(&png_bytes),
        Err(ImageError::DecodeTooLarge {
            width: 16384,
            height: 16384,
        })
    );
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.diagnostics().image_decodes, 0);
}

#[test]
fn decode_errors_stay_distinct_from_policy_rejections() {
    // Malformed and truncated inputs are decoder failures, not policy
    // rejections: the error taxonomy must tell them apart.
    let mut truncated = png(4, 4, 71);
    truncated.truncate(truncated.len() / 2);
    let mut cache = ImageCache::new();
    assert!(matches!(
        cache.load_bytes([0u8, 1, 2, 3]).unwrap_err(),
        ImageError::Decode(_)
    ));
    assert!(matches!(
        cache.load_bytes(&truncated).unwrap_err(),
        ImageError::Decode(_)
    ));
    assert!(matches!(
        cache.load_bytes(Vec::new()).unwrap_err(),
        ImageError::Decode(_)
    ));
    assert_eq!(cache.diagnostics().load_failures, 3);
    assert_eq!(cache.diagnostics().image_decodes, 0);
}

#[test]
fn grayscale_sources_convert_to_sized_rgba8() {
    // A 4x4 luminance PNG decodes through the RGBA8 conversion to exactly
    // 64 output bytes, and cache accounting follows the converted size.
    let gray: Vec<u8> = (0..16u8).map(|i| i.wrapping_mul(17)).collect();
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&gray, 4, 4, ExtendedColorType::L8)
        .expect("encode gray test png");
    let mut cache = ImageCache::new();
    let image = cache.load_bytes(&encoded).unwrap();
    assert_eq!((image.decoded().width(), image.decoded().height()), (4, 4));
    assert_eq!(image.decoded().byte_len(), 64);
    assert_eq!(cache.resident_bytes(), encoded.len() + 64);
}

#[test]
fn valid_images_decode_despite_tiny_cache_budgets() {
    // A 256x256 image (256 KiB decoded) fits the decode policy comfortably
    // but exceeds a ~100-byte cache budget: it still loads successfully and
    // is served fresh without admission — a cache budget never becomes a
    // decode limit.
    let big = png(256, 256, 81);
    // 256 KiB of decoded pixels alone dwarfs the cache budget below, while
    // the 4x4 entries (~150 bytes each) fit it comfortably.
    let small_a = png(4, 4, 82);
    let small_b = png(4, 4, 83);
    let mut cache = ImageCache::with_limits(ImageCacheLimits::new(100, 100_000));
    let a_first = cache.load_bytes(&small_a).unwrap();
    let b_first = cache.load_bytes(&small_b).unwrap();
    assert_eq!(cache.len(), 2);
    let before = cache.diagnostics();
    let big_image = cache.load_bytes(&big).unwrap();
    assert_eq!(
        (big_image.decoded().width(), big_image.decoded().height()),
        (256, 256)
    );
    // Not admitted, nothing evicted, and the working set still hits.
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.diagnostics().evictions, before.evictions);
    assert_eq!(cache.load_bytes(&small_a).unwrap(), a_first);
    assert_eq!(cache.load_bytes(&small_b).unwrap(), b_first);
    assert_eq!(cache.diagnostics().image_decodes, before.image_decodes + 1);
}

#[test]
fn rejections_never_evict_the_working_set() {
    let (a, b) = (png(4, 4, 91), png(4, 4, 92));
    let hostile = png_header_only(0x8000_0000, 0x8000_0000);
    let mut cache = ImageCache::new();
    let a_first = cache.load_bytes(&a).unwrap();
    let b_first = cache.load_bytes(&b).unwrap();
    let before = cache.diagnostics();
    assert!(cache.load_bytes([9u8, 9, 9]).is_err());
    assert!(cache.load_bytes(&hostile).is_err());
    // Both failures left the admitted set, its identities, and the
    // eviction count exactly alone.
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.load_bytes(&a).unwrap(), a_first);
    assert_eq!(cache.load_bytes(&b).unwrap(), b_first);
    assert_eq!(cache.diagnostics().evictions, before.evictions);
    assert_eq!(cache.diagnostics().load_failures, before.load_failures + 2);
}
