use std::time::Duration;

use bencher_callback::{CallbackAttemptClass, CallbackFinish};
use bencher_otel::{ApiCounter, ApiGauge, ApiHistogram, ApiMeter};

/// Count a sealed callback in the gauge, once the row that holds it has committed.
pub fn callback_sealed() {
    ApiMeter::up(ApiGauge::CallbackSealed);
}

/// Start the gauge at the number of callbacks the database holds sealed.
pub(super) fn sealed_at_start(count: i64) {
    ApiMeter::up_by(
        ApiGauge::CallbackSealed,
        u64::try_from(count).unwrap_or_default(),
    );
}

pub(super) fn attempted(class: CallbackAttemptClass) {
    ApiMeter::increment(ApiCounter::CallbackAttempt(class));
}

/// Count a settled callback, which no longer holds its sealed request.
pub(super) fn finished(finish: CallbackFinish, attempts: i32, latency: Duration) {
    ApiMeter::increment(ApiCounter::CallbackFinish(finish));
    ApiMeter::record(
        ApiHistogram::CallbackFinishDuration(finish),
        latency.as_secs_f64(),
    );
    ApiMeter::record(
        ApiHistogram::CallbackFinishAttempts(finish),
        f64::from(attempts),
    );
    ApiMeter::down(ApiGauge::CallbackSealed);
}
