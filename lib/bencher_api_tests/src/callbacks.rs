use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;
use bencher_callback::{CallbackAttempt, CallbackRequest, CallbackSender};
use http::StatusCode;

/// Answers every callback with 200 and keeps a copy of each request, standing in for a receiver.
#[derive(Default)]
pub struct RecordingSender(Mutex<Vec<CallbackRequest>>);

impl RecordingSender {
    pub fn requests(&self) -> Vec<CallbackRequest> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl CallbackSender for RecordingSender {
    async fn send(&self, request: &CallbackRequest) -> CallbackAttempt {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        CallbackAttempt::Delivered(StatusCode::OK)
    }
}
