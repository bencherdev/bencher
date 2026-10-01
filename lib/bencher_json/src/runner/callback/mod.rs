use std::collections::BTreeMap;
use std::fmt;

use http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use url::Url;

use crate::runner::JobStatus;
use crate::{JsonReport, Sanitize, Secret, strip_bearer_token};

mod body;
mod state;

pub use body::CallbackContext;
use body::{sends_report, validate_body};
pub use state::{JobCallbackState, JsonJobCallback};

pub const MAX_CALLBACK_URL_LEN: usize = 2048;
pub const MAX_CALLBACK_HEADERS: usize = 16;
pub const MAX_CALLBACK_HEADER_NAME_LEN: usize = 256;
pub const MAX_CALLBACK_HEADER_VALUE_LEN: usize = 8 << 10;
pub const MAX_CALLBACK_BODY_LEN: usize = 64 << 10;

/// The job statuses a callback fires on, and so the values `{{ job.status }}` can take.
pub const CALLBACK_JOB_STATUSES: [JobStatus; 3] =
    [JobStatus::Processed, JobStatus::Failed, JobStatus::Canceled];

// The `repository_dispatch` event type that released `bencher run` versions compose.
const DISPATCH_EVENT_TYPE: &str = "bencher_run";

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
#[serde(try_from = "Value")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewCallback {
    /// The `https` URL to send the request to
    url: Url,
    /// Request headers. Names are case-insensitive: send each name once; if one repeats in
    /// different case, only one is kept.
    /// A header named `Content-Type` or `User-Agent` replaces Bencher's default.
    // `default` only feeds the generated types: deserializing goes through `TryFrom<Value>`.
    // A `Secret` is never empty, so an empty value, which HTTP allows, is `None`.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "serialize_headers"
    )]
    #[cfg_attr(feature = "schema", schemars(with = "BTreeMap<String, Secret>"))]
    #[typeshare(typescript(type = "Record<string, string>"))]
    headers: BTreeMap<String, Option<Secret>>,
    /// The JSON request body. Anywhere inside a string value, `{{ job.uuid }}`, `{{ job.status }}`,
    /// `{{ report.uuid }}`, `{{ project.uuid }}`, `{{ project.name }}`, and `{{ project.slug }}` become
    /// their values, and a string that is exactly `{{ report }}` becomes the job's report, which a
    /// body can send only once. Whitespace inside the braces is optional, and any other name inside
    /// them is refused. Keys and every other value are sent as they are.
    /// Without a body, the body is the job's report, as the report endpoint returns it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "unknown"))]
    body: Option<Value>,
    #[serde(skip)]
    origin: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    #[error("callback must be an object")]
    NotAnObject,
    #[error("callback URL is missing")]
    MissingUrl,
    #[error("callback URL must be a string")]
    UrlNotAString,
    #[error("callback headers must be a map of names to values")]
    HeadersNotAMap,
    #[error("callback header value must be a string")]
    HeaderValueNotAString,
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
    #[error(
        "callback body placeholder {0:?} names an unknown value; the names are job.uuid, job.status, report.uuid, project.uuid, project.name, project.slug, and report"
    )]
    UnknownPlaceholder(String),
    #[error(
        "callback body sends the report inside a longer string; the report must be a whole string"
    )]
    ReportInText,
    #[error("callback body sends the report a second time; a body can send it only once")]
    RepeatedReport,
    #[error("callback body length {0} exceeds maximum {MAX_CALLBACK_BODY_LEN}")]
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

    /// Whether the callback is the GitHub Actions dispatch that `bencher run` composes: the
    /// `bencher_run` event, sent with a bearer token to a repository on github.com.
    pub fn is_bencher_run_dispatch(&self) -> bool {
        let has_bearer_token = self
            .headers
            .get(AUTHORIZATION.as_str())
            .and_then(Option::as_ref)
            .and_then(|value| strip_bearer_token(value.as_ref()))
            .is_some();
        let event_type = self
            .body
            .as_ref()
            .and_then(|body| body.get("event_type"))
            .and_then(Value::as_str);
        is_github_dispatch_url(&self.url)
            && has_bearer_token
            && event_type == Some(DISPATCH_EVENT_TYPE)
    }

    /// Whether the body sends the job's report, as a callback without a body does.
    pub fn sends_report(&self) -> bool {
        self.body.as_ref().is_none_or(sends_report)
    }

    /// Render the body, or without one the job's report, which it needs only if it sends the report.
    pub fn render(
        &self,
        context: &CallbackContext,
        report: Option<&JsonReport>,
    ) -> serde_json::Result<String> {
        body::render(self.body.as_ref(), context, report)
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

impl TryFrom<Value> for JsonNewCallback {
    type Error = CallbackError;

    // Every field is read by hand: a typed deserializer's error quotes a value of the wrong type,
    // and the API server logs deserialization errors.
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Value::Object(mut fields) = value else {
            return Err(CallbackError::NotAnObject);
        };
        let url = match fields.remove("url") {
            Some(Value::String(url)) => url,
            Some(_) => return Err(CallbackError::UrlNotAString),
            None => return Err(CallbackError::MissingUrl),
        };
        let headers = match fields.remove("headers") {
            Some(Value::Object(headers)) => headers,
            Some(Value::Null) | None => serde_json::Map::new(),
            Some(_) => return Err(CallbackError::HeadersNotAMap),
        };
        let headers = headers
            .into_iter()
            .map(|(name, value)| match value {
                Value::String(value) => Ok((name, value)),
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::Array(_)
                | Value::Object(_) => Err(CallbackError::HeaderValueNotAString),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(&url, headers, fields.remove("body"))
    }
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

/// `https://api.github.com/repos/{owner}/{repo}/dispatches`, and nothing else.
fn is_github_dispatch_url(url: &Url) -> bool {
    let Some(mut segments) = url.path_segments() else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("api.github.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && matches!(
            (
                segments.next(),
                segments.next(),
                segments.next(),
                segments.next(),
                segments.next(),
            ),
            (Some("repos"), Some(owner), Some(repo), Some("dispatches"), None)
                if !owner.is_empty() && !repo.is_empty()
        )
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

    use url::Url;

    use super::{
        CallbackError, JsonNewCallback, MAX_CALLBACK_HEADER_NAME_LEN,
        MAX_CALLBACK_HEADER_VALUE_LEN, MAX_CALLBACK_HEADERS, MAX_CALLBACK_URL_LEN,
        is_github_dispatch_url,
    };
    use crate::Sanitize as _;

    const URL: &str = "https://example.com/hook";
    const DISPATCH: &str = "https://api.github.com/repos/owner/repo/dispatches";
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
    fn a_github_dispatch_is_exactly_the_dispatch_endpoint_on_api_github_com() {
        for (url, expected) in [
            (DISPATCH, true),
            ("http://api.github.com/repos/owner/repo/dispatches", false),
            // A GitHub Enterprise Server host
            (
                "https://github.example.com/repos/owner/repo/dispatches",
                false,
            ),
            (
                "https://api.github.com:8443/repos/owner/repo/dispatches",
                false,
            ),
            (
                "https://user@api.github.com/repos/owner/repo/dispatches",
                false,
            ),
            (
                "https://:password@api.github.com/repos/owner/repo/dispatches",
                false,
            ),
            (
                "https://api.github.com/repos/owner/repo/dispatches?ref=main",
                false,
            ),
            (
                "https://api.github.com/repos/owner/repo/dispatches#top",
                false,
            ),
            ("https://api.github.com/users/owner/repo/dispatches", false),
            ("https://api.github.com/repos/owner/repo/issues", false),
            ("https://api.github.com/repos//repo/dispatches", false),
            ("https://api.github.com/repos/owner//dispatches", false),
            ("https://api.github.com/repos/owner/dispatches", false),
            (
                "https://api.github.com/repos/owner/repo/dispatches/extra",
                false,
            ),
        ] {
            assert_eq!(
                is_github_dispatch_url(&Url::parse(url).unwrap()),
                expected,
                "{url}"
            );
        }
    }

    /// Each false row differs from what `bencher run` v0.6.13 composes in one respect.
    #[test]
    fn a_bencher_run_dispatch_is_exactly_what_the_cli_composes() {
        const BEARER: Option<(&str, &str)> = Some(("Authorization", "Bearer github_pat_token"));
        let authorization = |value| Some(("Authorization", value));
        let with_event_type = |event_type: Option<Value>| {
            let mut body = json!({
                "client_payload": {
                    "bencher": { "project": "{{ project.slug }}", "job": "{{ job.uuid }}" },
                    "github": { "sha": "f1e2d3c4b5a697887766554433221100ffeeddcc" },
                },
            });
            if let Some(event_type) = event_type {
                body["event_type"] = event_type;
            }
            body
        };
        let composed = || Some(with_event_type(Some(json!("bencher_run"))));
        for (url, bearer, body, expected) in [
            (DISPATCH, BEARER, composed(), true),
            // HTTP auth schemes are case-insensitive.
            (
                DISPATCH,
                authorization("bearer github_pat_token"),
                composed(),
                true,
            ),
            // A GitHub Enterprise Server host
            (
                "https://github.example.com/api/v3/repos/owner/repo/dispatches",
                BEARER,
                composed(),
                false,
            ),
            (DISPATCH, None, composed(), false),
            // GitHub reads the token only from `Authorization`.
            (
                DISPATCH,
                Some(("Proxy-Authorization", "Bearer github_pat_token")),
                composed(),
                false,
            ),
            (
                DISPATCH,
                authorization("Basic dXNlcjpwYXNz"),
                composed(),
                false,
            ),
            (DISPATCH, authorization("Bearer "), composed(), false),
            (DISPATCH, authorization("Bearer \t "), composed(), false),
            (DISPATCH, BEARER, None, false),
            (
                DISPATCH,
                BEARER,
                Some(json!([with_event_type(Some(json!("bencher_run")))])),
                false,
            ),
            (DISPATCH, BEARER, Some(with_event_type(None)), false),
            (
                DISPATCH,
                BEARER,
                Some(with_event_type(Some(json!("deploy")))),
                false,
            ),
            (
                DISPATCH,
                BEARER,
                Some(with_event_type(Some(json!("BENCHER_RUN")))),
                false,
            ),
            (
                DISPATCH,
                BEARER,
                Some(with_event_type(Some(json!("bencher_run{{ job.status }}")))),
                false,
            ),
        ] {
            let headers = [
                Some(("Accept", "application/vnd.github+json")),
                bearer,
                Some(("X-GitHub-Api-Version", "2022-11-28")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            let callback = new_callback(url, &headers, body.clone()).unwrap();
            assert_eq!(
                callback.is_bencher_run_dispatch(),
                expected,
                "{url} {bearer:?} {body:?}"
            );
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
    fn a_header_name_repeated_in_another_case_keeps_one() {
        let kept = |json: &str| {
            let callback: JsonNewCallback = serde_json::from_str(json).unwrap();
            assert_eq!(
                callback.headers.keys().collect::<Vec<_>>(),
                ["x-token"],
                "{json}"
            );
            callback
        };
        assert_eq!(
            kept(
                r#"{"url":"https://example.com/hook","headers":{"x-token":"first","X-Token":"second"}}"#
            ),
            kept(
                r#"{"url":"https://example.com/hook","headers":{"X-Token":"second","x-token":"first"}}"#
            ),
            "the kept value does not depend on document order"
        );
        let exact: JsonNewCallback = serde_json::from_str(
            r#"{"url":"https://example.com/hook","headers":{"X-Token":"first","X-Token":"second"}}"#,
        )
        .unwrap();
        assert_eq!(
            exact.headers,
            BTreeMap::from([("x-token".to_owned(), Some("second".parse().unwrap()))]),
            "an exact duplicate resolves to the last"
        );
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
        assert!(err.to_string().contains("callback URL is missing"), "{err}");
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
            "url-marker",
            "array-marker",
            "object-marker",
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
            r#"{"url":123456789}"#,
            r#"{"url":["url-marker"]}"#,
            r#"{"url":{"url-marker":"url-marker"}}"#,
            r#"{"url":"https://example.com/hook","headers":{"X-Key":["array-marker"]}}"#,
            r#"{"url":"https://example.com/hook","headers":{"X-Key":{"object-marker":"object-marker"}}}"#,
            r#"{"url":"https://example.com/hook","headers":["header-marker"]}"#,
            r#"["url-marker"]"#,
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
