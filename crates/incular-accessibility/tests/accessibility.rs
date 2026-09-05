use accesskit::{
    Action, ActionData, ActionRequest, NodeId, Role as AccessKitRole, TextPosition,
    TextSelection as AccessKitSelection, Toggled, TreeId, TreeUpdate,
};
use incular_accessibility::{
    AccessKitProjection, AccessKitUpdateKind, MobileAccessibilityEventKind,
    MobileAccessibilityProjection, SemanticAction, SemanticActionKind, SemanticActionRequest,
    SemanticNode, SemanticNodeId, SemanticState, SemanticsTree,
};
use incular_core::{ArenaId, Offset, Rect, Size};
use incular_semantics::Role;

fn node(role: Role) -> SemanticNode {
    SemanticNode {
        id: SemanticNodeId(ArenaId::from_parts(0, 0)),
        role,
        label: None,
        value: None,
        description: None,
        bounds: Rect::from_origin_size(Offset::new(2.0, 3.0), Size::new(20.0, 10.0)),
        state: SemanticState::default(),
        actions: vec![],
        children: vec![],
    }
}

fn full(projection: &mut AccessKitProjection, tree: &SemanticsTree) -> TreeUpdate {
    projection.activate();
    projection
        .sync(tree, 2.0)
        .expect("full update")
        .into_accesskit()
}

#[test]
fn checkable_states_preserve_mixed_and_absent_values() {
    use incular_semantics::CheckedState;
    for (state, expected) in [
        (None, None),
        (Some(CheckedState::Unchecked), Some(Toggled::False)),
        (Some(CheckedState::Checked), Some(Toggled::True)),
        (Some(CheckedState::Indeterminate), Some(Toggled::Mixed)),
    ] {
        let mut tree = SemanticsTree::new();
        let mut checkbox = node(Role::Checkbox);
        checkbox.state.checked = state;
        let id = tree.insert(checkbox);
        tree.set_root(Some(id));
        let mut projection = AccessKitProjection::new();
        let update = full(&mut projection, &tree);
        let native = NodeId(projection.native_node_id(id).unwrap());
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == native)
                .unwrap()
                .1
                .toggled(),
            expected
        );
        let mut mobile = MobileAccessibilityProjection::new();
        mobile.activate();
        let update = mobile.sync(&tree).expect("mobile update");
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|node| node.semantic.id == id)
                .unwrap()
                .semantic
                .state
                .checked,
            state
        );
    }
}

#[test]
fn button_role_name_action_and_bounds_are_projected() {
    let mut tree = SemanticsTree::new();
    let mut button = node(Role::Button);
    button.label = Some("Save".into());
    button.state.enabled = true;
    button.actions = vec![SemanticActionKind::Focus, SemanticActionKind::Activate];
    let id = tree.insert(button);
    tree.set_root(Some(id));
    let mut projection = AccessKitProjection::new();
    let update = full(&mut projection, &tree);
    let native = NodeId(projection.native_node_id(id).expect("mapped"));
    let node = &update.nodes.iter().find(|(id, _)| *id == native).unwrap().1;
    assert_eq!(node.role(), AccessKitRole::Button);
    assert_eq!(node.label(), Some("Save"));
    assert!(node.supports_action(Action::Click));
    assert_eq!(node.bounds().unwrap().x1, 44.0);
}

#[test]
fn state_text_collection_and_scroll_properties_are_projected() {
    let mut tree = SemanticsTree::new();
    let mut input = node(Role::TextArea);
    input.value = Some("héllo".into());
    input.state.enabled = true;
    input.state.editable = true;
    input.state.read_only = true;
    input.state.checked = Some(true.into());
    input.state.selection = Some(incular_accessibility::TextSelection { base: 1, extent: 3 });
    input.actions = vec![
        SemanticActionKind::SetText,
        SemanticActionKind::SetSelection,
    ];
    let input_id = tree.insert(input);
    let mut list = node(Role::ListItem);
    list.state.selected = true;
    list.state.item_index = Some(4);
    list.state.set_size = Some(12);
    let list_id = tree.insert(list);
    let mut scroll = node(Role::ScrollView);
    scroll.value = Some("8/50".into());
    scroll.actions = vec![
        SemanticActionKind::ScrollForward,
        SemanticActionKind::ScrollBackward,
    ];
    let scroll_id = tree.insert(scroll);
    let mut root = node(Role::GenericContainer);
    root.children = vec![input_id, list_id, scroll_id];
    let root_id = tree.insert(root);
    tree.set_root(Some(root_id));
    let mut projection = AccessKitProjection::new();
    let update = full(&mut projection, &tree);
    let get = |id| {
        update
            .nodes
            .iter()
            .find(|(native, _)| native.0 == projection.native_node_id(id).unwrap())
            .map(|(_, node)| node)
            .unwrap()
    };
    let input = get(input_id);
    assert_eq!(input.role(), AccessKitRole::MultilineTextInput);
    assert_eq!(input.value(), Some("héllo"));
    assert!(input.is_read_only());
    assert_eq!(input.toggled(), Some(Toggled::True));
    assert!(input.supports_action(Action::SetValue));
    assert_eq!(input.text_selection().unwrap().focus.character_index, 2);
    let list = get(list_id);
    assert_eq!(list.is_selected(), Some(true));
    assert_eq!(list.position_in_set(), Some(5));
    assert_eq!(list.size_of_set(), Some(12));
    let scroll = get(scroll_id);
    assert_eq!(scroll.scroll_y(), Some(8.0));
    assert_eq!(scroll.scroll_y_max(), Some(50.0));
    assert!(scroll.supports_action(Action::ScrollDown));
}

