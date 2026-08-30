//! Standalone Incular DevTools, built with Incular itself.
//!
//! The inspector retains data rows, not widget rows. A 100k-node target thus
//! keeps a compact id/depth index while the retained sliver viewport creates
//! only rows near the viewport.

fn main() {
    incular_devtools_ui::run();
}
