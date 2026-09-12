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
fn shortcut_shutdown_resolves_queued_and_dispatched_work_while_application_stays_alive() {
    for dispatched in [false, true] {
        let mut application = application();
        application.set_platform_capabilities(shortcut_capabilities(CapabilitySupport::Supported));
        let shortcuts = application.global_shortcuts();
        let chord = GlobalShortcutChord::new(Code::KeyR, Modifiers::CONTROL).unwrap();
        let request = shortcuts.register(GlobalShortcutId::new(1), chord).unwrap();
        let native = dispatched.then(|| {
            application
                .take_native_global_shortcut_requests()
                .pop()
                .unwrap()
        });
        application.shutdown();
        assert!(matches!(
            pollster::block_on(request),
            Err(GlobalShortcutError::ApplicationStopped)
        ));
        assert!(matches!(
            shortcuts.register(GlobalShortcutId::new(2), chord),
            Err(GlobalShortcutError::ApplicationStopped)
        ));
        assert!(
            application
                .take_native_global_shortcut_requests()
                .is_empty()
        );
        if let Some(native) = native {
            let completion = incular_runtime::NativeGlobalShortcutCompletion {
                request_id: native.request_id,
                result: Ok(()),
            };
            assert_eq!(
                application.complete_global_shortcut_request(completion),
                GlobalShortcutCompletionStatus::UnknownRequest
            );
        }
        application.shutdown();
        assert!(
            application
                .take_native_global_shortcut_requests()
                .is_empty()
        );
    }
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

fn rambler_route(url: &str) -> Option<RouteInformation> {
    let parsed = Url::parse(url).ok()?;
    (parsed.scheme() == "rambler").then(|| RouteInformation::new(parsed.path()))
}

fn bridge_rambler(
    application: &Application,
) -> (
    Rc<MemoryRouteInformationProvider>,
    incular_runtime::ActivationRouteBridge,
) {
    let provider = Rc::new(MemoryRouteInformationProvider::root());
    let provider_trait: Rc<dyn RouteInformationProvider> = provider.clone();
    let bridge = application.bridge_activation_routes(provider_trait, rambler_route);
    (provider, bridge)
}

#[test]
fn queued_activations_deliver_when_bridge_installs_later() {
    let mut application = application();
    application.handle_application_activation(ApplicationActivation::open_urls([Url::parse(
        "rambler://settings/audio",
    )
    .expect("first URL")]));
    application.handle_application_activation(ApplicationActivation::open_urls([Url::parse(
        "rambler://settings/video",
    )
    .expect("second URL")]));
    assert_eq!(application.activations().pending_count(), 2);

    let (provider, bridge) = bridge_rambler(&application);
    assert_eq!(application.activations().pending_count(), 0);
    assert_eq!(provider.value().location(), "/video");
    assert_eq!(bridge.delivered_routes(), 2);
}

#[test]
fn bridge_replacement_redelivers_to_the_new_bridge_only() {
    let mut application = application();
    application.handle_application_activation(ApplicationActivation::open_urls([Url::parse(
        "rambler://settings/audio",
    )
    .expect("first URL")]));
    let (provider, first) = bridge_rambler(&application);
    assert_eq!(first.delivered_routes(), 1);
    assert_eq!(provider.value().location(), "/audio");
    drop(first);

    application.handle_application_activation(ApplicationActivation::open_urls([Url::parse(
        "rambler://settings/video",
    )
    .expect("second URL")]));
    assert_eq!(application.activations().pending_count(), 1);
    assert_eq!(provider.value().location(), "/audio");
    // Re-bridging onto the same provider delivers the buffered event to
    // the new bridge only.
    let provider_trait: Rc<dyn RouteInformationProvider> = provider.clone();
    let second = application.bridge_activation_routes(provider_trait, rambler_route);
    assert_eq!(second.delivered_routes(), 1);
    assert_eq!(provider.value().location(), "/video");
    assert_eq!(application.activations().pending_count(), 0);
}

#[test]
fn dropped_bridge_buffers_without_delivery() {
    let mut application = application();
    let provider = Rc::new(MemoryRouteInformationProvider::root());
    let initial = provider.value().location().to_owned();
    let provider_trait: Rc<dyn RouteInformationProvider> = provider.clone();
    let bridge = application.bridge_activation_routes(provider_trait, rambler_route);
    drop(bridge);
    application.handle_application_activation(ApplicationActivation::open_urls([Url::parse(
        "rambler://settings/audio",
    )
    .expect("URL")]));
    // No bridge is installed: the provider is untouched and the event waits.
    assert_eq!(provider.value().location(), initial);
    assert_eq!(application.activations().pending_count(), 1);
    // Re-bridging delivers the buffered activation exactly once.
    let provider_trait: Rc<dyn RouteInformationProvider> = provider.clone();
    let bridge = application.bridge_activation_routes(provider_trait, rambler_route);
    assert_eq!(bridge.delivered_routes(), 1);
    assert_eq!(provider.value().location(), "/audio");
    assert_eq!(application.activations().pending_count(), 0);
}

#[test]
fn repeated_equal_activations_both_deliver_without_dedup() {
    let mut application = application();
    let (provider, bridge) = bridge_rambler(&application);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_for_listener = seen.clone();
    let _subscription = provider.subscribe(Rc::new(move |information| {
        seen_for_listener
            .borrow_mut()
            .push(information.location().to_owned());
    }));
    let url = || Url::parse("rambler://settings/audio").expect("URL");
    application.handle_application_activation(ApplicationActivation::open_urls([url()]));
    application.handle_application_activation(ApplicationActivation::open_urls([url()]));
    assert_eq!(bridge.delivered_routes(), 2);
    assert_eq!(&*seen.borrow(), &["/audio", "/audio"]);
    assert_eq!(provider.value().location(), "/audio");
}

#[test]
fn bridge_level_reentrancy_delivers_nested_activation_once() {
    let mut application = application();
    let activations = application.activations();
    let (provider, bridge) = bridge_rambler(&application);
    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let nested = activations.clone();
    let _subscription = activations.subscribe(move |activation| {
        output.borrow_mut().push(activation.clone());
        if activation == ApplicationActivation::Reopen {
            nested.publish(ApplicationActivation::open_urls([Url::parse(
                "rambler://settings/nested",
            )
            .expect("nested URL")]));
        }
    });
    application.handle_application_activation(ApplicationActivation::Reopen);
    assert_eq!(received.borrow().len(), 2);
    assert_eq!(received.borrow()[0], ApplicationActivation::Reopen);
    assert_eq!(
        received.borrow()[1].urls()[0].as_str(),
        "rambler://settings/nested"
    );
    assert_eq!(bridge.delivered_routes(), 1);
    assert_eq!(provider.value().location(), "/nested");
}

#[test]
fn panicking_listener_releases_dispatch_without_losing_queue() {
    use std::panic::AssertUnwindSafe;

    let mut application = application();
    let activations = application.activations();
    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let nested = activations.clone();
    let _subscription = activations.subscribe(move |activation| {
        output.borrow_mut().push(activation.clone());
        if activation == ApplicationActivation::Reopen {
            nested.publish(ApplicationActivation::open_files([PathBuf::from(
                r"C:\capture\nested.ram",
            )]));
            panic!("listener failure");
        }
    });
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        application.handle_application_activation(ApplicationActivation::Reopen);
    }));
    assert!(outcome.is_err());
    // The nested activation queued before the panic is not lost, and a
    // later publish resumes delivery exactly once each, in order.
    let nested_file = PathBuf::from(r"C:\capture\nested.ram");
    let later_file = PathBuf::from(r"C:\capture\later.ram");
    application
        .handle_application_activation(ApplicationActivation::open_files([later_file.clone()]));
    assert_eq!(received.borrow().len(), 3);
    assert_eq!(received.borrow()[0], ApplicationActivation::Reopen);
    assert_eq!(received.borrow()[1].documents()[0].path(), nested_file);
    assert_eq!(received.borrow()[2].documents()[0].path(), later_file);
}

