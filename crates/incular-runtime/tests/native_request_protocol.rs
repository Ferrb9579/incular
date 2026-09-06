#[path = "../src/request_admission.rs"]
mod request_admission;
mod tasks {
    pub use incular_runtime::RuntimeWake;
}
#[allow(dead_code)]
#[path = "../src/request_channel.rs"]
mod channel;
#[allow(dead_code)]
#[path = "../src/request_registry.rs"]
mod registry;

use channel::{ChannelError, RequestChannel, RequestReceiver};
use registry::{CompletionRejection, RequestRegistry};
use std::sync::{Arc, Mutex, mpsc};
use tokio::sync::oneshot;

struct StopOnWake(std::sync::Weak<RequestChannel<u64>>);
impl tasks::RuntimeWake for StopOnWake {
    fn wake(&self) {
        self.0.upgrade().unwrap().stop();
    }
}

#[test]
fn wake_can_reenter_shutdown_and_all_accepted_work_is_drainable() {
    let (sender, receiver) = mpsc::channel();
    let channel = Arc::new(RequestChannel::new(sender));
    channel.set_wake(Arc::new(StopOnWake(Arc::downgrade(&channel))));
    channel.request(|id| (id, ())).unwrap();
    assert_eq!(receiver.try_iter().collect::<Vec<_>>(), vec![1]);
    assert_eq!(channel.send(2), Err(ChannelError::Stopped));
    let (cancel, cancellations) = mpsc::channel();
    channel.cancel(&cancel, 1);
    assert!(cancellations.try_recv().is_err());
}

#[test]
fn shutdown_waits_for_paused_admission_and_rejects_later_requests() {
    let (sender, receiver) = mpsc::channel();
    let channel = Arc::new(RequestChannel::new(sender));
    let (entered, observed) = mpsc::sync_channel(0);
    let (release, resume) = mpsc::sync_channel(0);
    let producer = {
        let channel = channel.clone();
        std::thread::spawn(move || {
            channel.request(|id| {
                entered.send(()).unwrap();
                resume.recv().unwrap();
                (id, ())
            })
        })
    };
    observed.recv().unwrap();
    let stopper = {
        let channel = channel.clone();
        std::thread::spawn(move || channel.stop())
    };
    release.send(()).unwrap();
    producer.join().unwrap().unwrap();
    stopper.join().unwrap();
    assert_eq!(receiver.try_iter().collect::<Vec<_>>(), vec![1]);
    assert_eq!(channel.request(|id| (id, ())), Err(ChannelError::Stopped));
}

#[test]
fn failed_send_releases_admission_before_dropping_the_caller_ticket() {
    let (sender, receiver) = mpsc::channel::<()>();
    drop(receiver);
    let channel = Arc::new(RequestChannel::new(sender));
    let clone = channel.clone();
    let result = channel.request(|_| {
        let (_, receiver) = oneshot::channel::<()>();
        ((), RequestReceiver::new(receiver, move || clone.stop()))
    });
    assert!(matches!(result, Err(ChannelError::Stopped)));
}

#[test]
fn reply_inspection_consumes_success_and_closed_channel_exactly_once() {
    for delivered in [true, false] {
        let (sender, receiver) = oneshot::channel();
        let mut receiver = RequestReceiver::new(receiver, || panic!("consumed request abandoned"));
        if delivered {
            sender.send(Ok::<_, ()>(7)).unwrap();
        } else {
            drop(sender);
        }
        assert_eq!(
            receiver.try_result(|| Err(())),
            Some(if delivered { Ok(7) } else { Err(()) })
        );
        assert_eq!(receiver.try_result(|| Err(())), None);
    }
}

#[test]
fn abandonment_closes_delivery_before_reentrant_host_work() {
    let (sender, receiver) = oneshot::channel::<()>();
    let sender = Arc::new(Mutex::new(Some(sender)));
    let observed = sender.clone();
    drop(RequestReceiver::new(receiver, move || {
        assert!(observed.lock().unwrap().as_ref().unwrap().is_closed())
    }));
    assert!(sender.lock().unwrap().take().unwrap().send(()).is_err());
}

#[test]
fn registry_rejects_early_wrong_owner_duplicate_and_retired_completions() {
    let mut requests = RequestRegistry::new();
    requests.insert(1_u64, "owner-a");
    requests.insert(2, "owner-b");
    assert_eq!(
        requests.complete(&1, |_| true),
        Err(CompletionRejection::NotDispatched)
    );
    assert!(requests.dispatch(&1));
    assert!(!requests.dispatch(&1));
    assert_eq!(
        requests.complete(&1, |owner| *owner == "owner-b"),
        Err(CompletionRejection::TargetMismatch)
    );
    requests.abandon(&1);
    assert_eq!(
        requests.complete(&1, |owner| *owner == "owner-a"),
        Ok("owner-a")
    );
    assert_eq!(
        requests.complete(&1, |_| true),
        Err(CompletionRejection::Unknown)
    );
    assert_eq!(requests.drain().collect::<Vec<_>>(), vec![(2, "owner-b")]);
    assert_eq!(requests.len(), 0);
    assert_eq!(
        requests.complete(&2, |_| true),
        Err(CompletionRejection::Unknown)
    );
}
