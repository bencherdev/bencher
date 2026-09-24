use std::collections::BTreeMap;
use std::fmt;

use http::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use url::Url;

use crate::{JsonReport, Sanitize, Secret};

mod body;

use body::validate_body;
pub use body::{CallbackContext, CallbackRenderError};

pub const MAX_CALLBACK_URL_LEN: usize = 2048;
pub const MAX_CALLBACK_HEADERS: usize = 16;
pub const MAX_CALLBACK_HEADER_NAME_LEN: usize = 256;
pub const MAX_CALLBACK_HEADER_VALUE_LEN: usize = 8 << 10;
pub const MAX_CALLBACK_BODY_LEN: usize = 64 << 10;

const NOT_AN_OBJECT: &str = "callback must be an object";
const HEADERS_NOT_A_MAP: &str = "callback headers must be a map of names to values";
const HEADER_VALUE_NOT_A_STRING: &str = "callback header value must be a string";
// The HTTP client owns message framing and the connection, so these are never the customer's.
const REFUSED_HEADERS: [&str; 9] = [
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
    "upgrade",
];

/// An HTTP request Bencher sends once, when a job reaches a terminal state.
/// Its body is JSON with placeholders or, without one, the job's report.
#[typeshare::typeshare]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "JsonUncheckedNewCallback")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewCallback {
    /// The `https` URL to send the request to
    url: Url,
    /// Request headers. Names are case-insensitive, and a later duplicate replaces an earlier one.
    /// A header named `Content-Type` or `User-Agent` replaces Bencher's default.
    // `default` only feeds the generated types: deserializing goes through `JsonUncheckedNewCallback`.
    // A `Secret` is never empty, so an empty value, which HTTP allows, is `None`.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "serialize_headers"
    )]
    #[cfg_attr(feature = "schema", schemars(with = "BTreeMap<String, Secret>"))]
    #[typeshare(typescript(type = "Record<string, string>"))]
    headers: BTreeMap<String, Option<Secret>>,
    /// The JSON request body. A string value that is exactly `{{ job.uuid }}`, `{{ job.status }}`,
    /// `{{ report.uuid }}`, `{{ project.uuid }}`, or `{{ project.slug }}` becomes that value, and one
    /// that is exactly `{{ report }}` becomes the job's report, which a body can send only once.
    /// Whitespace inside the braces is optional. Every other string, every key, and every other
    /// value is sent as it is.
    /// Without a body, the body is the job's report, as the report endpoint returns it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "unknown"))]
    body: Option<Value>,
    #[serde(skip)]
    origin: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    #[error("failed to parse callback URL: {0}")]
    Url(url::ParseError),
    #[error("callback URL must use https")]
    NotHttps,
    #[error("callback URL length {0} exceeds maximum {MAX_CALLBACK_URL_LEN}")]
    UrlTooLong(usize),
    #[error("callback header count exceeds maximum {MAX_CALLBACK_HEADERS}")]
    TooManyHeaders,
    #[error(
        "callback header {position} name length {len} exceeds maximum {MAX_CALLBACK_HEADER_NAME_LEN}"
    )]
    HeaderNameTooLong { position: usize, len: usize },
    #[error("invalid name for callback header {position}: {error}")]
    HeaderName {
        position: usize,
        error: http::header::InvalidHeaderName,
    },
    #[error("callback header {0} is set by the HTTP client")]
    RefusedHeader(String),
    #[error(
        "callback header {name} value length {len} exceeds maximum {MAX_CALLBACK_HEADER_VALUE_LEN}"
    )]
    HeaderValueTooLong { name: String, len: usize },
    #[error("invalid value for callback header {name}: {error}")]
    HeaderValue {
        name: String,
        error: http::header::InvalidHeaderValue,
    },
    /// The location is a JSON pointer, escaped so that a key cannot break a log line.
    #[error(
        "callback body placeholder at \"{0}\" names an unknown value; the names are job.uuid, job.status, report.uuid, project.uuid, project.slug, and report"
    )]
    UnknownPlaceholder(String),
    /// The location is a JSON pointer, escaped so that a key cannot break a log line.
    #[error(
        "callback body placeholder at \"{0}\" sends the report a second time; a body can send it only once"
    )]
    RepeatedReport(String),
    #[error(
        "callback body length {0}, with each placeholder at its longest value, exceeds maximum {MAX_CALLBACK_BODY_LEN}"
    )]
    BodyTooLong(usize),
}

