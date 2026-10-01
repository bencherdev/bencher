use std::collections::BTreeMap;

use bencher_json::project::report::{
    JsonReportWarning, ReportWarningAction, ReportWarningResource,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{context::DbConnection, error::resource_not_found_err, schema};

use super::ReportId;

/// What one report skipped, tallied by resource and action until it is written.
#[derive(Debug, Default)]
pub struct ReportWarnings(BTreeMap<(ReportWarningResource, ReportWarningAction), u32>);

impl ReportWarnings {
    pub fn skip(&mut self, resource: ReportWarningResource) {
        self.skip_with_count(resource, 1);
    }

    pub fn skip_with_count(&mut self, resource: ReportWarningResource, count: usize) {
        if count == 0 {
            return;
        }
        let tally = self
            .0
            .entry((resource, ReportWarningAction::Skip))
            .or_default();
        *tally = tally.saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

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
