//! Renderer- and platform-independent animation primitives.
//!
//! Controllers are driven by runtime-supplied monotonic timestamps. Curves,
//! typed tweens, composable values, and implicit retargeting live in separate
//! modules while the historical root exports remain available.

mod controller;
mod curves;
pub mod physics;
mod tween;
mod value;

pub use controller::{AnimationController, AnimationStatus};
pub use curves::{Curve, CurveChain, Curves};
pub use physics::{
    BoundedFrictionSimulation, ClampedSimulation, FrictionSimulation, GravitySimulation,
    ScrollSpringSimulation, Simulation, SpringDescription, SpringSimulation, SpringType, Tolerance,
};
pub use tween::{Tween, TweenSegment, TweenSequence, TweenValue};
pub use value::{
    Animatable, AnimatedValue, Animation, AnimationValue, ImplicitAnimatedValue, ImplicitAnimation,
    MappedAnimation,
};
