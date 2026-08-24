use incular::prelude::*;

#[test]
fn test_animation_curves_and_tweens_contract() {
    let curves = [
        Curves::LINEAR,
        Curves::EASE,
        Curves::EASE_IN,
        Curves::EASE_OUT,
        Curves::EASE_IN_OUT,
        Curves::FAST_OUT_SLOW_IN,
        Curves::BOUNCE_OUT,
        Curves::ELASTIC_OUT,
    ];

    for c in curves {
        assert_eq!(c.apply(0.0), 0.0);
        assert_eq!(c.apply(1.0), 1.0);
    }

    let tween = TweenValue::new(10.0, 50.0);
    assert_eq!(tween.evaluate(0.0), 10.0);
    assert_eq!(tween.evaluate(0.5), 30.0);
    assert_eq!(tween.evaluate(1.0), 50.0);
}
