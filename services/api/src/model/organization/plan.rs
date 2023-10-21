use crate::schema::plan as plan_table;
use bencher_json::{DateTime, LicensedPlanId, MeteredPlanId};

use super::{OrganizationId, QueryOrganization};

crate::util::typed_id::typed_id!(PlanId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plan_table)]
#[diesel(belongs_to(QueryOrganization, foreign_key = organization_id))]
pub struct QueryPlan {
    pub id: PlanId,
    pub organization_id: OrganizationId,
    pub metered_plan: MeteredPlanId,
    pub licensed_plan: LicensedPlanId,
    pub created: DateTime,
    pub modified: DateTime,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plan_table)]
pub struct InsertPlan {
    pub organization_id: OrganizationId,
    pub metered_plan: MeteredPlanId,
    pub licensed_plan: LicensedPlanId,
    pub created: DateTime,
    pub modified: DateTime,
}
