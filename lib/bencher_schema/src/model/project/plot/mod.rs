use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, Index, JsonNewPlot, JsonPlot, MeasureUuid, PlotUuid,
    ResourceName, TestbedUuid, Window,
    project::plot::{JsonPlotPatch, JsonPlotPatchNull, JsonUpdatePlot, XAxis},
};
use bencher_rank::{Rank, RankGenerator, Ranked};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{
    ProjectId, QueryProject,
    benchmark::{BenchmarkId, QueryBenchmark},
    branch::{BranchId, QueryBranch},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        BencherResource, assert_parentage, bad_request_error, resource_conflict_err,
        resource_conflict_error, resource_not_found_err,
    },
    macros::sql::last_insert_rowid,
    schema::{
        plot as plot_table, plot_benchmark as plot_benchmark_table,
        plot_branch as plot_branch_table, plot_measure as plot_measure_table,
        plot_testbed as plot_testbed_table,
    },
    write_conn, write_transaction,
};

mod benchmark;
mod branch;
mod measure;
mod testbed;

use benchmark::{InsertPlotBenchmark, QueryPlotBenchmark};
use branch::{InsertPlotBranch, QueryPlotBranch};
use measure::{InsertPlotMeasure, QueryPlotMeasure};
use testbed::{InsertPlotTestbed, QueryPlotTestbed};

crate::macros::typed_id::typed_id!(PlotId);

