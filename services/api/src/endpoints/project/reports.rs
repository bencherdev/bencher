use std::collections::HashMap;

use bencher_json::{JsonEmpty, JsonNewReport, JsonReport, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{dsl::count, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        report::{results::ReportResults, InsertReport, QueryReport},
        version::{InsertVersion, QueryVersion},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::database_map,
        same_project::SameProject,
    },
    ApiError,
};

use super::Resource;

const REPORT_RESOURCE: Resource = Resource::Report;

#[derive(Deserialize, JsonSchema)]
pub struct ProjReportsParams {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn proj_reports_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjReportsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn proj_reports_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjReportsParams>,
) -> Result<ResponseOk<Vec<JsonReport>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(REPORT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjReportsParams,
    endpoint: Endpoint,
) -> Result<Vec<JsonReport>, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    Ok(schema::report::table
        .filter(schema::report::project_id.eq(query_project.id))
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::project_id,
            schema::report::branch_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::created,
        ))
        .order((
            schema::report::start_time.desc(),
            schema::report::end_time.desc(),
        ))
        .load::<QueryReport>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| database_map(endpoint, query.into_json(conn)))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
// For simplicity, this query makes the assumption that all posts are perfectly
// chronological. That is, a report will never be posted for X after Y has
// already been submitted when X really happened before Y. For implementing git
// bisect more complex logic will be required.
pub async fn proj_report_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjReportsParams>,
    body: TypedBody<JsonNewReport>,
) -> Result<ResponseAccepted<JsonReport>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(REPORT_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjReportsParams,
    mut json_report: JsonNewReport,
    auth_user: &AuthUser,
) -> Result<JsonReport, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the branch and testbed are part of the same project
    let SameProject {
        project_id,
        branch_id,
        testbed_id,
    } = SameProject::validate(
        conn,
        &path_params.project,
        &json_report.branch,
        &json_report.testbed,
    )?;

    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        conn,
        &context.rbac,
        project_id,
        auth_user,
        Permission::Create,
    )?;

    // Check to see if the project is public or private
    // If private, then validate that there is an active subscription or license
    #[cfg(feature = "plus")]
    let plan_kind =
        plan_kind::PlanKind::new(conn, context.biller.as_ref(), &context.licensor, project_id)
            .await?;

    // If there is a hash then try to see if there is already a code version for
    // this branch with that particular hash.
    // Otherwise, create a new code version for this branch with/without the hash.
    let version_id = if let Some(hash) = &json_report.hash {
        if let Ok(version_id) = schema::version::table
            .left_join(
                schema::branch_version::table
                    .on(schema::version::id.eq(schema::branch_version::version_id)),
            )
            .filter(schema::branch_version::branch_id.eq(branch_id))
            .filter(schema::version::hash.eq(hash.as_ref()))
            .order(schema::version::number.desc())
            .select(schema::version::id)
            .first::<i32>(conn)
        {
            version_id
        } else {
            InsertVersion::increment(conn, project_id, branch_id, Some(hash.clone()))?
        }
    } else {
        InsertVersion::increment(conn, project_id, branch_id, None)?
    };

    let json_settings = json_report.settings.take().unwrap_or_default();
    let adapter = json_settings.adapter.unwrap_or_default();

    // Create a new report and add it to the database
    let insert_report = InsertReport::from_json(
        auth_user.id,
        project_id,
        branch_id,
        version_id,
        testbed_id,
        &json_report,
        adapter,
    );

    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(conn)
        .map_err(api_error!())?;

    let query_report = schema::report::table
        .filter(schema::report::uuid.eq(&insert_report.uuid))
        .first::<QueryReport>(conn)
        .map_err(api_error!())?;

    #[cfg(feature = "plus")]
    let mut usage = 0;

    // Process and record the report results
    let mut report_results = ReportResults::new(project_id, branch_id, testbed_id, query_report.id);
    let processed_report = report_results.process(
        conn,
        json_report
            .results
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>()
            .as_ref(),
        adapter,
        json_settings,
        #[cfg(feature = "plus")]
        &mut usage,
    );

    #[cfg(feature = "plus")]
    plan_kind
        .check_usage(context.biller.as_ref(), project_id, usage)
        .await?;

    // Don't return the error from processing the report
    // until after the metrics usage has been checked
    processed_report?;

    query_report.into_json(conn)
}

#[cfg(feature = "plus")]
mod plan_kind {
    use bencher_billing::{Biller, SubscriptionId};
    use bencher_license::Licensor;

    use crate::{context::DbConnection, model::project::QueryProject, ApiError};

