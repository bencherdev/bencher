use crate::error::api_error;

use super::FmtBody;

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
            title: _,
            preheader: _,
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
        } = self;

        format!("\n{greeting}\n{pre_body}\n{button_text}: {button_url}\n{clipboard_text}: {clipboard_target}\n{post_body}\n{closing}\n{signature}\nBencher - Continuous Benchmarking\nManage email settings ({settings_url})")
    }

    // https://github.com/leemunroe/responsive-html-email-template
    fn html(&self) -> String {
        let Self {
            title,
            preheader,
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
        <div class=\"columns is-centered\">
          <div class=\"column is-four-fifths\">
            <div class=\"card\">
              <div class=\"card-content\">
                <header class=\"card-header\">
                  <p class=\"card-header-title\">
                    {title}
                  </p>
                </header>
                <div class=\"content\">
                  <br/>
                  <p>{greeting}</p>
                  <p>{pre_body}</p>
                  <section class=\"section\">
                    <div class=\"container\">
                      <div class=\"columns is-centered\">
                        <div class=\"column is-half\">
                          <div class=\"content has-text-centered\">
                            <a class=\"button is-half\" href=\"{button_url}\" target=\"_blank\" style=\"color: white;background-color:#fc7300\">{button_text}</a>
                          </div>
                        </div>
                      </div>
                      <br>
                      <div class=\"columns is-centered\">
                        <div class=\"column is-half\">
                          <div class=\"content has-text-centered\">
                            <p>
                              <a href=\"{button_url}\" target=\"_blank\">Click Here to {button_text}</a>
                            </p>
                            <p>
                              <small>
                                <code id=\"clipboard-target\" style=\"overflow-wrap:break-word;\">
                                    {clipboard_target}
                                </code>
                              </small>
                            </p>
                            <button class=\"button is-small\" onclick=\"copy_to_clipboard('clipboard-target')\" alt=\"{clipboard_text}\">
                                <span class=\"icon\">
                                    📄
                                </span>
                                <span>Copy</span>
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  </section>
                  <p>{post_body}</p>
                  <br/>
                  <p>{closing}</p>
                  <p>{signature}</p>
                  <br/>
                </div>
              </div>
              <footer class=\"card-footer\">
                <div class=\"card-footer-item\">
                  <div class=\"content has-text-centered\">
                    <p>Bencher - Continuous Benchmarking</p>
                    <a href=\"{settings_url}\">Manage email settings</a>
                  </div>
                </div>
              </footer>
            </div>
          </div>
        </div>
  </body>
  <script>
    function copy_to_clipboard(id) {{
        navigator.clipboard.writeText(document.getElementById(id)?.innerHTML);
    }}
  </script>
</html>
"
        );

        css_inline::inline(&html)
            .map_err(api_error!())
            .unwrap_or(html)
    }
}