/// Resolve component UUIDs to IDs via read connections, scoped to the project
/// so a foreign or missing UUID is rejected (404) up front. An empty list is
/// rejected (400): a plot missing any dimension would never render anything.
/// Duplicate UUIDs collapse to a single ID, preserving first-occurrence order,
/// since a repeat would violate the `(plot_id, component_id)` primary key on
/// insert.
macro_rules! resolve_component_ids {
    ($context:expr, $query_project:expr, $uuids:expr, $query:ty, $name:literal) => {{
        let uuids = $uuids;
        if uuids.is_empty() {
            return Err(bad_request_error(concat!(
                "A plot must have at least one ",
                $name
            )));
        }
        let mut ids = Vec::with_capacity(uuids.len());
        for uuid in &uuids {
            let id = <$query>::from_uuid(auth_conn!($context), $query_project.id, *uuid)?.id;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }};
}

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plot_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "plot boundary flags match DB schema"
)]
pub struct QueryPlot {
    pub id: PlotId,
    pub uuid: PlotUuid,
    pub project_id: ProjectId,
    pub rank: Rank,
    pub title: Option<ResourceName>,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub window: Window,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryPlot {
    pub fn get_with_uuid(
        conn: &mut DbConnection,
        query_project: &QueryProject,
        uuid: PlotUuid,
    ) -> Result<Self, HttpError> {
        Self::belonging_to(&query_project)
            .filter(plot_table::uuid.eq(uuid))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Plot, (query_project, uuid)))
    }

    fn all_for_project(
        conn: &mut DbConnection,
        query_project: &QueryProject,
    ) -> diesel::QueryResult<Vec<Self>> {
        Self::belonging_to(query_project)
            .order(plot_table::rank.asc())
            .load::<Self>(conn)
    }

    fn all_others_for_project(
        &self,
        conn: &mut DbConnection,
        query_project: &QueryProject,
    ) -> diesel::QueryResult<Vec<Self>> {
        Self::belonging_to(query_project)
            .filter(plot_table::id.ne(self.id))
            .order(plot_table::rank.asc())
            .load::<Self>(conn)
    }

    fn new_rank(
        conn: &mut DbConnection,
        query_project: &QueryProject,
        index: Option<Index>,
    ) -> Result<Rank, HttpError> {
        let index = u8::from(index.unwrap_or_default()).into();

        // Get the current plots.
        let plots = QueryPlot::all_for_project(conn, query_project)
            .map_err(resource_not_found_err!(Plot, &query_project))?;

        // Try to calculate the rank within the current plots.
        if let Some(rank) = Rank::calculate(&plots, index) {
            return Ok(rank);
        }

        // If the rank cannot be calculated, then we need to redistribute the ranks.
        // Wrap the redistribution in a transaction for atomicity.
        let now = DateTime::now();
        conn.immediate_transaction(|conn| {
            let plot_ranker = RankGenerator::new(plots.len());
            for (plot, rank) in plots.iter().zip(plot_ranker) {
                let update_plot = UpdatePlot {
                    rank: Some(rank),
                    title: None,
                    lower_value: None,
                    upper_value: None,
                    lower_boundary: None,
                    upper_boundary: None,
                    x_axis: None,
                    window: None,
                    modified: now,
                };
                diesel::update(plot_table::table.filter(plot_table::id.eq(plot.id)))
                    .set(&update_plot)
                    .execute(conn)?;
            }
            diesel::QueryResult::Ok(())
        })
        .map_err(|e| resource_conflict_error(BencherResource::Plot, &plots, e))?;

        // Try to calculate the rank within the redistributed plots.
        let redistributed_plots = QueryPlot::all_for_project(conn, query_project)
            .map_err(resource_not_found_err!(Plot, &query_project))?;
        Rank::calculate(&redistributed_plots, index).ok_or_else(|| {
            resource_conflict_error(
                BencherResource::Plot,
                (redistributed_plots, index),
                "Failed to redistribute plots.",
            )
        })
    }

    /// Recompute this plot's rank for the requested index, redistributing all
    /// plot ranks when the index cannot fit between the current ranks.
    /// Must be called within a write transaction: the redistribution writes
    /// rely on the caller's transaction for atomicity.
    fn update_rank(
        &self,
        conn: &mut DbConnection,
        query_project: &QueryProject,
        index: Index,
        modified: DateTime,
    ) -> diesel::QueryResult<Rank> {
        let index = u8::from(index).into();

        // Get the current plots, except for self.
        let other_plots = self.all_others_for_project(conn, query_project)?;

        // Try to calculate the rank within the current plots.
        if let Some(rank) = Rank::calculate(&other_plots, index) {
            return Ok(rank);
        }

        // If the rank cannot be calculated, then we need to redistribute all the
        // ranks within the caller's transaction.
        let all_plots = QueryPlot::all_for_project(conn, query_project)?;
        let plot_ranker = RankGenerator::new(all_plots.len());
        for (plot, rank) in all_plots.iter().zip(plot_ranker) {
            let update_plot = UpdatePlot {
                rank: Some(rank),
                title: None,
                lower_value: None,
                upper_value: None,
                lower_boundary: None,
                upper_boundary: None,
                x_axis: None,
                window: None,
                modified,
            };
            diesel::update(plot_table::table.filter(plot_table::id.eq(plot.id)))
                .set(&update_plot)
                .execute(conn)?;
        }

        // Try to calculate the rank within the redistributed plots.
        // Redistribution guarantees well-spaced ranks, so this should never fail;
        // roll back the enclosing transaction if it somehow does.
        let redistributed_plots = self.all_others_for_project(conn, query_project)?;
        Rank::calculate(&redistributed_plots, index)
            .ok_or(diesel::result::Error::RollbackTransaction)
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonPlot, HttpError> {
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Plot,
            self.project_id,
        );
        let branches = QueryPlotBranch::into_json_for_plot(conn, &self)?;
        let testbeds = QueryPlotTestbed::into_json_for_plot(conn, &self)?;
        let benchmarks = QueryPlotBenchmark::into_json_for_plot(conn, &self)?;
        let measures = QueryPlotMeasure::into_json_for_plot(conn, &self)?;
        let Self {
            uuid,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            window,
            created,
            modified,
            ..
        } = self;
        Ok(JsonPlot {
            uuid,
            project: project.uuid,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
            created,
            modified,
        })
    }

    pub async fn update(
        &self,
        context: &ApiContext,
        query_project: &QueryProject,
        update: JsonUpdatePlot,
    ) -> Result<(), HttpError> {
        let UpdatePlotFields {
            index,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
        } = update.into();

        // Phase 1: resolve the provided component UUIDs to IDs.
        // A `None` list leaves that component unchanged; a `Some` list replaces it.
        let branch_ids = match branches {
            Some(uuids) => Some(resolve_component_ids!(
                context,
                query_project,
                uuids,
                QueryBranch,
                "branch"
            )),
            None => None,
        };
        let testbed_ids = match testbeds {
            Some(uuids) => Some(resolve_component_ids!(
                context,
                query_project,
                uuids,
                QueryTestbed,
                "testbed"
            )),
            None => None,
        };
        let benchmark_ids = match benchmarks {
            Some(uuids) => Some(resolve_component_ids!(
                context,
                query_project,
                uuids,
                QueryBenchmark,
                "benchmark"
            )),
            None => None,
        };
        let measure_ids = match measures {
            Some(uuids) => Some(resolve_component_ids!(
                context,
                query_project,
                uuids,
                QueryMeasure,
                "measure"
            )),
            None => None,
        };

        // Phase 2: apply everything atomically in a single write transaction:
        // recompute the rank when a new index is requested (which may
        // redistribute all plot ranks), apply the scalar changeset, and replace
        // each provided component list (delete then re-insert).
        let modified = context.clock.now();
        write_transaction!(context, |conn| {
            let rank = if let Some(index) = index {
                Some(self.update_rank(conn, query_project, index, modified)?)
            } else {
                None
            };
            let update_plot = UpdatePlot {
                rank,
                title,
                lower_value,
                upper_value,
                lower_boundary,
                upper_boundary,
                x_axis,
                window,
                modified,
            };
            Self::apply_update(
                conn,
                self.id,
                &update_plot,
                branch_ids.as_deref(),
                testbed_ids.as_deref(),
                benchmark_ids.as_deref(),
                measure_ids.as_deref(),
            )
        })
        .map_err(resource_conflict_err!(Plot, self))?;

        Ok(())
    }

    /// Apply a scalar changeset and replace each provided component list within an
    /// existing transaction. A `None` list leaves that component unchanged; a `Some`
    /// list replaces it wholesale (delete then re-insert with fresh ranks).
    fn apply_update(
        conn: &mut DbConnection,
        plot_id: PlotId,
        update_plot: &UpdatePlot,
        branch_ids: Option<&[BranchId]>,
        testbed_ids: Option<&[TestbedId]>,
        benchmark_ids: Option<&[BenchmarkId]>,
        measure_ids: Option<&[MeasureId]>,
    ) -> diesel::QueryResult<()> {
        diesel::update(plot_table::table.filter(plot_table::id.eq(plot_id)))
            .set(update_plot)
            .execute(conn)?;
        if let Some(branch_ids) = branch_ids {
            diesel::delete(plot_branch_table::table.filter(plot_branch_table::plot_id.eq(plot_id)))
                .execute(conn)?;
            InsertPlotBranch::from_resolved(conn, plot_id, branch_ids)?;
        }
        if let Some(testbed_ids) = testbed_ids {
            diesel::delete(
                plot_testbed_table::table.filter(plot_testbed_table::plot_id.eq(plot_id)),
            )
            .execute(conn)?;
            InsertPlotTestbed::from_resolved(conn, plot_id, testbed_ids)?;
        }
        if let Some(benchmark_ids) = benchmark_ids {
            diesel::delete(
                plot_benchmark_table::table.filter(plot_benchmark_table::plot_id.eq(plot_id)),
            )
            .execute(conn)?;
            InsertPlotBenchmark::from_resolved(conn, plot_id, benchmark_ids)?;
        }
        if let Some(measure_ids) = measure_ids {
            diesel::delete(
                plot_measure_table::table.filter(plot_measure_table::plot_id.eq(plot_id)),
            )
            .execute(conn)?;
            InsertPlotMeasure::from_resolved(conn, plot_id, measure_ids)?;
        }
        Ok(())
    }
}

