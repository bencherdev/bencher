use crate::schema::plan as plan_table;
use bencher_json::{DateTime, Jwt, LicensePlanId, MeteredPlanId};
use dropshot::HttpError;

use super::{OrganizationId, QueryOrganization};

crate::util::typed_id::typed_id!(PlanId);

#[derive(diesel::Queryable)]
pub struct QueryPlan {
    pub id: PlanId,
    pub organization_id: OrganizationId,
    pub metered_plan: MeteredPlanId,
    pub license_plan: LicensePlanId,
    pub created: DateTime,
    pub modified: DateTime,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plan_table)]
pub struct InsertPlan {
    pub organization_id: OrganizationId,
    pub metered_plan: MeteredPlanId,
    pub license_plan: LicensePlanId,
    pub created: DateTime,
    pub modified: DateTime,
}
