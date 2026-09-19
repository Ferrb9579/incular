#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliderInteraction {
    #[default]
    TapAndSlide,
    TapOnly,
    SlideOnly,
    SlideThumb,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShowValueIndicator {
    #[default]
    OnlyForDiscrete,
    OnlyForContinuous,
    Always,
    Never,
}
