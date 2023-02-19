#![cfg(feature = "plus")]

use bencher_json::{
    organization::billing::JsonNewMetered, JsonEmpty, JsonNewTestbed, JsonTestbed, JsonUser,
    ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::user::{auth::AuthUser, QueryUser},
    model::{
        organization::QueryOrganization,
        project::{
            testbed::{InsertTestbed, QueryTestbed},
            QueryProject,
        },
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        resource_id::fn_resource_id,
    },
    ApiError,
};

use super::Resource;

const PLAN_RESOURCE: Resource = Resource::Plan;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn dir_options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn post(
    rqctx: RequestContext<Context>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonNewMetered>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PLAN_RESOURCE, Method::Post);

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
    context: &Context,
    path_params: DirPath,
    json_metered: JsonNewMetered,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database.connection;

    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &api_context.biller else {
        return Err(ApiError::BencherCloudOnly(
            "/v0/organizations/organization/plan".into(),
        ));
    };

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    api_context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)?;

    // Check to make sure the organization does not already have a plan
    if let Some(subscription) = query_org.subscription {
        return Err(ApiError::PlanMetered(query_org.id, subscription));
    } else if let Some(license) = query_org.license {
        return Err(ApiError::PlanLicensed(query_org.id, license));
    }

    let json_user: JsonUser = schema::user::table
        .filter(schema::user::id.eq(auth_user.id))
        .first::<QueryUser>(conn)
        .map_err(api_error!())?
        .into_json()?;

    let customer = biller
        .get_or_create_customer(&json_user.name, &json_user.email)
        .await?;

    let payment_method = biller
        .create_payment_method(&customer, json_metered.card)
        .await?;

    // let insert_testbed = InsertTestbed::from_json(
    //     &mut api_context.database.connection,
    //     &path_params.project,
    //     json_testbed,
    // )?;
    // // Verify that the user is allowed
    // QueryProject::is_allowed_id(
    //     api_context,
    //     insert_testbed.project_id,
    //     auth_user,
    //     Permission::Create,
    // )?;
    // let conn = &mut api_context.database.connection;

    // diesel::insert_into(schema::testbed::table)
    //     .values(&insert_testbed)
    //     .execute(conn)
    //     .map_err(api_error!())?;

    // schema::testbed::table
    //     .filter(schema::testbed::uuid.eq(&insert_testbed.uuid))
    //     .first::<QueryTestbed>(conn)
    //     .map_err(api_error!())?
    //     .into_json(conn)

    Ok(JsonEmpty::default())
}

// #[derive(Deserialize, JsonSchema)]
// pub struct OnePath {
//     pub organization: ResourceId,
// }

// #[allow(clippy::unused_async)]
// #[endpoint {
//     method = OPTIONS,
//     path =  "/v0/organizations/{organization}/plan",
//     tags = ["organizations", "plan"]
// }]
// pub async fn one_options(
//     _rqctx: RequestContext<Context>,
//     _path_params: Path<OnePath>,
// ) -> Result<CorsResponse, HttpError> {
//     Ok(get_cors::<Context>())
// }

// #[endpoint {
//     method = GET,
//     path =  "/v0/organizations/{organization}/plan",
//     tags = ["organizations", "plan"]
// }]
// pub async fn get_one(
//     rqctx: RequestContext<Context>,
//     path_params: Path<OnePath>,
// ) -> Result<ResponseOk<JsonTestbed>, HttpError> {
//     let auth_user = AuthUser::new(&rqctx).await.ok();
//     let endpoint = Endpoint::new(PLAN_RESOURCE, Method::GetOne);

//     let json = get_one_inner(
//         rqctx.context(),
//         path_params.into_inner(),
//         auth_user.as_ref(),
//     )
//     .await
//     .map_err(|e| endpoint.err(e))?;

//     if auth_user.is_some() {
//         response_ok!(endpoint, json)
//     } else {
//         pub_response_ok!(endpoint, json)
//     }
// }

// fn_resource_id!(testbed);

// async fn get_one_inner(
//     context: &Context,
//     path_params: OnePath,
//     auth_user: Option<&AuthUser>,
// ) -> Result<JsonTestbed, ApiError> {
//     let api_context = &mut *context.lock().await;
//     let query_project =
//         QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?;
//     let conn = &mut api_context.database.connection;

//     schema::testbed::table
//         .filter(
//             schema::testbed::project_id
//                 .eq(query_project.id)
//                 .and(resource_id(&path_params.testbed)?),
//         )
//         .first::<QueryTestbed>(conn)
//         .map_err(api_error!())?
//         .into_json(conn)
// }
