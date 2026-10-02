use bencher_json::{
    JsonJob, JsonNewCallback, Secret,
    runner::{JobCallbackState, JsonJobCallback},
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
    match serde_json::to_value(callback).and_then(serde_json::from_value) {
        Ok(callback) => Ok(callback),
        // A serde error can quote the value it refused, header values included, so it is dropped.
        Err(_) => Err(RunError::ClientCallback),
    }
}

pub const CALLBACK_SKIPPED: &str = "callback skipped: claim this project to send a GitHub Actions dispatch or upgrade to Bencher Plus for custom callbacks";

pub fn callback_skipped(json_job: &JsonJob) -> bool {
    matches!(
        json_job.callback,
        Some(JsonJobCallback {
            state: JobCallbackState::Skipped,
            status: _,
        })
    )
}
