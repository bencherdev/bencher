//! What a failing answer from the store may say in a log, a report, or an error.
//!
//! An S3 error body can echo the request it refused: a `SignatureDoesNotMatch`
//! body carries the access key ID and the host, also hex-encoded. Nothing from a
//! body leaves this module but its S3 `Code` and `Message`.

use http::StatusCode;
use serde::Deserialize;

/// How `object_store` 0.14 opens the message of a request answered with a failing status.
const STATUS_MESSAGE: &str = "Server returned non-2xx status code: ";

/// How `object_store` 0.14 opens the message of a success whose body holds an `<Error>`.
const ERROR_RESPONSE_MESSAGE: &str = "Server returned error response: ";

/// The `Code` and `Message` of an S3 `<Error>` body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct S3Error {
    code: Option<String>,
    message: Option<String>,
}

impl S3Error {
    /// Reads a body as an S3 `<Error>`, or `None` when its root is anything else.
    pub(crate) fn parse(body: &[u8]) -> Option<Self> {
        if root_name(body)? != "Error" {
            return None;
        }
        quick_xml::de::from_reader(body).ok()
    }

    pub(crate) fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }
}

/// The local name of a body's root element, or `None` when the body is not XML.
pub(crate) fn root_name(body: &[u8]) -> Option<String> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_reader(body);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(element) | Event::Empty(element) => {
                return String::from_utf8(element.local_name().as_ref().to_vec()).ok();
            },
            Event::Eof => return None,
            Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_)
            | Event::Comment(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::GeneralRef(_)
            | Event::End(_) => buf.clear(),
        }
    }
}

/// A failing answer, described by its status and the `Code` and `Message` of its
/// body, and by nothing else from its body.
pub(crate) fn describe(status: Option<StatusCode>, body: &[u8]) -> String {
    describe_error(status, S3Error::parse(body).as_ref())
}

/// A failing answer, described by its status and the `Code` and `Message` of its `<Error>` body.
pub(crate) fn describe_error(status: Option<StatusCode>, error: Option<&S3Error>) -> String {
    let mut description = status.map_or_else(
        || "an error response".to_owned(),
        |status| format!("HTTP {status}"),
    );
    if let Some(code) = error.and_then(S3Error::code) {
        description.push_str(", ");
        description.push_str(code);
    }
    if let Some(message) = error.and_then(|error| error.message.as_deref()) {
        description.push_str(": ");
        description.push_str(message);
    }
    description
}

/// An object store error's text, with the body of any failing answer in it
/// reduced as [`describe`] reduces it.
///
/// The failing answer closes the text, since each error that wraps another
/// ends with the error it wraps.
pub(crate) fn redact(error: &object_store::Error) -> String {
    let text = error.to_string();
    let Some((context, answer)) = [STATUS_MESSAGE, ERROR_RESPONSE_MESSAGE]
        .iter()
        .filter_map(|opening| text.find(opening))
        .min()
        .and_then(|start| text.split_at_checked(start))
    else {
        return text;
    };
    let described = Answer::of_message(answer).map_or_else(
        || "an error response".to_owned(),
        |answer| describe(answer.status, answer.body.as_bytes()),
    );
    format!("{context}{described}")
}

/// The failing answer that an `object_store` error message reports.
pub(crate) struct Answer<'a> {
    pub status: Option<StatusCode>,
    pub body: &'a str,
}

