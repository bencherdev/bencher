use bencher_json::{
    project::plot::{JsonUpdatePlot, XAxis},
    DateTime, PlotUuid, ResourceName, TestbedUuid, Window,
};

use super::{ProjectId, QueryProject};
use crate::schema::plot as plot_table;

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
    pub rank: i64,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub window: Window,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryPlot {}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plot_table)]
#[allow(clippy::struct_excessive_bools)]
pub struct InsertPlot {
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub rank: i64,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub window: Window,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertPlot {}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = plot_table)]
pub struct UpdatePlot {
    pub name: Option<ResourceName>,
    pub modified: DateTime,
}

impl From<JsonUpdatePlot> for UpdatePlot {
    fn from(update: JsonUpdatePlot) -> Self {
        let JsonUpdatePlot { name } = update;
        Self {
            name,
            modified: DateTime::now(),
        }
    }
}
