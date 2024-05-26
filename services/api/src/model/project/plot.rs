use bencher_json::{
    project::plot::{JsonUpdatePlot, XAxis},
    DateTime, JsonNewPlot, PlotUuid, ResourceName, Window,
};
use bencher_rank::{Rank, Ranked};
use diesel::{
    BelongingToDsl, BoolExpressionMethods, ExpressionMethods, JoinOnDsl, NullableExpressionMethods,
    QueryDsl, RunQueryDsl, SelectableHelper, TextExpressionMethods,
};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
use crate::{context::DbConnection, error::resource_not_found_err, schema::plot as plot_table};

crate::util::typed_id::typed_id!(PlotId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plot_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
#[allow(clippy::struct_excessive_bools)]
pub struct QueryPlot {
    pub id: PlotId,
    pub uuid: PlotUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub rank: Rank,
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
    pub fn for_project(
        conn: &mut DbConnection,
        query_project: &QueryProject,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_project)
            .order(plot_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(Plot, &query_project))
    }
}

impl Ranked for QueryPlot {
    fn rank(&self) -> Rank {
        self.rank
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plot_table)]
#[allow(clippy::struct_excessive_bools)]
pub struct InsertPlot {
    pub uuid: PlotUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub rank: Rank,
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
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        plot: JsonNewPlot,
    ) -> Result<Self, HttpError> {
        let JsonNewPlot {
            name,
            index,
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
        let Some(rank) = Rank::calculate::<QueryPlot>(&[], index.unwrap_or_default().into()) else {
            todo!();
        };
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: PlotUuid::new(),
            project_id,
            name,
            rank,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            window,
            created: timestamp,
            modified: timestamp,
        })
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = plot_table)]
pub struct UpdatePlot {
    pub name: Option<ResourceName>,
    pub rank: Option<Rank>,
    pub modified: DateTime,
}

impl From<JsonUpdatePlot> for UpdatePlot {
    fn from(update: JsonUpdatePlot) -> Self {
        let JsonUpdatePlot { name, rank } = update;
        Self {
            name,
            rank: rank.map(Into::into),
            modified: DateTime::now(),
        }
    }
}