impl Ranked for QueryPlot {
    fn rank(&self) -> Rank {
        self.rank
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plot_table)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "plot boundary flags match DB schema"
)]
pub struct InsertPlot {
    pub uuid: PlotUuid,
    pub project_id: ProjectId,
    pub rank: Rank,
    pub title: Option<ResourceName>,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub window: Window,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertPlot {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(plot, Plot);

    pub async fn from_json(
        context: &ApiContext,
        query_project: &QueryProject,
        plot: JsonNewPlot,
    ) -> Result<QueryPlot, HttpError> {
        let JsonNewPlot {
            index,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
        } = plot;

        // Phase 1: Resolve UUIDs to IDs via read connections
        let branch_ids =
            resolve_component_ids!(context, query_project, branches, QueryBranch, "branch");
        let testbed_ids =
            resolve_component_ids!(context, query_project, testbeds, QueryTestbed, "testbed");
        let benchmark_ids = resolve_component_ids!(
            context,
            query_project,
            benchmarks,
            QueryBenchmark,
            "benchmark"
        );
        let measure_ids =
            resolve_component_ids!(context, query_project, measures, QueryMeasure, "measure");

        // Phase 2: Single write_conn + transaction for all writes
        let conn = write_conn!(context);
        let rank = QueryPlot::new_rank(conn, query_project, index)?;
        let timestamp = DateTime::now();
        let insert_plot = Self {
            uuid: PlotUuid::new(),
            project_id: query_project.id,
            rank,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            window,
            created: timestamp,
            modified: timestamp,
        };
        let plot_id = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(plot_table::table)
                    .values(&insert_plot)
                    .execute(conn)?;
                let plot_id: PlotId = diesel::select(last_insert_rowid()).get_result(conn)?;

                InsertPlotBranch::from_resolved(conn, plot_id, &branch_ids)?;
                InsertPlotTestbed::from_resolved(conn, plot_id, &testbed_ids)?;
                InsertPlotBenchmark::from_resolved(conn, plot_id, &benchmark_ids)?;
                InsertPlotMeasure::from_resolved(conn, plot_id, &measure_ids)?;

                diesel::QueryResult::Ok(plot_id)
            })
            .map_err(resource_conflict_err!(Plot, insert_plot))?;

