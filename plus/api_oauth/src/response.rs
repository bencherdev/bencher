use bencher_json::{OAuthIssuer, OAuthRedirectUri, OAuthRequestState};
use dropshot::{Body, HttpError};
use http::{
    HeaderValue, Response, StatusCode,
    header::{
        ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_TYPE, LOCATION, RETRY_AFTER,
        WWW_AUTHENTICATE,
    },
};
use serde::Serialize;
use url::Url;

/// The console page where a user answers an app's request to connect.
const CONSENT_PATH: &str = "/auth/authorize";
const APPLICATION_JSON: &str = "application/json";
/// Discovery documents change only with the server's configuration.
const METADATA_CACHE_CONTROL: &str = "public, max-age=300";
/// The shortest window the rate limits count over.
const RETRY_AFTER_SECONDS: &str = "60";

/// Send the browser to `location`. The response is never cached, since it can carry
/// a client's `state`.
pub(crate) fn found(location: &Url) -> Result<Response<Body>, HttpError> {
    let mut response = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, location.as_str())
        .body(Body::empty())?;
    insert_headers(&mut response);
    Ok(response)
}

/// Answer with JSON that is never cached, since it can carry an authorization code or a token.
pub(crate) fn json<T>(body: &T) -> Result<Response<Body>, HttpError>
where
    T: Serialize,
{
    json_with_status(StatusCode::OK, body)
}

/// Refuse a request with an OAuth error (RFC 6749, section 5.2), which is never cached.
/// A refused client is told the one authentication scheme the protocol defines for clients.
pub(crate) fn oauth_error(
    status: StatusCode,
    error: &str,
    description: &str,
    issuer: &OAuthIssuer,
) -> Result<Response<Body>, HttpError> {
    let mut response = json_with_status(
        status,
        &OAuthErrorBody {
            error,
            error_description: description,
        },
    )?;
    if status == StatusCode::UNAUTHORIZED {
        let challenge = HeaderValue::from_str(&format!(r#"Basic realm="{issuer}""#))
            .map_err(|error| HttpError::for_internal_error(error.to_string()))?;
        response.headers_mut().insert(WWW_AUTHENTICATE, challenge);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        response
            .headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from_static(RETRY_AFTER_SECONDS));
    }
    Ok(response)
}

/// Answer with an empty body that is never cached.
pub(crate) fn empty() -> Result<Response<Body>, HttpError> {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())?;
    insert_headers(&mut response);
    Ok(response)
}

/// Answer with a discovery document, which any origin may read and any cache may keep a while.
pub(crate) fn metadata<T>(body: &T) -> Result<Response<Body>, HttpError>
where
    T: Serialize,
{
    let body = serde_json::to_vec(body)
        .map_err(|error| HttpError::for_internal_error(error.to_string()))?;
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, APPLICATION_JSON)
        .header(CACHE_CONTROL, METADATA_CACHE_CONTROL)
        .body(body.into())?;
    response
        .headers_mut()
        .insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    Ok(response)
}

fn json_with_status<T>(status: StatusCode, body: &T) -> Result<Response<Body>, HttpError>
where
    T: Serialize,
{
    let body = serde_json::to_vec(body)
        .map_err(|error| HttpError::for_internal_error(error.to_string()))?;
    let mut response = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, APPLICATION_JSON)
        .body(body.into())?;
    insert_headers(&mut response);
    Ok(response)
}

fn insert_headers(response: &mut Response<Body>) {
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
}

/// The console's consent page, with `query` as its only query.
pub(crate) fn consent_page(console_url: &Url, query: (&str, &str)) -> Url {
    let mut url = console_url.clone();
    url.set_path(CONSENT_PATH);
    url.set_fragment(None);
    url.set_query(None);
    url.query_pairs_mut().append_pair(query.0, query.1);
    url
}

/// The client's redirect URI with `params`, its `state`, and the issuer added to its query,
/// which is kept as it is.
pub(crate) fn client_redirect(
    redirect_uri: &OAuthRedirectUri,
    params: &[(&str, &str)],
    state: Option<&OAuthRequestState>,
    issuer: &OAuthIssuer,
) -> Url {
    let mut url = redirect_uri.url().clone();
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in params {
            query.append_pair(name, value);
        }
        if let Some(state) = state {
            query.append_pair("state", state.as_ref());
        }
        query.append_pair("iss", issuer.as_ref());
    }
    url
}

#[derive(Serialize)]
struct OAuthErrorBody<'a> {
    error: &'a str,
    error_description: &'a str,
}
