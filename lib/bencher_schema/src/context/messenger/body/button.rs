use slog::Logger;

use super::FmtBody;

#[derive(Debug)]
pub struct ButtonBody {
    pub title: String,
    pub preheader: String,
    pub greeting: String,
    pub pre_body: String,
    pub button_text: String,
    pub button_url: String,
    pub clipboard_text: String,
    pub clipboard_target: String,
    pub post_body: String,
    pub closing: String,
    pub signature: String,
    pub settings_url: String,
}

impl FmtBody for ButtonBody {
    fn text(&self) -> String {
        let Self {
            greeting,
            pre_body,
            button_text,
            button_url,
            clipboard_text,
            clipboard_target,
            post_body,
            closing,
            signature,
            settings_url,
            ..
        } = self;

        format!("\n{greeting}\n{pre_body}\n{button_text}: {button_url}\n{clipboard_text}: {clipboard_target}\n{post_body}\n{closing}\n{signature}\nBencher - Continuous Benchmarking\nManage email settings ({settings_url})")
    }

    // https://github.com/leemunroe/responsive-html-email-template
    #[allow(clippy::too_many_lines)]
    fn html(&self, log: &Logger) -> String {
        let Self {
            title,
            preheader,
            greeting,
            pre_body,
            button_text,
            button_url,
            clipboard_text: _,
            clipboard_target,
            post_body,
            closing,
            signature,
            settings_url,
        } = self;

        let html = format!(
            "<!doctype html>
<html>
  <head>
    <meta charset=\"utf-8\" />
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1, shrink-to-fit=no\" />
    <meta name=\"theme-color\" content=\"#ffffff\" />
    <title>{title}</title>
    <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css\">
    <script defer src=\"https://use.fontawesome.com/releases/v5.15.4/js/all.js\"></script>
  </head>
  <body>
    <span class=\"preheader\" style=\"
    color: transparent;
    display: none;
    height: 0;
    max-height: 0;
    max-width: 0;
    opacity: 0;
    overflow: hidden;
    mso-hide: all;
    visibility: hidden;
    width: 0;
    \">{preheader}</span>
    <section class=\"section\">
    <div class=\"container\">
    <div class=\"box\">
      <div class=\"columns is-centered\">
        <div class=\"column\">
          <h1 class=\"title\"><img src=\"https://bencher.dev/favicon.png\" width=\"32\" height=\"32\" alt=\"🐰\" /> {title}</h1>
          <br/>
          <p>{greeting}</p>
          <p>{pre_body}</p>
          <br/>
          <div class=\"columns is-centered\">
            <div class=\"column\">
              <div class=\"content has-text-centered\">
                <a class=\"button is-medium is-responsive\" href=\"{button_url}\" target=\"_blank\" style=\"color:white;background-color:#fc7300;\">{button_text}</a>
              </div>
            </div>
          </div>
          <br/>
          <div class=\"content has-text-centered\">
            <code style=\"word-break: break-all;\">
              {clipboard_target}
            </code>
          </div>
          <p>{post_body}</p>
          <br/>
          <p>{closing}</p>
          <p>{signature}</p>
          <br/>
          <hr/>
          <div class=\"content has-text-centered\">
            <p>Bencher - Continuous Benchmarking</p>
            <a href=\"{settings_url}\">Manage email settings</a>
          </div>
        </div>
      </div>
    </div>
    </div>
    </section>
  </body>
</html>
"
        );

        slog::debug!(log, "Inlining CSS into HTML");
        if let Ok(inlined_html) = std::panic::catch_unwind(|| match css_inline::inline(&html) {
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
}