    pub enum PlanKind {
        Metered(SubscriptionId),
        Licensed(u64),
        None,
    }

    impl PlanKind {
        pub async fn new(
            conn: &mut DbConnection,
            biller: Option<&Biller>,
            licensor: &Licensor,
            project_id: i32,
        ) -> Result<Self, ApiError> {
            if let Some(subscription) = QueryProject::get_subscription(conn, project_id)? {
                if let Some(biller) = biller {
                    let plan_status = biller.get_plan_status(&subscription).await?;
                    if plan_status.is_active() {
                        Ok(PlanKind::Metered(subscription))
                    } else {
                        Err(ApiError::InactivePlanProject(project_id))
                    }
                } else {
                    Err(ApiError::NoBillerProject(project_id))
                }
            } else if let Some((uuid, license)) = QueryProject::get_license(conn, project_id)? {
                let _token_data = licensor.validate_organization(&license, uuid)?;
                // TODO check license entitlements for usage so far
                Ok(PlanKind::Licensed(0))
            } else if QueryProject::is_public(conn, project_id)? {
                Ok(Self::None)
            } else {
                Err(ApiError::NoPlanProject(project_id))
            }
        }

        pub async fn check_usage(
            &self,
            biller: Option<&Biller>,
            project_id: i32,
            usage: u64,
        ) -> Result<(), ApiError> {
            match self {
                Self::Metered(subscription) => {
                    let Some(biller) = biller else {
                        return Err(ApiError::NoBillerProject(project_id));
                    };
                    biller.record_usage(subscription, usage).await?;
                },
                // TODO check for usage overage
                Self::Licensed(_) => {},
                Self::None => {},
            }

            Ok(())
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjReportParams {
    pub project: ResourceId,
    pub report_uuid: Uuid,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjReportParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjReportParams>,
) -> Result<ResponseOk<JsonReport>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(REPORT_RESOURCE, Method::GetOne);

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjReportParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonReport, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    schema::report::table
        .filter(schema::report::project_id.eq(query_project.id))
        .filter(schema::report::uuid.eq(path_params.report_uuid.to_string()))
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::project_id,
            schema::report::branch_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::created,
        ))
        .first::<QueryReport>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjReportParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(REPORT_RESOURCE, Method::Delete);

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjReportParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let (report_id, version_id) = schema::report::table
        .filter(schema::report::project_id.eq(query_project.id))
        .filter(schema::report::uuid.eq(path_params.report_uuid.to_string()))
        .select((schema::report::id, schema::report::version_id))
        .first::<(i32, i32)>(conn)
        .map_err(api_error!())?;
    diesel::delete(schema::report::table.filter(schema::report::id.eq(report_id)))
        .execute(conn)
        .map_err(api_error!())?;

    // If there are no more reports for this version, delete the version
    // This is necessary because multiple reports can use the same version via a git hash
    // This will cascade and delete all branch versions for this version
    // Before doing so, decrement all greater versions
    if schema::report::table
        .filter(schema::report::version_id.eq(version_id))
        .select(count(schema::report::id))
        .first::<i64>(conn)
        .map_err(api_error!())?
        == 0
    {
        let query_version = QueryVersion::get(conn, version_id)?;
        // Get all branches that use this version
        let branches = schema::branch::table
            .inner_join(
                schema::branch_version::table
                    .on(schema::branch_version::branch_id.eq(schema::branch::id)),
            )
            .filter(schema::branch_version::version_id.eq(version_id))
            .select(schema::branch::id)
            .load::<i32>(conn)
            .map_err(api_error!())?;

        let mut version_map = HashMap::new();
        // Get all versions greater than this one for each of the branches
        for branch_id in branches {
            schema::version::table
                .filter(schema::version::number.gt(query_version.number))
                .inner_join(
                    schema::branch_version::table
                        .on(schema::branch_version::version_id.eq(schema::version::id)),
                )
                .filter(schema::branch_version::branch_id.eq(branch_id))
                .select((schema::version::id, schema::version::number))
                .load::<(i32, i32)>(conn)
                .map_err(api_error!())?
                .into_iter()
                .for_each(|(version_id, version_number)| {
                    version_map.insert(version_id, version_number);
                });
        }

        for (version_id, version_number) in version_map {
            diesel::update(schema::version::table.filter(schema::version::id.eq(version_id)))
                .set(schema::version::number.eq(version_number - 1))
                .execute(conn)
                .map_err(api_error!())
                .unwrap();
        }

        diesel::delete(schema::version::table.filter(schema::version::id.eq(version_id)))
            .execute(conn)
            .map_err(api_error!())?;
    }

    Ok(JsonEmpty {})
}
