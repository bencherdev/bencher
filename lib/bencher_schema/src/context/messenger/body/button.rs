use slog::Logger;

use super::{FmtBody, escape, inline_css};

#[derive(Debug)]
pub struct ButtonBody {
    pub title: String,
    pub preheader: String,
    pub greeting: String,
    pub pre_body: String,
    pub button_text: String,
    pub button_url: String,
    pub clipboard: Option<Clipboard>,
    pub post_body: String,
    pub closing: String,
    pub signature: String,
    pub settings_url: String,
}

/// A value to copy, under its label, below the button.
#[derive(Debug)]
pub struct Clipboard {
    pub text: String,
    pub target: String,
}

impl FmtBody for ButtonBody {
    fn text(&self) -> String {
        let Self {
            greeting,
            pre_body,
            button_text,
            button_url,
            clipboard,
            post_body,
            closing,
            signature,
            settings_url,
            ..
        } = self;
        let clipboard = clipboard
            .as_ref()
            .map(|Clipboard { text, target }| format!("\n{text}: {target}"))
            .unwrap_or_default();

        format!(
            "\n{greeting}\n{pre_body}\n{button_text}: {button_url}{clipboard}\n{post_body}\n{closing}\n{signature}\nBencher - Continuous Benchmarking\nManage email settings ({settings_url})"
        )
    }

    fn html(&self, log: &Logger) -> String {
        inline_css(log, self.template())
    }
}

impl ButtonBody {
    /// The HTML before its styles are inlined, with every value escaped.
    // https://github.com/leemunroe/responsive-html-email-template
    fn template(&self) -> String {
        let Self {
            title,
            preheader,
            greeting,
            pre_body,
            button_text,
            button_url,
            clipboard,
            post_body,
            closing,
            signature,
            settings_url,
        } = self;
        let title = escape(title);
        let preheader = escape(preheader);
        let greeting = escape(greeting);
        let pre_body = escape(pre_body);
        let button_text = escape(button_text);
        let button_url = escape(button_url);
        let clipboard = clipboard
            .as_ref()
            .map(|Clipboard { target, .. }| {
                format!(
                    "
          <br/>
          <div class=\"content has-text-centered\">
            <code style=\"word-break: break-all;\">
              {target}
            </code>
          </div>",
                    target = escape(target)
                )
            })
            .unwrap_or_default();
        let post_body = escape(post_body);
        let closing = escape(closing);
        let signature = escape(signature);
        let settings_url = escape(settings_url);

        format!(
            "<!doctype html>
<html>
  <head>
    <meta charset=\"utf-8\" />
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1, shrink-to-fit=no\" />
    <meta name=\"theme-color\" content=\"#ffffff\" />
    <title>{title}</title>
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
          </div>{clipboard}
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
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ButtonBody, Clipboard};

    const CHOSEN: &str = "<b>&\"x'";
    const ESCAPED: &str = "&lt;b&gt;&amp;&quot;x&#39;";

    // A name a person chooses, such as an organization's, reaches every place the template puts a
    // value, and none of it is markup there.
    #[test]
    fn template_escapes_every_value() {
        let body = ButtonBody {
            title: format!("Invitation to join {CHOSEN}"),
            preheader: CHOSEN.to_owned(),
            greeting: format!("Ahoy {CHOSEN}!"),
            pre_body: format!("{CHOSEN} invited you to join {CHOSEN}."),
            button_text: format!("Join {CHOSEN}"),
            button_url: format!("https://bencher.example/auth/login?a=1&b={CHOSEN}"),
            clipboard: Some(Clipboard {
                text: "Token".to_owned(),
                target: CHOSEN.to_owned(),
            }),
            post_body: CHOSEN.to_owned(),
            closing: CHOSEN.to_owned(),
            signature: CHOSEN.to_owned(),
            settings_url: format!("https://bencher.example/help?{CHOSEN}"),
        };
        let template = body.template();
        assert!(!template.contains(CHOSEN), "{template}");
        assert!(!template.contains("a=1&b"), "{template}");
        // The title twice, then each other value, the body's two.
        assert_eq!(template.matches(ESCAPED).count(), 13, "{template}");
        assert!(
            template.contains(&format!(
                "href=\"https://bencher.example/auth/login?a=1&amp;b={ESCAPED}\""
            )),
            "{template}"
        );
    }
}
