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
            "<!doctype html>
<html>
    <head>
        <meta charset=\"utf-8\" />
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1, shrink-to-fit=no\" />
        <meta name=\"theme-color\" content=\"#ffffff\" />
        <title>New user {invited_or_joined} Bencher</title>
    </head>
    <body>
        <p>Ahoy {admin}</p>,
        <p>A new user {invited_or_joined} your Bencher instance ({endpoint})!</p>
        <br />
        <p>Name: {name}</p>
        <p>Email: {email}</p>
        <br/>
        <p>🐰 Bencher</p>
    </body>
</html>"
        )
    }
}
