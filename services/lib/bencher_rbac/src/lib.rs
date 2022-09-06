use std::{collections::HashMap, fmt, str::FromStr};

use oso::{ClassBuilder, Oso, PolarClass, PolarValue, ToPolar};
use uuid::Uuid;

pub const POLAR: &str = include_str!("../bencher.polar");

#[derive(Clone, PolarClass)]
struct User {
    #[polar(attribute)]
    pub admin: bool,
    #[polar(attribute)]
    pub locked: bool,
    #[polar(attribute)]
    pub organizations: HashMap<String, OrgRole>,
    #[polar(attribute)]
    pub projects: HashMap<String, ProjRole>,
}

#[derive(Clone, Copy, PolarClass)]
struct Server {}

#[derive(Clone, Copy)]
enum ServerRole {
    Admin,
    User,
    Locked,
}

impl fmt::Display for ServerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Admin => "admin",
                Self::User => "user",
                Self::Locked => "locked",
            }
        )
    }
}

impl ToPolar for ServerRole {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}

#[derive(Clone, PolarClass)]
struct Organization {
    #[polar(attribute)]
    pub uuid: String,
}

#[derive(Clone, Copy)]
enum OrgRole {
    Member,
    Leader,
}

impl fmt::Display for OrgRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Member => "member",
                Self::Leader => "leader",
            }
        )
    }
}

impl ToPolar for OrgRole {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}

#[derive(Clone, PolarClass)]
struct Project {
    #[polar(attribute)]
    pub uuid: String,
    #[polar(attribute)]
    pub parent: String,
}

#[derive(Clone, Copy)]
enum ProjRole {
    Viewer,
    Developer,
    Maintainer,
}

impl fmt::Display for ProjRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Viewer => "viewer",
                Self::Developer => "developer",
                Self::Maintainer => "maintainer",
            }
        )
    }
}

impl ToPolar for ProjRole {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}

#[test]
fn test_user() {
    let mut oso = Oso::new();

    oso.register_class(User::get_polar_class()).unwrap();

    oso.register_class(ClassBuilder::with_constructor(|| Server {}).build())
        .unwrap();

    oso.register_class(
        Organization::get_polar_class_builder()
            .set_constructor(|uuid| Organization { uuid })
            .build(),
    )
    .unwrap();

    oso.register_class(Project::get_polar_class()).unwrap();

    oso.load_str(POLAR).unwrap();

    let server = Server {};

    let admin = User {
        admin: true,
        locked: false,
        organizations: HashMap::new(),
        projects: HashMap::new(),
    };

    assert!(oso.is_allowed(admin.clone(), "administer", server).unwrap());

    let user = User {
        admin: false,
        locked: false,
        organizations: HashMap::new(),
        projects: HashMap::new(),
    };

    assert!(!oso.is_allowed(user.clone(), "administer", server).unwrap());
    assert!(oso.is_allowed(user.clone(), "session", server).unwrap());

    let locked_admin = User {
        admin: true,
        locked: true,
        organizations: HashMap::new(),
        projects: HashMap::new(),
    };

    assert!(!oso
        .is_allowed(locked_admin.clone(), "administer", server)
        .unwrap());
    assert!(!oso.is_allowed(locked_admin, "session", server).unwrap());

    let locked_user = User {
        admin: false,
        locked: true,
        organizations: HashMap::new(),
        projects: HashMap::new(),
    };

    assert!(!oso
        .is_allowed(locked_user.clone(), "administer", server)
        .unwrap());
    assert!(!oso.is_allowed(locked_user, "session", server).unwrap());

    let org_uuid = Uuid::new_v4();
    let proj_uuid = Uuid::new_v4();

    let org_leader = User {
        admin: false,
        locked: false,
        organizations: literally::hmap! {
            org_uuid.to_string() => OrgRole::Leader
        },
        projects: HashMap::new(),
    };

    let org_member = User {
        admin: false,
        locked: false,
        organizations: literally::hmap! {
            org_uuid.to_string() => OrgRole::Member
        },
        projects: HashMap::new(),
    };

    let proj_member = User {
        admin: false,
        locked: false,
        organizations: literally::hmap! {
            org_uuid.to_string() => OrgRole::Member
        },
        projects: literally::hmap! {
            proj_uuid.to_string() => ProjRole::Developer
        },
    };

    let org = Organization {
        uuid: org_uuid.to_string(),
    };
    let proj = Project {
        uuid: proj_uuid.to_string(),
        parent: org_uuid.to_string(),
    };

    assert!(oso.is_allowed(admin.clone(), "read", org.clone()).unwrap());
    assert!(oso
        .is_allowed(admin.clone(), "create_projects", org.clone())
        .unwrap());

    assert!(!oso.is_allowed(user.clone(), "read", org.clone()).unwrap());
    assert!(!oso
        .is_allowed(user.clone(), "create_projects", org.clone())
        .unwrap());

    assert!(oso
        .is_allowed(org_leader.clone(), "read", org.clone())
        .unwrap());
    assert!(oso
        .is_allowed(org_leader.clone(), "create_projects", org.clone())
        .unwrap());

    assert!(oso
        .is_allowed(org_member.clone(), "read", org.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(org_member.clone(), "create_projects", org.clone())
        .unwrap());

    assert!(oso
        .is_allowed(proj_member.clone(), "read", org.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(proj_member.clone(), "create_projects", org.clone())
        .unwrap());

    assert!(oso
        .is_allowed(admin.clone(), "create", proj.clone())
        .unwrap());
    assert!(oso
        .is_allowed(admin.clone(), "manage", proj.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(user.clone(), "create", proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(user.clone(), "manage", proj.clone())
        .unwrap());

    assert!(oso
        .is_allowed(org_leader.clone(), "create", proj.clone())
        .unwrap());
    assert!(oso
        .is_allowed(org_leader.clone(), "manage", proj.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(org_member.clone(), "create", proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(org_member.clone(), "manage", proj.clone())
        .unwrap());

    assert!(oso
        .is_allowed(proj_member.clone(), "create", proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(proj_member.clone(), "manage", proj.clone())
        .unwrap());

    let other_org_uuid = Uuid::new_v4();
    let other_org = Organization {
        uuid: other_org_uuid.to_string(),
    };
    let other_proj = Project {
        uuid: Uuid::new_v4().to_string(),
        parent: other_org_uuid.to_string(),
    };

    assert!(oso
        .is_allowed(admin.clone(), "read", other_org.clone())
        .unwrap());
    assert!(oso
        .is_allowed(admin.clone(), "create_projects", other_org.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(user.clone(), "read", other_org.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(user.clone(), "create_projects", other_org.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(org_leader.clone(), "read", other_org.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(org_leader.clone(), "create_projects", other_org.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(org_member.clone(), "read", other_org.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(org_member.clone(), "create_projects", other_org.clone())
        .unwrap());

    assert!(oso
        .is_allowed(admin.clone(), "create", other_proj.clone())
        .unwrap());
    assert!(oso
        .is_allowed(admin.clone(), "manage", other_proj.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(user.clone(), "create", other_proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(user.clone(), "manage", other_proj.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(org_leader.clone(), "create", other_proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(org_leader.clone(), "manage", other_proj.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(org_member.clone(), "create", other_proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(org_member.clone(), "manage", other_proj.clone())
        .unwrap());

    assert!(!oso
        .is_allowed(proj_member.clone(), "create", other_proj.clone())
        .unwrap());
    assert!(!oso
        .is_allowed(proj_member.clone(), "manage", other_proj.clone())
        .unwrap());
}
