//! Backend evidence for transformed clip fallbacks: the world-space paths
//! the compositor emits tessellate through the real backend tessellator
//! into stencil meshes that follow the shape, not its bounding box.
//!
//! CPU-deterministic, no device needed: tessellation is pure geometry.
//! Pixel presentation stays with the native swap chain; what this pins
//! is the exact mesh content the stencil pass consumes.

use incular_core::{Offset, Rect, Size, Transform};
use incular_rendering::{DisplayList, FillRule, LayerTree, PaintCommand, Path, RRect};
use incular_wgpu::tessellate_path;
use kurbo::Shape;
use std::f32::consts::FRAC_PI_4;

fn flatten_clip_under(
    world: Transform,
    make: impl FnOnce(&mut LayerTree) -> incular_rendering::LayerId,
) -> Path {
    let mut tree = LayerTree::new();
    let clip = make(&mut tree);
    let picture = tree.create_picture(
        DisplayList::new(),
        Rect::from_origin_size(Offset::new(200., 200.), Size::new(10., 10.)),
    );
    tree.set_children(clip, vec![picture]);
    let shift = tree.create_transform(world);
    tree.set_children(shift, vec![clip]);
    let root = tree.create_transform(Transform::IDENTITY);
    tree.set_children(root, vec![shift]);
    tree.set_root(root);
    let list = tree.flatten();
    list.commands()
        .iter()
        .find_map(|command| match command {
            PaintCommand::PushClipPath { path, .. } => Some(path.as_ref().clone()),
            _ => None,
        })
        .expect("fallback path clip")
}

fn mesh_area(vertices: &[[f32; 2]], indices: &[u32]) -> f64 {
    let (triangles, remainder) = indices.as_chunks::<3>();
    assert!(remainder.is_empty(), "triangle mesh");
    let mut area = 0.;
    for triangle in triangles {
        let [a, b, c] = [
            vertices[triangle[0] as usize],
            vertices[triangle[1] as usize],
            vertices[triangle[2] as usize],
        ];
        area += ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() as f64 * 0.5;
    }
    area
}

/// The stencil mesh follows the emitted path (area match) rather than
/// filling its bounding box (area mismatch by ~2x for the diamond).
fn assert_mesh_follows_shape(path: &Path) {
    let mesh = tessellate_path(path, FillRule::NonZero, None).expect("tessellatable clip path");
    assert!(mesh.vertices.len() >= 3);
    assert!(!mesh.indices.is_empty());
    let area = mesh_area(&mesh.vertices, &mesh.indices);
    let expected = path.bez_path().area().abs();
    assert!(
        (area - expected).abs() / expected < 0.05,
        "mesh area {area} must match path area {expected}"
    );
    let bounds = path.bounds().expect("nonempty clip path");
    let boxed = (bounds.size.width * bounds.size.height) as f64;
    assert!(
        area < boxed * 0.7,
        "mesh area {area} must stay well under bbox area {boxed}"
    );
}

#[test]
fn rotated_rrect_fallback_tessellates_to_diamond_mesh() {
    let path = flatten_clip_under(Transform::rotation(FRAC_PI_4), |tree| {
        tree.create_clip_rrect(RRect::uniform(
            Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.)),
            8.,
        ))
    });
    assert_mesh_follows_shape(&path);
}

#[test]
fn rotated_oval_fallback_tessellates_to_ellipse_mesh() {
    let path = flatten_clip_under(Transform::rotation(0.5), |tree| {
        tree.create_clip_oval(Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)))
    });
    assert_mesh_follows_shape(&path);
}
