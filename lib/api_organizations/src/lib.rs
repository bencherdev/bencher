mod allowed;
mod claim;
mod members;
mod organizations;
mod plan;
mod projects;
mod sso;
mod usage;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Organizations
        if http_options {
            api_description.register(organizations::organizations_options)?;
            api_description.register(organizations::organization_options)?;
        }
        api_description.register(organizations::organizations_get)?;
        api_description.register(organizations::organization_post)?;
        api_description.register(organizations::organization_get)?;
        api_description.register(organizations::organization_patch)?;
        api_description.register(organizations::organization_delete)?;

        // Project Claim
        if http_options {
            api_description.register(claim::org_claim_options)?;
        }
        api_description.register(claim::org_claim_post)?;

        // Organization Permission
        if http_options {
            api_description.register(allowed::org_allowed_options)?;
        }
        api_description.register(allowed::org_allowed_get)?;

        // Organization Members
        if http_options {
            api_description.register(members::org_members_options)?;
            api_description.register(members::org_member_options)?;
        }
        api_description.register(members::org_members_get)?;
        api_description.register(members::org_member_post)?;
        api_description.register(members::org_member_get)?;
        api_description.register(members::org_member_patch)?;
        api_description.register(members::org_member_delete)?;

        // Organization Projects
        if http_options {
            api_description.register(projects::org_projects_options)?;
        }
        api_description.register(projects::org_projects_get)?;
        api_description.register(projects::org_project_post)?;

        #[cfg(feature = "plus")]
        {
            // Organization Plan
            // Bencher Cloud only
            if is_bencher_cloud {
                if http_options {
                    api_description.register(plan::org_plan_options)?;
                }
                api_description.register(plan::org_plan_get)?;
                api_description.register(plan::org_plan_post)?;
                api_description.register(plan::org_plan_delete)?;
            }

            // Organization Usage
            if http_options {
                api_description.register(usage::org_usage_options)?;
            }
            api_description.register(usage::org_usage_get)?;

            // Organization SSO
            if http_options {
                api_description.register(sso::org_sso_post_options)?;
                api_description.register(sso::org_sso_delete_options)?;
            }
            api_description.register(sso::org_sso_post)?;
            api_description.register(sso::org_sso_delete)?;
        }

        Ok(())
    }
}
