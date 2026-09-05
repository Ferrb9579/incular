mod admission {
    include!("../src/request_admission.rs");

    // White-box assertion stays in tests/. It makes an early-unlock regression
    // fail deterministically, independent of which worker is scheduled first.
    pub(super) fn assert_enqueue_holds_gate(gate: &RequestAdmission) {
        assert!(matches!(
            gate.accepting.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
    }
}

use admission::RequestAdmission;
use std::sync::{Arc, mpsc};

#[test]
fn shutdown_drain_contains_an_admitted_enqueue_that_was_paused() {
    let gate = Arc::new(RequestAdmission::new());
    let (queue, receiver) = mpsc::channel();
    let (entered, admitted) = mpsc::sync_channel(0);
    let (release, resume) = mpsc::sync_channel(0);
    let producer_gate = gate.clone();
    let producer = std::thread::spawn(move || {
        producer_gate
            .admit(|| {
                entered.send(()).unwrap();
                resume.recv().unwrap();
                queue.send(42).unwrap();
            })
            .unwrap();
    });
    admitted.recv().unwrap();
    admission::assert_enqueue_holds_gate(&gate);
    let stopping_gate = gate.clone();
    let stopper = std::thread::spawn(move || {
        stopping_gate.stop();
        receiver.try_iter().collect::<Vec<_>>()
    });
    release.send(()).unwrap();
    producer.join().unwrap();
    assert_eq!(stopper.join().unwrap(), vec![42]);
    assert!(gate.admit(|| panic!("stopped gate ran enqueue")).is_none());
}

#[test]
fn repeated_shutdown_rejects_work_without_invoking_it() {
    let gate = RequestAdmission::new();
    gate.stop();
    gate.stop();
    assert!(gate.admit(|| panic!("stopped gate ran enqueue")).is_none());
}
