use std::str::FromStr;

use bencher_json::{BranchName, JsonBranch, JsonNewBranch, ResourceId, Slug};
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use uuid::Uuid;

use super::{
    metric::{InsertMetric, QueryMetric},
    perf::{InsertPerf, QueryPerf},
    report::{InsertReport, QueryReport},
    version::{InsertVersion, QueryVersion},
    QueryProject,
};
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
        let branch = QueryBranch::from_resource_id(conn, self.project_id, start_point)?;
        let new_branch_id = QueryBranch::get_id(conn, &self.uuid)?;

        start_point_versions(conn, branch.id, new_branch_id)
    }
}

fn start_point_versions(
    conn: &mut SqliteConnection,
    start_point_branch_id: i32,
    new_branch_id: i32,
) -> Result<(), ApiError> {
    // Get all versions for the start point
    let versions = schema::version::table
        .filter(schema::version::branch_id.eq(start_point_branch_id))
        .load::<QueryVersion>(conn)?;

    for version in versions {
        let new_version_uuid = Uuid::new_v4();
        let insert_version = InsertVersion {
            uuid: new_version_uuid.to_string(),
            branch_id: new_branch_id,
            number: version.number,
            hash: version.hash,
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)
            .map_err(api_error!())?;

        let new_version_id = QueryVersion::get_id(conn, &new_version_uuid)?;

        start_point_reports(conn, version.id, new_version_id)?;
    }
    Ok(())
}

fn start_point_reports(
    conn: &mut SqliteConnection,
    start_point_version_id: i32,
    new_version_id: i32,
) -> Result<(), ApiError> {
    // Get all the reports for the start point version
    let reports = schema::report::table
        .filter(schema::report::version_id.eq(start_point_version_id))
        .load::<QueryReport>(conn)?;

    for report in reports {
        let QueryReport {
            user_id,
            testbed_id,
            adapter,
            start_time,
            end_time,
            ..
        } = report;

        let new_report_uuid = Uuid::new_v4();
        let insert_report = InsertReport {
            uuid: new_report_uuid.to_string(),
            user_id,
            version_id: new_version_id,
            testbed_id,
            adapter,
            start_time,
            end_time,
        };

        diesel::insert_into(schema::report::table)
            .values(&insert_report)
            .execute(conn)
            .map_err(api_error!())?;

        let new_report_id = QueryReport::get_id(conn, &new_report_uuid)?;

        start_point_perfs(conn, report.id, new_report_id)?;
    }

    Ok(())
}

fn start_point_perfs(
    conn: &mut SqliteConnection,
    start_point_report_id: i32,
    new_report_id: i32,
) -> Result<(), ApiError> {
    // Get all perfs for the start point report
    let perfs = schema::perf::table
        .filter(schema::perf::report_id.eq(start_point_report_id))
        .load::<QueryPerf>(conn)?;

    for perf in perfs {
        let QueryPerf {
            iteration,
            benchmark_id,
            ..
        } = perf;

        let new_perf_uuid = Uuid::new_v4();
        let insert_perf = InsertPerf {
            uuid: new_perf_uuid.to_string(),
            report_id: new_report_id,
            iteration,
            benchmark_id,
        };

        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(api_error!())?;

        let new_perf_id = QueryPerf::get_id(conn, &new_perf_uuid)?;

        start_point_metrics(conn, perf.id, new_perf_id)?;
    }

    Ok(())
}

fn start_point_metrics(
    conn: &mut SqliteConnection,
    start_point_perf_id: i32,
    new_perf_id: i32,
) -> Result<(), ApiError> {
    // Get all metrics for the start point perf
    let metrics = schema::metric::table
        .filter(schema::metric::perf_id.eq(start_point_perf_id))
        .load::<QueryMetric>(conn)?;

    for metric in metrics {
        let QueryMetric {
            metric_kind_id,
            value,
            lower_bound,
            upper_bound,
            ..
        } = metric;

        let new_metric_uuid = Uuid::new_v4();
        let insert_metric = InsertMetric {
            uuid: new_metric_uuid.to_string(),
            perf_id: new_perf_id,
            metric_kind_id,
            value,
            lower_bound,
            upper_bound,
        };

        diesel::insert_into(schema::metric::table)
            .values(&insert_metric)
            .execute(conn)
            .map_err(api_error!())?;
    }

    Ok(())
}
