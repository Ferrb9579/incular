use incular_core::{Code, Color, Modifiers, Size};
use incular_platform::{
    ApplicationActivation, CapabilitySupport, GlobalShortcutChord, GlobalShortcutError,
    GlobalShortcutId, PlatformCapabilities,
};
use incular_runtime::{
    Application, GlobalShortcutCompletionStatus, MemoryGlobalShortcutAdapter,
    NativeGlobalShortcutOperation,
};
use incular_widgets::{
    MemoryRouteInformationProvider, RouteInformation, RouteInformationProvider, Widget,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use url::Url;

fn application() -> Application {
    Application::new(|_| Widget::box_(Size::new(1.0, 1.0), Color::WHITE)).expect("application")
}

fn shortcut_capabilities(support: CapabilitySupport) -> PlatformCapabilities {
    let mut capabilities = PlatformCapabilities::unsupported();
    capabilities.application_services.global_shortcuts = support;
    capabilities
}

#[test]
fn activations_buffer_before_first_listener_and_drain_in_exact_order() {
    let mut application = application();
    let activations = application.activations();
    let file = PathBuf::from(r"C:\capture\session.ram");
    let url = Url::parse("rambler://settings/microphone").expect("URL");
    application.handle_application_activation(ApplicationActivation::Reopen);
    application.handle_application_activation(ApplicationActivation::open_files([file.clone()]));
    application.handle_application_activation(ApplicationActivation::open_urls([url.clone()]));
    assert_eq!(activations.pending_count(), 3);

    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let _subscription = activations.subscribe(move |activation| {
        output.borrow_mut().push(activation);
    });
    assert_eq!(activations.pending_count(), 0);
    assert_eq!(received.borrow().len(), 3);
    assert_eq!(received.borrow()[0], ApplicationActivation::Reopen);
    assert_eq!(received.borrow()[1].documents()[0].path(), file);
    assert_eq!(received.borrow()[2].urls(), &[url]);
}

#[test]
fn reentrant_activation_delivery_keeps_fifo_order_and_delivers_once() {
    let mut application = application();
    let activations = application.activations();
    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let nested = activations.clone();
    let _subscription = activations.subscribe(move |activation| {
        output.borrow_mut().push(activation.clone());
        if activation == ApplicationActivation::Reopen {
            nested.publish(ApplicationActivation::open_urls([Url::parse(
                "rambler://nested",
            )
            .expect("nested URL")]));
        }
    });
    application.handle_application_activation(ApplicationActivation::Reopen);
    assert_eq!(received.borrow().len(), 2);
    assert_eq!(received.borrow()[0], ApplicationActivation::Reopen);
    assert_eq!(received.borrow()[1].urls()[0].as_str(), "rambler://nested");
}

#[test]
fn deep_links_enter_the_existing_route_information_provider_path() {
    let mut application = application();
    let provider = Rc::new(MemoryRouteInformationProvider::root());
    let provider_trait: Rc<dyn RouteInformationProvider> = provider.clone();
    let _bridge = application.bridge_activation_routes(provider_trait, |url| {
        let parsed = Url::parse(url).ok()?;
        (parsed.scheme() == "rambler").then(|| {
            let location = format!(
                "/{}{}{}",
                parsed.host_str().unwrap_or_default(),
                parsed.path(),
                parsed
                    .query()
                    .map(|query| format!("?{query}"))
                    .unwrap_or_default()
            );
            RouteInformation::new(location)
        })
    });

    application.handle_application_activation(ApplicationActivation::open_urls([Url::parse(
        "rambler://settings/audio?source=mic",
    )
    .expect("deep link")]));
    assert_eq!(provider.value().location(), "/settings/audio?source=mic");
    assert_eq!(
        provider.report_count(),
        0,
        "native route input is not a router report"
    );
}

#[test]
fn global_shortcut_registration_conflict_unregister_and_stable_ids_are_typed() {
    let mut application = application();
    application.set_platform_capabilities(shortcut_capabilities(CapabilitySupport::Supported));
    let shortcuts = application.global_shortcuts();
    let mut adapter = MemoryGlobalShortcutAdapter::default();
    let chord = GlobalShortcutChord::new(Code::KeyR, Modifiers::CONTROL | Modifiers::SHIFT)
        .expect("valid chord");
    let other_chord = GlobalShortcutChord::new(Code::KeyM, Modifiers::CONTROL | Modifiers::SHIFT)
        .expect("valid second chord");
    let first_id = GlobalShortcutId::new(11);
    let second_id = GlobalShortcutId::new(29);

    let first_request = shortcuts.register(first_id, chord).expect("first request");
    let first_native = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("first native request");
    assert_eq!(
        first_native.operation,
        NativeGlobalShortcutOperation::Register {
            id: first_id,
            chord
        }
    );
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(first_native)),
        GlobalShortcutCompletionStatus::Completed
    );
    let first = pollster::block_on(first_request).expect("first registration");

    let conflicting_request = shortcuts
        .register(second_id, chord)
        .expect("conflicting request reaches native adapter");
    let conflicting_native = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("conflicting native request");
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(conflicting_native)),
        GlobalShortcutCompletionStatus::Completed
    );
    assert!(matches!(
        pollster::block_on(conflicting_request),
        Err(GlobalShortcutError::Conflict(value)) if value == chord
    ));

    let second_request = shortcuts
        .register(second_id, other_chord)
        .expect("second request");
    let second_native = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("second native request");
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(second_native)),
        GlobalShortcutCompletionStatus::Completed
    );
    let second = pollster::block_on(second_request).expect("second registration");
    assert_eq!(adapter.registration_count(), 2);
    assert_eq!(
        adapter.invoke(first_id),
        Some(ApplicationActivation::GlobalShortcutInvoked(first_id))
    );
    assert_eq!(
        adapter.invoke(second_id),
        Some(ApplicationActivation::GlobalShortcutInvoked(second_id))
    );

    let explicit_unregister = second.unregister().expect("explicit unregister request");
    let native_unregister = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("native unregister");
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(native_unregister)),
        GlobalShortcutCompletionStatus::Completed
    );
    pollster::block_on(explicit_unregister).expect("unregister result");
    assert_eq!(adapter.registration_count(), 1);

    drop(first);
    let dropped_unregister = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("RAII unregister");
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(dropped_unregister)),
        GlobalShortcutCompletionStatus::Completed
    );
    assert_eq!(adapter.registration_count(), 0);
}

