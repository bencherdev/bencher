use std::collections::HashMap;

use oso::{Oso, PolarClass};
use uuid::Uuid;

pub const POLAR: &str = include_str!("../bencher.polar");

#[derive(Clone, PolarClass)]
struct User {
    #[polar(attribute)]
    pub admin: bool,
    #[polar(attribute)]
    pub locked: bool,
    #[polar(attribute)]
    pub roles: HashMap<String, String>,
}

#[derive(Clone, Copy, PolarClass)]
struct Server {}

#[derive(Clone, Copy, PolarClass)]
struct Org {
    uuid: Uuid,
}

#[test]
fn test_user() {
    let mut oso = Oso::new();

    oso.register_class(
        User::get_polar_class_builder()
            .add_attribute_getter("admin", |user| user.admin)
            .add_attribute_getter("locked", |user| user.locked)
            .build(),
    )
    .unwrap();

    oso.register_class(Server::get_polar_class()).unwrap();

    oso.register_class(
        Org::get_polar_class_builder()
            .add_attribute_getter("uuid", |org| org.uuid.to_string())
            .build(),
    )
    .unwrap();

    oso.load_str(POLAR).unwrap();

    let server = Server {};

    let admin = User {
        admin: true,
        locked: false,
        roles: HashMap::new(),
    };

    assert!(oso.is_allowed(admin, "administer", server).unwrap());

    let user = User {
        admin: false,
        locked: false,
        roles: HashMap::new(),
    };

    assert!(!oso.is_allowed(user.clone(), "administer", server).unwrap());
    assert!(oso.is_allowed(user, "session", server).unwrap());

    let locked_admin = User {
        admin: true,
        locked: true,
        roles: HashMap::new(),
    };

    assert!(!oso
        .is_allowed(locked_admin.clone(), "administer", server)
        .unwrap());
    assert!(!oso.is_allowed(locked_admin, "session", server).unwrap());

    let locked_user = User {
        admin: false,
        locked: true,
        roles: HashMap::new(),
    };

    assert!(!oso
        .is_allowed(locked_user.clone(), "administer", server)
        .unwrap());
    assert!(!oso.is_allowed(locked_user, "session", server).unwrap());

    let org_uuid = Uuid::new_v4();

    let user = User {
        admin: false,
        locked: false,
        roles: literally::hmap! {
            org_uuid.to_string() => "member"
        },
    };

    let org = Org { uuid: org_uuid };

    assert!(oso.is_allowed(user.clone(), "read", org).unwrap());
    assert!(!oso.is_allowed(user, "create_projects", org).unwrap());
}

#[derive(Clone, PolarClass)]
struct OsoUser {
    #[polar(attribute)]
    pub username: String,
}

impl OsoUser {
    fn superuser() -> Vec<String> {
        return vec!["alice".to_string(), "charlie".to_string()];
    }
}

#[test]
fn demo() {
    let mut oso = Oso::new();

    oso.register_class(
        OsoUser::get_polar_class_builder()
            .add_class_method("superusers", OsoUser::superuser)
            .build(),
    )
    .unwrap();

    oso.load_str(
        r#"allow(actor: OsoUser, _action, _resource) if
                         actor.username.ends_with("example.com");"#,
    )
    .unwrap();

    let user = OsoUser {
        username: "alice@example.com".to_owned(),
    };

    assert!(oso.is_allowed(user, "foo", "bar").unwrap());
}