#[test]
fn static_content_controls_and_modal_roles_have_documented_native_mappings() {
    let mut tree = SemanticsTree::new();
    let mut button = node(Role::Button);
    button.label = Some("Unavailable".into());
    button.actions = vec![SemanticActionKind::Activate];
    let button = tree.insert(button);
    let mut text = node(Role::Text);
    text.label = Some("Read this static text".into());
    let text = tree.insert(text);
    let mut heading = node(Role::Heading);
    heading.label = Some("Preferences".into());
    let heading = tree.insert(heading);
    let mut image = node(Role::Image);
    image.label = Some("Company logo".into());
    let image = tree.insert(image);
    let mut dialog = node(Role::Dialog);
    dialog.label = Some("Confirm deletion".into());
    let dialog = tree.insert(dialog);
    let mut root = node(Role::GenericContainer);
    root.children = vec![button, text, heading, image, dialog];
    let root = tree.insert(root);
    tree.set_root(Some(root));

    let mut projection = AccessKitProjection::new();
    let update = full(&mut projection, &tree);
    let get = |semantic| {
        update
            .nodes
            .iter()
            .find(|(native, _)| native.0 == projection.native_node_id(semantic).unwrap())
            .map(|(_, node)| node)
            .unwrap()
    };
    assert!(get(button).is_disabled());
    assert_eq!(get(text).role(), AccessKitRole::Label);
    assert_eq!(get(text).value(), Some("Read this static text"));
    assert_eq!(get(heading).role(), AccessKitRole::Heading);
    assert_eq!(get(image).role(), AccessKitRole::Image);
    assert_eq!(get(image).label(), Some("Company logo"));
    assert_eq!(get(dialog).role(), AccessKitRole::Dialog);
}

#[test]
fn unchanged_tree_emits_no_update_but_label_focus_and_removal_are_incremental() {
    let mut tree = SemanticsTree::new();
    let mut button = node(Role::Button);
    button.label = Some("Save".into());
    button.state.enabled = true;
    button.actions = vec![SemanticActionKind::Focus, SemanticActionKind::Activate];
    let id = tree.insert(button);
    tree.set_root(Some(id));
    let mut projection = AccessKitProjection::new();
    let _ = full(&mut projection, &tree);
    assert!(projection.sync(&tree, 2.0).is_none());
    let mut changed = tree.node(id).unwrap().clone();
    changed.label = Some("Saved".into());
    tree.update(id, changed);
    let update = projection.sync(&tree, 2.0).unwrap();
    assert_eq!(update.kind(), AccessKitUpdateKind::Incremental);
    assert_eq!(update.into_accesskit().nodes.len(), 1);
    let mut focused = tree.node(id).unwrap().clone();
    focused.state.focused = true;
    tree.update(id, focused);
    assert!(projection.sync(&tree, 2.0).is_some());
    tree.remove(id);
    assert!(projection.sync(&tree, 2.0).is_some());
    assert!(projection.diagnostics().nodes_removed >= 1);
}

