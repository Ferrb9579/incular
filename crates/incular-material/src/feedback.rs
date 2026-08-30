//! Material feedback surfaces and presentation descriptors.
//!
//! This module owns the visual and semantic policy for feedback widgets.  It
//! deliberately does not own an application shell, navigator, or notification
//! queue: those services belong to the runtime/navigation layers.  The types in
//! this module are ordinary retained descriptors and can be composed into any
//! widget tree.

mod dialog;
mod dialog_handles;
mod helpers;
mod progress;
mod transient;

pub use dialog::{AlertDialog, Dialog, SimpleDialog, SimpleDialogOption};
pub use dialog_handles::{
    DialogHandle, DialogResultHandle, DialogRoute, show_dialog, show_dialog_result,
};
pub use progress::{
    ProgressIndicatorStrokeCap, ProgressIndicatorTheme, ProgressIndicatorThemeData,
    current_progress_indicator_theme,
};
pub use transient::{SnackBar, SnackBarAction, Tooltip, TooltipController, TooltipTriggerMode};