impl<'a> Answer<'a> {
    /// Reads a message that opens with a failing answer: a status and its body,
    /// or an `<Error>` body that came with a success.
    pub(crate) fn of_message(message: &'a str) -> Option<Self> {
        if let Some(body) = message.strip_prefix(ERROR_RESPONSE_MESSAGE) {
            return Some(Self { status: None, body });
        }
        let answer = message.strip_prefix(STATUS_MESSAGE)?;
        let status = StatusCode::from_bytes(answer.get(..3)?.as_bytes()).ok()?;
        let body = answer
            .strip_prefix(status.to_string().as_str())?
            .strip_prefix(": ")?;
        Some(Self {
            status: Some(status),
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use http::StatusCode;

    use super::{describe, redact};

    const ACCESS_KEY_ID: &str = "AKIAFAKEKEYID0123456";
    const HOST: &str = "fakeaccount.r2.cloudflarestorage.com";

    /// A space-separated hex form, as S3 echoes the bytes it signed.
    fn hex_bytes(text: &str) -> String {
        text.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A `SignatureDoesNotMatch` body echoes the access key ID and the host, plain
    /// and hex-encoded, so only its status, `Code`, and `Message` may be told.
    #[test]
    fn a_failing_answer_is_described_by_its_status_code_and_message() {
        let string_to_sign = format!("AWS4-HMAC-SHA256\n20261120T000000Z\n{ACCESS_KEY_ID}");
        let canonical_request =
            format!("PUT\n/my-bucket/key\n\nhost:{HOST}\n\nhost\nUNSIGNED-PAYLOAD");
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Error><Code>SignatureDoesNotMatch</Code><Message>The request signature we calculated does not match the signature you provided.</Message><AWSAccessKeyId>{ACCESS_KEY_ID}</AWSAccessKeyId><StringToSign>{string_to_sign}</StringToSign><SignatureProvided>0123</SignatureProvided><StringToSignBytes>{}</StringToSignBytes><CanonicalRequest>{canonical_request}</CanonicalRequest><CanonicalRequestBytes>{}</CanonicalRequestBytes></Error>"#,
            hex_bytes(&string_to_sign),
            hex_bytes(&canonical_request),
        );

        let described = describe(Some(StatusCode::FORBIDDEN), body.as_bytes());

        assert_eq!(
            described,
            "HTTP 403 Forbidden, SignatureDoesNotMatch: The request signature we calculated does not match the signature you provided."
        );
        for secret in [
            ACCESS_KEY_ID.to_owned(),
            HOST.to_owned(),
            hex_bytes(ACCESS_KEY_ID),
            hex_bytes(HOST),
        ] {
            assert!(!described.contains(&secret), "{described} holds {secret}");
        }
    }

    /// `object_store` reports a failing status, and an `<Error>` body that came
    /// with a success, each in its own form; both keep only the `Code` and `Message`.
    #[test]
    fn an_object_store_error_keeps_its_context_but_not_its_body() {
        let body = format!(
            "<Error><Code>InternalError</Code><Message>Try again.</Message><HostId>{HOST}</HostId><AWSAccessKeyId>{ACCESS_KEY_ID}</AWSAccessKeyId></Error>"
        );
        for (message, expected) in [
            (
                format!("Server returned non-2xx status code: 500 Internal Server Error: {body}"),
                "HTTP 500 Internal Server Error, InternalError: Try again.",
            ),
            (
                format!("Server returned error response: {body}"),
                "an error response, InternalError: Try again.",
            ),
        ] {
            let error = object_store::Error::Generic {
                store: "S3",
                source: format!("Error performing PUT /my-bucket/key in 2ms - {message}").into(),
            };

            assert_eq!(
                redact(&error),
                format!(
                    "Generic S3 error: Error performing PUT /my-bucket/key in 2ms - {expected}"
                )
            );
        }
    }

    /// A body that is not an S3 `<Error>` says nothing at all.
    #[test]
    fn a_failing_answer_without_an_error_body_is_described_by_its_status() {
        assert_eq!(
            describe(
                Some(StatusCode::BAD_GATEWAY),
                format!("<html>{HOST} {ACCESS_KEY_ID}</html>").as_bytes()
            ),
            "HTTP 502 Bad Gateway"
        );
        assert_eq!(
            describe(None, format!("{HOST} {ACCESS_KEY_ID}").as_bytes()),
            "an error response"
        );
    }
}
