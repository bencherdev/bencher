use std::{
    collections::hash_map::DefaultHasher,
    hash::{
        Hash,
        Hasher,
    },
    sync::{
        Arc,
        Mutex,
    },
};

use diesel::{
    pg::PgConnection,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
    RequestContext,
    TypedBody,
};
use email_address_parser::EmailAddress;
use reports::{
    Metrics,
    Report,
};
use util::db::{
    model::{
        JsonNewReport as NewDbReport,
        Report as DbReport,
    },
    schema::report,
};

pub const DEFAULT_PROJECT: &str = "default";

#[endpoint {
    method = PUT,
    path = "/v0/reports",
    tags = ["report"]
}]
pub async fn api_put_reports(
    rqctx: Arc<RequestContext<Mutex<PgConnection>>>,
    body: TypedBody<Report>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let report = body.into_inner();
    let Report {
        email,
        token,
        project,
        testbed,
        date_time,
        metrics,
    } = report;

    // TODO actually use these values
    let email = map_email(email)?;
    let claims = map_token(token)?;
    let project = map_project(project.as_deref());

    let MetricsForDb {
        value,
        hash,
        length,
    } = map_metrics(metrics)?;

    let new_report = NewDbReport {
        date_time,
        metrics: value,
        hash: hash as i64,
        length: length as i32,
    };

    if let Ok(db_conn) = db_connection.lock() {
        let db_conn = &*db_conn;
        let report: DbReport = diesel::insert_into(report::table)
            .values(&new_report)
            .get_result(db_conn)
            .expect("Error saving new report");
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

// TODO return Claims from jsonwebtoken::decode()
fn map_token(token: String) -> Result<(), HttpError> {
    Ok(())
}

fn map_project(project: Option<&str>) -> String {
    if let Some(project) = project {
        slug::slugify(project)
    } else {
        DEFAULT_PROJECT.into()
    }
}

struct MetricsForDb {
    value:  serde_json::Value,
    hash:   u64,
    length: usize,
}

fn map_metrics(metrics: Metrics) -> Result<MetricsForDb, HttpError> {
    let err = |e| {
        HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("Failed to parse metrics {metrics:?}: {e}"),
        )
    };
    let metrics_str = serde_json::to_string(&metrics).map_err(err)?;
    let value: serde_json::Value = serde_json::from_str(&metrics_str).map_err(err)?;
    let hash = calculate_hash(&metrics_str);
    let length = metrics_str.len();
    Ok(MetricsForDb {
        value,
        hash,
        length,
    })
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
