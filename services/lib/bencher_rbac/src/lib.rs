use oso::{Oso, PolarClass};

pub const POLAR: &str = include_str!("../bencher.polar");

#[derive(Clone, Copy, PolarClass)]
struct User {
    #[polar(attribute)]
    pub admin: bool,
    #[polar(attribute)]
    pub locked: bool,
}

#[derive(Clone, Copy, PolarClass)]
struct Server {}

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

    println!("POLAR: {POLAR}");

    oso.load_str(POLAR).unwrap();

    let server = Server {};

    let admin = User {
        admin: true,
        locked: false,
    };

    assert!(oso.is_allowed(admin, "administer", server).unwrap());

    let user = User {
        admin: false,
        locked: false,
    };

    assert!(!oso.is_allowed(user, "administer", server).unwrap());
    assert!(oso.is_allowed(user, "session", server).unwrap());

    let locked_admin = User {
        admin: true,
        locked: true,
    };

    assert!(!oso.is_allowed(locked_admin, "administer", server).unwrap());
    assert!(!oso.is_allowed(locked_admin, "session", server).unwrap());

    let locked_user = User {
        admin: false,
        locked: true,
    };

    assert!(!oso.is_allowed(locked_user, "administer", server).unwrap());
    assert!(!oso.is_allowed(locked_user, "session", server).unwrap());
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
