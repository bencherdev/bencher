#![cfg(feature = "plus")]

use crate::error::api_error;

use super::FmtBody;

pub struct AdviceBody {
    pub name: String,
}

impl FmtBody for AdviceBody {
    fn text(&self) -> String {
        let Self { name } = self;

        format!("Hey {name}, I think you signed up for the site I started (Bencher)?\nJust got your name/email from your signup form.\nHow are things so far? Do you have any advice on how we can improve? Can be brutally honest.\nI try to check in with a few people randomly to see if we're falling short, and usually reply within the day.\n\nAll the best,\nEverett Pompeii\nFounder + Maintainer\nhttps://calendly.com/epompeii")
    }

    fn html(&self) -> String {
        let Self { name } = self;

        let html = format!(
          "<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <title>Any Advice?</title>
</head>
<body>
  <p>Hey ${name}, I think you signed up for the site I started (Bencher)?</p>
  <p>Just got your name/email from your signup form.</p>
  <p>How are things so far? Do you have any advice on how we can improve? Can be brutally honest.</p>
  <p>I try to check in with a few people randomly to see if we're falling short, and usually reply within the day.</p>
  <br />
  <p>All the best,</p>
  <p>Everett Pompeii</p>
  <p>Founder + Maintainer</p>
  <p><a href=\"https://calendly.com/epompeii\">calendly.com/epompeii</a>
</body>
</html>
"
      );

        css_inline::inline(&html)
            .map_err(api_error!())
            .unwrap_or(html)
    }
}
