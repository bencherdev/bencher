use std::sync::Arc;

use bencher_json::{JsonMember, JsonNewTestbed, JsonTestbed, ResourceId};
use bencher_rbac::organization::Permission;
use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, JoinOnDsl,
    NullableExpressionMethods, QueryDsl, RunQueryDsl,
};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::user::auth::AuthUser,
    model::{organization::QueryOrganization, user::member::QueryMember},
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        resource_id::fn_resource_id,
        Context,
    },
    ApiError,
};

use super::Resource;

const MEMBER_RESOURCE: Resource = Resource::Member;

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub organization: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<ResponseOk<Vec<JsonMember>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetLs);

    let json = get_ls_inner(rqctx.context(), &auth_user, path_params.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &Context,
    auth_user: &AuthUser,
    path_params: GetLsParams,
) -> Result<Vec<JsonMember>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    Ok(schema::user::table
        .inner_join(
            schema::organization_role::table
                .on(schema::user::id.eq(schema::organization_role::user_id)),
        )
        .filter(schema::organization_role::organization_id.eq(query_organization.id))
        .select((
            schema::user::uuid,
            schema::user::name,
            schema::user::slug,
            schema::user::email,
            schema::organization_role::role,
        ))
        .order(schema::user::email)
        .load::<QueryMember>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| query.into_json().ok())
        .collect())
}

// #[endpoint {
//     method = OPTIONS,
//     path =  "/v0/members",
//     tags = ["members"]
// }]
// pub async fn post_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
//     Ok(get_cors::<Context>())
// }

// #[endpoint {
//     method = POST,
//     path = "/v0/members",
//     tags = ["members"]
// }]
// pub async fn post(
//     rqctx: Arc<RequestContext<Context>>,
//     body: TypedBody<JsonNewTestbed>,
// ) -> Result<ResponseAccepted<JsonTestbed>, HttpError> {
//     let auth_user = AuthUser::new(&rqctx).await?;
//     let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::Post);

//     let json = post_inner(rqctx.context(), body.into_inner(), &auth_user)
//         .await
//         .map_err(|e| endpoint.err(e))?;

//     response_accepted!(endpoint, json)
// }

// async fn post_inner(
//     context: &Context,
//     json_testbed: JsonNewTestbed,
//     auth_user: &AuthUser,
// ) -> Result<JsonTestbed, ApiError> {
//     let api_context = &mut *context.lock().await;
//     let insert_testbed = InsertTestbed::from_json(&mut api_context.database, json_testbed)?;
//     // Verify that the user is allowed
//     QueryProject::is_allowed_id(
//         api_context,
//         insert_testbed.project_id,
//         auth_user,
//         Permission::Create,
//     )?;
//     let conn = &mut api_context.database;

//     diesel::insert_into(schema::user::table)
//         .values(&insert_testbed)
//         .execute(conn)
//         .map_err(api_error!())?;

//     schema::member::table
//         .filter(schema::member::uuid.eq(&insert_testbed.uuid))
//         .first::<QueryTestbed>(conn)
//         .map_err(api_error!())?
//         .into_json(conn)
// }

// #[derive(Deserialize, JsonSchema)]
// pub struct GetOneParams {
//     pub organization: ResourceId,
//     pub member: ResourceId,
// }

// #[endpoint {
//     method = OPTIONS,
//     path =  "/v0/organizations/{organization}/members/{member}",
//     tags = ["organizations", "members"]
// }]
// pub async fn one_options(
//     _rqctx: Arc<RequestContext<Context>>,
//     _path_params: Path<GetOneParams>,
// ) -> Result<CorsResponse, HttpError> {
//     Ok(get_cors::<Context>())
// }

// #[endpoint {
//     method = GET,
//     path =  "/v0/organizations/{organization}/members/{member}",
//     tags = ["organizations", "members"]
// }]
// pub async fn get_one(
//     rqctx: Arc<RequestContext<Context>>,
//     path_params: Path<GetOneParams>,
// ) -> Result<ResponseOk<JsonTestbed>, HttpError> {
//     let auth_user = AuthUser::new(&rqctx).await?;
//     let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetOne);

//     let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
//         .await
//         .map_err(|e| endpoint.err(e))?;

//     response_ok!(endpoint, json)
// }

// fn_resource_id!(member);

// async fn get_one_inner(
//     context: &Context,
//     path_params: GetOneParams,
//     auth_user: &AuthUser,
// ) -> Result<JsonTestbed, ApiError> {
//     let api_context = &mut *context.lock().await;
//     let query_project = QueryProject::is_allowed_resource_id(
//         api_context,
//         &path_params.project,
//         auth_user,
//         Permission::View,
//     )?;
//     let conn = &mut api_context.database;

//     schema::member::table
//         .filter(
//             schema::member::project_id
//                 .eq(query_project.id)
//                 .and(resource_id(&path_params.member)?),
//         )
//         .first::<QueryTestbed>(conn)
//         .map_err(api_error!())?
//         .into_json(conn)
// }
