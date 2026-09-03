use incular_desktop::{SingleInstanceRole, acquire_single_instance_in};
use incular_platform::{ApplicationActivation, LaunchActivation, SingleInstancePolicy};
use std::{
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tempfile::tempdir;

fn launch(argument: &str) -> ApplicationActivation {
    ApplicationActivation::Launch(LaunchActivation::new([OsString::from(argument)]))
}

#[test]
fn secondary_instances_forward_once_in_connection_order_and_do_not_become_primary() {
    let directory = tempdir().expect("temporary instance directory");
    let policy = SingleInstancePolicy::new("dev.winterhoax.incular.activation-test")
        .expect("single-instance policy");
    let primary = match acquire_single_instance_in(&policy, launch("primary"), directory.path())
        .expect("primary ownership")
    {
        SingleInstanceRole::Primary(primary) => primary,
        SingleInstanceRole::SecondaryForwarded => panic!("first process unexpectedly forwarded"),
    };
    let wake_count = Arc::new(AtomicUsize::new(0));
    let wakes = wake_count.clone();
    primary.set_wake(Arc::new(move || {
        wakes.fetch_add(1, Ordering::SeqCst);
    }));

    for argument in ["second", "third", "fourth"] {
        assert!(matches!(
            acquire_single_instance_in(&policy, launch(argument), directory.path())
                .expect("secondary forwarding"),
            SingleInstanceRole::SecondaryForwarded
        ));
    }

    let forwarded = primary.take_activations();
    assert_eq!(forwarded.len(), 3);
    let arguments = forwarded
        .iter()
        .map(|activation| match activation {
            ApplicationActivation::Launch(launch) => launch.arguments()[0].clone(),
            other => panic!("unexpected forwarded activation: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        arguments,
        ["second", "third", "fourth"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
    );
    assert!(wake_count.load(Ordering::SeqCst) >= 3);
}

#[test]
fn stale_metadata_does_not_prevent_crash_recovery_after_lock_release() {
    let directory = tempdir().expect("temporary instance directory");
    let policy = SingleInstancePolicy::new("dev.winterhoax.incular.stale-test")
        .expect("single-instance policy");
    let primary = match acquire_single_instance_in(&policy, launch("first"), directory.path())
        .expect("first primary")
    {
        SingleInstanceRole::Primary(primary) => primary,
        SingleInstanceRole::SecondaryForwarded => panic!("first process unexpectedly forwarded"),
    };
    drop(primary);

    // Deliberately leave invalid stale endpoint metadata behind. Ownership is
    // determined by the kernel lock, so stale bytes cannot strand the app.
    std::fs::write(
        directory.path().join("primary.json"),
        b"stale primary metadata",
    )
    .expect("write stale metadata");
    let replacement = acquire_single_instance_in(&policy, launch("replacement"), directory.path())
        .expect("replacement primary");
    assert!(matches!(replacement, SingleInstanceRole::Primary(_)));
}
