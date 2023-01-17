use std::str::FromStr;

use bencher_json::{BranchName, JsonBranch, JsonNewBranch, ResourceId, Slug};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use super::{version::InsertBranchVersion, QueryProject};
use crate::{
    error::api_error,
    schema,
    schema::branch as branch_table,
    util::{query::fn_get_id, resource_id::fn_resource_id, slug::unwrap_child_slug},
    ApiError,
};

fn_resource_id!(branch);

#[derive(Queryable)]
pub struct QueryBranch {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
}

impl QueryBranch {
    fn_get_id!(branch);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::branch::table
            .filter(schema::branch::id.eq(id))
            .select(schema::branch::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn from_resource_id(
        conn: &mut SqliteConnection,
        project_id: i32,
        branch: &ResourceId,
    ) -> Result<Self, ApiError> {
        schema::branch::table
            .filter(schema::branch::project_id.eq(project_id))
            .filter(resource_id(branch)?)
            .first::<QueryBranch>(conn)
            .map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonBranch, ApiError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            ..
        } = self;
        Ok(JsonBranch {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: BranchName::from_str(&name).map_err(api_error!())?,
            slug: Slug::from_str(&slug).map_err(api_error!())?,
        })
    }
}

#[derive(Insertable)]
#[diesel(table_name = branch_table)]
pub struct InsertBranch {
    pub uuid: String,
    pub project_id: i32,
    pub name: String,
    pub slug: String,
}

impl InsertBranch {
    pub fn from_json(
        conn: &mut SqliteConnection,
        project: &ResourceId,
        branch: JsonNewBranch,
    ) -> Result<Self, ApiError> {
        let project_id = QueryProject::from_resource_id(conn, project)?.id;
        Ok(Self::from_json_inner(conn, project_id, branch))
    }

    pub fn main(conn: &mut SqliteConnection, project_id: i32) -> Self {
        Self::from_json_inner(conn, project_id, JsonNewBranch::main())
    }

    pub fn from_json_inner(
        conn: &mut SqliteConnection,
        project_id: i32,
        branch: JsonNewBranch,
    ) -> Self {
        let JsonNewBranch { name, slug, .. } = branch;
        let slug = unwrap_child_slug!(conn, project_id, name.as_ref(), slug, branch, QueryBranch);
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name: name.into(),
            slug,
        }
    }

    pub fn start_point(
        &self,
        conn: &mut SqliteConnection,
        start_point: &ResourceId,
    ) -> Result<(), ApiError> {
        let start_point_branch_id =
            QueryBranch::from_resource_id(conn, self.project_id, start_point)?.id;
        let new_branch_id = QueryBranch::get_id(conn, &self.uuid)?;

        // Get all versions for the start point branch
        let version_ids = schema::branch_version::table
            .filter(schema::branch_version::branch_id.eq(start_point_branch_id))
            .select(schema::branch_version::version_id)
            .load::<i32>(conn)?;

        for version_id in version_ids {
            let insert_branch_version = InsertBranchVersion {
                branch_id: new_branch_id,
                version_id,
            };

            diesel::insert_into(schema::branch_version::table)
                .values(&insert_branch_version)
                .execute(conn)
                .map_err(api_error!())?;
        }

        Ok(())
    }
}
