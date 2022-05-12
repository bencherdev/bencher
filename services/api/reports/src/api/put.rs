use std::sync::Arc;
use std::sync::Mutex;

use chrono::DateTime;
use chrono::Utc;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::RunQueryDsl;
use dropshot::endpoint;
use dropshot::HttpError;
use dropshot::HttpResponseAccepted;
use dropshot::RequestContext;
use dropshot::TypedBody;
use email_address_parser::EmailAddress;

use reports::Report as ReportJson;

use util::db::model::{NewReport, Report};
use util::db::schema::report;

#[endpoint {
    method = PUT,
    path = "/v0/reports",
}]
pub async fn api_put_reports(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
    body: TypedBody<ReportJson>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let db_connection = rqctx.context();

    let report = body.into_inner();
    let email = map_email(report.email)?;

    let new_report = NewReport {
        date_time: Utc::now(),
        hash: 55,
        length: 55,
    };

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;
        let report: Report = diesel::insert_into(report::table)
            .values(&new_report)
            .get_result(db_conn)
            .expect("Error saving new post");
        println!("{report:?}")
    }

    Ok(HttpResponseAccepted(()))
}

fn map_email(email: String) -> Result<EmailAddress, HttpError> {
    EmailAddress::parse(&email, None).ok_or(HttpError::for_bad_request(
        Some(String::from("BadInput")),
        format!("Failed to parse email: {email}"),
    ))
}