impl JsonNewCallback {
    pub fn new<H>(url: &str, headers: H, body: Option<Value>) -> Result<Self, CallbackError>
    where
        H: IntoIterator<Item = (String, String)>,
    {
        let (url, origin) = parse_url(url)?;
        let headers = collect_headers(headers)?;
        // A `null` body is no body, as it is when a request is deserialized.
        let body = body.filter(|body| !body.is_null());
        if let Some(body) = &body {
            validate_body(body)?;
        }
        Ok(Self {
            url,
            headers,
            body,
            origin,
        })
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    /// The headers to send: Bencher's defaults, replaced by any customer header of the same name.
    /// Every value is sensitive, so the map's `Debug` never prints one.
    pub fn delivery_headers(&self, version: &str) -> Result<HeaderMap, http::Error> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, format!("bencher/{version}").try_into()?);
        for (name, value) in &self.headers {
            headers.insert(
                HeaderName::try_from(name.as_str())?,
                header_str(value.as_ref()).try_into()?,
            );
        }
        headers
            .values_mut()
            .for_each(|value| value.set_sensitive(true));
        Ok(headers)
    }

    /// Render the body, or without one the job's report. `report` builds the report, and runs
    /// only for a body that sends it.
    pub async fn render<F, Fut, E>(
        &self,
        context: &CallbackContext,
        report: F,
    ) -> Result<String, CallbackRenderError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<JsonReport, E>>,
    {
        body::render(self.body.as_ref(), context, report).await
    }
}

impl fmt::Debug for JsonNewCallback {
    // Like `Secret`, a release build hides what can hold a secret: the URL's userinfo, path, and query.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            url,
            headers,
            body,
            origin,
        } = self;
        let url = if cfg!(debug_assertions) { url } else { origin };
        f.debug_struct("JsonNewCallback")
            .field("url", &url.as_str())
            .field("headers", headers)
            .field("body", body)
            .finish()
    }
}

impl Sanitize for JsonNewCallback {
    fn sanitize(&mut self) {
        let Self {
            url,
            headers,
            body: _,
            origin,
        } = self;
        url.clone_from(origin);
        headers.values_mut().for_each(Sanitize::sanitize);
    }
}

fn serialize_headers<S>(
    headers: &BTreeMap<String, Option<Secret>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_map(
        headers
            .iter()
            .map(|(name, value)| (name, header_str(value.as_ref()))),
    )
}

fn header_str(value: Option<&Secret>) -> &str {
    value.map_or("", AsRef::as_ref)
}

/// Deserialized by hand, because serde quotes a string or number of the wrong type in its
/// error, and the API server logs deserialization errors.
struct JsonUncheckedNewCallback(JsonCallbackFields);

#[derive(Deserialize)]
struct JsonCallbackFields {
    url: String,
    #[serde(default)]
    headers: HeaderEntries,
    body: Option<Value>,
}

impl TryFrom<JsonUncheckedNewCallback> for JsonNewCallback {
    type Error = CallbackError;

    fn try_from(unchecked: JsonUncheckedNewCallback) -> Result<Self, Self::Error> {
        let JsonUncheckedNewCallback(JsonCallbackFields {
            url,
            headers: HeaderEntries(headers),
            body,
        }) = unchecked;
        Self::new(&url, headers, body)
    }
}

/// Refuses a number of the wrong type with a fixed message instead of serde's, which quotes it.
macro_rules! refuse_numbers {
    ($message:expr) => {
        fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Err(E::custom($message))
        }

        fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Err(E::custom($message))
        }

        fn visit_i128<E>(self, _: i128) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Err(E::custom($message))
        }

        fn visit_u128<E>(self, _: u128) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Err(E::custom($message))
        }

        fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Err(E::custom($message))
        }
    };
}

impl<'de> Deserialize<'de> for JsonUncheckedNewCallback {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonUncheckedNewCallbackVisitor)
    }
}

struct JsonUncheckedNewCallbackVisitor;

impl<'de> Visitor<'de> for JsonUncheckedNewCallbackVisitor {
    type Value = JsonUncheckedNewCallback;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a callback object")
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        JsonCallbackFields::deserialize(MapAccessDeserializer::new(map))
            .map(JsonUncheckedNewCallback)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom(NOT_AN_OBJECT))
    }

    refuse_numbers!(NOT_AN_OBJECT);
}

/// Header entries in document order, which a map type would lose.
#[derive(Default)]
struct HeaderEntries(Vec<(String, String)>);

