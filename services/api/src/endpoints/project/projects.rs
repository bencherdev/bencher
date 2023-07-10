use bencher_json::{JsonDirection, JsonPagination, JsonProject, ResourceId};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        project::{visibility::Visibility, QueryProject},
        user::auth::AuthUser,
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const PROJECT_RESOURCE: Resource = Resource::Project;

#[derive(Clone, Deserialize, JsonSchema)]
pub struct ProjectsQuery {
    pub sort: Option<ProjectsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,

    pub name: Option<String>,
    pub public: Option<bool>,
}

impl From<&ProjectsQuery> for JsonPagination<ProjectsSort, ()> {
    fn from(query: &ProjectsQuery) -> Self {
        let ProjectsQuery {
            sort,
            direction,
            per_page,
            page,
            ..
        } = *query;
        Self {
            sort,
            direction,
            per_page,
            page,
            query: (),
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectsSort {
    #[default]
    Name,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn projects_options(
    _rqctx: RequestContext<ApiContext>,
    _query_params: Query<ProjectsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn projects_get(
    rqctx: RequestContext<ApiContext>,
    query_params: Query<ProjectsQuery>,
) -> Result<ResponseOk<Vec<JsonProject>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        query_params.into_inner(),
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
    query_params: ProjectsQuery,
    endpoint: Endpoint,
) -> Result<Vec<JsonProject>, ApiError> {
    let conn = &mut *context.conn().await;

    let mut query = schema::project::table.into_boxed();

    // All users should just see the public projects if the query is for public projects
    if let Some(true) = query_params.public {
        query = query.filter(schema::project::visibility.eq(Visibility::Public as i32));
    } else if let Some(auth_user) = auth_user {
        if !auth_user.is_admin(&context.rbac) {
            let projects =
                auth_user.projects(&context.rbac, bencher_rbac::project::Permission::View);
            query = query.filter(schema::project::id.eq_any(projects));
        }
    } else {
        return Err(ApiError::PrivateProjects);
    }

    if let Some(name) = &query_params.name {
        query = query.filter(schema::project::name.eq(name));
    }

    let json_pagination = JsonPagination::from(&query_params);
    query = match json_pagination.order() {
        ProjectsSort::Name => match json_pagination.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::project::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::project::name.desc()),
        },
    };

    Ok(query
        .offset(json_pagination.offset())
        .limit(json_pagination.limit())
        .load::<QueryProject>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjectParams {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn project_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjectParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjectParams>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetOne);

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
    path_params: ProjectParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonProject, ApiError> {
    let conn = &mut *context.conn().await;

    QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?
        .into_json(conn)
}
