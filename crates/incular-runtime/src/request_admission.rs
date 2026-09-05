// Admission and shutdown share one critical section. Keep callbacks limited to
// preparing/enqueueing requests; invoke external wake callbacks after admission.
use std::sync::Mutex;

pub(crate) struct RequestAdmission {
    accepting: Mutex<bool>,
}

impl RequestAdmission {
    pub(crate) fn new() -> Self {
        Self {
            accepting: Mutex::new(true),
        }
    }

    /// Runs enqueue work only while accepting. Shutdown cannot complete until
    /// this work finishes, so its subsequent drain includes every accepted item.
    pub(crate) fn admit<T>(&self, enqueue: impl FnOnce() -> T) -> Option<T> {
        let accepting = self.accepting.lock().expect("request admission gate");
        if !*accepting {
            return None;
        }
        let result = enqueue();
        drop(accepting);
        Some(result)
    }

    pub(crate) fn stop(&self) {
        *self.accepting.lock().expect("request admission gate") = false;
    }
}