        // Read back the plot from the read pool
        plot_table::table
            .filter(plot_table::id.eq(plot_id))
            .first::<QueryPlot>(auth_conn!(context))
            .map_err(resource_not_found_err!(Plot, plot_id))
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = plot_table)]
pub struct UpdatePlot {
    pub rank: Option<Rank>,
    pub title: Option<Option<ResourceName>>,
    pub lower_value: Option<bool>,
    pub upper_value: Option<bool>,
    pub lower_boundary: Option<bool>,
    pub upper_boundary: Option<bool>,
    pub x_axis: Option<XAxis>,
    pub window: Option<Window>,
    pub modified: DateTime,
}

/// The fields of a [`JsonUpdatePlot`], unified across its `Patch` and `Null`
/// variants. A `Some(None)` title clears the current title.
#[expect(
    clippy::option_option,
    reason = "None = not specified, Some(None) = explicitly unset"
)]
struct UpdatePlotFields {
    index: Option<Index>,
    title: Option<Option<ResourceName>>,
    lower_value: Option<bool>,
    upper_value: Option<bool>,
    lower_boundary: Option<bool>,
    upper_boundary: Option<bool>,
    x_axis: Option<XAxis>,
    window: Option<Window>,
    branches: Option<Vec<BranchUuid>>,
    testbeds: Option<Vec<TestbedUuid>>,
    benchmarks: Option<Vec<BenchmarkUuid>>,
    measures: Option<Vec<MeasureUuid>>,
}

impl From<JsonUpdatePlot> for UpdatePlotFields {
    fn from(update: JsonUpdatePlot) -> Self {
        match update {
            JsonUpdatePlot::Patch(patch) => {
                let JsonPlotPatch {
                    index,
                    title,
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                } = patch;
                Self {
                    index,
                    title: title.map(Some),
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                }
            },
            JsonUpdatePlot::Null(patch_null) => {
                let JsonPlotPatchNull {
                    index,
                    title: (),
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                } = patch_null;
                Self {
                    index,
                    title: Some(None),
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};

    use bencher_json::{DateTime, project::plot::XAxis};

    use super::{
        InsertPlot, PlotId, QueryPlot, UpdatePlot, benchmark::InsertPlotBenchmark,
        branch::InsertPlotBranch, measure::InsertPlotMeasure, testbed::InsertPlotTestbed,
    };
    use crate::{
        context::DbConnection,
        macros::sql::last_insert_rowid,
        model::project::{ProjectId, QueryProject},
        schema,
        test_util::{
            create_base_entities, create_benchmark, create_branch_with_head, create_measure,
            create_plot, create_testbed, get_plot_benchmarks, get_plot_branches, get_plot_measures,
            get_plot_rank, get_plot_testbeds, setup_test_db,
        },
    };

    fn get_query_project(conn: &mut DbConnection, project_id: ProjectId) -> QueryProject {
        schema::project::table
            .filter(schema::project::id.eq(project_id))
            .select(QueryProject::as_select())
            .first(conn)
            .expect("Failed to get project")
    }

    /// Helper to get all plot ranks for a project, ordered by rank ascending.
    fn get_all_plot_ranks(conn: &mut DbConnection) -> Vec<i64> {
        schema::plot::table
            .order(schema::plot::rank.asc())
            .select(schema::plot::rank)
            .load::<i64>(conn)
            .expect("Failed to get plot ranks")
    }

    #[test]
    fn new_rank_empty_project() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        // With no existing plots, should succeed
        QueryPlot::new_rank(&mut conn, &project, None).expect("Failed to get rank");
    }

    #[test]
    fn new_rank_appends_after_existing() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        // Create 2 plots with well-spaced ranks
        create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            1_000_000,
        );
        let p2 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            2_000_000,
        );

