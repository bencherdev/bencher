mod button;
mod new_user;
mod runner_status;
mod server_stats;
mod sign_in;

pub use button::ButtonBody;
pub use new_user::NewUserBody;
#[cfg(feature = "plus")]
pub use runner_status::RunnerStatusBody;
#[cfg(feature = "plus")]
pub use server_stats::ServerStatsBody;
pub use sign_in::SignInBody;
use slog::Logger;

static CSS_INLINER: std::sync::LazyLock<css_inline::CSSInliner<'static>> =
    std::sync::LazyLock::new(|| {
        css_inline::CSSInliner::new(
            css_inline::InlineOptions::default()
                .load_remote_stylesheets(false)
                .extra_css(Some(std::borrow::Cow::Borrowed(include_str!(
                    "bulma.min.css"
                )))),
        )
    });

pub trait FmtBody {
    fn text(&self) -> String;
    fn html(&self, log: &Logger) -> String;
}

#[derive(Debug)]
pub enum Body {
    Button(Box<ButtonBody>),
    NewUser(NewUserBody),
    SignIn(SignInBody),
    #[cfg(feature = "plus")]
    ServerStats(ServerStatsBody),
    #[cfg(feature = "plus")]
    RunnerStatus(RunnerStatusBody),
}

impl FmtBody for Body {
    fn text(&self) -> String {
        match self {
            Self::Button(body) => body.text(),
            Self::NewUser(body) => body.text(),
            Self::SignIn(body) => body.text(),
            #[cfg(feature = "plus")]
            Self::ServerStats(body) => body.text(),
            #[cfg(feature = "plus")]
            Self::RunnerStatus(body) => body.text(),
        }
    }

    fn html(&self, log: &Logger) -> String {
        match self {
            Self::Button(body) => body.html(log),
            Self::NewUser(body) => body.html(log),
            Self::SignIn(body) => body.html(log),
            #[cfg(feature = "plus")]
            Self::ServerStats(body) => body.html(log),
            #[cfg(feature = "plus")]
            Self::RunnerStatus(body) => body.html(log),
        }
    }
}

/// The HTML with the Bulma styles inlined, or as it is if they cannot be.
fn inline_css(log: &Logger, html: String) -> String {
    slog::debug!(log, "Inlining CSS into HTML");
    if let Ok(inlined_html) = std::panic::catch_unwind(|| match CSS_INLINER.inline(&html) {
        Ok(html) => Some(html),
        Err(err) => {
            slog::error!(log, "Failed to inline CSS: {err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            None
        },
    }) {
        inlined_html.unwrap_or(html)
    } else {
        // This fails when run locally on macOS,
        // but works when run in a Debian container.
        let msg = "Panicked trying to inline CSS";
        slog::error!(log, "{msg}");
        #[cfg(feature = "sentry")]
        sentry::capture_message(msg, sentry::Level::Error);
        html
    }
}

/// The text with every character that HTML gives a meaning escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            c => escaped.push(c),
        }
    }
    escaped
}
