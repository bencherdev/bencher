use std::{fmt, ops::BitOr as _};

use bencher_json::GitHash;
use url::Url;

use crate::{
    AlertColumns, BENCHER_REPORT_TITLE, EMPTY_CELL, ReportComment, Resource, SectionCheck,
    TRUNCATED_NOTICE, counted, html_alerts_heading, html_link_row, tag::group_tag,
};

const SHORT_SHA_LEN: usize = 7;
const SUCCESS: &str = "success";

/// The one pull request comment that every job of a Project writes, with a section per job.
pub struct GroupComment<'r> {
    writer: &'r ReportComment,
    sections: Vec<Section<'r>>,
    head_sha: GitHash,
}

/// The section of one job: its label, the check run it ran under, and its report once fetched.
pub struct Section<'r> {
    label: String,
    check: SectionCheck,
    report: Option<&'r ReportComment>,
}

impl<'r> GroupComment<'r> {
    /// The writer's own section always has its report, and the header links come from it.
    pub fn new(
        label: String,
        check: SectionCheck,
        report: &'r ReportComment,
        others: Vec<Section<'r>>,
        head_sha: GitHash,
    ) -> Self {
        let mut sections = others;
        sections.push(Section::new(label, check, Some(report)));
        sections.sort_by(|a, b| {
            a.label
                .cmp(&b.label)
                .then_with(|| a.check.id.cmp(&b.check.id))
        });
        Self {
            writer: report,
            sections,
            head_sha,
        }
    }

    pub fn has_alert(&self) -> bool {
        self.reports().any(ReportComment::has_alert)
    }

    pub fn has_threshold(&self) -> bool {
        self.reports().any(ReportComment::has_threshold)
    }

    pub fn html(&self, require_threshold: bool, max_length: usize) -> String {
        let trailer = self.trailer();
        if self.sections.len() == 1 {
            return self
                .writer
                .html_with_tag(require_threshold, &trailer, max_length);
        }

        let mut html = String::new();
        self.html_header(&mut html);
        self.html_summary(&mut html);
        let mut alerts = self.alerts_block();
        let mut details = self
            .sections
            .iter()
            .filter_map(|section| section.details_block(require_threshold))
            .collect::<Vec<_>>();
        fit(
            &mut alerts,
            &mut details,
            html.len() + trailer.len(),
            TRUNCATED_NOTICE.len(),
            max_length,
        );
        html.push_str(alerts.html());
        for block in &details {
            html.push_str(block.html());
        }
        // A reader can no longer see every section, so the comment says it is partial.
        if has_dropped(&details) {
            html.push_str(TRUNCATED_NOTICE);
        }
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        html.push_str(&trailer);
        html
    }