#[test]
fn stale_native_action_cannot_reach_a_reused_semantic_slot() {
    let mut tree = SemanticsTree::new();
    let mut first = node(Role::Button);
    first.state.enabled = true;
    first.actions = vec![SemanticActionKind::Activate];
    let first = tree.insert(first);
    tree.set_root(Some(first));
    let mut projection = AccessKitProjection::new();
    let _ = full(&mut projection, &tree);
    let stale_native = NodeId(projection.native_node_id(first).unwrap());
    tree.remove(first);
    let replacement = tree.insert(node(Role::Button));
    tree.set_root(Some(replacement));
    let _ = projection.sync(&tree, 1.0);
    let request = ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: stale_native,
        data: None,
    };
    assert!(projection.translate_action(&request).is_none());
    assert_eq!(projection.diagnostics().stale_actions_rejected, 1);
    assert_ne!(projection.native_node_id(replacement), Some(stale_native.0));
}

#[test]
fn native_actions_translate_to_existing_semantic_actions() {
    let mut tree = SemanticsTree::new();
    let mut field = node(Role::TextField);
    field.value = Some("héllo".into());
    field.state.enabled = true;
    field.state.editable = true;
    field.actions = vec![
        SemanticActionKind::Focus,
        SemanticActionKind::SetText,
        SemanticActionKind::SetSelection,
    ];
    let id = tree.insert(field);
    tree.set_root(Some(id));
    let mut projection = AccessKitProjection::new();
    let _ = full(&mut projection, &tree);
    let native = NodeId(projection.native_node_id(id).unwrap());
    let set_text = ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: native,
        data: Some(ActionData::Value("edited".into())),
    };
    assert_eq!(
        projection.translate_action(&set_text),
        Some(SemanticActionRequest {
            node: id,
            action: SemanticAction::SetText("edited".into())
        })
    );
    let selection = ActionRequest {
        action: Action::SetTextSelection,
        target_tree: TreeId::ROOT,
        target_node: native,
        data: Some(ActionData::SetTextSelection(AccessKitSelection {
            anchor: TextPosition {
                node: native,
                character_index: 1,
            },
            focus: TextPosition {
                node: native,
                character_index: 3,
            },
        })),
    };
    assert_eq!(
        projection.translate_action(&selection),
        Some(SemanticActionRequest {
            node: id,
            action: SemanticAction::SetSelection { base: 1, extent: 4 }
        })
    );

    let mut scroll_tree = SemanticsTree::new();
    let mut scroll = node(Role::ScrollView);
    scroll.actions = vec![
        SemanticActionKind::ScrollForward,
        SemanticActionKind::ScrollBackward,
    ];
    let scroll_id = scroll_tree.insert(scroll);
    scroll_tree.set_root(Some(scroll_id));
    let mut scroll_projection = AccessKitProjection::new();
    let _ = full(&mut scroll_projection, &scroll_tree);
    let request = ActionRequest {
        action: Action::ScrollDown,
        target_tree: TreeId::ROOT,
        target_node: NodeId(scroll_projection.native_node_id(scroll_id).unwrap()),
        data: None,
    };
    assert_eq!(
        scroll_projection.translate_action(&request),
        Some(SemanticActionRequest {
            node: scroll_id,
            action: SemanticAction::ScrollForward
        })
    );
}

#[test]
fn mobile_projection_is_incremental_and_rejects_unsupported_actions() {
    let mut tree = SemanticsTree::new();
    let mut button = node(Role::Button);
    button.label = Some("Open".into());
    button.actions = vec![SemanticActionKind::Activate];
    let button_id = tree.insert(button);
    tree.set_root(Some(button_id));

    let mut projection = MobileAccessibilityProjection::new();
    let first = projection.sync(&tree).expect("initial mobile tree");
    assert_eq!(first.nodes.len(), 1);
    assert_eq!(
        first.events[0].kind,
        MobileAccessibilityEventKind::ContentChanged
    );
    let native = projection.native_node_id(button_id).expect("native id");
    assert!(
        projection
            .translate_action(native, SemanticAction::Activate)
            .is_some()
    );
    assert!(
        projection
            .translate_action(native, SemanticAction::SetText("no".into()))
            .is_none()
    );
    assert!(projection.sync(&tree).is_none());

    let mut changed = tree.node(button_id).expect("button").clone();
    changed.label = Some("Opened".into());
    tree.update(button_id, changed);
    let incremental = projection.sync(&tree).expect("incremental mobile tree");
    assert_eq!(incremental.nodes.len(), 1);
    assert!(
        incremental
            .events
            .iter()
            .any(|event| event.kind == MobileAccessibilityEventKind::ContentChanged)
    );

    tree.remove(button_id);
    tree.set_root(None);
    let removed = projection.sync(&tree).expect("removal update");
    assert_eq!(removed.removed, vec![native]);
    assert!(projection.semantic_node_id(native).is_none());
}
