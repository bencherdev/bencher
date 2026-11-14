use std::collections::BTreeMap;

use bencher_json::project::report::{
    JsonReportWarning, ReportWarningAction, ReportWarningResource,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, upsert::excluded};
use dropshot::HttpError;

use crate::{context::DbConnection, error::resource_not_found_err, macros::sql::min, schema};

use super::ReportId;

/// What one report skipped, tallied by resource and action until it is written.
#[derive(Debug, Default)]
pub struct ReportWarnings(BTreeMap<(ReportWarningResource, ReportWarningAction), u32>);

impl ReportWarnings {
    pub fn skip(&mut self, resource: ReportWarningResource) {
        self.skip_with_count(resource, 1);
    }

    pub fn skip_with_count(&mut self, resource: ReportWarningResource, count: usize) {
        self.add(resource, ReportWarningAction::Skip, count);
    }

    pub fn ignore_with_count(&mut self, resource: ReportWarningResource, count: usize) {
        self.add(resource, ReportWarningAction::Ignore, count);
    }

    fn add(&mut self, resource: ReportWarningResource, action: ReportWarningAction, count: usize) {
        if count == 0 {
            return;
        }
        let tally = self.0.entry((resource, action)).or_default();
        *tally = tally.saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Write the counts, adding to any the report already holds: a job's report holds what its
    /// run skipped before its results are processed.
    pub fn insert(&self, conn: &mut DbConnection, report_id: ReportId) -> diesel::QueryResult<()> {
        let rows = self
            .0
            .iter()
            .map(|(&(resource, action), &count)| {
                (
                    schema::report_warning::report_id.eq(report_id),
                    schema::report_warning::resource.eq(resource),
                    schema::report_warning::action.eq(action),
                    schema::report_warning::count.eq(i32::try_from(count).unwrap_or(i32::MAX)),
                )
            })
            .collect::<Vec<_>>();
        diesel::insert_into(schema::report_warning::table)
            .values(&rows)
            .on_conflict((
                schema::report_warning::report_id,
                schema::report_warning::resource,
                schema::report_warning::action,
            ))
            .do_update()
            .set(schema::report_warning::count.eq(min(
                schema::report_warning::count + excluded(schema::report_warning::count),
                i32::MAX,
            )))
            .execute(conn)?;
        Ok(())
    }
}

pub fn get_report_warnings(
    conn: &mut DbConnection,
    report_id: ReportId,
) -> Result<Option<Vec<JsonReportWarning>>, HttpError> {
    schema::report_warning::table
        .filter(schema::report_warning::report_id.eq(report_id))
        .order((
            schema::report_warning::resource,
            schema::report_warning::action,
        ))
        .select((
            schema::report_warning::resource,
            schema::report_warning::action,
            schema::report_warning::count,
        ))
        .load::<(ReportWarningResource, ReportWarningAction, i32)>(conn)
        .map(|warnings| {
            (!warnings.is_empty()).then(|| {
                warnings
                    .into_iter()
                    .map(|(resource, action, count)| JsonReportWarning {
                        resource,
                        action,
                        count: u32::try_from(count).unwrap_or_default(),
                    })
                    .collect()
            })
        })
        .map_err(resource_not_found_err!(Report, report_id))
}

#[cfg(test)]
mod tests {
    use bencher_json::project::report::{
        JsonReportWarning, ReportWarningAction, ReportWarningResource,
    };
    use diesel::SqliteConnection;

    use super::{ReportWarnings, get_report_warnings};
    use crate::{
        model::project::report::ReportId,
        test_util::{
            create_base_entities, create_branch_with_head, create_head_version, create_report,
            create_testbed, create_version, setup_test_db,
        },
    };

    /// A job's report holds what its run skipped before its results are processed, so the second
    /// write adds to each count the report already holds.
    ///
    /// Kills: a second write that inserts a count the report already holds, which the table's
    /// unique key refuses.
    #[test]
    fn insert_adds_to_the_counts_a_report_holds() {
        let mut conn = setup_test_db();
        let report_id = report(&mut conn);

        let mut run = ReportWarnings::default();
        run.skip(ReportWarningResource::Threshold);
        run.skip_with_count(ReportWarningResource::Benchmark, 2);
        run.insert(&mut conn, report_id)
            .expect("Failed to write the run's warnings");
        let mut results = ReportWarnings::default();
        results.skip_with_count(ReportWarningResource::Threshold, 3);
        results
            .insert(&mut conn, report_id)
            .expect("Failed to write the results' warnings");

        assert_eq!(
            get_report_warnings(&mut conn, report_id).expect("Failed to read the warnings"),
            Some(vec![
                skipped(ReportWarningResource::Benchmark, 2),
                skipped(ReportWarningResource::Threshold, 4),
            ])
        );
    }

    /// A sum past `i32::MAX` is stored as `i32::MAX`, since a larger value reads back truncated.
    ///
    /// Kills: a sum written without the cap.
    #[test]
    fn insert_caps_the_sum_at_i32_max() {
        let mut conn = setup_test_db();
        let report_id = report(&mut conn);

        let max = usize::try_from(i32::MAX).expect("i32::MAX fits a usize");
        let mut warnings = ReportWarnings::default();
        warnings.skip_with_count(ReportWarningResource::Threshold, max);
        for _ in 0..2 {
            warnings
                .insert(&mut conn, report_id)
                .expect("Failed to write the warnings");
        }

        let max = u32::try_from(i32::MAX).expect("i32::MAX fits a u32");
        assert_eq!(
            get_report_warnings(&mut conn, report_id).expect("Failed to read the warnings"),
            Some(vec![skipped(ReportWarningResource::Threshold, max)])
        );
    }

    fn skipped(resource: ReportWarningResource, count: u32) -> JsonReportWarning {
        JsonReportWarning {
            resource,
            action: ReportWarningAction::Skip,
            count,
        }
    }

    /// A report on a project of its own, to hold warnings.
    fn report(conn: &mut SqliteConnection) -> ReportId {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );
        let testbed_id = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            None,
        );
        create_head_version(conn, branch.head_id, version_id);
        create_report(
            conn,
            "00000000-0000-0000-0000-000000000050",
            base.project_id,
            branch.head_id,
            version_id,
            testbed_id,
        )
    }
}
