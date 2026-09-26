use std::fmt;

use bencher_json::{
    JsonJob, JsonNewCallback, Secret,
    runner::{JobCallbackState, JsonJobCallback},
    sanitize_json,
};
use serde_json::Value;

use crate::parser::run::split_callback_header;

use super::RunError;

/// Build the callback with the constructor the server validates with, so it fails before anything is sent.
pub fn new_callback(
    url: Option<Secret>,
    headers: Option<Vec<Secret>>,
    body: Option<Value>,
) -> Result<Option<JsonNewCallback>, RunError> {
    let Some(url) = url else {
        return Ok(None);
    };
    let headers = (1..)
        .zip(headers.unwrap_or_default())
        .map(|(position, header)| {
            split_callback_header(header.as_ref()).ok_or(RunError::CallbackHeader(position))
        })
        .collect::<Result<Vec<_>, _>>()?;
    JsonNewCallback::new(url.as_ref(), headers, body)
        .map(Some)
        .map_err(RunError::Callback)
}

/// The API client's type, filled from the validated callback so its headers are already case-folded and deduplicated.
pub fn client_callback(
    callback: &JsonNewCallback,
) -> Result<bencher_client::types::JsonNewCallback, RunError> {
    into_client_callback(serde_json::to_value(callback))
}

/// The callback as printed, through `sanitize_json`: in full in a debug build and sanitized in a release build.
pub fn echo_callback(
    callback: &JsonNewCallback,
) -> Result<bencher_client::types::JsonNewCallback, RunError> {
    into_client_callback(Ok(sanitize_json(callback)))
}

fn into_client_callback(
    json: serde_json::Result<Value>,
) -> Result<bencher_client::types::JsonNewCallback, RunError> {
    match json.and_then(serde_json::from_value) {
        Ok(callback) => Ok(callback),
        // A serde error can quote the value it refused, header values included, so it is dropped.
        Err(_) => Err(RunError::ClientCallback),
    }
}

/// What the one read after a detached submit says about its callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackNotice {
    Skipped,
}

impl CallbackNotice {
    pub fn new(json_job: &JsonJob) -> Option<Self> {
        matches!(
            json_job.callback,
            Some(JsonJobCallback {
                state: JobCallbackState::Skipped,
                status: _,
            })
        )
        .then_some(Self::Skipped)
    }
}

impl fmt::Display for CallbackNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skipped => write!(f, "callback skipped: requires a Bencher Plus plan"),
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::JsonJob;
    use serde_json::json;

    use super::CallbackNotice;

    fn notice(state: &str) -> Option<CallbackNotice> {
        let json_job: JsonJob = serde_json::from_value(json!({
            "uuid": "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f",
            "report": "4f6d1b2a-3c5e-4d7f-8a9b-0c1d2e3f4a5b",
            "status": "pending",
            "spec": {
                "uuid": "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
                "name": "Test Spec",
                "slug": "test-spec",
                "os": "linux",
                "architecture": "x86_64",
                "cpu": 2,
                "memory": 4096,
                "disk": 8192,
                "network": false,
                "created": "2026-01-01T00:00:00Z",
                "modified": "2026-01-01T00:00:00Z"
            },
            "timeout": 60,
            "callback": { "state": state, "status": null },
            "created": "2026-01-01T00:00:00Z",
            "modified": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        CallbackNotice::new(&json_job)
    }

    #[test]
    fn skipped_callback_gets_a_notice() {
        assert_eq!(notice("skipped"), Some(CallbackNotice::Skipped));
    }

    #[test]
    fn taken_callback_gets_no_notice() {
        for state in ["pending", "delivered", "failed"] {
            assert_eq!(notice(state), None, "{state}");
        }
    }
}
