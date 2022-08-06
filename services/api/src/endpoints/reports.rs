use std::sync::Arc;

use bencher_json::{
    JsonNewReport,
    JsonReport,
    ResourceId,
};
use diesel::{
    expression_methods::BoolExpressionMethods,
    JoinOnDsl,
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
    TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db::{
        model::{
            adapter::QueryAdapter,
            branch::QueryBranch,
            project::QueryProject,
            report::{
                InsertReport,
                QueryReport,
            },
            testbed::QueryTestbed,
            user::QueryUser,
            version::{
                InsertVersion,
                QueryVersion,
            },
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        auth::get_token,
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn get_ls_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonReport>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();

    let conn = db_connection.lock().await;
    let query_project = QueryProject::from_resource_id(&*conn, &path_params.project)?;
    let json: Vec<JsonReport> = schema::report::table
        .left_join(schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)))
        .filter(schema::testbed::project_id.eq(&query_project.id))
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter_id,
            schema::report::start_time,
            schema::report::end_time,
        ))
        .order(schema::report::start_time.desc())
        .load::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to get reports."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}

// let json: Vec<JsonReport> = schema::report::table
//         .left_join(schema::version::table.on(schema::report::version_id.
// eq(schema::version::id)))         .left_join(schema::branch::table.on(schema:
// :version::branch_id.eq(schema::branch::id)))         .filter(schema::branch::
// project_id.eq(&query_project.id))         .select((
//             schema::report::id,
//             schema::report::uuid,
//             schema::report::user_id,
//             schema::report::version_id,
//             schema::report::testbed_id,
//             schema::report::adapter_id,
//             schema::report::start_time,
//             schema::report::end_time,
//         ))
//         // .order(schema::report::start_time)
//         // .desc()
//         .load::<QueryReport>(&*conn)
//         .map_err(|_| http_error!("Failed to get reports."))?
//         .into_iter()
//         .filter_map(|query| query.to_json(&*conn).ok())
//         .collect();

#[endpoint {
    method = OPTIONS,
    path =  "/v0/reports",
    tags = ["reports"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewReport>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonReport>, CorsHeaders>, HttpError> {
    const ERROR: &str = "Failed to create report.";

    let user_uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let json_report = body.into_inner();

    let conn = db_connection.lock().await;
    // Verify that the branch and testbed are part of the same project
    let branch_id = QueryBranch::get_id(&*conn, json_report.branch)?;
    let testbed_id = QueryTestbed::get_id(&*conn, json_report.testbed)?;
    let branch_project_id = schema::branch::table
        .filter(schema::branch::id.eq(&branch_id))
        .select(schema::branch::project_id)
        .first::<i32>(&*conn)
        .map_err(|_| http_error!(ERROR))?;
    let testbed_project_id = schema::testbed::table
        .filter(schema::testbed::id.eq(&testbed_id))
        .select(schema::testbed::project_id)
        .first::<i32>(&*conn)
        .map_err(|_| http_error!(ERROR))?;
    if branch_project_id != testbed_project_id {
        return Err(http_error!(ERROR));
    }
    // Verify that the user has access to the project
    let user_id = QueryUser::get_id(&*conn, &user_uuid)?;
    schema::project::table
        .filter(
            schema::project::id
                .eq(branch_project_id)
                .and(schema::project::owner_id.eq(user_id)),
        )
        .select(schema::project::id)
        .first::<i32>(&*conn)
        .map_err(|_| http_error!(ERROR))?;

    // If there is a hash then try to see if there is already a code version for
    // this branch with that particular hash.
    // Otherwise, create a new code version for this branch with/without the hash.
    let version_id = if let Some(hash) = json_report.hash {
        if let Ok(version_id) = schema::version::table
            .filter(
                schema::version::branch_id
                    .eq(branch_id)
                    .and(schema::version::hash.eq(&hash)),
            )
            .select(schema::version::id)
            .first::<i32>(&*conn)
        {
            version_id
        } else {
            InsertVersion::increment(&*conn, branch_id, Some(hash))?
        }
    } else {
        InsertVersion::increment(&*conn, branch_id, None)?
    };

    let insert_report = InsertReport {
        uuid: Uuid::new_v4().to_string(),
        user_id,
        version_id,
        testbed_id,
        adapter_id: QueryAdapter::get_id(&*conn, json_report.adapter.to_string())?,
        start_time: json_report.start_time.naive_utc(),
        end_time: json_report.end_time.naive_utc(),
    };

    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create report."))?;

    let query_report = schema::report::table
        .filter(schema::report::uuid.eq(&insert_report.uuid))
        .first::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to create report."))?;
    let json = query_report.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

// #[derive(Deserialize, JsonSchema)]
// pub struct GetOneParams {
//     pub project:        ResourceId,
//     pub branch:         ResourceId,
//     pub version_number: ResourceId,
// }
// #[endpoint {
//     method = OPTIONS,
//     path =  "/v0/projects/{project}/branches/{branch}/{version_number}",
//     tags = ["projects", "branches", "reports"]
// }]

// pub async fn get_one_options(
//     _rqctx: Arc<RequestContext<Context>>,
//     _path_params: Path<GetOneParams>,
// ) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
//     Ok(get_cors::<Context>())
// }

// #[endpoint {
//     method = GET,
//     path =  "/v0/projects/{project}/branches/{branch}/{version_number}",
//     tags = ["projects", "branches",  "reports"]
// }]
// pub async fn get_one(
//     rqctx: Arc<RequestContext<Context>>,
//     path_params: Path<GetOneParams>,
// ) -> Result<HttpResponseHeaders<HttpResponseOk<JsonReport>, CorsHeaders>,
// HttpError> {     let db_connection = rqctx.context();
//     let path_params = path_params.into_inner();
//     let resource_id = path_params.report.as_str();

//     let conn = db_connection.lock().await;
//     let project = QueryProject::from_resource_id(&*conn,
// &path_params.project)?;     let query = if let Ok(query) =
// schema::report::table         .filter(
//             schema::report::project_id.eq(project.id).and(
//                 schema::report::version_id
//                     .eq(version_id)
//                     .or(schema::report::uuid.eq(resource_id)),
//             ),
//         )
//         .first::<QueryReport>(&*conn)
//     {
//         Ok(query)
//     } else {
//         Err(http_error!("Failed to get report."))
//     }?;
//     let json = query.to_json(&*conn)?;

//     Ok(HttpResponseHeaders::new(
//         HttpResponseOk(json),
//         CorsHeaders::new_pub("GET".into()),
//     ))
// }
