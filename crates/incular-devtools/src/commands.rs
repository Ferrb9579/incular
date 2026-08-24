//! UI-thread command envelope. The runner drains these every frame via
//! `try_recv`, so DevTools can never block application rendering.

use incular_devtools_protocol::{ErrorCode, RequestMethod, ResponsePayload};

/// One request routed to the UI thread together with the reply id.
#[derive(Debug)]
pub struct UiCommand {
    pub request_id: u64,
    pub body: RequestMethod,
}

/// Reply built by the UI thread after executing a command.
#[derive(Debug)]
pub struct UiReply {
    pub request_id: u64,
    pub payload: ResponsePayload,
}
impl UiReply {
    pub fn ok(request_id: u64) -> Self {
        Self {
            request_id,
            payload: ResponsePayload::Ok,
        }
    }
    pub fn with(request_id: u64, payload: ResponsePayload) -> Self {
        Self {
            request_id,
            payload,
        }
    }
    pub fn error(request_id: u64, code: ErrorCode) -> Self {
        Self::with(
            request_id,
            ResponsePayload::Error {
                code,
                message: String::new(),
            },
        )
    }
}
