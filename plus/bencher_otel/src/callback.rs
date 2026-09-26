use bencher_callback::{CallbackAttemptClass, CallbackFinish};
use opentelemetry::KeyValue;

pub(crate) fn attempt_attributes(class: CallbackAttemptClass) -> Vec<KeyValue> {
    let mut attributes = vec![KeyValue::new("class", class.to_string())];
    if let CallbackAttemptClass::Blocked(reason) = class {
        attributes.push(KeyValue::new("reason", reason.to_string()));
    }
    attributes
}

pub(crate) fn finish_attributes(finish: CallbackFinish) -> Vec<KeyValue> {
    let mut attributes = vec![KeyValue::new("outcome", finish.to_string())];
    if let CallbackFinish::Failed(failure) = finish {
        attributes.push(KeyValue::new("reason", failure.to_string()));
    }
    attributes
}
