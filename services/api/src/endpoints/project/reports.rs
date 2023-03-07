use bencher_json::{JsonNewReport, JsonReport, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl,
};
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
        version::InsertVersion,
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        same_project::SameProject,
    },
    ApiError,
};

use super::Resource;

const REPORT_RESOURCE: Resource = Resource::Report;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn dir_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn get_ls(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<DirPath>,
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
    path_params: DirPath,
    endpoint: Endpoint,
) -> Result<Vec<JsonReport>, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    Ok(schema::report::table
        .left_join(schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)))
        .filter(schema::testbed::project_id.eq(query_project.id))
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter,
            schema::report::start_time,
            schema::report::end_time,
        ))
        .order((
            schema::report::start_time.desc(),
            schema::report::end_time.desc(),
        ))
        .load::<QueryReport>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
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
pub async fn post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<DirPath>,
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
    path_params: DirPath,
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
            InsertVersion::increment(conn, branch_id, Some(hash.clone()))?
        }
    } else {
        InsertVersion::increment(conn, branch_id, None)?
    };

    let json_settings = json_report.settings.take().unwrap_or_default();
    let adapter = json_settings.adapter.unwrap_or_default();

    // Create a new report and add it to the database
    let insert_report =
        InsertReport::from_json(auth_user.id, version_id, testbed_id, &json_report, adapter);

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
pub struct OnePath {
    pub project: ResourceId,
    pub report_uuid: Uuid,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn one_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn get_one(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OnePath>,
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
    path_params: OnePath,
    auth_user: Option<&AuthUser>,
) -> Result<JsonReport, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    schema::report::table
        .left_join(schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)))
        .filter(
            schema::testbed::project_id
                .eq(query_project.id)
                .and(schema::report::uuid.eq(path_params.report_uuid.to_string())),
        )
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter,
            schema::report::start_time,
            schema::report::end_time,
        ))
        .first::<QueryReport>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
