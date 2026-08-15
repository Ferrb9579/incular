# incular-linux

Owns the Linux `winit` application handler and native window lifetime. It
forwards normalized pointer events to `incular-runtime`, requests redraws only
for scheduled work/resize/input, and keeps the window alive while
`incular-wgpu` owns its raw-handle surface.
