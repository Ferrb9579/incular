use incular::prelude::*;

#[test]
fn test_focus_and_traversal_contract() {
    let node1 = FocusNode::new();
    let node2 = FocusNode::new();
    let node3 = FocusNode::new();

    node1.request_focus();
    assert!(node1.has_focus());
    assert!(node1.has_primary_focus());

    node1.unfocus();
    assert!(!node1.has_focus());

    node2.set_skip_traversal(true);
    assert!(node2.skip_traversal());

    let policy = WidgetOrderTraversalPolicy;
    let nodes = [node1.clone(), node2.clone(), node3.clone()];

    assert_eq!(policy.find_first_focus(&nodes), Some(node1.clone()));
    assert_eq!(policy.next(&node1, &nodes), Some(node3.clone()));
    assert_eq!(policy.find_last_focus(&nodes), Some(node3.clone()));

    let reading_policy = ReadingOrderTraversalPolicy;
    assert_eq!(reading_policy.find_first_focus(&nodes), Some(node1));
}
