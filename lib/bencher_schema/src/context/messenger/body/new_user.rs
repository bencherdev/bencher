use slog::Logger;
use url::Url;

use super::FmtBody;

#[derive(Debug)]
pub struct NewUserBody {
    pub admin: String,
    pub console_url: Url,
    pub name: String,
    pub email: String,
    pub invited: bool,
    pub method: String,
    /// The user's profile on the server's GitHub, for a user who signed up with GitHub.
    pub github: Option<Url>,
}

impl FmtBody for NewUserBody {
    fn text(&self) -> String {
        let Self {
            admin,
            console_url,
            name,
            email,
            invited,
            method,
            github: _,
        } = self;
        format!(
            "Ahoy {admin},
        A new user {invited_or_joined} your Bencher instance ({console_url}) via {method}!

        Name: {name}
        Email: {email}

        🐰 Bencher
        ",
            invited_or_joined = invited_or_joined(*invited)
        )
    }

    fn html(&self, _log: &Logger) -> String {
        let Self {
            admin,
            console_url,
            name,
            email,
            invited,
            method,
            github,
        } = self;
        let github_link = github
            .as_ref()
            .map(|github| format!(r#"<a href="{github}">View {name} on GitHub</a><br/>"#))
            .unwrap_or_default();
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
        <p>Ahoy {admin},</p>
        <p>A new user {invited_or_joined} your Bencher instance ({console_url}) via {method}!</p>
        <br />
        <p>Name: {name}</p>
        <p>Email: {email}</p>
        <br />
        {github_link}
        <br />
        <a href=\"mailto:{email}?subject=Any%20advice%3F\">Ask for advice</a>
        <br />
        <p>🐰 Bencher</p>
    </body>
</html>",
            invited_or_joined = invited_or_joined(*invited)
        )
    }
}

fn invited_or_joined(invited: bool) -> &'static str {
    if invited {
        "was invited to"
    } else {
        "has joined"
    }
}

#[cfg(test)]
mod tests {
    use slog::{Logger, o};

    use super::{FmtBody as _, NewUserBody};

    fn body(github: Option<&str>) -> NewUserBody {
        NewUserBody {
            admin: "Admin User".to_owned(),
            console_url: "https://bencher.example.com".parse().unwrap(),
            name: "octo-cat".to_owned(),
            email: "octo-cat@example.com".to_owned(),
            invited: false,
            method: "GitHub OAuth2".to_owned(),
            github: github.map(|github| github.parse().unwrap()),
        }
    }

    // Kills a link to github.com for a user of a GitHub Enterprise Server, and a link for a user
    // with no GitHub profile.
    #[test]
    fn the_github_link_goes_to_the_user_profile() {
        let log = Logger::root(slog::Discard, o!());
        let enterprise = body(Some("https://github.example.com/octo-cat")).html(&log);
        let without = body(None).html(&log);

        assert!(
            enterprise.contains(
                r#"<a href="https://github.example.com/octo-cat">View octo-cat on GitHub</a>"#
            ),
            "{enterprise}"
        );
        assert!(!enterprise.contains("https://github.com/"), "{enterprise}");
        assert!(!without.contains("on GitHub"), "{without}");
    }
}
