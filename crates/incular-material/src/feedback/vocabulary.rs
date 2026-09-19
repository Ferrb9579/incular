#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RefreshIndicatorStatus {
    #[default]
    Drag,
    Armed,
    Snap,
    Refresh,
    Done,
    Canceled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RefreshIndicatorTriggerMode {
    Anywhere,
    #[default]
    OnEdge,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnackBarBehavior {
    #[default]
    Fixed,
    Floating,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnackBarClosedReason {
    Action,
    Dismiss,
    Swipe,
    #[default]
    Hide,
    Timeout,
    Remove,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepState {
    #[default]
    Indexed,
    Editing,
    Complete,
    Disabled,
    Error,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepperType {
    #[default]
    Vertical,
    Horizontal,
}
