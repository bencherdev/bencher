use oso::{ClassBuilder, Oso, PolarClass};

pub mod organization;
pub mod project;
pub mod server;
pub mod user;

pub use organization::Organization;
pub use project::Project;
pub use server::Server;
pub use user::User;

const VIEW_PERM: &str = "view";
const CREATE_PERM: &str = "create";
const EDIT_PERM: &str = "edit";
const DELETE_PERM: &str = "delete";
const MANAGE_PERM: &str = "manage";

const VIEW_ROLE_PERM: &str = "view_role";
const CREATE_ROLE_PERM: &str = "create_role";
const EDIT_ROLE_PERM: &str = "edit_role";
const DELETE_ROLE_PERM: &str = "delete_role";

pub const POLAR: &str = include_str!("../bencher.polar");

pub fn init_rbac() -> oso::Result<Oso> {
    let mut oso = Oso::new();
    oso.register_class(User::get_polar_class())?;
    oso.register_class(ClassBuilder::with_constructor(|| Server {}).build())?;
    oso.register_class(
        Organization::get_polar_class_builder()
            .set_constructor(|id| Organization { id })
            .build(),
    )?;
    oso.register_class(Project::get_polar_class())?;
    oso.load_str(POLAR)?;
    Ok(oso)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod test {
    use std::collections::HashMap;

    use super::*;
    use crate::organization::Permission as OrgPerm;
    use crate::organization::Role as OrgRole;
    use crate::project::Permission as ProjPerm;
    use crate::project::Role as ProjRole;
    use crate::server::Permission as SvrPerm;
    use once_cell::sync::Lazy;
    use uuid::Uuid;

    const OSO_ERROR: &str = "Failed to initialize RBAC";

    static OSO: Lazy<Oso> = Lazy::new(|| init_rbac().expect(OSO_ERROR));

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_rbac() {
        let oso = &*OSO;

        let server = Server {};

        let admin = User {
            admin: true,
            locked: false,
            organizations: HashMap::new(),
            projects: HashMap::new(),
        };

        assert!(oso
            .is_allowed(admin.clone(), SvrPerm::Administer, server)
            .unwrap());
        assert!(oso
            .is_allowed(admin.clone(), SvrPerm::Session, server)
            .unwrap());

        let user = User {
            admin: false,
            locked: false,
            organizations: HashMap::new(),
            projects: HashMap::new(),
        };

        assert!(!oso
            .is_allowed(user.clone(), SvrPerm::Administer, server)
            .unwrap());
        assert!(oso
            .is_allowed(user.clone(), SvrPerm::Session, server)
            .unwrap());

        let locked_admin = User {
            admin: true,
            locked: true,
            organizations: HashMap::new(),
            projects: HashMap::new(),
        };

        assert!(!oso
            .is_allowed(locked_admin.clone(), SvrPerm::Administer, server)
            .unwrap());
        assert!(!oso
            .is_allowed(locked_admin, SvrPerm::Session, server)
            .unwrap());

        let locked_user = User {
            admin: false,
            locked: true,
            organizations: HashMap::new(),
            projects: HashMap::new(),
        };

        assert!(!oso
            .is_allowed(locked_user.clone(), SvrPerm::Administer, server)
            .unwrap());
        assert!(!oso
            .is_allowed(locked_user, SvrPerm::Session, server)
            .unwrap());

        let org_id = Uuid::new_v4();
        let proj_id = Uuid::new_v4();

        let org_leader = User {
            admin: false,
            locked: false,
            organizations: literally::hmap! {
                org_id.to_string() => OrgRole::Leader
            },
            projects: HashMap::new(),
        };

        let org_member = User {
            admin: false,
            locked: false,
            organizations: literally::hmap! {
                org_id.to_string() => OrgRole::Member
            },
            projects: HashMap::new(),
        };

        let proj_member = User {
            admin: false,
            locked: false,
            organizations: literally::hmap! {
                org_id.to_string() => OrgRole::Member
            },
            projects: literally::hmap! {
                proj_id.to_string() => ProjRole::Developer
            },
        };

        let org = Organization {
            id: org_id.to_string(),
        };
        let proj = Project {
            id: proj_id.to_string(),
            organization_id: org_id.to_string(),
        };

        assert!(oso
            .is_allowed(admin.clone(), OrgPerm::View, org.clone())
            .unwrap());
        assert!(oso
            .is_allowed(admin.clone(), OrgPerm::Create, org.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(user.clone(), OrgPerm::View, org.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(user.clone(), OrgPerm::Create, org.clone())
            .unwrap());

        assert!(oso
            .is_allowed(org_leader.clone(), OrgPerm::View, org.clone())
            .unwrap());
        assert!(oso
            .is_allowed(org_leader.clone(), OrgPerm::Create, org.clone())
            .unwrap());

        assert!(oso
            .is_allowed(org_member.clone(), OrgPerm::View, org.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(org_member.clone(), OrgPerm::Create, org.clone())
            .unwrap());

        assert!(oso
            .is_allowed(proj_member.clone(), OrgPerm::View, org.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(proj_member.clone(), OrgPerm::Create, org)
            .unwrap());

        assert!(oso
            .is_allowed(admin.clone(), ProjPerm::Create, proj.clone())
            .unwrap());
        assert!(oso
            .is_allowed(admin.clone(), ProjPerm::Manage, proj.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(user.clone(), ProjPerm::Create, proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(user.clone(), ProjPerm::Manage, proj.clone())
            .unwrap());

        assert!(oso
            .is_allowed(org_leader.clone(), ProjPerm::Create, proj.clone())
            .unwrap());
        assert!(oso
            .is_allowed(org_leader.clone(), ProjPerm::Manage, proj.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(org_member.clone(), ProjPerm::Create, proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(org_member.clone(), ProjPerm::Manage, proj.clone())
            .unwrap());

        assert!(oso
            .is_allowed(proj_member.clone(), ProjPerm::Create, proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(proj_member.clone(), ProjPerm::Manage, proj)
            .unwrap());

        let other_org_id = Uuid::new_v4();
        let other_org = Organization {
            id: other_org_id.to_string(),
        };
        let other_proj = Project {
            id: Uuid::new_v4().to_string(),
            organization_id: other_org_id.to_string(),
        };

        assert!(oso
            .is_allowed(admin.clone(), OrgPerm::View, other_org.clone())
            .unwrap());
        assert!(oso
            .is_allowed(admin.clone(), OrgPerm::Create, other_org.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(user.clone(), OrgPerm::View, other_org.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(user.clone(), OrgPerm::Create, other_org.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(org_leader.clone(), OrgPerm::View, other_org.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(org_leader.clone(), OrgPerm::Create, other_org.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(org_member.clone(), OrgPerm::View, other_org.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(org_member.clone(), OrgPerm::Create, other_org)
            .unwrap());

        assert!(oso
            .is_allowed(admin.clone(), ProjPerm::Create, other_proj.clone())
            .unwrap());
        assert!(oso
            .is_allowed(admin, ProjPerm::Manage, other_proj.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(user.clone(), ProjPerm::Create, other_proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(user, ProjPerm::Manage, other_proj.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(org_leader.clone(), ProjPerm::Create, other_proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(org_leader, ProjPerm::Manage, other_proj.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(org_member.clone(), ProjPerm::Create, other_proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(org_member, ProjPerm::Manage, other_proj.clone())
            .unwrap());

        assert!(!oso
            .is_allowed(proj_member.clone(), ProjPerm::Create, other_proj.clone())
            .unwrap());
        assert!(!oso
            .is_allowed(proj_member, ProjPerm::Manage, other_proj)
            .unwrap());
    }
}
