use super::super::*;
use crate::tree::widget::LoweringFamily;

#[doc(hidden)]
pub fn render_kind(widget: &Widget, context: &DependencyContext) -> RenderKind {
    match widget.kind().structure().lowering_family {
        LoweringFamily::Visual => lower_visual(widget, context),
        LoweringFamily::Layout => lower_layout(widget),
        LoweringFamily::Scrolling => lower_scrolling(widget),
        LoweringFamily::Effects => lower_effects(widget),
    }
}

mod effects;
mod layout;
mod scrolling;
mod visual;

use effects::lower_effects;
use layout::lower_layout;
use scrolling::lower_scrolling;
use visual::lower_visual;
