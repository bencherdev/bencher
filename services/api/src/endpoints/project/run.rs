use bencher_json::{JsonNewReport, JsonReport};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::Logger;

use crate::{
    context::ApiContext,
    endpoints::endpoint::{Post, ResponseCreated},
    model::{
        project::{report::QueryReport, QueryProject},
        user::auth::{AuthUser, PubBearerToken},
    },
};

/// Create a run
///
/// Create a run for a project.
/// The project may or may not yet exist.
#[endpoint {
    method = POST,
    path =  "/v0/run",
    tags = ["projects", "reports"]
}]
// For simplicity, this query makes the assumption that all posts are perfectly
// chronological. That is, a report will never be posted for X after Y has
// already been submitted when X really happened before Y. For implementing git
// bisect more complex logic will be required.
pub async fn proj_report_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    body: TypedBody<JsonNewReport>,
) -> Result<ResponseCreated<JsonReport>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        body.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    mut json_report: JsonNewReport,
    auth_user: Option<&AuthUser>,
) -> Result<JsonReport, HttpError> {
    let todo_pub_run = || -> QueryProject {
        unimplemented!("pub run creation is not yet implemented");
    };
    let query_project = todo_pub_run();
    QueryReport::create(log, context, &query_project, json_report, auth_user).await
}
