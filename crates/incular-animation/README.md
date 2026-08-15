# incular-animation

Renderer- and platform-independent animation primitives, including durations,
curves, tweens, controllers, and animation progress values.

`AnimationController` is cloneable and is ticked only with a runtime-supplied
monotonic `Instant`; it creates no per-widget timer or event loop. It currently
supports forward/reverse/stop/reset, linear/ease-in-out curves, and interpolation
for `f32` and `Offset`.