#[test]
fn panicking_first_listener_skips_remaining_without_replay() {
    use std::panic::AssertUnwindSafe;

    let mut application = application();
    let activations = application.activations();
    let first_seen = Rc::new(RefCell::new(Vec::new()));
    let second_seen = Rc::new(RefCell::new(Vec::new()));
    let nested = activations.clone();
    let _first = activations.subscribe({
        let first_seen = first_seen.clone();
        move |activation| {
            first_seen.borrow_mut().push(activation.clone());
            if activation == ApplicationActivation::Reopen {
                nested.publish(ApplicationActivation::open_files([PathBuf::from(
                    r"C:\capture\nested.ram",
                )]));
                panic!("first listener failure");
            }
        }
    });
    let _second = activations.subscribe({
        let second_seen = second_seen.clone();
        move |activation| {
            second_seen.borrow_mut().push(activation);
        }
    });
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        application.handle_application_activation(ApplicationActivation::Reopen);
    }));
    assert!(outcome.is_err());
    // The interrupted activation reached only its panicking listener: it
    // is not replayed to the second listener. The queued activation is
    // preserved.
    assert_eq!(&*first_seen.borrow(), &[ApplicationActivation::Reopen]);
    assert!(second_seen.borrow().is_empty());
    assert_eq!(activations.pending_count(), 1);

    // A subsequent publish resumes delivery: both listeners receive the
    // queued and the new activation exactly once each, in order.
    let later_file = PathBuf::from(r"C:\capture\later.ram");
    application
        .handle_application_activation(ApplicationActivation::open_files([later_file.clone()]));
    assert_eq!(first_seen.borrow().len(), 3);
    assert_eq!(
        first_seen.borrow()[1].documents()[0].path(),
        PathBuf::from(r"C:\capture\nested.ram")
    );
    assert_eq!(first_seen.borrow()[2].documents()[0].path(), later_file);
    assert_eq!(second_seen.borrow().len(), 2);
    assert_eq!(
        second_seen.borrow()[0].documents()[0].path(),
        PathBuf::from(r"C:\capture\nested.ram")
    );
    assert_eq!(second_seen.borrow()[1].documents()[0].path(), later_file);
    assert_eq!(activations.pending_count(), 0);
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
