//! A visible retained-semantics showcase. It prints Incular's portable tree
//! before opening the window; a desktop AccessKit adapter projects that same
//! retained tree only after a native accessibility client activates it.
use incular::{prelude::*, widgets::WidgetTree};

fn state(checked: Option<bool>, enabled: bool) -> SemanticState {
    SemanticState {
        enabled,
        focusable: true,
        checked,
        ..SemanticState::default()
    }
}

fn logo() -> ImageHandle {
    ImageHandle::from_rgba8(
        3,
        2,
        vec![
            255, 183, 77, 255, 71, 123, 214, 255, 255, 183, 77, 255, 71, 123, 214, 255, 255, 183,
            77, 255, 71, 123, 214, 255,
        ],
    )
    .expect("valid generated gallery image")
}

fn dashboard() -> Widget {
    let name = TextEditingController::with_text("Ari");
    let biography = TextEditingController::with_text(
        "A short multi-line biography.\nScreen readers can edit it.",
    );
    let title: Widget = Text::new("Accessibility gallery")
        .style(TextStyle {
            size: 28.,
            color: Color::rgba(255, 230, 165, 255),
            ..TextStyle::default()
        })
        .into();
    let decorative: Widget = Text::new("✦ decorative sparkle")
        .color(Color::rgba(180, 190, 220, 255))
        .into();
    let meaningful_logo = Widget::from(Image::new(logo()).width(72.).height(48.))
        .semantics(Semantics::new(SemanticRole::Image).label("Incular logo"));
    let list = Widget::column(vec![
        Widget::box_(Size::new(260., 28.), Color::rgba(56, 80, 125, 255)).semantics(
            Semantics::new(SemanticRole::ListItem)
                .label("Release notes")
                .state(SemanticState {
                    item_index: Some(0),
                    set_size: Some(2),
                    ..SemanticState::default()
                }),
        ),
        Widget::box_(Size::new(260., 28.), Color::rgba(56, 80, 125, 255)).semantics(
            Semantics::new(SemanticRole::ListItem)
                .label("Keyboard shortcuts")
                .state(SemanticState {
                    item_index: Some(1),
                    set_size: Some(2),
                    ..SemanticState::default()
                }),
        ),
    ])
    .semantics(Semantics::new(SemanticRole::List).label("Documentation topics"));
    let modal_demo = Widget::stack(
        Alignment::CENTER,
        vec![
            Text::new("This background is blocked while the dialog is active.").into(),
            Widget::box_(Size::new(360., 70.), Color::rgba(67, 45, 105, 255))
                .semantics(
                    Semantics::new(SemanticRole::Dialog)
                        .label("Delete draft")
                        .description("A modal dialog example")
                        .actions([SemanticActionKind::Focus]),
                )
                .merge_semantics()
                .block_semantics(),
        ],
    );

    Widget::column(vec![
        title
            .semantics(Semantics::new(SemanticRole::Heading).label("Accessibility gallery"))
            .accessibility_description("Live retained semantics and AccessKit projection example"),
        Text::new("Every native accessibility update starts from this Incular tree.")
            .color(Color::rgba(215, 225, 245, 255))
            .into(),
        Widget::row(vec![
            meaningful_logo,
            decorative.exclude_semantics(),
            Widget::box_(Size::new(180., 44.), Color::rgba(48, 187, 147, 255)).semantics(
                Semantics::new(SemanticRole::Checkbox)
                    .label("Sync over Wi-Fi")
                    .state(state(Some(true), true))
                    .actions([SemanticActionKind::Focus, SemanticActionKind::Activate]),
            ),
            Widget::box_(Size::new(180., 44.), Color::rgba(95, 95, 105, 255)).semantics(
                Semantics::new(SemanticRole::Checkbox)
                    .label("Experimental uploads")
                    .state(state(Some(false), false)),
            ),
        ]),
        Widget::from(TextField::new(name).placeholder("Your display name"))
            .accessibility_label("Display name")
            .accessibility_description("Editable single-line name"),
        Widget::from(
            TextArea::new(biography)
                .size(Size::new(460., 88.))
                .placeholder("Write a biography"),
        )
        .accessibility_label("Biography")
        .accessibility_description("Editable multi-line text"),
        list,
        modal_demo,
        Widget::row(vec![
            Button::new("Save")
                .on_press(|| println!("save activated"))
                .into(),
            Button::new("Dismiss")
                .color(Color::rgba(112, 76, 156, 255))
                .on_press(|| println!("dismiss activated"))
                .into(),
        ]),
    ])
}

fn main() {
    let mut tree = WidgetTree::new();
    tree.mount(dashboard()).expect("mount semantics gallery");
    tree.layout(Constraints::tight(Size::new(640., 760.)));
    tree.update_semantics();
    println!("{}", tree.semantics_debug_dump());
    println!(
        "Inc﻿ular semantics diagnostics: {:?}",
        tree.semantics_diagnostics()
    );
    println!("Native adapter diagnostics are per window and activate lazily when AT connects.");

    let app = Application::new(move |_| dashboard()).expect("valid semantics application");
    incular::run(app).expect("native semantics application");
}