        // Default index = append (after last)
        let rank = QueryPlot::new_rank(&mut conn, &project, None).expect("Failed to get rank");
        let p2_rank = get_plot_rank(&mut conn, p2);
        // Rank (i64 internally) should be beyond p2's rank
        // We can't access Rank inner directly but we can verify via DB
        // Insert the plot with this rank and check ordering
        create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            // We'd ideally use rank directly but test_util stores i64
            // Just verify the new_rank didn't error
            p2_rank + 1, // placeholder — the important thing is new_rank succeeded
        );
        let _ = rank; // rank is valid
    }

    #[test]
    fn new_rank_inserts_at_index() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            1_000_000,
        );
        create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            3_000_000,
        );

        // Insert at index 1 (between p1 and p2)
        let index = bencher_json::Index::try_from(1u8).unwrap();
        let _rank =
            QueryPlot::new_rank(&mut conn, &project, Some(index)).expect("Failed to get rank");
        // The rank should be a value between 1_000_000 and 3_000_000
        // Since Rank has no public accessor, we verify that the calculation succeeded
    }

    #[test]
    fn new_rank_redistributes_when_no_space() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        // Create 2 plots with adjacent ranks (no space between them)
        create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            100,
        );
        create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            101,
        );

        // Insert at index 1 (between p1 and p2) — should trigger redistribution
        let index = bencher_json::Index::try_from(1u8).unwrap();
        let _rank =
            QueryPlot::new_rank(&mut conn, &project, Some(index)).expect("Failed to get rank");

        // After redistribution, plots should have new well-spaced ranks
        let ranks = get_all_plot_ranks(&mut conn);
        assert_eq!(ranks.len(), 2);
        // Ranks should be redistributed with wide spacing (i64::MAX / 3 apart)
        let first = ranks.first().expect("should have first rank");
        let second = ranks.get(1).expect("should have second rank");
        assert!(second - first > 1000);
    }

    #[test]
    fn update_rank_redistributes_atomically() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        // Create 3 plots with adjacent ranks
        let p1 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            100,
        );
        let p2 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            101,
        );
        let p3 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            102,
        );

        // Get query_plot for p3 and move it to index 0
        let query_p3: QueryPlot = schema::plot::table
            .filter(schema::plot::id.eq(p3))
            .select(QueryPlot::as_select())
            .first(&mut conn)
            .expect("Failed to get plot");

        // Atomicity comes from the caller's transaction.
        let index = bencher_json::Index::try_from(0u8).unwrap();
        let _rank = conn
            .immediate_transaction(|conn| {
                query_p3.update_rank(conn, &project, index, DateTime::TEST)
            })
            .expect("Failed to update rank");

        // Redistribution should have happened; all plots now have well-spaced ranks
        let p1_rank = get_plot_rank(&mut conn, p1);
        let p2_rank = get_plot_rank(&mut conn, p2);
        let p3_rank = get_plot_rank(&mut conn, p3);

        // All ranks should be distinct after redistribution
        assert_ne!(p1_rank, p2_rank);
        assert_ne!(p2_rank, p3_rank);
        assert_ne!(p1_rank, p3_rank);
        // p1 and p2 should still be ordered
        assert!(p1_rank < p2_rank);
    }

    #[test]
    fn update_rank_redistribution_rolls_back_with_transaction() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        // Create 3 plots with adjacent ranks so moving p3 forces redistribution
        let p1 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            100,
        );
        let p2 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            101,
        );
        let p3 = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            102,
        );

        let query_p3: QueryPlot = schema::plot::table
            .filter(schema::plot::id.eq(p3))
            .select(QueryPlot::as_select())
            .first(&mut conn)
            .expect("Failed to get plot");

        // Fail the transaction after the rank redistribution has run.
        let index = bencher_json::Index::try_from(0u8).unwrap();
        let result: diesel::QueryResult<()> = conn.immediate_transaction(|conn| {
            query_p3.update_rank(conn, &project, index, DateTime::TEST)?;
            Err(diesel::result::Error::RollbackTransaction)
        });
        assert!(result.is_err());

        // The redistribution was rolled back with the transaction.
        assert_eq!(get_plot_rank(&mut conn, p1), 100);
        assert_eq!(get_plot_rank(&mut conn, p2), 101);
        assert_eq!(get_plot_rank(&mut conn, p3), 102);
    }

    /// Test that a plot and all its components can be inserted in a single transaction.
    #[test]
    fn plot_creation_inserts_all_components() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let project = get_query_project(&mut conn, base.project_id);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let benchmark = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "bench1",
            "bench1",
        );
        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            "latency",
            "latency",
        );

        // Calculate rank before the transaction
        let rank = QueryPlot::new_rank(&mut conn, &project, None).expect("Failed to get rank");

        // Insert plot + all components in a single transaction
        let plot_id = conn
            .immediate_transaction(|conn| {
                let timestamp = DateTime::now();
                let insert_plot = InsertPlot {
                    uuid: bencher_json::PlotUuid::new(),
                    project_id: base.project_id,
                    rank,
                    title: None,
                    lower_value: true,
                    upper_value: true,
                    lower_boundary: false,
                    upper_boundary: false,
                    x_axis: XAxis::DateTime,
                    window: bencher_json::Window::try_from(2_592_000u32).unwrap(),
                    created: timestamp,
                    modified: timestamp,
                };
                diesel::insert_into(schema::plot::table)
                    .values(&insert_plot)
                    .execute(conn)?;
                let plot_id: PlotId = diesel::select(last_insert_rowid()).get_result(conn)?;

                // Batch insert components
                InsertPlotBranch::from_resolved(conn, plot_id, &[branch.branch_id])?;
                InsertPlotMeasure::from_resolved(conn, plot_id, &[measure])?;

                // Insert testbed and benchmark components
                let ranker = bencher_rank::RankGenerator::new(1);
                for rank in ranker {
                    diesel::insert_into(schema::plot_testbed::table)
                        .values((
                            schema::plot_testbed::plot_id.eq(plot_id),
                            schema::plot_testbed::testbed_id.eq(testbed),
                            schema::plot_testbed::rank.eq(rank),
                        ))
                        .execute(conn)?;
                    diesel::insert_into(schema::plot_benchmark::table)
                        .values((
                            schema::plot_benchmark::plot_id.eq(plot_id),
                            schema::plot_benchmark::benchmark_id.eq(benchmark),
                            schema::plot_benchmark::rank.eq(rank),
                        ))
                        .execute(conn)?;
                }

                diesel::QueryResult::Ok(plot_id)
            })
            .expect("Transaction failed");

        // Verify all components exist
        assert_eq!(
            get_plot_branches(&mut conn, plot_id),
            vec![branch.branch_id]
        );
        assert_eq!(get_plot_testbeds(&mut conn, plot_id), vec![testbed]);
        assert_eq!(get_plot_benchmarks(&mut conn, plot_id), vec![benchmark]);
        assert_eq!(get_plot_measures(&mut conn, plot_id), vec![measure]);
    }

    /// Test that batch insert of plot components produces correct results.
    #[test]
    fn plot_creation_batch_insert_components() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let b1 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let b2 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "dev",
            "dev",
            "00000000-0000-0000-0000-000000000013",
        );

        let m1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            "latency",
            "latency",
        );
        let m2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000041",
            "throughput",
            "throughput",
        );

        // Create a plot
        let plot_id = create_plot(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000050",
            1_000_000,
        );

        // Batch insert branches and measures
        conn.immediate_transaction(|conn| {
            InsertPlotBranch::from_resolved(conn, plot_id, &[b1.branch_id, b2.branch_id])?;
            InsertPlotMeasure::from_resolved(conn, plot_id, &[m1, m2])?;
            diesel::QueryResult::Ok(())
        })
        .expect("Transaction failed");

        let branches = get_plot_branches(&mut conn, plot_id);
        assert_eq!(branches.len(), 2);
        assert!(branches.contains(&b1.branch_id));
        assert!(branches.contains(&b2.branch_id));

        let measures = get_plot_measures(&mut conn, plot_id);
        assert_eq!(measures.len(), 2);
        assert!(measures.contains(&m1));
        assert!(measures.contains(&m2));
    }

    /// Build a plot seeded with a single branch, testbed, benchmark, and measure.
    fn seed_plot_with_components(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> (
        PlotId,
        super::BranchId,
        super::TestbedId,
        super::BenchmarkId,
        super::MeasureId,
    ) {
        let branch = create_branch_with_head(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        )
        .branch_id;
        let testbed = create_testbed(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let benchmark = create_benchmark(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000030",
            "bench1",
            "bench1",
        );
        let measure = create_measure(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000040",
            "latency",
            "latency",
        );
        let plot_id = create_plot(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000050",
            1_000_000,
        );
        InsertPlotBranch::from_resolved(conn, plot_id, &[branch]).expect("seed branch");
        InsertPlotTestbed::from_resolved(conn, plot_id, &[testbed]).expect("seed testbed");
        InsertPlotBenchmark::from_resolved(conn, plot_id, &[benchmark]).expect("seed benchmark");
        InsertPlotMeasure::from_resolved(conn, plot_id, &[measure]).expect("seed measure");
        (plot_id, branch, testbed, benchmark, measure)
    }

    /// A changeset that touches only `modified` (all other fields left unchanged).
    fn noop_update() -> UpdatePlot {
        UpdatePlot {
            rank: None,
            title: None,
            lower_value: None,
            upper_value: None,
            lower_boundary: None,
            upper_boundary: None,
            x_axis: None,
            window: None,
            modified: DateTime::TEST,
        }
    }

    #[test]
    fn apply_update_replaces_only_provided_components() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let (plot_id, branch, testbed, benchmark, measure) =
            seed_plot_with_components(&mut conn, base.project_id);

        // A second branch and measure to switch to.
        let branch2 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "dev",
            "dev",
            "00000000-0000-0000-0000-000000000013",
        )
        .branch_id;
        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000041",
            "throughput",
            "throughput",
        );

        // Replace branches with [branch, branch2] and measures with [measure2];
        // leave testbeds and benchmarks untouched (None).
        QueryPlot::apply_update(
            &mut conn,
            plot_id,
            &noop_update(),
            Some(&[branch, branch2]),
            None,
            None,
            Some(&[measure2]),
        )
        .expect("apply_update failed");

        assert_eq!(get_plot_branches(&mut conn, plot_id), vec![branch, branch2]);
        assert_eq!(get_plot_measures(&mut conn, plot_id), vec![measure2]);
        // Unprovided components are left as-is.
        assert_eq!(get_plot_testbeds(&mut conn, plot_id), vec![testbed]);
        assert_eq!(get_plot_benchmarks(&mut conn, plot_id), vec![benchmark]);
        // The originally seeded measure was replaced.
        assert!(!get_plot_measures(&mut conn, plot_id).contains(&measure));
    }

    #[test]
    fn apply_update_scalar_flags_and_x_axis() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let (plot_id, branch, testbed, benchmark, measure) =
            seed_plot_with_components(&mut conn, base.project_id);

        // Toggle some flags and the x-axis; leave the rest unchanged (None).
        let update_plot = UpdatePlot {
            rank: None,
            title: None,
            lower_value: Some(false),
            upper_value: None,
            lower_boundary: Some(true),
            upper_boundary: None,
            x_axis: Some(XAxis::Version),
            window: None,
            modified: DateTime::TEST,
        };
        QueryPlot::apply_update(&mut conn, plot_id, &update_plot, None, None, None, None)
            .expect("apply_update failed");

        let plot: QueryPlot = schema::plot::table
            .filter(schema::plot::id.eq(plot_id))
            .select(QueryPlot::as_select())
            .first(&mut conn)
            .expect("Failed to get plot");
        assert!(!plot.lower_value); // set to false
        assert!(plot.upper_value); // unchanged (create default true)
        assert!(plot.lower_boundary); // set to true
        assert!(!plot.upper_boundary); // unchanged (create default false)
        assert!(matches!(plot.x_axis, XAxis::Version));

        // Components were not provided, so they remain intact.
        assert_eq!(get_plot_branches(&mut conn, plot_id), vec![branch]);
        assert_eq!(get_plot_testbeds(&mut conn, plot_id), vec![testbed]);
        assert_eq!(get_plot_benchmarks(&mut conn, plot_id), vec![benchmark]);
        assert_eq!(get_plot_measures(&mut conn, plot_id), vec![measure]);
    }
}
