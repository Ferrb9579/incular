//! Focus autofocus through real runtime mount: the tree advertises the
//! autofocus element, and mount mirrors focus onto the attached node.

use incular_config::Constraints;
use incular_core::{Color, Size};
use incular_runtime::Runtime;
use incular_widgets::{Focus, FocusNode, FocusScope, Widget};

#[test]
fn focus_autofocus_mount_requests_node() {
    let node = FocusNode::new();
    let mut runtime = Runtime::new(
        Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE))
            .node(node.clone())
            .autofocus(true)
            .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .expect("frame");
    assert!(node.has_focus());
    assert!(runtime.focused_element().is_some());
}

#[test]
fn focus_scope_autofocus_mount_focuses_descendant() {
    let node = FocusNode::new();
    let mut runtime = Runtime::new(
        FocusScope::new(
            Focus::new(Widget::box_(Size::new(40., 40.), Color::WHITE)).node(node.clone()),
        )
        .autofocus(true)
        .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(100., 100.)))
        .expect("frame");
    assert!(
        node.has_focus(),
        "scope autofocus must resolve to the first focusable descendant"
    );
}
