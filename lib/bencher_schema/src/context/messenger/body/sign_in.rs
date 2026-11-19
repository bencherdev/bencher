use std::fmt;

use bencher_json::SignInCode;
use slog::Logger;
use url::Url;

use super::{FmtBody, escape, inline_css};

const EXPIRY: &str = "The code and the button work once, in the next 30 minutes.";
const IGNORE: &str = "If you did not ask to sign in, ignore this email.";
const LOCKED: &str = "Your Bencher account is locked";
const SOMEONE_ASKED: &str = "Someone asked to sign in with this address.";
const ADMINISTRATOR: &str =
    "To unlock the account, contact the administrator of this Bencher server.";

/// The email a sign-in sends: its code and its link, or, to a locked account, how to unlock it.
pub enum SignInBody {
    Code {
        /// Whether the address has no account yet.
        sign_up: bool,
        code: SignInCode,
        link: Url,
    },
    Locked {
        /// Where to reach support, on Bencher Cloud.
        support: Option<Url>,
    },
}

impl SignInBody {
    pub fn subject(&self) -> String {
        match self {
            Self::Code { code, .. } => format!("Your Bencher code is {}", code.as_ref()),
            Self::Locked { .. } => LOCKED.to_owned(),
        }
    }

    /// The HTML before its styles are inlined, with every interpolated value escaped.
    fn template(&self) -> String {
        let content = match self {
            Self::Code {
                sign_up,
                code,
                link,
            } => format!(
                "<h1 class=\"title\">{heading}</h1>
          <p class=\"is-size-3 has-text-weight-bold has-text-centered\">{code}</p>
          <div class=\"content has-text-centered\">
            <a class=\"button is-medium is-responsive\" href=\"{link}\" target=\"_blank\" style=\"color:white;background-color:#fc7300;\">{button}</a>
          </div>
          <p>{EXPIRY}</p>
          <p>{IGNORE}</p>",
                heading = escape(heading(*sign_up)),
                code = escape(code.as_ref()),
                link = escape(link.as_str()),
                button = escape(button(*sign_up)),
            ),
            Self::Locked { support } => {
                let unlock = if let Some(support) = support {
                    format!(
                        "To unlock the account, <a href=\"{support}\">contact support</a>.",
                        support = escape(support.as_str())
                    )
                } else {
                    ADMINISTRATOR.to_owned()
                };
                format!(
                    "<h1 class=\"title\">{LOCKED}</h1>
          <p>{SOMEONE_ASKED}</p>
          <p>{unlock}</p>"
                )
            },
        };
        format!(
            "<!doctype html>
<html>
  <head>
    <meta charset=\"utf-8\" />
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1, shrink-to-fit=no\" />
    <title>{title}</title>
  </head>
  <body>
    <section class=\"section\">
    <div class=\"container\">
    <div class=\"box\">
      <div class=\"columns is-centered\">
        <div class=\"column\">
          {content}
        </div>
      </div>
    </div>
    </div>
    </section>
  </body>
</html>
",
            title = escape(&self.subject()),
        )
    }
}

// Never prints the code or the link.
impl fmt::Debug for SignInBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code { .. } => f.write_str("SignInBody::Code"),
            Self::Locked { .. } => f.write_str("SignInBody::Locked"),
        }
    }
}

impl FmtBody for SignInBody {
    fn text(&self) -> String {
        match self {
            Self::Code {
                sign_up,
                code,
                link,
            } => format!(
                "\n{heading}\n\n{code}\n\n{button}: {link}\n\n{EXPIRY}\n{IGNORE}\n",
                heading = heading(*sign_up),
                code = code.as_ref(),
                button = button(*sign_up),
            ),
            Self::Locked { support } => {
                let unlock = if let Some(support) = support {
                    format!("To unlock the account, contact support: {support}")
                } else {
                    ADMINISTRATOR.to_owned()
                };
                format!("\n{LOCKED}\n\n{SOMEONE_ASKED}\n{unlock}\n")
            },
        }
    }

    fn html(&self, log: &Logger) -> String {
        inline_css(log, self.template())
    }
}

fn heading(sign_up: bool) -> &'static str {
    if sign_up {
        "Sign up for Bencher"
    } else {
        "Sign in to Bencher"
    }
}

fn button(sign_up: bool) -> &'static str {
    if sign_up { "Sign up" } else { "Sign in" }
}

#[cfg(test)]
mod tests {
    use super::SignInBody;

    // Every value the template interpolates is escaped, links included.
    #[test]
    fn template_escapes_every_interpolated_value() {
        let link = "https://bencher.example/it's/auth/confirm?a=1&b=2#secret"
            .parse()
            .unwrap();
        let code = SignInBody::Code {
            sign_up: false,
            code: "123456".parse().unwrap(),
            link,
        };
        let template = code.template();
        assert!(
            template.contains(
                "href=\"https://bencher.example/it&#39;s/auth/confirm?a=1&amp;b=2#secret\""
            ),
            "{template}"
        );
        assert!(!template.contains("a=1&b=2"), "{template}");

        let locked = SignInBody::Locked {
            support: Some("https://bencher.example/it's/help?a=1&b=2".parse().unwrap()),
        };
        let template = locked.template();
        assert!(
            template.contains("href=\"https://bencher.example/it&#39;s/help?a=1&amp;b=2\""),
            "{template}"
        );
        assert!(!template.contains("a=1&b=2"), "{template}");
    }
}
