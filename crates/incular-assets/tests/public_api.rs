use incular_assets::{FontHandle, FontId};

#[test]
fn collection_face_identity_is_retained() {
    let font = FontHandle::with_face_index(FontId(9), [1, 2, 3], 2);
    assert_eq!(font.id(), FontId(9));
    assert_eq!(font.face_index(), 2);
    assert_eq!(&**font.bytes(), &[1, 2, 3]);
}
