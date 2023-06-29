use url::Url;

use super::FmtBody;

pub struct NewUserBody {
    pub admin: String,
    pub endpoint: Url,
    pub name: String,
    pub email: String,
    pub invited: bool,
}

impl FmtBody for NewUserBody {
    fn text(&self) -> String {
        let Self {
            admin,
            endpoint,
            name,
            email,
            invited,
        } = self;
        let invited_or_joined = if *invited {
            "was invited to"
        } else {
            "has joined"
        };
        format!(
            r#"Ahoy {admin},
        A new user {invited_or_joined} your Bencher instance ({endpoint})!

        Name: {name}
        Email: {email}

        🐰 Bencher
        "#
        )
    }

    fn html(&self) -> String {
        self.text()
    }
}
