use std::{cmp::Ordering, fmt, ops::BitOr as _};

use bencher_json::{GitHash, project::report::Adapter};

use crate::{
    AlertColumns, BENCHER_REPORT_TITLE, EMPTY_CELL, ReportComment, Resource, SectionCheck,
    TRUNCATED_NOTICE, counted, html_alerts_heading, html_link_row, tag::group_tag,
};

const SHORT_SHA_LEN: usize = 7;
const SUCCESS: &str = "success";

/// The one pull request comment that every job of a Project writes, with a section per job.
pub struct GroupComment<'r> {
    writer: &'r ReportComment,
    rows: Vec<Row<'r>>,
    head_sha: GitHash,
}

/// The section of one job: its `--ci-id`, the check run it ran under, and its report once fetched.
pub struct Section<'r> {
    ci_id: Option<String>,
    check: SectionCheck,
    report: Option<&'r ReportComment>,
}

impl<'r> GroupComment<'r> {
    /// The writer's own section always has its report, and the header links come from it.
    pub fn new(
        ci_id: Option<String>,
        check: SectionCheck,
        report: &'r ReportComment,
        others: Vec<Section<'r>>,
        head_sha: GitHash,
    ) -> Self {
        let mut sections = others;
        sections.push(Section::new(ci_id, check, Some(report)));
        sections.sort_by(Section::start_order);
        let mut number = 0;
        let mut rows = Vec::with_capacity(sections.len());
        for section in sections {
            let label = if section.report.is_some() {
                number += 1;
                section.label(Some(number))
            } else {
                section.label(None)
            };
            rows.push(Row { label, section });
        }
        Self {
            writer: report,
            rows,
            head_sha,
        }
    }

    pub fn has_alert(&self) -> bool {
        self.reports().any(ReportComment::has_alert)
    }

    pub fn has_threshold(&self) -> bool {
        self.reports().any(ReportComment::has_threshold)
    }

    /// The first of Full, Reduced, and Summary that fits under `max_length`.
    pub fn html(&self, require_threshold: bool, max_length: usize) -> String {
        let trailer = self.trailer();
        if self.rows.len() == 1 {
            return self
                .writer
                .html_with_tag(require_threshold, &trailer, max_length);
        }

        let mut alerts_heading = String::new();
        let alerts_len = self.reports().map(|report| report.alerts().len()).sum();
        if alerts_len > 0 {
            html_alerts_heading(&mut alerts_heading, alerts_len);
        }
        let alerts = format!("{alerts_heading}{}", self.html_alerts_table());
        let details = self
            .rows
            .iter()
            .filter_map(|row| row.html_details(require_threshold))
            .collect::<String>();
        let all_rows = self.rows.len();
        let full = self.html_parts(all_rows, &alerts, &details, &trailer);
        if full.len() <= max_length {
            return full;
        }
        let reduced = self.html_parts(all_rows, &alerts, TRUNCATED_NOTICE, &trailer);
        if reduced.len() <= max_length {
            return reduced;
        }
        let mut kept = all_rows;
        loop {
            let summary = self.html_parts(kept, &alerts_heading, TRUNCATED_NOTICE, &trailer);
            if summary.len() <= max_length || kept == 0 {
                return summary;
            }
            kept -= 1;
        }
    }

    fn html_parts(&self, kept: usize, alerts: &str, notes: &str, trailer: &str) -> String {
        let mut html = String::new();
        self.html_header(&mut html);
        self.html_summary(&mut html, kept);
        html.push_str(alerts);
        html.push_str(notes);
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        html.push_str(trailer);
        html
    }

    fn reports(&self) -> impl Iterator<Item = &'r ReportComment> {
        self.rows.iter().filter_map(|row| row.section.report)
    }

    fn trailer(&self) -> String {
        let mut trailer = self
            .rows
            .iter()
            .map(|row| row.section.check.html())
            .collect::<String>();
        trailer.push_str(&group_tag(self.writer.project_slug(), &self.head_sha));
        trailer
    }

    fn html_header(&self, html: &mut String) {
        let writer = self.writer;
        let project_url = writer.resource_url(Resource::Project);
        html.push_str(&format!(
            r#"<h2><a href="{project_url}">{BENCHER_REPORT_TITLE}</a></h2>"#,
        ));
        html.push_str("<table>");
        html_link_row(
            html,
            "Project",
            writer.json_report.project.name.as_ref(),
            &project_url,
        );
        if !self.has_branch_column() {
            html_link_row(
                html,
                "Branch",
                writer.json_report.branch.name.as_ref(),
                &writer.resource_url(Resource::Branch(writer.json_report.branch.slug.clone())),
            );
        }
        let head_sha = self.head_sha.as_ref();
        html.push_str(&format!(
            "<tr><td>Commit</td><td><code>{short_sha}</code></td></tr>",
            short_sha = head_sha.get(..SHORT_SHA_LEN).unwrap_or(head_sha),
        ));
        html.push_str("</table>");
    }

    fn html_summary(&self, html: &mut String, kept: usize) {
        let branch_column = self.has_branch_column();
        html.push_str("<table><thead><tr><th>Report</th>");
        if branch_column {
            html.push_str("<th>Branch</th>");
        }
        html.push_str("<th>Testbed</th><th>Adapter</th><th>Status</th></tr></thead><tbody>");
        let (shown, cut) = self.rows.split_at(kept);
        for row in shown {
            html.push_str("<tr>");
            if let Some(report) = row.section.report {
                html_link_cell(html, report.report_url().as_str(), &row.label);
                if branch_column {
                    html_link_cell(
                        html,
                        report
                            .resource_url(Resource::Branch(report.json_report.branch.slug.clone()))
                            .as_str(),
                        report.json_report.branch.name.as_ref(),
                    );
                }
                html_link_cell(
                    html,
                    report
                        .resource_url(Resource::Testbed(report.json_report.testbed.slug.clone()))
                        .as_str(),
                    report.json_report.testbed.name.as_ref(),
                );
                let (name, fragment) = adapter_docs(report.json_report.adapter);
                html_link_cell(
                    html,
                    &format!(
                        "https://bencher.dev/docs/explanation/adapters/{utm}#{fragment}",
                        utm = report.utm_query(),
                    ),
                    name,
                );
            } else {
                html.push_str(&format!("<td>{}</td>", row.label));
                if branch_column {
                    html.push_str(EMPTY_CELL);
                }
                html.push_str(EMPTY_CELL);
                html.push_str(EMPTY_CELL);
            }
            html.push_str(&format!("<td>{}</td>", row.section.status()));
            html.push_str("</tr>");
        }
        if !cut.is_empty() {
            html.push_str(&format!(
                r#"<tr><td colspan="{columns}">and {more}</td></tr>"#,
                columns = if branch_column { 5 } else { 4 },
                more = counted(cut.len(), "more report"),
            ));
        }
        html.push_str("</tbody></table>");
    }

    fn has_branch_column(&self) -> bool {
        let branch = self.writer.json_report.branch.uuid;
        self.reports()
            .any(|report| report.json_report.branch.uuid != branch)
    }

    /// Every alert of every section in one table.
    fn html_alerts_table(&self) -> String {
        let Some(columns) = self
            .reports()
            .filter(|report| report.has_alert())
            .map(ReportComment::alert_columns)
            .reduce(AlertColumns::bitor)
        else {
            return String::new();
        };
        let mut html = String::from("<table><thead><tr><th>Report</th>");
        columns.html_header_cells(&mut html);
        html.push_str("</tr></thead><tbody>");
        for row in &self.rows {
            if let Some(report) = row.section.report {
                let report_url = report.report_url();
                for alert in report.alerts() {
                    html.push_str("<tr>");
                    html_link_cell(&mut html, report_url.as_str(), &row.label);
                    report.html_alert_cells(&mut html, alert, columns);
                    html.push_str("</tr>");
                }
            }
        }
        html.push_str("</tbody></table>");
        html
    }
}

