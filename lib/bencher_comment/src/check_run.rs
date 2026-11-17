use bencher_json::ResourceName;

use crate::BENCHER_REPORT_TITLE;

pub const BENCHER_REPORT: &str = "Bencher Report";
pub const IN_PROGRESS_SUMMARY: &str =
    "<p>Benchmarks are running. Results will be posted here when they complete.</p>";

// There is an undocumented maximum length of 65536 characters for comments.
// - Check Run (https://github.com/bencherdev/bencher/issues/534):
//   - REST: https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#create-a-check-run
//   - GraphQL:
//     - https://docs.github.com/en/graphql/reference/mutations#createcheckrun
//     - https://docs.github.com/en/graphql/reference/objects#checkrun
// - PR Comment (https://github.com/bencherdev/bencher/issues/644#issuecomment-3716253808):
//   - REST: https://docs.github.com/en/rest/pulls/comments?apiVersion=2022-11-28#create-a-review-comment-for-a-pull-request
//   - GraphQL:
//     - https://docs.github.com/en/graphql/reference/mutations#addcomment
//     - https://docs.github.com/en/graphql/reference/input-objects#addcommentinput
pub const MAX_LENGTH: usize = 1 << 16;

// GitHub does not document a maximum length for the Check Run name, so cap it
// conservatively at the 255 that commit status contexts are capped at. The cap
// is counted in bytes, which is stricter than characters for a multi-byte ID.
// A Project name is a `ResourceName`, so it can never reach this. Only a custom
// `--ci-id`, which is not length validated, can.
const MAX_NAME_LEN: usize = 255;
// The ` `, `(`, and `)` that `check_run_name` wraps around the ID
const NAME_FORMAT_LEN: usize = 3;
const MAX_ID_LEN: usize = MAX_NAME_LEN - BENCHER_REPORT.len() - NAME_FORMAT_LEN;

// An explicit `--ci-id` takes the Project name's place in the check run name.
// Every writer of a check must resolve the ID the same way, or a required status
// check would never be reported, so all of them go through here.
pub fn check_run_id<'a>(
    ci_id: Option<&'a str>,
    project_name: Option<&'a ResourceName>,
) -> Option<&'a str> {
    ci_id.or_else(|| project_name.map(ResourceName::as_ref))
}

// Required status checks in branch protection match by exact check run name,
// so the name must be stable across runs for a given `bencher run` invocation.
// Without an ID the in-progress check falls back to the bare `Bencher Report`
// name, which its completion renames to the Report's Project.
pub fn check_run_name(id: Option<&str>) -> String {
    if let Some(id) = id {
        format!("{BENCHER_REPORT} ({id})", id = truncate_id(id))
    } else {
        BENCHER_REPORT.to_owned()
    }
}

// Render the same branded header that `ReportComment` renders for the
// completed check, so the check reads as a Bencher Report in every state.
pub fn branded_summary(body: &str) -> String {
    format!("<h2>{BENCHER_REPORT_TITLE}</h2>{body}")
}

// A Project name is a `ResourceName`, so it is always well under `MAX_ID_LEN`.
// Only a custom `--ci-id`, which is not length validated, can ever be truncated.
// `floor_char_boundary` always lands on a boundary at or below the length, so
// `get` is always `Some`. It is used over a slice to satisfy `clippy::string_slice`,
// and defaults to empty so that an unreachable `None` still fails closed.
pub fn truncate_id(id: &str) -> &str {
    id.get(..id.floor_char_boundary(MAX_ID_LEN))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use bencher_json::ResourceName;

    use super::{
        BENCHER_REPORT, IN_PROGRESS_SUMMARY, MAX_ID_LEN, MAX_NAME_LEN, NAME_FORMAT_LEN,
        branded_summary, check_run_id, check_run_name,
    };

    #[test]
    fn branded_summary_in_progress() {
        let summary = branded_summary(IN_PROGRESS_SUMMARY);
        assert!(summary.starts_with("<h2>"));
        assert!(summary.contains("Bencher Report"));
        assert!(summary.ends_with(IN_PROGRESS_SUMMARY));
    }

    fn project_name() -> ResourceName {
        "Slang Solidity".parse().expect("Invalid Project name")
    }

    #[test]
    fn check_run_id_project_name() {
        assert_eq!(
            check_run_id(None, Some(&project_name())),
            Some("Slang Solidity")
        );
    }

    // An explicit `--ci-id` takes the Project name's place
    #[test]
    fn check_run_id_ci_id_wins() {
        assert_eq!(
            check_run_id(Some("embedded"), Some(&project_name())),
            Some("embedded")
        );
    }

    // Both create paths resolve the ID here, so they can never disagree
    #[test]
    fn check_run_id_none() {
        assert_eq!(check_run_id(None, None), None);
    }

    #[test]
    fn check_run_name_project_name() {
        assert_eq!(
            check_run_name(Some("Slang Solidity")),
            "Bencher Report (Slang Solidity)"
        );
    }

    #[test]
    fn check_run_name_ci_id() {
        assert_eq!(
            check_run_name(Some("embedded")),
            "Bencher Report (embedded)"
        );
    }

    // Neither a Project name nor a `--ci-id` is known before the benchmarks run
    #[test]
    fn check_run_name_default() {
        assert_eq!(check_run_name(None), "Bencher Report");
    }

    // The regression from https://github.com/bencherdev/bencher/issues/944:
    // runs for different Projects on the same commit must not collide.
    #[test]
    fn check_run_name_distinct_per_project() {
        assert_ne!(
            check_run_name(Some("Slang Solidity")),
            check_run_name(Some("Slang Testlang"))
        );
    }

    #[test]
    fn check_run_name_max_project_name() {
        // A Project name is a `ResourceName`, so it can never reach `MAX_NAME_LEN`
        let project_name = "a".repeat(ResourceName::MAX_LEN);
        let name = check_run_name(Some(&project_name));
        assert_eq!(
            name.len(),
            BENCHER_REPORT.len() + NAME_FORMAT_LEN + ResourceName::MAX_LEN
        );
        assert!(name.len() < MAX_NAME_LEN, "{}", name.len());
        assert!(name.ends_with(&format!("({project_name})")));
    }

    // An ID at exactly the cap is kept whole, so the boundary is not off by one
    #[test]
    fn check_run_name_exact_max_id() {
        let ci_id = "a".repeat(MAX_ID_LEN);
        let name = check_run_name(Some(&ci_id));
        assert_eq!(name.len(), MAX_NAME_LEN);
        assert!(name.ends_with(&format!("({ci_id})")));
    }

    #[test]
    fn check_run_name_truncates_long_ci_id() {
        // `--ci-id` is unvalidated, so only truncation keeps the name under the max
        let ci_id = "a".repeat(MAX_NAME_LEN * 2);
        let name = check_run_name(Some(&ci_id));
        assert_eq!(name.len(), MAX_NAME_LEN);
        assert!(name.starts_with("Bencher Report ("));
        assert!(name.ends_with(')'));
    }

    #[test]
    fn check_run_name_truncates_on_char_boundary() {
        // Truncating mid-code-point would panic, so the cut must land on a boundary
        let ci_id = "🐰".repeat(MAX_NAME_LEN);
        let name = check_run_name(Some(&ci_id));
        assert!(name.len() <= MAX_NAME_LEN, "{}", name.len());
        assert!(name.ends_with("🐰)"), "{name}");
    }
}
