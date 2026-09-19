use incular::controls::slider::{RangeSliderModel, RangeThumb, RangeValues};

#[test]
fn neutral_range_model_owns_quantization_separation_and_noop_filtering() {
    let model = RangeSliderModel::new(RangeValues::new(0.2, 0.8), 0.0, 1.0, 0.1, 0.2);
    let revision = model.revision();
    assert_eq!(revision.get(), 0);
    assert_eq!(
        model.set_from_origin(RangeThumb::Start, model.values(), 0.21),
        Some(RangeValues::new(0.4, 0.8))
    );
    assert_eq!(revision.get(), 1);
    assert_eq!(model.set_thumb(RangeThumb::Start, 0.4), None);
    assert_eq!(revision.get(), 1, "no-op range writes must not notify");
    assert_eq!(
        model.set_thumb(RangeThumb::Start, 0.75),
        Some(RangeValues::new(0.6, 0.8)),
        "minimum separation clamps the start thumb"
    );
}