impl<'r> Section<'r> {
    pub fn new(
        ci_id: Option<String>,
        check: SectionCheck,
        report: Option<&'r ReportComment>,
    ) -> Self {
        Self {
            ci_id: ci_id.filter(|ci_id| !ci_id.is_empty()),
            check,
            report,
        }
    }

    /// Reports in start order, as a run lists them, then the sections with no report yet.
    fn start_order(&self, other: &Self) -> Ordering {
        match (self.report, other.report) {
            (Some(report), Some(other_report)) => run_order(report).cmp(&run_order(other_report)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.ci_id.cmp(&other.ci_id),
        }
        .then_with(|| self.check.id.cmp(&other.check.id))
    }

    /// The row's number, then its `--ci-id` in parentheses, as HTML.
    fn label(&self, number: Option<usize>) -> String {
        let ci_id = self
            .ci_id
            .as_deref()
            .map(|ci_id| format!("({})", escape(ci_id)));
        match (number, ci_id) {
            (Some(number), Some(ci_id)) => format!("{number} {ci_id}"),
            (Some(number), None) => number.to_string(),
            (None, Some(ci_id)) => ci_id,
            (None, None) => String::new(),
        }
    }

    fn status(&self) -> Status {
        let Some(conclusion) = self.check.conclusion.as_deref() else {
            return Status::Pending;
        };
        if let Some(report) = self.report
            && report.has_alert()
        {
            Status::Alerts(report.alerts().len())
        } else if conclusion == SUCCESS {
            Status::NoAlerts
        } else {
            Status::Failed
        }
    }
}

/// The console's order for a run's reports, with the v7 report UUID in place of its row ID.
fn run_order(report: &ReportComment) -> impl Ord {
    let report = &report.json_report;
    (
        report.start_time.into_inner(),
        report.end_time.into_inner(),
        report.created.into_inner(),
        report.uuid,
    )
}

/// A section in its place in the comment, with its label as HTML.
struct Row<'r> {
    label: String,
    section: Section<'r>,
}

impl Row<'_> {
    /// The notices and the benchmark tables of the section's report, in a `<details>` block.
    /// Its alerts are in the alerts table.
    fn html_details(&self, require_threshold: bool) -> Option<String> {
        let report = self.section.report?;
        let alerts_len = report.alerts().len();
        let open = if alerts_len > 0 { " open" } else { "" };
        let mut html = format!(
            "<details{open}><summary>Report {label}: {benchmarks}",
            label = self.label,
            benchmarks = counted(report.benchmark_count, "benchmark"),
        );
        if alerts_len > 0 {
            html.push_str(&format!(", {}", counted(alerts_len, "alert")));
        }
        html.push_str("</summary><br />");
        report.html_no_benchmarks(&mut html, false);
        report.html_no_threshold(&mut html, require_threshold, false);
        if report.benchmark_count > 0 {
            report.html_iteration_tables(&mut html, require_threshold);
        }
        html.push_str("</details>");
        Some(html)
    }
}