    fn reports(&self) -> impl Iterator<Item = &'r ReportComment> {
        self.sections.iter().filter_map(|section| section.report)
    }

    fn trailer(&self) -> String {
        let mut trailer = self
            .sections
            .iter()
            .map(|section| section.check.html())
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
            // Each job with `--start-point-reset` makes its own head, so no head is linked.
            html_link_row(
                html,
                "Branch",
                writer.json_report.branch.name.as_ref(),
                &writer.branch_url(None),
            );
        }
        let head_sha = self.head_sha.as_ref();
        html.push_str(&format!(
            "<tr><td>Commit</td><td><code>{short_sha}</code></td></tr>",
            short_sha = head_sha.get(..SHORT_SHA_LEN).unwrap_or(head_sha),
        ));
        html.push_str("</table>");
    }

    fn html_summary(&self, html: &mut String) {
        let branch_column = self.has_branch_column();
        html.push_str("<table><thead><tr><th>Section</th>");
        if branch_column {
            html.push_str("<th>Branch</th>");
        }
        html.push_str("<th>Testbed</th><th>Status</th><th>Report</th></tr></thead><tbody>");
        for section in &self.sections {
            html.push_str(&format!("<tr><td>{}</td>", escape(&section.label)));
            if branch_column {
                html_link_cell(
                    html,
                    section.report.map(|report| {
                        (
                            report.branch_url(None),
                            report.json_report.branch.name.to_string(),
                        )
                    }),
                );
            }
            html_link_cell(
                html,
                section.report.map(|report| {
                    (
                        report.testbed_url(),
                        report.json_report.testbed.name.to_string(),
                    )
                }),
            );
            html.push_str(&format!("<td>{}</td>", section.status()));
            if let Some(report) = section.report {
                html.push_str(&format!(
                    r#"<td>🐰 <a href="{url}">view report</a></td>"#,
                    url = report.resource_url(Resource::Report(report.json_report.uuid)),
                ));
            } else {
                html.push_str(EMPTY_CELL);
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody></table>");
    }

    fn has_branch_column(&self) -> bool {
        let branch = self.writer.json_report.branch.uuid;
        self.reports()
            .any(|report| report.json_report.branch.uuid != branch)
    }

    /// Every alert of every section in one table, falling back to its heading at the cap.
    fn alerts_block(&self) -> Block {
        let sections = self
            .sections
            .iter()
            .filter_map(|section| {
                section
                    .report
                    .filter(|report| report.has_alert())
                    .map(|report| (section, report))
            })
            .collect::<Vec<_>>();
        let Some(columns) = sections
            .iter()
            .map(|(_, report)| report.alert_columns())
            .reduce(AlertColumns::bitor)
        else {
            return Block::new(String::new(), String::new());
        };
        let mut truncated = String::new();
        html_alerts_heading(
            &mut truncated,
            sections
                .iter()
                .map(|(_, report)| report.alerts().len())
                .sum(),
        );

        let mut full = truncated.clone();
        full.push_str("<table><thead><tr><th>Section</th>");
        columns.html_header_cells(&mut full);
        full.push_str("</tr></thead><tbody>");
        for (section, report) in sections {
            for alert in report.alerts() {
                full.push_str(&format!("<tr><td>{}</td>", escape(&section.label)));
                report.html_alert_cells(&mut full, alert, columns);
                full.push_str("</tr>");
            }
        }
        full.push_str("</tbody></table>");
        Block::new(full, truncated)
    }
}

impl<'r> Section<'r> {
    /// `label` is plain text, escaped when rendered.
    pub fn new(label: String, check: SectionCheck, report: Option<&'r ReportComment>) -> Self {
        Self {
            label,
            check,
            report,
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

    /// The notices and the benchmark tables of the section's report, in a `<details>` block.
    /// Its alerts are in the alerts table.
    fn details_block(&self, require_threshold: bool) -> Option<Block> {
        let report = self.report?;
        let alerts_len = report.alerts().len();
        let mut summary = format!(
            "{label}: {benchmarks}",
            label = escape(&self.label),
            benchmarks = counted(report.benchmark_count, "benchmark"),
        );
        if alerts_len > 0 {
            summary.push_str(&format!(", {}", counted(alerts_len, "alert")));
        }
        let open = if alerts_len > 0 { " open" } else { "" };
        let details = |truncated: bool| {
            let mut html = format!("<details{open}><summary>{summary}</summary><br />");
            report.html_no_benchmarks(&mut html, truncated);
            report.html_no_threshold(&mut html, require_threshold, truncated);
            if report.benchmark_count > 0 && !truncated {
                report.html_iteration_tables(&mut html, require_threshold);
            }
            html.push_str("</details>");
            html
        };
        Some(Block::new(details(false), details(true)))
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

/// A part of the body that gives way at the cap.
struct Block {
    full: String,
    truncated: String,
    form: Form,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Full,
    Truncated,
    Dropped,
}

impl Block {
    fn new(full: String, truncated: String) -> Self {
        Self {
            full,
            truncated,
            form: Form::Full,
        }
    }

    fn html(&self) -> &str {
        match self.form {
            Form::Full => &self.full,
            Form::Truncated => &self.truncated,
            Form::Dropped => "",
        }
    }
}

fn fit(
    alerts: &mut Block,
    details: &mut [Block],
    fixed_len: usize,
    notice_len: usize,
    max_length: usize,
) {
    let body_len = |alerts: &Block, details: &[Block]| {
        let notice_len = if has_dropped(details) { notice_len } else { 0 };
        fixed_len
            + notice_len
            + alerts.html().len()
            + details
                .iter()
                .map(|block| block.html().len())
                .sum::<usize>()
    };
    while body_len(alerts, details) > max_length {
        if let Some(block) = largest(details, Form::Full) {
            block.form = Form::Truncated;
        } else if let Some(block) = largest(details, Form::Truncated) {
            block.form = Form::Dropped;
        } else if alerts.form == Form::Full {
            alerts.form = Form::Truncated;
        } else {
            break;
        }
    }
}

fn largest(blocks: &mut [Block], form: Form) -> Option<&mut Block> {
    blocks
        .iter_mut()
        .filter(|block| block.form == form)
        .max_by_key(|block| block.html().len())
}

fn has_dropped(details: &[Block]) -> bool {
    details.iter().any(|block| block.form == Form::Dropped)
}

fn html_link_cell(html: &mut String, link: Option<(Url, String)>) {
    if let Some((url, name)) = link {
        html.push_str(&format!(r#"<td><a href="{url}">{name}</a></td>"#));
    } else {
        html.push_str(EMPTY_CELL);
    }
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
    use bencher_json::{
        GitHash,
        project::{Visibility, boundary::BoundaryLimit},
    };

    use super::{Block, Form, GroupComment, Section, escape, fit};
    use crate::{
        ReportComment, SectionCheck, group_tag,
        tests::{
            job_report, json_report, named_alert_report, named_alert_results, report_comment,
            report_comment_for, report_on, value_less_results, with_alerts,
        },
    };

    const REPORT_A: &str = "a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0";
    const REPORT_B: &str = "b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0";
    const REPORT_C: &str = "c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0";
    const MAIN: (&str, &str) = ("33333333-3333-3333-3333-333333333333", "main");
    const FEATURE: (&str, &str) = ("f1f1f1f1-f1f1-f1f1-f1f1-f1f1f1f1f1f1", "feature");
    const TESTBED_A: (&str, &str) = ("a5a5a5a5-a5a5-a5a5-a5a5-a5a5a5a5a5a5", "testbed-a");
    const TESTBED_B: (&str, &str) = ("b5b5b5b5-b5b5-b5b5-b5b5-b5b5b5b5b5b5", "testbed-b");
    const TESTBED_C: (&str, &str) = ("c5c5c5c5-c5c5-c5c5-c5c5-c5c5c5c5c5c5", "testbed-c");
    const NOTICE: &str = "<blockquote><h3>⚠️ WARNING: Truncated view!</h3><p>The full continuous benchmarking report exceeds the maximum length allowed on this platform.</p></blockquote>";

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

    fn section<'r>(label: &str, id: u64, report: Option<&'r ReportComment>) -> Section<'r> {
        Section::new(label.to_owned(), check(id, Some("success")), report)
    }

    fn group<'r>(
        label: &str,
        writer: &'r ReportComment,
        others: Vec<Section<'r>>,
    ) -> GroupComment<'r> {
        GroupComment::new(
            label.to_owned(),
            check(1, Some("success")),
            writer,
            others,
            head_sha(),
        )
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
                    "a".to_owned(),
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
            Section::new("a".to_owned(), check(1, conclusion), report)
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

    // Kills sections that are not sorted by label then check run ID, a Testbed cell
    // that names or links anything but the testbed, a section without a report that
    // borrows another's cells, and an unescaped label.
    #[test]
    fn sections_order_by_label_then_check_run() {
        let first = report_comment_for(report_on(REPORT_A, MAIN, TESTBED_A));
        let second = report_comment_for(report_on(REPORT_B, MAIN, TESTBED_B));
        let writer = report_comment_for(report_on(REPORT_C, MAIN, TESTBED_C));
        let group = GroupComment::new(
            "b".to_owned(),
            check(5, Some("success")),
            &writer,
            vec![
                Section::new("c<d".to_owned(), check(1, None), None),
                section("a", 9, Some(&second)),
                section("a", 3, Some(&first)),
            ],
            head_sha(),
        );
        let html = group.html(false, usize::MAX);

        let (_, summary) = html.split_once("<th>Section</th>").unwrap();
        let (summary, _) = summary.split_once("</tbody></table>").unwrap();
        let rows = summary.split("<tr>").skip(1).collect::<Vec<_>>();
        assert_eq!(rows.len(), 4, "{summary}");
        assert_eq!(
            rows[0],
            r#"<td>a</td><td><a href="https://bencher.example.com/perf/my-project/testbeds/testbed-a">testbed-a</a></td><td>✅ No Alerts</td><td>🐰 <a href="https://bencher.example.com/perf/my-project/reports/a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0">view report</a></td></tr>"#
        );
        assert!(rows[1].starts_with("<td>a</td>") && rows[1].contains(REPORT_B));
        assert!(rows[2].starts_with("<td>b</td>") && rows[2].contains(REPORT_C));
        assert_eq!(
            rows[3],
            "<td>c&lt;d</td><td></td><td>⏳ Pending</td><td></td></tr>"
        );

        let ids = SectionCheck::parse_all(&html)
            .into_iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, [3, 9, 5, 1], "every hidden comment, in label order");
    }

    // Kills a header that links a report or a head, a Branch row or column that does not
    // follow the number of branches, and a section without a report that borrows a Branch cell.
    #[test]
    fn branch_row_or_column() {
        let writer = report_comment_for(report_on(REPORT_A, MAIN, TESTBED_A));
        let same = report_comment_for(report_on(REPORT_B, MAIN, TESTBED_B));
        let other = report_comment_for(report_on(REPORT_C, FEATURE, TESTBED_C));

        let html = group("a", &writer, vec![section("b", 2, Some(&same))]).html(false, usize::MAX);
        assert!(
            html.starts_with(r#"<h2><a href="https://bencher.example.com/perf/my-project"><img"#),
            "{html}"
        );
        let (header, summary) = html.split_once("</table>").unwrap();
        assert!(
            header.ends_with(r#"<tr><td>Branch</td><td><a href="https://bencher.example.com/perf/my-project/branches/main">main</a></td></tr><tr><td>Commit</td><td><code>0123456</code></td></tr>"#),
            "{header}"
        );
        assert!(!summary.contains("<th>Branch</th>"), "{summary}");

        let html = group(
            "a",
            &writer,
            vec![
                section("b", 2, Some(&same)),
                section("c", 3, Some(&other)),
                section("d", 4, None),
            ],
        )
        .html(false, usize::MAX);
        let (header, summary) = html.split_once("</table>").unwrap();
        assert!(!header.contains("<td>Branch</td>"), "{header}");
        assert!(
            summary.contains("<th>Section</th><th>Branch</th><th>Testbed</th>"),
            "{summary}"
        );
        assert!(
            summary.contains(r#"<td>c</td><td><a href="https://bencher.example.com/perf/my-project/branches/feature">feature</a></td>"#),
            "{summary}"
        );
        assert!(
            summary.contains("<td>d</td><td></td><td></td><td>✅ No Alerts</td><td></td></tr>"),
            "{summary}"
        );
    }

    // Kills alert columns taken from one section, an Iteration column from a section
    // without alerts, alert links taken from the writer's report, and a label left
    // unescaped in the alert rows or the details summary.
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
        let html =
            group("a", &upper, vec![section("c<d", 2, Some(&lower))]).html(false, usize::MAX);
        assert!(
            html.contains("<details open><summary>c&lt;d: 2 benchmarks, 2 alerts</summary>"),
            "{html}"
        );
        let (_, table) = html.split_once("<h3>🚨 3 Alerts</h3>").unwrap();
        let (table, _) = table.split_once("</tbody></table>").unwrap();
        assert!(
            table.starts_with("<table><thead><tr><th>Section</th><th>Iteration</th><th>Benchmark</th><th>Measure<br />Units</th><th>View</th><th>Benchmark Result<br />(Result Δ%)</th><th>Lower Boundary<br />(Limit %)</th><th>Upper Boundary<br />(Limit %)</th></tr></thead>"),
            "{table}"
        );
        let rows = table.split("<tr>").skip(2).collect::<Vec<_>>();
        let expected = [
            ("a", 0, REPORT_A),
            ("c&lt;d", 0, REPORT_B),
            ("c&lt;d", 1, REPORT_B),
        ];
        assert_eq!(rows.len(), expected.len(), "{table}");
        for (row, (label, iteration, report)) in rows.iter().zip(expected) {
            assert!(
                row.starts_with(&format!("<td>{label}</td><td>{iteration}</td>")),
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
            "a",
            &upper,
            vec![section("b", 2, Some(&quiet(REPORT_B, TESTBED_B)))],
        )
        .html(false, usize::MAX);
        assert!(
            html.contains("<h3>🚨 1 Alert</h3><table><thead><tr><th>Section</th><th>Benchmark</th><th>Measure<br />Units</th><th>View</th><th>Benchmark Result<br />(Result Δ%)</th><th>Upper Boundary<br />(Limit %)</th></tr></thead>"),
            "{html}"
        );
    }

    // Kills details that open without alerts, repeat the alerts, render for a section
    // without a report, or leave label order.
    #[test]
    fn details_for_each_section_with_a_report() {
        let alerting = report_comment_for(with_alerts(
            report_on(REPORT_A, MAIN, TESTBED_A),
            BoundaryLimit::Upper,
            1,
        ));
        let calm = quiet(REPORT_B, TESTBED_B);
        let pending = report_comment_for(job_report()).with_pending(true);
        let html = group(
            "a",
            &alerting,
            vec![
                section("b", 2, Some(&calm)),
                section("c", 3, None),
                Section::new("d".to_owned(), check(4, None), Some(&pending)),
            ],
        )
        .html(false, usize::MAX);
        assert!(
            html.contains("<details open><summary>a: 1 benchmark, 1 alert</summary>"),
            "{html}"
        );
        assert!(
            html.contains("<details><summary>b: 2 benchmarks</summary>"),
            "{html}"
        );
        assert!(!html.contains("<summary>c: "), "{html}");
        let summaries = ["<summary>a: ", "<summary>b: ", "<summary>d: "].map(|summary| {
            html.find(summary)
                .unwrap_or_else(|| panic!("{summary}: {html}"))
        });
        assert!(summaries.is_sorted(), "details in label order: {html}");
        assert!(
            html.contains("<details><summary>d: 0 benchmarks</summary><br /><blockquote><h3>Results pending until the remote job finishes.</h3></blockquote></details>"),
            "{html}"
        );
        assert_eq!(html.matches("<h3>🚨").count(), 1, "{html}");
    }

    // Kills a cap that drops before it truncates, takes the smallest block first,
    // touches the alerts table before every details block is gone,
    // or leaves the partial notice out of the count once a block drops.
    #[test]
    fn cap_shrinks_in_order() {
        use Form::{Dropped as D, Full as F, Truncated as T};

        let forms = |max_length| {
            let mut alerts = Block::new("a".repeat(500), "a".repeat(20));
            let mut details = [(100, 50), (300, 60), (200, 40)]
                .map(|(full, truncated)| Block::new("d".repeat(full), "d".repeat(truncated)));
            fit(&mut alerts, &mut details, 10, 30, max_length);
            (alerts.form, details.map(|block| block.form))
        };
        assert_eq!(forms(1110), (F, [F, F, F]));
        assert_eq!(forms(1109), (F, [F, T, F]));
        assert_eq!(forms(700), (F, [T, T, T]));
        assert_eq!(forms(630), (F, [T, D, T]));
        assert_eq!(forms(600), (F, [D, D, T]));
        assert_eq!(forms(540), (F, [D, D, D]));
        assert_eq!(forms(539), (T, [D, D, D]));
        assert_eq!(forms(0), (T, [D, D, D]));
    }

    // Kills a cap that falls back on a block other than the largest first, that drops the
    // header, the summary, the alerts heading, the hidden comments, or the tag, that adds
    // the partial notice before a block drops, or that goes over a cap it could meet.
    #[test]
    fn cap_keeps_header_summary_records_and_tag() {
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
            "a",
            &one,
            vec![section("b", 2, Some(&two)), section("c", 3, None)],
        );
        let mut head = String::new();
        group.html_header(&mut head);
        group.html_summary(&mut head);
        let trailer = group.trailer();
        let block = |label: &str, report| {
            Section::new(label.to_owned(), check(0, None), Some(report))
                .details_block(false)
                .unwrap()
        };
        let (small, large) = (block("a", &one), block("b", &two));
        assert!(large.full.len() > small.full.len());

        let full = group.html(false, usize::MAX);
        let html = group.html(false, full.len() - 1);
        assert!(
            html.contains(&large.truncated) && !html.contains(&large.full),
            "{html}"
        );
        assert!(html.contains(&small.full), "{html}");
        assert!(!html.contains(&format!("{NOTICE}<!--")), "{html}");

        let smallest = format!("{head}<h3>🚨 3 Alerts</h3>{NOTICE}{trailer}");
        assert_eq!(group.html(false, 0), smallest);
        for max_length in (0..=full.len()).step_by(16) {
            let html = group.html(false, max_length);
            assert!(html.starts_with(&head), "{max_length}: {html}");
            assert!(html.ends_with(&trailer), "{max_length}: {html}");
            assert!(
                html.contains("<h3>🚨 3 Alerts</h3>"),
                "{max_length}: {html}"
            );
            if max_length >= smallest.len() {
                assert!(html.len() <= max_length, "{max_length}: {html}");
            }
        }
    }

    // Kills a partial notice that is missing once any block drops, repeated per dropped
    // block, or not right before the hidden comments.
    #[test]
    fn dropped_section_leaves_one_notice() {
        let writer = unchecked(REPORT_A, TESTBED_A);
        let other = unchecked(REPORT_B, TESTBED_B);
        let group = group("a", &writer, vec![section("b", 2, Some(&other))]);
        let trailer = group.trailer();

        let all_dropped = group.html(false, 0);
        assert_eq!(all_dropped.matches(NOTICE).count(), 1, "{all_dropped}");
        assert!(
            all_dropped.ends_with(&format!("</table>{NOTICE}{trailer}")),
            "{all_dropped}"
        );

        let one_left = (0..all_dropped.len() * 2)
            .map(|max_length| group.html(false, max_length))
            .find(|html| html.contains("<summary>a: ") != html.contains("<summary>b: "))
            .expect("a cap that drops one block and keeps the other");
        assert!(
            one_left.ends_with(&format!("</details>{NOTICE}{trailer}")),
            "{one_left}"
        );
    }

    // Kills a truncated details block that keeps its tables or its threshold list,
    // or that loses today's truncated and threshold notices.
    #[test]
    fn truncated_details_keep_only_the_notices() {
        let writer = unchecked(REPORT_A, TESTBED_A);
        let group = group("a", &writer, vec![section("b", 2, None)]);
        let full = group.html(false, usize::MAX);
        let html = group.html(false, full.len() - 1);
        assert!(
            html.contains("<details><summary>a: 1 benchmark</summary><br /><blockquote><h3>⚠️ WARNING: Truncated view!</h3><p>The full continuous benchmarking report exceeds the maximum length allowed on this platform.</p></blockquote><blockquote><h3>⚠️ WARNING: No Threshold found!</h3><p>Without a Threshold, no Alerts will ever be generated.</p></blockquote></details><!-- bencher.dev/section "),
            "{html}"
        );
    }

    // Kills details that render the threshold notice or the tables without `require_threshold`.
    #[test]
    fn details_follow_require_threshold() {
        let writer = unchecked(REPORT_A, TESTBED_A);
        let group = group("a", &writer, vec![section("b", 2, None)]);
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
            (vec![section("b", 2, None)], vec![1, 2]),
        ] {
            let html = group("a", &writer, others).html(false, usize::MAX);
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
        let alone = group("a", &writer, Vec::new());
        assert!(!alone.has_alert() && !alone.has_threshold());
        let both = group(
            "a",
            &writer,
            vec![section("b", 2, Some(&other)), section("c", 3, None)],
        );
        assert!(both.has_alert() && both.has_threshold());
    }
}
