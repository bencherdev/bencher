use std::{collections::HashSet, sync::Arc};

use dropshot::{HttpError, HttpResponseHeaders, HttpResponseOk, RequestContext, ServerContext};
use http::{header::HeaderValue, StatusCode};

pub type CorsResponse = HttpResponseHeaders<HttpResponseOk<String>>;

// https://github.com/oxidecomputer/cio/blob/95545d29f25712a917b85593492217f4e989b04c/webhooky/src/cors.rs

#[derive(Debug, PartialEq, Eq)]
pub enum CorsFailure {
    InvalidValue(String),
    Missing,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CorsError {
    pub failures: Vec<CorsFailure>,
}

impl From<CorsError> for HttpError {
    fn from(_: CorsError) -> Self {
        // Currently all CORS errors collapse to a Forbidden response, they do not report on what the expected values are
        HttpError::for_status(None, StatusCode::FORBIDDEN)
    }
}

pub fn get_cors<Context>() -> HttpResponseHeaders<HttpResponseOk<String>>
where
    Context: ServerContext,
{
    let mut resp = HttpResponseHeaders::new_unnamed(HttpResponseOk(String::new()));
    let headers = resp.headers_mut();

    headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    headers.insert(
        "Access-Control-Allow-Methods",
        HeaderValue::from_static("*"),
    );
    headers.insert(
        "Access-Control-Allow-Headers",
        HeaderValue::from_static("*"),
    );

    resp
}

/// Given a request context and a list of valid  origins, check that the
/// incoming request has specified one of those origins. If a valid origin has
/// been specified then a [`http::header::HeaderValue`] that should be added as
/// the `Access-Control-Allow-Origin` header is returned. If the origin header
/// is missing, malformed, or not valid, a CORS error report is returned.
pub async fn get_cors_origin_header<C: ServerContext>(
    rqctx: Arc<RequestContext<C>>,
    allowed_origins: &[&'static str],
) -> Result<HeaderValue, CorsError> {
    get_cors_header(rqctx, "Origin", allowed_origins).await
}

/// Given a request context and a list of valid headers, checks that the
/// incoming request has specified a list of headers to be checked and that all
/// headers including in that list are allowed. If all of the requested headers
/// are allowed, then a [`http::header::HeaderValue`] that should be used as the
/// `Access-Control-Allow-Headers` header is returned. If the request headers is
/// missing, malformed, or not valid a CORS error report is returned.
#[allow(dead_code)]
pub async fn get_cors_headers_header<C: ServerContext>(
    rqctx: Arc<RequestContext<C>>,
    allowed_headers: &[&'static str],
) -> Result<HeaderValue, CorsError> {
    get_cors_header(rqctx, "Access-Control-Request-Headers", allowed_headers).await
}

/// Constructs a header value to use in conjunction with a
/// Access-Control-Allow-Methods header
#[allow(dead_code, clippy::expect_used)]
pub fn get_cors_method_header(allowed_methods: &[http::Method]) -> HeaderValue {
    // This should never fail has we know that [`http::Method`] converts to valid
    // str values and joining those values with , remains valid
    HeaderValue::from_str(
        &allowed_methods
            .iter()
            .map(dropshot::Method::as_str)
            .collect::<Vec<&str>>()
            .join(", "),
    )
    .expect("Converting method to str generated invalid string")
}

#[allow(clippy::unused_async)]
pub async fn get_cors_header<C: ServerContext>(
    rqctx: Arc<RequestContext<C>>,
    header_name: &str,
    allowed: &[&'static str],
) -> Result<HeaderValue, CorsError> {
    let request = &rqctx.request;
    let incoming_headers = request.headers();

    let req_value = incoming_headers
        .get(header_name)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| CorsError {
            failures: vec![CorsFailure::Missing],
        })?;

    validate_header(req_value, allowed)
}

#[allow(clippy::expect_used)]
pub fn validate_header(
    req_header: &str,
    allowed: &[&'static str],
) -> Result<HeaderValue, CorsError> {
    // Split the header value on ", " to handle headers that pass in multiple values
    // in a single header like Access-Control-Request-Headers
    let req_values: HashSet<&str> = req_header.split(", ").collect();
    let allowed_values: HashSet<&str> = allowed.iter().copied().collect();

    // The remaining headers are those that the client requested, but are not
    // allowed
    let diff: HashSet<&str> = req_values.difference(&allowed_values).copied().collect();

    // If the diff is empty, then all of the headers requested by the client are
    // allowed by the provided configuration list. Therefore we can echo back
    // the exact list of headers the client requested.
    if diff.is_empty() {
        // This should never panic as we are reusing the str value that was taken from a
        // HeaderValue on the request
        Ok(HeaderValue::from_str(req_header).expect("Rejoining passed in header values failed"))
    } else {
        Err(CorsError {
            failures: diff
                .into_iter()
                .map(|v| CorsFailure::InvalidValue(v.to_string()))
                .collect::<Vec<CorsFailure>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use http::{header::HeaderValue, Method};

    use super::{get_cors_method_header, validate_header, CorsError, CorsFailure};

    #[test]
    fn test_cors_simple_origin() {
        assert_eq!(
            Ok(HeaderValue::from_static("https://website.com")),
            validate_header("https://website.com", &["https://website.com"])
        );
    }

    #[test]
    fn test_cors_simple_methods_header() {
        assert_eq!(
            HeaderValue::from_static("POST, OPTIONS, GET"),
            get_cors_method_header(&[Method::POST, Method::OPTIONS, Method::GET])
        );
    }

    #[test]
    fn test_cors_simple_content_type() {
        assert_eq!(
            Ok(HeaderValue::from_static("Content-Type")),
            validate_header("Content-Type", &["Content-Type"])
        );
    }

    #[test]
    fn test_cors_returns_values_when_all_are_valid() {
        assert_eq!(
            Ok(HeaderValue::from_static("Content-Type, X-Nonstandard")),
            validate_header(
                "Content-Type, X-Nonstandard",
                &["Content-Type", "X-Nonstandard"]
            )
        );
    }

    #[test]
    fn test_cors_handles_duplicate_valid_values() {
        assert_eq!(
            Ok(HeaderValue::from_static("Content-Type, Content-Type")),
            validate_header("Content-Type, Content-Type", &["Content-Type"])
        );
    }

    #[test]
    fn test_cors_returns_subset_of_allowed_values() {
        assert_eq!(
            Ok(HeaderValue::from_static("Content-Type")),
            validate_header("Content-Type", &["Content-Type", "X-Nonstandard"])
        );
    }

    #[test]
    fn test_cors_returns_invalid_error_when_missing() {
        assert_eq!(
            Err(CorsError {
                failures: vec![CorsFailure::InvalidValue("X-Unsupported-Thing".to_string())],
            }),
            validate_header("X-Unsupported-Thing", &["Content-Type"])
        );
    }
}