/// The name of an adapter and the id of its heading on the docs adapters page, percent encoded.
/// The server stores a language-level adapter as Magic, and a format version takes its family's heading.
fn adapter_docs(adapter: Adapter) -> (&'static str, &'static str) {
    match adapter {
        Adapter::Magic
        | Adapter::Rust
        | Adapter::Cpp
        | Adapter::Go
        | Adapter::Java
        | Adapter::CSharp
        | Adapter::Js
        | Adapter::Python
        | Adapter::Ruby
        | Adapter::Shell
        | Adapter::Dart => ("Magic", "-magic-default"),
        Adapter::Json | Adapter::JsonV0 | Adapter::JsonV1 => ("JSON", "-json"),
        Adapter::RustBench | Adapter::RustBenchJson | Adapter::RustBenchText => {
            ("Rust Bench", "-rust-bench")
        },
        Adapter::RustCriterion | Adapter::RustCriterionJson | Adapter::RustCriterionText => {
            ("Rust Criterion", "-rust-criterion")
        },
        Adapter::RustIai => ("Rust Iai", "-rust-iai"),
        Adapter::RustGungraun | Adapter::RustGungraunStdout | Adapter::RustGungraunJson => {
            ("Rust Gungraun", "-rust-gungraun")
        },
        Adapter::CppGoogle => ("C++ Google", "-c-google"),
        Adapter::CppCatch2 | Adapter::CppCatch2Xml | Adapter::CppCatch2Text => {
            ("C++ Catch2", "-c-catch2")
        },
        Adapter::GoBench | Adapter::GoBenchJson | Adapter::GoBenchText => ("Go Bench", "-go-bench"),
        Adapter::JavaJmh => ("Java JMH", "%EF%B8%8F-java-jmh"),
        Adapter::CSharpDotNet => ("C# DotNet", "%EF%B8%8F%E2%83%A3-c-dotnet"),
        Adapter::JsBenchmark => ("JavaScript Benchmark", "-javascript-benchmark"),
        Adapter::JsTime => ("JavaScript Time", "-javascript-time"),
        Adapter::JsVitest | Adapter::JsVitestV1 | Adapter::JsVitestV5 => {
            ("JavaScript Vitest", "-javascript-vitest")
        },
        Adapter::PythonAsv => ("Python ASV", "-python-asv"),
        Adapter::PythonPytest => ("Python Pytest", "-python-pytest"),
        Adapter::RubyBenchmark => ("Ruby Benchmark", "%EF%B8%8F-ruby-benchmark"),
        Adapter::ShellHyperfine | Adapter::ShellHyperfineV1 | Adapter::ShellHyperfineV2 => {
            ("Shell Hyperfine", "_%EF%B8%8F-shell-hyperfine")
        },
        Adapter::DartBenchmarkHarness => ("Dart benchmark_harness", "-dart-benchmark_harness"),
    }
}

enum Status {
    Pending,
    Alerts(usize),
    NoAlerts,
    Failed,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "⏳ Pending"),
            Self::Alerts(alerts_len) => write!(f, "🚨 {}", counted(*alerts_len, "Alert")),
            Self::NoAlerts => write!(f, "✅ No Alerts"),
            Self::Failed => write!(f, "❌ Failed"),
        }
    }
}