impl<'de> Deserialize<'de> for HeaderEntries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(HeaderEntriesVisitor)
    }
}

struct HeaderEntriesVisitor;

impl<'de> Visitor<'de> for HeaderEntriesVisitor {
    type Value = HeaderEntries;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a map of header names to values")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::new();
        while let Some((name, HeaderValueString(value))) = map.next_entry()? {
            entries.push((name, value));
        }
        Ok(HeaderEntries(entries))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HeaderEntries::default())
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom(HEADERS_NOT_A_MAP))
    }

    refuse_numbers!(HEADERS_NOT_A_MAP);
}

struct HeaderValueString(String);

impl<'de> Deserialize<'de> for HeaderValueString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(HeaderValueStringVisitor)
    }
}

struct HeaderValueStringVisitor;

impl Visitor<'_> for HeaderValueStringVisitor {
    type Value = HeaderValueString;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a header value string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HeaderValueString(v.to_owned()))
    }

    refuse_numbers!(HEADER_VALUE_NOT_A_STRING);
}

fn parse_url(url: &str) -> Result<(Url, Url), CallbackError> {
    // Parsing refuses an `https` URL without a host.
    let url = Url::parse(url).map_err(CallbackError::Url)?;
    if url.scheme() != "https" {
        return Err(CallbackError::NotHttps);
    }
    // The limit applies to the URL as sent, so a serialized request validates again.
    let len = url.as_str().len();
    if len > MAX_CALLBACK_URL_LEN {
        return Err(CallbackError::UrlTooLong(len));
    }
    let origin = Url::parse(&url.origin().ascii_serialization()).map_err(CallbackError::Url)?;
    Ok((url, origin))
}

fn collect_headers<H>(headers: H) -> Result<BTreeMap<String, Option<Secret>>, CallbackError>
where
    H: IntoIterator<Item = (String, String)>,
{
    let mut collected = BTreeMap::new();
    for (position, (name, value)) in (1..).zip(headers) {
        let name = header_name(position, &name)?;
        let value = header_value(&name, value)?;
        collected.insert(name, value);
        if collected.len() > MAX_CALLBACK_HEADERS {
            return Err(CallbackError::TooManyHeaders);
        }
    }
    Ok(collected)
}

fn header_name(position: usize, name: &str) -> Result<String, CallbackError> {
    if name.len() > MAX_CALLBACK_HEADER_NAME_LEN {
        return Err(CallbackError::HeaderNameTooLong {
            position,
            len: name.len(),
        });
    }
    let name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|error| CallbackError::HeaderName { position, error })?
        .as_str()
        .to_owned();
    if REFUSED_HEADERS.contains(&name.as_str()) {
        return Err(CallbackError::RefusedHeader(name));
    }
    Ok(name)
}

