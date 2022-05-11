use std::sync::Arc;

use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseAccepted;
use dropshot::RequestContext;
use dropshot::TypedBody;
use email_address_parser::EmailAddress;

use reports::Report;

#[endpoint {
    method = PUT,
    path = "/v0/reports",
}]
pub async fn api_put_reports(
    _rqctx: Arc<RequestContext<()>>,
    body: TypedBody<Report>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let report = body.into_inner();
    let email = map_email(report.email)?;
    Ok(HttpResponseAccepted(()))
}

fn map_email(email: String) -> Result<EmailAddress, HttpError> {
    EmailAddress::parse(&email, None).ok_or(HttpError::for_bad_request(
        Some(String::from("BadInput")),
        format!("Failed to parse email: {email}"),
    ))
}