fn html_link_cell(html: &mut String, url: &str, text: &str) {
    html.push_str(&format!(r#"<td><a href="{url}">{text}</a></td>"#));
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bencher_json::{
        DateTime, GitHash, JsonReport,
        project::{Visibility, boundary::BoundaryLimit, report::Adapter},
    };

    use super::{GroupComment, Section, escape};
    use crate::{
        ReportComment, SectionCheck, TRUNCATED_NOTICE, group_tag,
        tests::{
            job_report, json_report, named_alert_report, named_alert_results, report_comment,
            report_comment_for, report_on, value_less_results, with_alerts,
        },
    };

    const REPORT_A: &str = "a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0";
    const REPORT_B: &str = "b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0";
    const REPORT_C: &str = "c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0";
    const REPORT_D: &str = "d0d0d0d0-d0d0-d0d0-d0d0-d0d0d0d0d0d0";
    const MAIN: (&str, &str) = ("33333333-3333-3333-3333-333333333333", "main");
    const FEATURE: (&str, &str) = ("f1f1f1f1-f1f1-f1f1-f1f1-f1f1f1f1f1f1", "feature");
    const TESTBED_A: (&str, &str) = ("a5a5a5a5-a5a5-a5a5-a5a5-a5a5a5a5a5a5", "testbed-a");
    const TESTBED_B: (&str, &str) = ("b5b5b5b5-b5b5-b5b5-b5b5-b5b5b5b5b5b5", "testbed-b");
    const TESTBED_C: (&str, &str) = ("c5c5c5c5-c5c5-c5c5-c5c5-c5c5c5c5c5c5", "testbed-c");
    const REPORTS: &str = "https://bencher.example.com/console/projects/my-project/reports";
    const MAX_LENGTH: usize = 1 << 16;

    fn head_sha() -> GitHash {
        "0123456789abcdef0123456789abcdef01234567".parse().unwrap()
    }

    fn check(id: u64, conclusion: Option<&str>) -> SectionCheck {
        SectionCheck {
            id,
            name: format!("Bencher Report ({id})"),
            conclusion: conclusion.map(str::to_owned),
            external_id: format!("bencher:{id}"),
        }
    }

    fn section<'r>(ci_id: Option<&str>, id: u64, report: Option<&'r ReportComment>) -> Section<'r> {
        Section::new(ci_id.map(str::to_owned), check(id, Some("success")), report)
    }

    fn group<'r>(
        ci_id: Option<&str>,
        writer: &'r ReportComment,
        others: Vec<Section<'r>>,
    ) -> GroupComment<'r> {
        GroupComment::new(
            ci_id.map(str::to_owned),
            check(1, Some("success")),
            writer,
            others,
            head_sha(),
        )
    }

    /// The report, started `seconds` before the fixtures' start time.
    fn started_before(mut report: JsonReport, seconds: u64) -> JsonReport {
        report.start_time = (report.start_time.into_inner() - Duration::from_secs(seconds)).into();
        report
    }

    fn quiet(report: &str, testbed: (&str, &str)) -> ReportComment {
        let mut report = with_alerts(report_on(report, MAIN, testbed), BoundaryLimit::Lower, 2);
        report.alerts = Some(Vec::new());
        report_comment_for(report)
    }

    /// One benchmark whose `latency` measure has no threshold.
    fn unchecked(report: &str, testbed: (&str, &str)) -> ReportComment {
        let mut report = report_on(report, MAIN, testbed);
        report.results = Some(value_less_results());
        report_comment_for(report)
    }

    fn cloud(report: JsonReport) -> ReportComment {
        ReportComment::new(
            "https://bencher.dev".parse().unwrap(),
            report,
            crate::SubAdapter {
                build_time: false,
                file_size: false,
            },
            "cli".to_owned(),
        )
    }

    /// `len` reports on Bencher Cloud as a large matrix makes them: each with a `--ci-id`,
    /// and every fourth with an alert.
    fn matrix(len: u64) -> Vec<(String, SectionCheck, ReportComment)> {
        (1..=len)
            .map(|n| {
                let uuid = format!("{n:08x}-1111-4111-8111-111111111111");
                let mut report = report_on(&uuid, MAIN, (TESTBED_A.0, "ubuntu-latest"));
                report.adapter = Adapter::RustCriterion;
                report.results = Some(serde_json::from_value(named_alert_results()).unwrap());
                let alerting = n % 4 == 0;
                if alerting {
                    report = with_alerts(report, BoundaryLimit::Upper, 1);
                }
                let ci_id = format!("shard-{n:02}");
                let check = SectionCheck {
                    id: 40_000_000_000 + n,
                    name: format!("Bencher Report ({ci_id})"),
                    conclusion: Some(if alerting { "failure" } else { "success" }.to_owned()),
                    external_id: format!("bencher:11111111-1111-1111-1111-111111111111:{uuid}:c"),
                };
                (ci_id, check, cloud(report))
            })
            .collect()
    }

    fn matrix_group(sections: &[(String, SectionCheck, ReportComment)]) -> GroupComment<'_> {
        let ((ci_id, check, writer), others) = sections.split_first().unwrap();
        let others = others
            .iter()
            .map(|(ci_id, check, report)| {
                Section::new(Some(ci_id.clone()), check.clone(), Some(report))
            })
            .collect();
        GroupComment::new(
            Some(ci_id.clone()),
            check.clone(),
            writer,
            others,
            head_sha(),
        )
    }

    fn summary_rows(html: &str) -> Vec<&str> {
        let (_, summary) = html
            .split_once("<th>Status</th></tr></thead><tbody>")
            .unwrap();
        let (summary, _) = summary.split_once("</tbody></table>").unwrap();
        summary.split("<tr>").skip(1).collect()
    }

    fn report_cell(report: &str, label: &str) -> String {
        format!(r#"<td><a href="{REPORTS}/{report}">{label}</a></td>"#)
    }

    // Kills a single section that drifts from today's comment,
    // or a cap that leaves the hidden comment and the tag out of the count.
    #[test]
    fn one_section_is_today_s_comment() {
        let comments = [
            report_comment(Visibility::Public),
            #[cfg(feature = "plus")]
            report_comment(Visibility::Private),
            report_comment_for(job_report()).with_pending(true),
            report_comment_for(job_report()).with_pending(false),
            report_comment_for({
                let mut report = json_report(Visibility::Public);
                report.results = Some(value_less_results());
                report
            }),
            report_comment_for(named_alert_report()),
            report_comment_for(with_alerts(
                json_report(Visibility::Public),
                BoundaryLimit::Upper,
                1,
            )),
            report_comment_for(with_alerts(
                json_report(Visibility::Public),
                BoundaryLimit::Lower,
                2,
            )),
        ];
        let mut boundaries = 0;
        for comment in &comments {
            for require_threshold in [false, true] {
                let check = check(1, Some("success"));
                let group = GroupComment::new(
                    Some("a".to_owned()),
                    check.clone(),
                    comment,
                    Vec::new(),
                    head_sha(),
                );
                let trailer = format!(
                    "{}{}",
                    check.html(),
                    group_tag(comment.project_slug(), &head_sha())
                );
                let today = |max_length| {
                    let html = comment.html_with_max_length(require_threshold, None, max_length);
                    let body = html.strip_suffix(&comment.bencher_tag(None)).unwrap();
                    format!("{body}{trailer}")
                };

                let full = group.html(require_threshold, usize::MAX);
                assert_eq!(full, today(usize::MAX));
                let truncated = group.html(require_threshold, 0);
                assert_eq!(truncated, today(0));
                assert_eq!(group.html(require_threshold, full.len()), full);
                if truncated != full {
                    boundaries += 1;
                    assert_eq!(group.html(require_threshold, full.len() - 1), truncated);
                }
            }
        }
        assert!(boundaries > 0, "some fixture has a truncated form");
    }

    // Kills a status order that puts alerts before an in-progress check,
    // or a conclusion before the report's alerts.
    #[test]
    fn status_order() {
        let alerting = report_comment_for(with_alerts(
            json_report(Visibility::Public),
            BoundaryLimit::Upper,
            1,
        ));
        let calm = report_comment(Visibility::Public);
        let status = |conclusion, report| {
            Section::new(None, check(1, conclusion), report)
                .status()
                .to_string()
        };
        assert_eq!(status(None, Some(&alerting)), "⏳ Pending");
        assert_eq!(status(Some("failure"), Some(&alerting)), "🚨 1 Alert");
        assert_eq!(status(Some("success"), Some(&calm)), "✅ No Alerts");
        assert_eq!(status(Some("success"), None), "✅ No Alerts");
        assert_eq!(status(Some("failure"), Some(&calm)), "❌ Failed");
        assert_eq!(status(Some("cancelled"), None), "❌ Failed");
    }

    // Kills rows in CI ID or check run order instead of start order,
    // rows with no report out of CI ID order or ahead of a report,
    // a number on a row with no report, a missing or unescaped ` (ID)`, a Report cell
    // without its link, a link or a Testbed or Adapter cell on a row with no report,
    // a Testbed cell that names or links anything but the testbed,
    // and hidden comments out of row order.
    #[test]
    fn rows_in_start_order() {
        let first = report_comment_for(started_before(report_on(REPORT_B, MAIN, TESTBED_B), 30));
        let tied = report_comment_for(started_before(report_on(REPORT_D, MAIN, TESTBED_A), 30));
        let writer = report_comment_for(started_before(report_on(REPORT_C, MAIN, TESTBED_C), 20));
        let last = report_comment_for(started_before(report_on(REPORT_A, MAIN, TESTBED_A), 10));
        let group = GroupComment::new(
            Some("zeta".to_owned()),
            check(1, Some("success")),
            &writer,
            vec![
                Section::new(Some("beta".to_owned()), check(4, None), None),
                section(Some("d"), 6, Some(&tied)),
                section(Some("a<b"), 3, Some(&last)),
                section(None, 2, Some(&first)),
                Section::new(Some("alpha".to_owned()), check(7, None), None),
            ],
            head_sha(),
        );
        let html = group.html(false, usize::MAX);

        let rows = summary_rows(&html);
        assert_eq!(rows.len(), 6, "{html}");
        assert_eq!(
            rows[0],
            format!(
                r#"{}<td><a href="https://bencher.example.com/console/projects/my-project/testbeds/testbed-b">testbed-b</a></td><td><a href="https://bencher.dev/docs/explanation/adapters/#-magic-default">Magic</a></td><td>✅ No Alerts</td></tr>"#,
                report_cell(REPORT_B, "1")
            )
        );
        for (row, (report, label)) in rows[1..4].iter().zip([
            (REPORT_D, "2 (d)"),
            (REPORT_C, "3 (zeta)"),
            (REPORT_A, "4 (a&lt;b)"),
        ]) {
            assert!(row.starts_with(&report_cell(report, label)), "{row}");
        }
        assert_eq!(
            rows[4],
            "<td>(alpha)</td><td></td><td></td><td>⏳ Pending</td></tr>"
        );
        assert_eq!(
            rows[5],
            "<td>(beta)</td><td></td><td></td><td>⏳ Pending</td></tr>"
        );

        let ids = SectionCheck::parse_all(&html)
            .into_iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [2, 6, 1, 3, 7, 4],
            "every hidden comment, in row order"
        );
    }

    // Kills a start time tie broken other than as the console orders a run's reports: by end
    // time, then created time, then report UUID. Each tie-break disagrees with the ones after
    // it and with the check run IDs, so dropping any of them, or putting the check run ID
    // first, reorders the rows.
    #[test]
    fn start_time_ties_break_as_the_console_does() {
        let at = |seconds| -> DateTime {
            (DateTime::TEST.into_inner() + Duration::from_secs(seconds)).into()
        };
        let report = |uuid, end_time, created| {
            let mut report = report_on(uuid, MAIN, TESTBED_A);
            report.end_time = at(end_time);
            report.created = at(created);
            report_comment_for(report)
        };
        let ends_first = report(REPORT_D, 1, 5);
        let created_first = report(REPORT_C, 2, 1);
        let lower_uuid = report(REPORT_A, 2, 2);
        let higher_uuid = report(REPORT_B, 2, 2);
        let html = GroupComment::new(
            None,
            check(4, Some("success")),
            &ends_first,
            vec![
                section(None, 1, Some(&higher_uuid)),
                section(None, 2, Some(&lower_uuid)),
                section(None, 3, Some(&created_first)),
            ],
            head_sha(),
        )
        .html(false, usize::MAX);
        let rows = summary_rows(&html);
        assert_eq!(rows.len(), 4, "{html}");
        let expected = [REPORT_D, REPORT_C, REPORT_A, REPORT_B];
        for (n, (row, report)) in (1..).zip(rows.iter().zip(expected)) {
            assert!(
                row.starts_with(&report_cell(report, &n.to_string())),
                "{html}"
            );
        }
    }

    // Kills an Adapter cell with the wrong name or docs heading for its adapter,
    // a language-level adapter shown as anything but Magic, as the server stores it,
    // and a harness adapter folded into Magic.
    // The fragments are the heading ids on the docs adapters page.
    #[test]
    fn adapter_cell_links_its_docs_heading() {
        let adapters = [
            (Adapter::RustCriterion, "Rust Criterion", "-rust-criterion"),
            (
                Adapter::CSharpDotNet,
                "C# DotNet",
                "%EF%B8%8F%E2%83%A3-c-dotnet",
            ),
            (
                Adapter::ShellHyperfineV2,
                "Shell Hyperfine",
                "_%EF%B8%8F-shell-hyperfine",
            ),
            (Adapter::CppGoogle, "C++ Google", "-c-google"),
            (Adapter::JsonV1, "JSON", "-json"),
            (Adapter::Rust, "Magic", "-magic-default"),
            (Adapter::Python, "Magic", "-magic-default"),
        ];
        let comments = adapters.map(|(adapter, ..)| {
            let mut report = json_report(Visibility::Public);
            report.adapter = adapter;
            report_comment_for(report)
        });
        let [writer, others @ ..] = &comments;
        let others = others
            .iter()
            .zip(2..)
            .map(|(comment, id)| section(None, id, Some(comment)))
            .collect();
        let html = group(None, writer, others).html(false, usize::MAX);
        let rows = summary_rows(&html);
        assert_eq!(rows.len(), adapters.len(), "{html}");
        for (row, (adapter, name, fragment)) in rows.iter().zip(adapters) {
            assert!(
                row.contains(&format!(
                    r#"<td><a href="https://bencher.dev/docs/explanation/adapters/#{fragment}">{name}</a></td>"#
                )),
                "{adapter}: {row}"
            );
        }
    }

    // Kills an Adapter link that drops the comment's usual UTM query.
    #[cfg(feature = "plus")]
    #[test]
    fn adapter_link_carries_the_utm_query() {
        let writer = cloud(report_on(REPORT_A, MAIN, TESTBED_A));
        let other = cloud(report_on(REPORT_B, MAIN, TESTBED_B));
        let html =
            group(None, &writer, vec![section(None, 2, Some(&other))]).html(false, usize::MAX);
        assert!(
            html.contains(r#"<a href="https://bencher.dev/docs/explanation/adapters/?utm_medium=referral&utm_source=cli&utm_content=comment&utm_campaign=pr+comments&utm_term=my-project#-magic-default">Magic</a>"#),
            "{html}"
        );
    }

    // Kills a header that links a report or a head, a Branch row or column that does not
    // follow the number of branches, a section without a report that borrows a Branch cell,
    // and an `and N more reports` line that does not span the Branch column.
    #[test]
    fn branch_row_or_column() {
        let writer = report_comment_for(report_on(REPORT_A, MAIN, TESTBED_A));
        let same = report_comment_for(report_on(REPORT_B, MAIN, TESTBED_B));
        let other = report_comment_for(report_on(REPORT_C, FEATURE, TESTBED_C));

        let html =
            group(None, &writer, vec![section(None, 2, Some(&same))]).html(false, usize::MAX);
        assert!(
            html.starts_with(
                r#"<h2><a href="https://bencher.example.com/console/projects/my-project"><img"#
            ),
            "{html}"
        );
        let (header, summary) = html.split_once("</table>").unwrap();
        assert!(
            header.ends_with(r#"<tr><td>Branch</td><td><a href="https://bencher.example.com/console/projects/my-project/branches/main">main</a></td></tr><tr><td>Commit</td><td><code>0123456</code></td></tr>"#),
            "{header}"
        );
        assert!(
            summary.contains("<th>Report</th><th>Testbed</th><th>Adapter</th><th>Status</th>"),
            "{summary}"
        );

        let group = group(
            None,
            &writer,
            vec![
                section(None, 2, Some(&same)),
                section(None, 3, Some(&other)),
                section(Some("d"), 4, None),
            ],
        );
        let html = group.html(false, usize::MAX);
        let (header, summary) = html.split_once("</table>").unwrap();
        assert!(!header.contains("<td>Branch</td>"), "{header}");
        assert!(
            summary.contains(
                "<th>Report</th><th>Branch</th><th>Testbed</th><th>Adapter</th><th>Status</th>"
            ),
            "{summary}"
        );
        assert!(
            summary.contains(&format!(
                r#"{}<td><a href="https://bencher.example.com/console/projects/my-project/branches/feature">feature</a></td>"#,
                report_cell(REPORT_C, "3")
            )),
            "{summary}"
        );
        assert!(
            summary.contains("<td>(d)</td><td></td><td></td><td></td><td>✅ No Alerts</td></tr>"),
            "{summary}"
        );

        let cut = group.html(false, 0);
        assert_eq!(
            summary_rows(&cut),
            [r#"<td colspan="5">and 4 more reports</td></tr>"#],
            "{cut}"
        );
    }

    // Kills alert columns taken from one section, an Iteration column from a section
    // without alerts, alert links taken from the writer's report, and a Report column
    // that is unlinked or does not carry the row's label.
    #[test]
    fn alerts_table_unites_columns_and_keeps_links() {
        let upper = report_comment_for(with_alerts(
            report_on(REPORT_A, MAIN, TESTBED_A),
            BoundaryLimit::Upper,
            1,
        ));
        let lower = report_comment_for(with_alerts(
            report_on(REPORT_B, MAIN, TESTBED_B),
            BoundaryLimit::Lower,
            2,
        ));
        let html = group(None, &upper, vec![section(Some("c<d"), 2, Some(&lower))])
            .html(false, usize::MAX);
        let (_, table) = html.split_once("<h3>🚨 3 Alerts</h3>").unwrap();
        let (table, _) = table.split_once("</tbody></table>").unwrap();
        assert!(
            table.starts_with("<table><thead><tr><th>Report</th><th>Iteration</th><th>Benchmark</th><th>Measure<br />Units</th><th>View</th><th>Benchmark Result<br />(Result Δ%)</th><th>Lower Boundary<br />(Limit %)</th><th>Upper Boundary<br />(Limit %)</th></tr></thead>"),
            "{table}"
        );
        let rows = table.split("<tr>").skip(2).collect::<Vec<_>>();
        let expected = [
            (REPORT_A, "1", 0),
            (REPORT_B, "2 (c&lt;d)", 0),
            (REPORT_B, "2 (c&lt;d)", 1),
        ];
        assert_eq!(rows.len(), expected.len(), "{table}");
        for (row, (report, label, iteration)) in rows.iter().zip(expected) {
            assert!(
                row.starts_with(&format!(
                    "{}<td>{iteration}</td>",
                    report_cell(report, label)
                )),
                "{row}"
            );
            let links = row.matches("report=").count();
            assert_eq!(
                row.matches(&format!("report={report}")).count(),
                links,
                "{row}"
            );
            assert!(links > 0, "{row}");
        }

        let html = group(
            None,
            &upper,
            vec![section(None, 2, Some(&quiet(REPORT_B, TESTBED_B)))],
        )
        .html(false, usize::MAX);
        assert!(
            html.contains("<h3>🚨 1 Alert</h3><table><thead><tr><th>Report</th><th>Benchmark</th><th>Measure<br />Units</th><th>View</th><th>Benchmark Result<br />(Result Δ%)</th><th>Upper Boundary<br />(Limit %)</th></tr></thead>"),
            "{html}"
        );
    }

    // Kills details that open without alerts, repeat the alerts, render for a section
    // without a report, leave row order, or read without `Report ` and the row's label.
    #[test]
    fn details_for_each_section_with_a_report() {
        let alerting = report_comment_for(with_alerts(
            report_on(REPORT_A, MAIN, TESTBED_A),
            BoundaryLimit::Upper,
            1,
        ));
        let calm = quiet(REPORT_B, TESTBED_B);
        let mut job = job_report();
        job.uuid = REPORT_C.parse().unwrap();
        let pending = report_comment_for(job).with_pending(true);
        let html = group(
            None,
            &alerting,
            vec![
                section(None, 2, Some(&calm)),
                section(Some("c"), 3, None),
                Section::new(Some("d<e".to_owned()), check(4, None), Some(&pending)),
            ],
        )
        .html(false, usize::MAX);
        assert!(
            html.contains("<details open><summary>Report 1: 1 benchmark, 1 alert</summary>"),
            "{html}"
        );
        assert!(
            html.contains("<details><summary>Report 2: 2 benchmarks</summary>"),
            "{html}"
        );
        assert!(!html.contains("(c): "), "{html}");
        assert!(
            html.contains("<details><summary>Report 3 (d&lt;e): 0 benchmarks</summary><br /><blockquote><h3>Results pending until the remote job finishes.</h3></blockquote></details>"),
            "{html}"
        );
        let summaries = [
            "<summary>Report 1: ",
            "<summary>Report 2: ",
            "<summary>Report 3 (d&lt;e): ",
        ]
        .map(|summary| {
            html.find(summary)
                .unwrap_or_else(|| panic!("{summary}: {html}"))
        });
        assert!(summaries.is_sorted(), "details in row order: {html}");
        assert_eq!(html.matches("<h3>🚨").count(), 1, "{html}");
    }

    // Kills a Reduced comment when the Full one fits, or a Full one over the cap.
    #[test]
    fn full_when_it_fits() {
        let writer = unchecked(REPORT_A, TESTBED_A);
        let other = unchecked(REPORT_B, TESTBED_B);
        let group = group(None, &writer, vec![section(None, 2, Some(&other))]);
        let full = group.html(false, usize::MAX);
        assert!(full.contains("<summary>Report 2: "), "{full}");
        assert_eq!(group.html(false, full.len()), full);
        assert_ne!(group.html(false, full.len() - 1), full);
    }

    // Kills a Reduced comment that keeps a details block, loses the alert table or the
    // notice, or cuts a row.
    #[test]
    fn reduced_drops_every_details_block() {
        let one = report_comment_for(with_alerts(
            report_on(REPORT_A, MAIN, TESTBED_A),
            BoundaryLimit::Upper,
            1,
        ));
        let two = report_comment_for(with_alerts(
            report_on(REPORT_B, MAIN, TESTBED_B),
            BoundaryLimit::Lower,
            2,
        ));
        let group = group(
            None,
            &one,
            vec![section(None, 2, Some(&two)), section(Some("c"), 3, None)],
        );
        let full = group.html(false, usize::MAX);
        let (before_details, _) = full.split_once("<details open><summary>Report ").unwrap();
        let html = group.html(false, full.len() - 1);
        assert_eq!(
            html,
            format!("{before_details}{TRUNCATED_NOTICE}{}", group.trailer())
        );
        assert!(html.contains("<h3>🚨 3 Alerts</h3><table>"), "{html}");
        assert!(!html.contains(" more "), "{html}");
    }

    // Kills a Summary comment that keeps an alert row, drops the alert heading or counts
    // only some reports' alerts in it, loses the notice, cuts a row it has room for,
    // or ends the summary with an `and 0 more reports` line.
    #[test]
    fn summary_keeps_every_row_and_the_alert_total() {
        let alerting = report_comment_for(with_alerts(
            report_on(REPORT_A, MAIN, TESTBED_A),
            BoundaryLimit::Upper,
            2000,
        ));
        let calm = report_comment_for(report_on(REPORT_B, MAIN, TESTBED_B));
        let alerted = report_comment_for(with_alerts(
            report_on(REPORT_C, MAIN, TESTBED_C),
            BoundaryLimit::Upper,
            1,
        ));
        let group = group(
            None,
            &alerting,
            vec![
                section(None, 2, Some(&calm)),
                section(None, 3, Some(&alerted)),
            ],
        );
        let full = group.html(false, usize::MAX);
        let (head, _) = full.split_once("<h3>🚨").unwrap();
        let html = group.html(false, 1 << 16);
        assert_eq!(
            html,
            format!(
                "{head}<h3>🚨 2001 Alerts</h3>{TRUNCATED_NOTICE}{}",
                group.trailer()
            )
        );
        assert_eq!(summary_rows(&html).len(), 3, "{html}");
    }

    // Kills a Summary cut that keeps other than the first rows in row order, miscounts the
    // cut rows (a section with no report included) or their plural, changes the alert
    // heading's total, drops the hidden record of a cut row, keeps fewer rows than fit,
    // or does not fit.
    #[test]
    fn summary_cuts_rows_from_the_end() {
        let uuid = |n: u64| format!("{n:08x}-0000-0000-0000-000000000000");
        let comments = (1..=6)
            .map(|n| {
                report_comment_for(with_alerts(
                    report_on(&uuid(n), MAIN, TESTBED_A),
                    BoundaryLimit::Upper,
                    1,
                ))
            })
            .collect::<Vec<_>>();
        let (writer, others) = comments.split_first().unwrap();
        let mut others = others
            .iter()
            .zip(2..)
            .map(|(comment, id)| section(None, id, Some(comment)))
            .collect::<Vec<_>>();
        others.push(Section::new(Some("late".to_owned()), check(7, None), None));
        let group = group(None, writer, others);
        let trailer = group.trailer();
        let full = group.html(false, usize::MAX);
        let (head, _) = full.split_once("<h3>🚨").unwrap();
        let tail = format!("<h3>🚨 6 Alerts</h3>{TRUNCATED_NOTICE}{trailer}");

        let every_row = format!("{head}{tail}");
        assert_eq!(group.html(false, every_row.len()), every_row);

        let mut max_length = every_row.len();
        for (kept, more) in [(6, "1 more report"), (5, "2 more reports")] {
            max_length -= 1;
            let html = group.html(false, max_length);
            assert!(html.len() <= max_length, "{max_length}: {html}");
            assert!(html.ends_with(&format!("</tbody></table>{tail}")), "{html}");
            let rows = summary_rows(&html);
            assert_eq!(rows.len(), kept + 1, "{html}");
            for (row, n) in rows.iter().zip(1..) {
                if n <= kept {
                    assert!(
                        row.starts_with(&report_cell(&uuid(n.try_into().unwrap()), &n.to_string())),
                        "{row}"
                    );
                } else {
                    assert_eq!(*row, format!(r#"<td colspan="4">and {more}</td></tr>"#));
                }
            }
            max_length = html.len();
        }

        let html = group.html(false, 0);
        assert_eq!(
            summary_rows(&html),
            [r#"<td colspan="4">and 7 more reports</td></tr>"#],
            "{html}"
        );
        assert!(html.ends_with(&tail), "{html}");
        let ids = SectionCheck::parse_all(&html)
            .into_iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, (1..=7).collect::<Vec<_>>(), "{html}");
    }

    // Kills a Summary comment that cuts a row it has room for once the comment has more than
    // 64 rows, that caps its rows at 64, or that drops a cut row's hidden record, at the size
    // of a large matrix. With `plus`, every link carries the UTM query, so Summary cuts rows
    // and the kill is a row that would still fit; without it, every row fits, so the kill is
    // a cut row.
    #[test]
    fn ninety_six_reports_keep_every_row_that_fits() {
        let sections = matrix(96);
        let group = matrix_group(&sections);
        let html = group.html(false, MAX_LENGTH);
        assert!(html.len() <= MAX_LENGTH, "{}", html.len());
        assert_eq!(SectionCheck::parse_all(&html).len(), 96, "{html}");
        assert!(
            html.contains(&format!(
                "</tbody></table><h3>🚨 24 Alerts</h3>{TRUNCATED_NOTICE}"
            )),
            "Summary: {html}"
        );
        assert!(
            !html.contains("<th>Benchmark</th>"),
            "no alert table: {html}"
        );

        let full = group.html(false, usize::MAX);
        let every_row = summary_rows(&full);
        let rows = summary_rows(&html);
        let kept = rows
            .iter()
            .take_while(|row| !row.starts_with("<td colspan="))
            .count();
        assert_eq!(rows[..kept], every_row[..kept], "{html}");
        if cfg!(feature = "plus") {
            assert!(kept > 64, "{kept} rows: {html}");
            let more = 96 - kept;
            assert_eq!(
                rows[kept..],
                [format!(
                    r#"<td colspan="4">and {more} more reports</td></tr>"#
                )],
                "{html}"
            );
            assert!(
                html.len() + "<tr>".len() + every_row[kept].len() > MAX_LENGTH,
                "room for row {}: {html}",
                kept + 1
            );
        } else {
            assert_eq!((kept, rows.len()), (96, 96), "{html}");
        }
    }

    // Kills a Reduced or Summary comment that cuts a row it has room for at 64 rows,
    // which lands in Summary with `plus` and in Reduced without it.
    #[test]
    fn sixty_four_reports_keep_every_row() {
        let sections = matrix(64);
        let group = matrix_group(&sections);
        let html = group.html(false, MAX_LENGTH);
        assert!(html.len() <= MAX_LENGTH, "{}", html.len());
        assert_eq!(summary_rows(&html).len(), 64, "{html}");
        assert!(!html.contains(" more report"), "{html}");
        assert!(
            !html.contains("<summary>Report "),
            "no details block: {html}"
        );
        let after_heading = if cfg!(feature = "plus") {
            TRUNCATED_NOTICE
        } else {
            "<table>"
        };
        assert!(
            html.contains(&format!(
                "</tbody></table><h3>🚨 16 Alerts</h3>{after_heading}"
            )),
            "{html}"
        );
        assert!(
            html.ends_with(&format!("{TRUNCATED_NOTICE}{}", group.trailer())),
            "{html}"
        );
    }

    // Kills a section that keeps an empty `--ci-id`, so a label shows ` ()` or `()`.
    #[test]
    fn empty_ci_id_is_no_ci_id() {
        let writer = report_comment_for(with_alerts(
            report_on(REPORT_A, MAIN, TESTBED_A),
            BoundaryLimit::Upper,
            1,
        ));
        let html = |ci_id: Option<&str>| {
            group(
                ci_id,
                &writer,
                vec![Section::new(ci_id.map(str::to_owned), check(2, None), None)],
            )
            .html(false, usize::MAX)
        };
        assert_eq!(html(Some("")), html(None));
    }

    // Kills details that render the threshold notice or the tables without `require_threshold`.
    #[test]
    fn details_follow_require_threshold() {
        let writer = unchecked(REPORT_A, TESTBED_A);
        let group = group(None, &writer, vec![section(Some("b"), 2, None)]);
        let loose = group.html(false, usize::MAX);
        assert!(loose.contains("No Threshold found"), "{loose}");
        assert!(loose.contains(">Latency</a></th>"), "{loose}");
        let strict = group.html(true, usize::MAX);
        assert!(!strict.contains("No Threshold found"), "{strict}");
        assert!(!strict.contains(">Latency</a></th>"), "{strict}");
    }

    // Kills a record parser that reads a record planted in a benchmark name,
    // which renders unescaped.
    #[test]
    fn record_planted_in_a_benchmark_name_is_not_read() {
        let planted = check(99, Some("success")).html();
        let name = format!("{planted}<div id=\"bencher.dev/projects/");
        let mut report = report_on(REPORT_A, MAIN, TESTBED_A);
        let mut results = named_alert_results();
        results[0][0]["benchmark"]["name"] = name.clone().into();
        report.results = Some(serde_json::from_value(results).unwrap());
        let writer = report_comment_for(report);
        for (others, ids) in [
            (Vec::new(), vec![1]),
            (vec![section(Some("b"), 2, None)], vec![1, 2]),
        ] {
            let html = group(None, &writer, others).html(false, usize::MAX);
            assert!(html.contains(&format!(">{name}</a>")), "{html}");
            let read = SectionCheck::parse_all(&html)
                .into_iter()
                .map(|check| check.id)
                .collect::<Vec<_>>();
            assert_eq!(read, ids, "{html}");
        }
    }

    // Kills an escape that lets a quote or an ampersand through.
    #[test]
    fn escape_html_text() {
        assert_eq!(
            escape(r#"<a title="x">&</a>"#),
            "&lt;a title=&quot;x&quot;&gt;&amp;&lt;/a&gt;"
        );
    }

    // Kills gates that read only the writer's report.
    #[test]
    fn gates_read_every_report() {
        let writer = report_comment(Visibility::Public);
        let other = report_comment_for(with_alerts(
            report_on(REPORT_B, MAIN, TESTBED_B),
            BoundaryLimit::Upper,
            1,
        ));
        let alone = group(None, &writer, Vec::new());
        assert!(!alone.has_alert() && !alone.has_threshold());
        let both = group(
            None,
            &writer,
            vec![section(None, 2, Some(&other)), section(Some("c"), 3, None)],
        );
        assert!(both.has_alert() && both.has_threshold());
    }
}
