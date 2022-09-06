use oso::{Oso, PolarClass};

pub const POLAR: &str = include_str!("../bencher.polar");

#[derive(Clone, PolarClass)]
struct User {
    #[polar(attribute)]
    pub username: String,
}

impl User {
    fn superuser() -> Vec<String> {
        return vec!["alice".to_string(), "charlie".to_string()];
    }
}

fn demo() {
    let mut oso = Oso::new();

    oso.register_class(
        User::get_polar_class_builder()
            .add_class_method("superusers", User::superuser)
            .build(),
    )
    .unwrap();

    oso.load_str(
        r#"allow(actor: User, _action, _resource) if
                         actor.username.ends_with("example.com");"#,
    )
    .unwrap();

    let user = User {
        username: "alice@example.com".to_owned(),
    };

    assert!(oso.is_allowed(user, "foo", "bar").unwrap());
}