#[test]
fn unsupported_global_shortcut_capability_fails_before_native_execution() {
    let mut application = application();
    application.set_platform_capabilities(shortcut_capabilities(CapabilitySupport::Unsupported));
    let shortcuts = application.global_shortcuts();
    let chord = GlobalShortcutChord::new(Code::KeyR, Modifiers::META).expect("valid chord");
    assert!(matches!(
        shortcuts.register(GlobalShortcutId::new(1), chord),
        Err(GlobalShortcutError::Unsupported)
    ));
    assert!(
        application
            .take_native_global_shortcut_requests()
            .is_empty()
    );
}

#[test]
fn global_shortcut_can_be_queued_before_platform_capability_discovery() {
    let mut application = application();
    assert_eq!(
        application.global_shortcuts().support(),
        CapabilitySupport::Unknown
    );
    let chord = GlobalShortcutChord::new(Code::KeyR, Modifiers::CONTROL | Modifiers::SHIFT)
        .expect("valid chord");
    let id = GlobalShortcutId::new(41);
    let request = application
        .global_shortcuts()
        .register(id, chord)
        .expect("unknown capability should queue until native discovery");
    let native = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("queued native registration");
    assert_eq!(
        native.operation,
        NativeGlobalShortcutOperation::Register { id, chord }
    );
    let mut adapter = MemoryGlobalShortcutAdapter::default();
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(native)),
        GlobalShortcutCompletionStatus::Completed
    );
    let registration = pollster::block_on(request).expect("native registration result");
    assert_eq!(registration.id(), id);
    drop(registration);
    let unregister = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("RAII unregister after pre-run registration");
    assert_eq!(
        application.complete_global_shortcut_request(adapter.respond(unregister)),
        GlobalShortcutCompletionStatus::Completed
    );
    assert_eq!(adapter.registration_count(), 0);
}

#[test]
fn abandoned_successful_native_registration_requests_immediate_rollback() {
    let mut application = application();
    application.set_platform_capabilities(shortcut_capabilities(CapabilitySupport::Supported));
    let shortcuts = application.global_shortcuts();
    let chord = GlobalShortcutChord::new(Code::KeyR, Modifiers::ALT).expect("valid chord");
    let id = GlobalShortcutId::new(7);
    let request = shortcuts.register(id, chord).expect("registration request");
    let native = application
        .take_native_global_shortcut_requests()
        .pop()
        .expect("native request");
    drop(request);
    assert_eq!(
        application.complete_global_shortcut_request(
            incular_runtime::NativeGlobalShortcutCompletion {
                request_id: native.request_id,
                result: Ok(()),
            }
        ),
        GlobalShortcutCompletionStatus::AbandonedRegistration(id)
    );
}
