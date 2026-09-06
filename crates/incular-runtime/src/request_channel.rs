//! Private transport and caller lifetime shared by native services.
use crate::{request_admission::RequestAdmission, tasks::RuntimeWake};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    task::{Context, Poll},
};
use tokio::sync::oneshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChannelError {
    Stopped,
    IdExhausted,
}

pub(crate) struct RequestChannel<Q> {
    sender: mpsc::Sender<Q>,
    admission: RequestAdmission,
    next_id: AtomicU64,
    wake: Mutex<Option<Arc<dyn RuntimeWake>>>,
}
impl<Q> RequestChannel<Q> {
    pub(crate) fn new(sender: mpsc::Sender<Q>) -> Self {
        Self {
            sender,
            admission: RequestAdmission::new(),
            next_id: AtomicU64::new(1),
            wake: Mutex::new(None),
        }
    }
    pub(crate) fn send(&self, value: Q) -> Result<(), ChannelError> {
        self.admission
            .admit(|| self.sender.send(value))
            .ok_or(ChannelError::Stopped)?
            .map_err(|_| ChannelError::Stopped)?;
        self.wake();
        Ok(())
    }
    pub(crate) fn request<R>(
        &self,
        prepare: impl FnOnce(u64) -> (Q, R),
    ) -> Result<R, ChannelError> {
        let (sent, result) = self
            .admission
            .admit(|| {
                let id = self
                    .next_id
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .map_err(|_| ChannelError::IdExhausted)?;
                let (queued, result) = prepare(id);
                Ok((self.sender.send(queued), result))
            })
            .ok_or(ChannelError::Stopped)??;
        sent.map_err(|_| ChannelError::Stopped)?;
        self.wake();
        Ok(result)
    }
    pub(crate) fn cancel<I>(&self, sender: &mpsc::Sender<I>, id: I) {
        if self.admission.admit(|| sender.send(id)).is_some() {
            self.wake();
        }
    }
    pub(crate) fn stop(&self) {
        self.admission.stop();
        let previous = self.wake.lock().expect("native wake slot").take();
        drop(previous);
    }
    pub(crate) fn set_wake(&self, wake: Arc<dyn RuntimeWake>) {
        let previous = self.wake.lock().expect("native wake slot").replace(wake);
        drop(previous);
    }
    pub(crate) fn wake(&self) {
        let wake = self.wake.lock().expect("native wake slot").clone();
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}

/// A reply is consumed once. Closing before abandonment makes the sender see
/// cancellation even if a synchronous wake immediately drains the host queue.
pub(crate) struct RequestReceiver<T> {
    receiver: Option<oneshot::Receiver<T>>,
    abandoned: Option<Box<dyn FnOnce() + Send + Sync>>,
}
impl<T> RequestReceiver<T> {
    pub(crate) fn new(
        receiver: oneshot::Receiver<T>,
        abandoned: impl FnOnce() + Send + Sync + 'static,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            abandoned: Some(Box::new(abandoned)),
        }
    }
    pub(crate) fn try_result(&mut self, closed: impl FnOnce() -> T) -> Option<T> {
        let receiver = self.receiver.as_mut()?;
        let result = match receiver.try_recv() {
            Ok(value) => value,
            Err(oneshot::error::TryRecvError::Empty) => return None,
            Err(oneshot::error::TryRecvError::Closed) => closed(),
        };
        self.receiver.take();
        self.abandoned.take();
        Some(result)
    }
    pub(crate) fn poll_result(
        &mut self,
        cx: &mut Context<'_>,
        closed: impl FnOnce() -> T,
    ) -> Poll<T> {
        let receiver = self
            .receiver
            .as_mut()
            .expect("native request polled after completion");
        let result = match Pin::new(receiver).poll(cx) {
            Poll::Ready(Ok(value)) => value,
            Poll::Ready(Err(_)) => closed(),
            Poll::Pending => return Poll::Pending,
        };
        self.receiver.take();
        self.abandoned.take();
        Poll::Ready(result)
    }
}
impl<T> Drop for RequestReceiver<T> {
    fn drop(&mut self) {
        if let Some(receiver) = &mut self.receiver {
            receiver.close();
        }
        if let Some(abandoned) = self.abandoned.take() {
            abandoned();
        }
    }
}