fn header_value(name: &str, value: String) -> Result<Option<Secret>, CallbackError> {
    if value.len() > MAX_CALLBACK_HEADER_VALUE_LEN {
        return Err(CallbackError::HeaderValueTooLong {
            name: name.to_owned(),
            len: value.len(),
        });
    }
    HeaderValue::from_str(&value).map_err(|error| CallbackError::HeaderValue {
        name: name.to_owned(),
        error,
    })?;
    // Only an empty value fails to become a `Secret`.
    Ok(Secret::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use serde_json::{Value, json};

    use super::{
        CallbackError, JsonNewCallback, MAX_CALLBACK_HEADER_NAME_LEN,
        MAX_CALLBACK_HEADER_VALUE_LEN, MAX_CALLBACK_HEADERS, MAX_CALLBACK_URL_LEN,
    };
    use crate::Sanitize as _;

    const URL: &str = "https://example.com/hook";
    const REFUSED_HEADERS: [&str; 9] = [
        "Host",
        "Content-Length",
        "Transfer-Encoding",
        "Connection",
        "Keep-Alive",
        "Proxy-Connection",
        "TE",
        "Trailer",
        "Upgrade",
    ];

    fn new_callback(
        url: &str,
        headers: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<JsonNewCallback, CallbackError> {
        JsonNewCallback::new(
            url,
            headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
            body,
        )
    }

    #[test]
    fn round_trips_through_json() {
        let json = r#"{"url":"https://example.com/hook?event=run","headers":{"Authorization":"Bearer token","X-Custom":"value"},"body":{"job":"{{ job.uuid }}","n":[1,true,null]}}"#;
        let callback: JsonNewCallback = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string(&callback).unwrap();
        let round_tripped: JsonNewCallback = serde_json::from_str(&serialized).unwrap();
        assert_eq!(callback, round_tripped);
        assert_eq!(
            callback.url().as_str(),
            "https://example.com/hook?event=run"
        );
        assert_eq!(
            serde_json::to_value(&callback).unwrap()["body"],
            json!({ "job": "{{ job.uuid }}", "n": [1, true, null] })
        );
    }

    /// A `null` body is no body, however the request is built.
    #[test]
    fn a_null_body_is_no_body() {
        let without = new_callback(URL, &[], None).unwrap();
        let null = new_callback(URL, &[], Some(Value::Null)).unwrap();
        assert_eq!(null, without);
        assert_eq!(serde_json::to_value(&null).unwrap(), json!({ "url": URL }));
    }

    #[test]
    fn refuses_a_url_that_is_not_https() {
        for url in [
            "http://example.com/hook",
            "ftp://example.com/hook",
            "wss://example.com",
        ] {
            let err = new_callback(url, &[], None).unwrap_err();
            assert!(matches!(err, CallbackError::NotHttps), "{url}: {err}");
        }
    }

    #[test]
    fn refuses_an_overlong_url() {
        let base = "https://example.com/";
        let url = format!("{base}{}", "a".repeat(MAX_CALLBACK_URL_LEN - base.len()));
        assert_eq!(new_callback(&url, &[], None).unwrap().url().as_str(), url);

        let url = format!("{url}a");
        let err = new_callback(&url, &[], None).unwrap_err();
        assert!(
            matches!(err, CallbackError::UrlTooLong(len) if len == MAX_CALLBACK_URL_LEN + 1),
            "{err}"
        );
    }

    /// The limit applies to the URL as sent, so the value a client serializes validates again.
    #[test]
    fn refuses_a_url_that_grows_over_the_limit_when_encoded() {
        let base = "https://example.com/";
        let url = format!(
            "{base}{}é",
            "a".repeat(MAX_CALLBACK_URL_LEN - base.len() - 2)
        );
        assert_eq!(url.len(), MAX_CALLBACK_URL_LEN);
        let err = new_callback(&url, &[], None).unwrap_err();
        assert!(
            matches!(err, CallbackError::UrlTooLong(len) if len == MAX_CALLBACK_URL_LEN + 4),
            "{err}"
        );
    }

    #[test]
    fn refuses_framing_and_hop_by_hop_headers_in_any_case() {
        for name in REFUSED_HEADERS {
            for name in [name.to_owned(), name.to_lowercase(), name.to_uppercase()] {
                let err = new_callback(URL, &[(&name, "value")], None).unwrap_err();
                assert!(
                    matches!(&err, CallbackError::RefusedHeader(refused) if *refused == name.to_lowercase()),
                    "{name}: {err}"
                );
            }
        }
    }

    #[test]
    fn refuses_a_control_character_in_a_header_value() {
        for value in ["a\rb", "a\nb", "a\0b", "token\r\nX-Injected: 1", "a\u{7f}b"] {
            let err = new_callback(URL, &[("X-Token", value)], None).unwrap_err();
            assert!(
                matches!(&err, CallbackError::HeaderValue { name, .. } if name == "x-token"),
                "{value:?}: {err}"
            );
        }
        new_callback(URL, &[("X-Token", "tab\tand obs-text é")], None).unwrap();
    }

    #[test]
    fn refuses_an_invalid_header_name() {
        for name in ["", "X Token", "X-Token:", "X-Token\r\n", "(X-Token)"] {
            let err =
                new_callback(URL, &[("X-First", "value"), (name, "value")], None).unwrap_err();
            assert!(
                matches!(err, CallbackError::HeaderName { position: 2, .. }),
                "{name:?}: {err}"
            );
        }
    }

    #[test]
    fn refuses_more_than_the_maximum_headers() {
        let headers: Vec<(String, String)> = (0..MAX_CALLBACK_HEADERS)
            .map(|i| (format!("x-header-{i}"), "value".to_owned()))
            .collect();
        JsonNewCallback::new(URL, headers.clone(), None).unwrap();

        let mut too_many = headers.clone();
        too_many.push(("x-one-too-many".to_owned(), "value".to_owned()));
        let err = JsonNewCallback::new(URL, too_many, None).unwrap_err();
        assert!(matches!(err, CallbackError::TooManyHeaders), "{err}");

        // The limit counts headers as sent, after names fold together.
        let mut folded = headers;
        folded.push(("X-Header-0".to_owned(), "again".to_owned()));
        let callback = JsonNewCallback::new(URL, folded, None).unwrap();
        assert_eq!(callback.headers.len(), MAX_CALLBACK_HEADERS);
    }

    #[test]
    fn refuses_an_overlong_header_name() {
        let name = "x".repeat(MAX_CALLBACK_HEADER_NAME_LEN);
        new_callback(URL, &[(&name, "value")], None).unwrap();

        let name = format!("{name}x");
        let err = new_callback(URL, &[(&name, "value")], None).unwrap_err();
        assert!(
            matches!(err, CallbackError::HeaderNameTooLong { position: 1, len } if len == MAX_CALLBACK_HEADER_NAME_LEN + 1),
            "{err}"
        );
    }

    #[test]
    fn refuses_an_overlong_header_value() {
        let value = "v".repeat(MAX_CALLBACK_HEADER_VALUE_LEN);
        new_callback(URL, &[("X-Token", &value)], None).unwrap();

        let value = format!("{value}v");
        let err = new_callback(URL, &[("X-Token", &value)], None).unwrap_err();
        assert!(
            matches!(&err, CallbackError::HeaderValueTooLong { name, len } if name == "x-token" && *len == MAX_CALLBACK_HEADER_VALUE_LEN + 1),
            "{err}"
        );
    }

    #[test]
    fn an_empty_header_value_is_sent_empty() {
        let callback = new_callback(URL, &[("X-Empty", ""), ("X-Blank", " ")], None).unwrap();
        let delivery = callback.delivery_headers("1.2.3").unwrap();
        assert_eq!(delivery["x-empty"], "");
        assert_eq!(delivery["x-blank"], " ");

        let serialized = serde_json::to_value(&callback).unwrap();
        assert_eq!(
            serialized,
            json!({ "url": URL, "headers": { "x-blank": " ", "x-empty": "" } })
        );
        let round_tripped: JsonNewCallback = serde_json::from_value(serialized).unwrap();
        assert_eq!(round_tripped, callback);

        let mut sanitized = callback;
        sanitized.sanitize();
        assert_eq!(
            serde_json::to_value(&sanitized).unwrap()["headers"],
            json!({ "x-blank": "************", "x-empty": "" })
        );
    }

    #[test]
    fn duplicate_headers_resolve_to_the_last_in_document_order() {
        for json in [
            r#"{"url":"https://example.com/hook","headers":{"x-token":"first","X-Token":"second"}}"#,
            r#"{"url":"https://example.com/hook","headers":{"X-Token":"first","x-token":"second"}}"#,
        ] {
            let callback: JsonNewCallback = serde_json::from_str(json).unwrap();
            assert_eq!(
                callback.headers,
                BTreeMap::from([("x-token".to_owned(), Some("second".parse().unwrap()))]),
                "{json}"
            );
        }
    }

    #[test]
    fn deserialization_keeps_its_shape() {
        let callback: JsonNewCallback =
            serde_json::from_str(r#"{"url":"https://example.com/hook","headers":null}"#).unwrap();
        assert!(callback.headers.is_empty());

        let callback: JsonNewCallback =
            serde_json::from_str(r#"{"url":"https://example.com/hook","method":"PUT"}"#).unwrap();
        assert_eq!(callback.url().as_str(), URL);

        let err = serde_json::from_str::<JsonNewCallback>(r#"{"headers":{}}"#).unwrap_err();
        assert!(err.to_string().contains("missing field `url`"), "{err}");
    }

    #[test]
    fn delivery_headers_let_a_customer_header_replace_a_default() {
        let callback = new_callback(
            URL,
            &[
                ("Content-Type", "application/vnd.github+json"),
                ("Authorization", "Bearer token"),
            ],
            None,
        )
        .unwrap();
        assert_eq!(
            callback.delivery_headers("1.2.3").unwrap(),
            HeaderMap::from_iter([
                (AUTHORIZATION, HeaderValue::from_static("Bearer token")),
                (
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/vnd.github+json")
                ),
                (USER_AGENT, HeaderValue::from_static("bencher/1.2.3")),
            ])
        );

        let user_agent = new_callback(URL, &[("User-Agent", "my-agent")], None).unwrap();
        assert_eq!(
            user_agent.delivery_headers("1.2.3").unwrap()[USER_AGENT],
            "my-agent"
        );
    }

    #[test]
    fn every_header_value_is_sensitive() {
        const MARKER: &str = "MARKER-a41c";
        let callback = new_callback(
            URL,
            &[
                ("Authorization", &format!("Bearer {MARKER}")),
                ("X-Key", MARKER),
            ],
            None,
        )
        .unwrap();
        let headers = callback.delivery_headers(MARKER).unwrap();

        let debug = format!("{headers:?}");
        assert!(!debug.contains(MARKER), "{debug}");
        assert!(debug.contains("authorization"), "{debug}");
        assert!(headers.values().all(HeaderValue::is_sensitive));
    }

    #[test]
    fn sanitize_hides_every_secret() {
        let body = json!({ "job": "{{ job.uuid }}" });
        let callback = new_callback(
            "https://user-marker:password-marker@example.com:8443/path-marker?query-marker=1#fragment-marker",
            &[("Authorization", "value-marker"), ("X-Key", "key-marker")],
            Some(body.clone()),
        )
        .unwrap();
        let markers = [
            "user-marker",
            "password-marker",
            "path-marker",
            "query-marker",
            "fragment-marker",
            "value-marker",
            "key-marker",
        ];

        // The real request is what the CLI sends and the server seals.
        let serialized = serde_json::to_string(&callback).unwrap();
        for marker in markers {
            assert!(
                serialized.contains(marker),
                "{marker} missing from {serialized}"
            );
        }

        let mut sanitized = callback.clone();
        sanitized.sanitize();
        let sanitized_json = serde_json::to_value(&sanitized).unwrap();
        let printed = sanitized_json.to_string();
        for marker in markers {
            assert!(!printed.contains(marker), "{marker} in {printed}");
        }
        assert_eq!(
            sanitized_json,
            json!({
                "url": "https://example.com:8443/",
                "headers": {
                    "authorization": "************",
                    "x-key": "************",
                },
                "body": body,
            })
        );

        let reparsed: JsonNewCallback = serde_json::from_value(sanitized_json).unwrap();
        assert_eq!(reparsed, sanitized);
    }

    #[test]
    fn errors_never_echo_a_secret() {
        let overlong = "value-marker".repeat(MAX_CALLBACK_HEADER_VALUE_LEN);
        for (url, headers) in [
            (
                "http://user-marker:password-marker@example.com/path-marker?query-marker=1",
                vec![],
            ),
            (URL, vec![("X-Token", "value-marker\r\n")]),
            (URL, vec![("X-Token", overlong.as_str())]),
            (URL, vec![("name marker", "value")]),
        ] {
            let err = new_callback(url, &headers, None).unwrap_err().to_string();
            for marker in [
                "user-marker",
                "password-marker",
                "path-marker",
                "query-marker",
                "value-marker",
                "name marker",
            ] {
                assert!(!err.contains(marker), "{marker} in {err}");
            }
        }
    }

    /// serde's own type errors quote the offending value, and the API server logs them.
    #[test]
    fn type_errors_never_echo_a_secret() {
        #[derive(Deserialize)]
        struct JsonNewRunJob {
            #[expect(dead_code, reason = "only deserialization is under test")]
            callback: Option<JsonNewCallback>,
        }

        let markers = [
            "header-marker",
            "user-marker",
            "password-marker",
            "path-marker",
            "query-marker",
            "123456789",
            "987654321",
            "1234.5678",
        ];
        for callback in [
            r#"{"url":"https://example.com/hook","headers":"Authorization: Bearer header-marker"}"#,
            r#""https://user-marker:password-marker@example.com/path-marker?query-marker=1""#,
            r#"{"url":"https://example.com/hook","headers":{"X-Key":123456789}}"#,
            r#"{"url":"https://example.com/hook","headers":987654321}"#,
            "123456789",
            r#"{"url":"https://example.com/hook","headers":{"X-Key":-123456789}}"#,
            r#"{"url":"https://example.com/hook","headers":{"X-Key":1234.5678}}"#,
            r#"{"url":"https://example.com/hook","headers":-123456789}"#,
            "1234.5678",
        ] {
            let direct = serde_json::from_str::<JsonNewCallback>(callback)
                .unwrap_err()
                .to_string();
            let nested =
                serde_json::from_str::<JsonNewRunJob>(&format!(r#"{{"callback":{callback}}}"#))
                    .map(drop)
                    .unwrap_err()
                    .to_string();
            for err in [direct, nested] {
                for marker in markers {
                    assert!(!err.contains(marker), "{marker} in {err}");
                }
            }
        }
    }
}
