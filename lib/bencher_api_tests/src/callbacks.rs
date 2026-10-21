use std::{
    collections::VecDeque,
    sync::{Mutex, PoisonError},
};

use async_trait::async_trait;
use bencher_callback::{CallbackAttempt, CallbackRequest, CallbackSender, RateLimit};
use http::StatusCode;

/// Answers every callback with 200, or with a status queued for it, and keeps a copy of each
/// request, standing in for a receiver.
#[derive(Default)]
pub struct RecordingSender {
    requests: Mutex<Vec<CallbackRequest>>,
    answers: Mutex<VecDeque<StatusCode>>,
}

impl RecordingSender {
    pub fn requests(&self) -> Vec<CallbackRequest> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn answer_next(&self, status: StatusCode) {
        self.answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(status);
    }
}

#[async_trait]
impl CallbackSender for RecordingSender {
    async fn send(&self, request: &CallbackRequest) -> CallbackAttempt {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        let status = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or(StatusCode::OK);
        if status.is_success() {
            CallbackAttempt::Delivered(status)
        } else {
            CallbackAttempt::Refused(status, RateLimit::default())
        }
    }
}
