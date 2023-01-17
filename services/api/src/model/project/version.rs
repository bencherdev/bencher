use std::str::FromStr;

use bencher_json::{GitHash, ResourceId};
use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, Queryable, RunQueryDsl,
    SqliteConnection,
};
use uuid::Uuid;

use crate::{
    error::api_error, schema, schema::version as version_table, util::query::fn_get_id, ApiError,
};

use super::{
    branch::QueryBranch,
    metric::{InsertMetric, QueryMetric},
    perf::{InsertPerf, QueryPerf},
    report::{InsertReport, QueryReport},
};

#[derive(Queryable)]
pub struct QueryVersion {
    pub id: i32,
    pub uuid: String,
    pub branch_id: i32,
    pub number: i32,
    pub hash: Option<String>,
}

impl QueryVersion {
    fn_get_id!(version);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::version::table
            .filter(schema::version::id.eq(id))
            .select(schema::version::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }
}

#[derive(Insertable)]
#[diesel(table_name = version_table)]
pub struct InsertVersion {
    pub uuid: String,
    pub branch_id: i32,
    pub number: i32,
    pub hash: Option<String>,
}

impl InsertVersion {
    pub fn increment(
        conn: &mut SqliteConnection,
        branch_id: i32,
        hash: Option<GitHash>,
    ) -> Result<i32, ApiError> {
        // Get the most recent code version number for this branch and increment it.
        // Otherwise, start a new branch code version number count from zero.
        let number = if let Ok(number) = schema::version::table
            .filter(schema::version::branch_id.eq(branch_id))
            .select(schema::version::number)
            .order(schema::version::number.desc())
            .first::<i32>(conn)
        {
            number.checked_add(1).ok_or(ApiError::BadMath)?
        } else {
            0
        };

        let uuid = Uuid::new_v4();
        let insert_version = InsertVersion {
            uuid: uuid.to_string(),
            branch_id,
            number,
            hash: hash.map(Into::into),
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)
            .map_err(api_error!())?;

        QueryVersion::get_id(conn, &uuid)
    }

    pub fn start_point(
        conn: &mut SqliteConnection,
        project_id: i32,
        start_point: &ResourceId,
        branch: Uuid,
    ) -> Result<(), ApiError> {
        let start_point_branch = QueryBranch::from_resource_id(conn, project_id, &start_point)?;
        let branch_id = QueryBranch::get_id(conn, &branch)?;

        // Get all versions for the start point
        let versions = schema::version::table
            .filter(schema::version::branch_id.eq(start_point_branch.id))
            .load::<QueryVersion>(conn)?;

        for version in versions {
            let version_uuid = Uuid::new_v4();
            let insert_version = InsertVersion {
                uuid: version_uuid.to_string(),
                branch_id,
                number: version.number,
                hash: version.hash,
            };

            diesel::insert_into(schema::version::table)
                .values(&insert_version)
                .execute(conn)
                .map_err(api_error!())?;

            let version_id = QueryVersion::get_id(conn, &version_uuid)?;

            // Get all the reports for that version
            let reports = schema::report::table
                .filter(schema::report::version_id.eq(version.id))
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

                let report_uuid = Uuid::new_v4();
                let insert_report = InsertReport {
                    uuid: report_uuid.to_string(),
                    user_id,
                    version_id,
                    testbed_id,
                    adapter,
                    start_time,
                    end_time,
                };

                diesel::insert_into(schema::report::table)
                    .values(&insert_report)
                    .execute(conn)
                    .map_err(api_error!())?;

                let report_id = QueryReport::get_id(conn, &report_uuid)?;

                // Get all perfs for the report
                let perfs = schema::perf::table
                    .filter(schema::perf::report_id.eq(report_id))
                    .load::<QueryPerf>(conn)?;

                for perf in perfs {
                    let QueryPerf {
                        // uuid,
                        //  report_id: i32,
                        iteration,
                        benchmark_id,
                        ..
                    } = perf;

                    let perf_uuid = Uuid::new_v4();
                    let insert_perf = InsertPerf {
                        uuid: perf_uuid.to_string(),
                        report_id,
                        iteration,
                        benchmark_id,
                    };

                    diesel::insert_into(schema::perf::table)
                        .values(&insert_perf)
                        .execute(conn)
                        .map_err(api_error!())?;

                    let perf_id = QueryPerf::get_id(conn, &perf_uuid)?;

                    // Get all metrics for the perf
                    let metrics = schema::metric::table
                        .filter(schema::metric::perf_id.eq(perf_id))
                        .load::<QueryMetric>(conn)?;

                    for metric in metrics {
                        let QueryMetric {
                            metric_kind_id,
                            value,
                            lower_bound,
                            upper_bound,
                            ..
                        } = metric;

                        let metric_uuid = Uuid::new_v4();
                        let insert_metric = InsertMetric {
                            uuid: metric_uuid.to_string(),
                            perf_id,
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
                }
            }
        }

        Ok(())
    }
}
