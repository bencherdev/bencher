use serde::{Deserialize, Serialize};

use crate::tag::TAG_START;

const RECORD_START: &str = "<!-- bencher.dev/section ";
const RECORD_END: &str = " -->";

/// The check run that a section's writer saw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionCheck {
    pub id: u64,
    pub name: String,
    /// GitHub's conclusion, or `None` while the check run is in progress.
    pub conclusion: Option<String>,
    pub external_id: String,
}

impl SectionCheck {
    /// The records directly before the tag that ends `body`, in body order,
    /// passing over any that cannot be read.
    /// Records anywhere else are not read, since the rest of the body holds unescaped names.
    pub fn parse_all(body: &str) -> Vec<Self> {
        let mut rest = body
            .rsplit_once(TAG_START)
            .map_or(body, |(records, _)| records);
        let mut checks = Vec::new();
        while let Some(record) = rest.strip_suffix(RECORD_END)
            && let Some((before, json)) = record.rsplit_once(RECORD_START)
        {
            if let Ok(check) = serde_json::from_str(json) {
                checks.push(check);
            }
            rest = before;
        }
        checks.reverse();
        checks
    }

    /// The record as a hidden HTML comment.
    /// Its JSON holds no `<`, `>`, or `&`, so no check name can end the comment early.
    pub fn html(&self) -> String {
        let json = serde_json::to_string(self)
            .unwrap_or_default()
            .replace('&', "\\u0026")
            .replace('<', "\\u003c")
            .replace('>', "\\u003e");
        format!("{RECORD_START}{json}{RECORD_END}")
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{GitHash, ProjectSlug};

    use super::SectionCheck;
    use crate::group_tag;

    fn check(id: u64, name: &str, conclusion: Option<&str>) -> SectionCheck {
        SectionCheck {
            id,
            name: name.to_owned(),
            conclusion: conclusion.map(str::to_owned),
            external_id: format!("bencher:{id}"),
        }
    }

    fn tag() -> String {
        let project: ProjectSlug = "my-project".parse().unwrap();
        let head_sha: GitHash = "0123456789abcdef0123456789abcdef01234567".parse().unwrap();
        group_tag(&project, &head_sha)
    }

    // Kills a record that does not escape `>` (the name closes the comment early)
    // or that escapes in a way the parser does not read back.
    #[test]
    fn record_round_trips_a_hostile_name() {
        let hostile = check(1, r#"Bencher Report (a"--> <b>&amp;</b> --!>)"#, None);
        let html = hostile.html();
        assert!(!html.contains('&'), "{html}");
        assert_eq!(
            html.find("-->"),
            Some(html.len() - "-->".len()),
            "only the end closes the comment: {html}"
        );
        assert_eq!(
            html.rfind('<'),
            Some(0),
            "no `<` but the comment's own: {html}"
        );

        let done = check(2, "Bencher Report (b)", Some("success"));
        let body = format!("<h2>Report</h2>{html}{}{}", done.html(), tag());
        assert_eq!(SectionCheck::parse_all(&body), vec![hostile, done]);
    }

    // Kills a parser that stops at a record it cannot read, that does not strip the tag,
    // or that returns the records out of body order.
    // The truncated record before them is not a record, so it ends the run.
    #[test]
    fn unreadable_record_is_skipped() {
        let first = check(1, "Bencher Report (a)", Some("failure"));
        let second = check(2, "Bencher Report (b)", None);
        let body = format!(
            "<h2>Report</h2><!-- bencher.dev/section {{\"id\":4,{}<!-- bencher.dev/section {{\"id\":3 -->{}{}",
            first.html(),
            second.html(),
            tag()
        );
        assert_eq!(SectionCheck::parse_all(&body), vec![first, second]);
    }
}
