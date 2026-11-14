use std::collections::{BTreeMap, BTreeSet};

use bencher_json::{
    MetricName,
    project::report::{JsonReportIteration, JsonReportMeasure, JsonReportMetric, JsonReportResult},
};

use crate::{BoundaryLimits, Measure};

/// Each measure's default metrics, computed once over the whole report: the metrics a
/// threshold checked, else the first by preference, in the order their column groups take.
pub(crate) struct DefaultMetrics(BTreeMap<Measure, Vec<MetricName>>);

/// The column group of one default metric in one iteration's table.
pub(crate) struct ColumnGroup<'r> {
    pub measure: &'r Measure,
    pub metric: &'r MetricName,
    pub limits: BoundaryLimits,
}

impl DefaultMetrics {
    pub(crate) fn new(results: &[JsonReportIteration]) -> Self {
        let mut names = BTreeMap::<Measure, (BTreeSet<&MetricName>, BTreeSet<&MetricName>)>::new();
        for result in results.iter().flatten() {
            for report_measure in &result.measures {
                let (checked, all) = names
                    .entry(Measure::from(report_measure.measure.clone()))
                    .or_default();
                for report_metric in &report_measure.metrics {
                    all.insert(&report_metric.name);
                    if !report_metric.boundaries.is_empty() {
                        checked.insert(&report_metric.name);
                    }
                }
            }
        }
        Self(
            names
                .into_iter()
                .map(|(measure, (checked, all))| {
                    let mut metrics: Vec<&MetricName> = if checked.is_empty() {
                        all.into_iter()
                            .min_by_key(|&name| preference(name))
                            .into_iter()
                            .collect()
                    } else {
                        checked.into_iter().collect()
                    };
                    metrics.sort_by_key(|&name| preference(name));
                    (measure, metrics.into_iter().cloned().collect())
                })
                .collect(),
        )
    }

    pub(crate) fn get(&self, measure: &Measure) -> &[MetricName] {
        self.0.get(measure).map(Vec::as_slice).unwrap_or_default()
    }

    /// The groups of an iteration's table: each default metric that a result of the iteration
    /// holds, and with `require_threshold` only those a threshold checked in the iteration.
    pub(crate) fn column_groups(
        &self,
        iteration: &JsonReportIteration,
        require_threshold: bool,
    ) -> Vec<ColumnGroup<'_>> {
        self.0
            .iter()
            .flat_map(|(measure, metrics)| metrics.iter().map(move |metric| (measure, metric)))
            .filter_map(|(measure, metric)| {
                let mut limits: Option<BoundaryLimits> = None;
                let mut checked = false;
                for (_, report_metric) in iteration
                    .iter()
                    .filter_map(|result| report_metric(result, measure, metric))
                {
                    checked |= !report_metric.boundaries.is_empty();
                    let metric_limits = BoundaryLimits::of(report_metric);
                    limits = Some(limits.map_or(metric_limits, |limits| limits | metric_limits));
                }
                let limits = limits?;
                (checked || !require_threshold).then_some(ColumnGroup {
                    measure,
                    metric,
                    limits,
                })
            })
            .collect()
    }
}

/// The measure and the metric of a result that a column group draws.
pub(crate) fn report_metric<'r>(
    result: &'r JsonReportResult,
    measure: &Measure,
    metric: &MetricName,
) -> Option<(&'r JsonReportMeasure, &'r JsonReportMetric)> {
    let report_measure = result
        .measures
        .iter()
        .find(|report_measure| report_measure.measure.slug == measure.slug)?;
    let report_metric = report_measure
        .metrics
        .iter()
        .find(|report_metric| report_metric.name == *metric)?;
    Some((report_measure, report_metric))
}

/// `value`, `median`, and `mean` first, then name order.
fn preference(name: &MetricName) -> (u8, &MetricName) {
    let rank = match name.as_ref() {
        "value" => 0,
        "median" => 1,
        "mean" => 2,
        _ => 3,
    };
    (rank, name)
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, JsonReport, project::Visibility};
    use serde_json::{Value, json};

    use crate::{
        ReportComment,
        tests::{json_report, report_comment_for},
    };

    const PROJECT: &str = "11111111-1111-1111-1111-111111111111";
    const BENCHMARK: &str = "66666666-6666-6666-6666-666666666666";
    const VARIANT_A: &str = "77777777-7777-7777-7777-77777777777a";
    const VARIANT_B: &str = "77777777-7777-7777-7777-77777777777b";
    const VARIANT_C: &str = "77777777-7777-7777-7777-77777777777c";
    const THRESHOLD_1: &str = "10000000-0000-0000-0000-000000000000";
    const THRESHOLD_2: &str = "20000000-0000-0000-0000-000000000000";
    const THRESHOLD_3: &str = "30000000-0000-0000-0000-000000000000";
    const ALERT: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";
    const CONSOLE: &str = "https://bencher.example.com/console/projects/my-project";

    fn date() -> Value {
        serde_json::to_value(DateTime::TEST).unwrap()
    }

    fn measure(slug: &str) -> Value {
        let (n, name) = match slug {
            "allocated" => (1, "Allocated"),
            "cpu-time" => (2, "CPU Time"),
            "cycles" => (3, "Cycles"),
            "instructions" => (4, "Instructions"),
            "latency" => (5, "Latency"),
            "memory" => (6, "Memory"),
            "throughput" => (7, "Throughput"),
            _ => panic!("no measure {slug}"),
        };
        json!({
            "uuid": format!("8888888{n}-8888-8888-8888-888888888888"),
            "project": PROJECT,
            "name": name,
            "slug": slug,
            "units": "nanoseconds (ns)",
            "created": date(),
            "modified": date(),
            "archived": null,
        })
    }

    fn benchmark() -> Value {
        json!({
            "uuid": BENCHMARK,
            "project": PROJECT,
            "name": "bench",
            "slug": "bench",
            "created": date(),
            "modified": date(),
            "archived": null,
        })
    }

    /// A threshold as a boundary carries it: `metric` is absent for `value`.
    fn threshold(uuid: &str, test: &str, metric: &str) -> Value {
        let mut threshold = json!({
            "uuid": uuid,
            "project": PROJECT,
            "model": {
                "uuid": "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
                "test": test,
                "min_sample_size": null,
                "max_sample_size": null,
                "window": null,
                "lower_boundary": null,
                "upper_boundary": 0.99,
                "created": date(),
                "replaced": null,
            },
            "created": date(),
        });
        if metric != "value" {
            threshold["metric"] = metric.into();
        }
        threshold
    }

    fn boundary(
        threshold_uuid: &str,
        test: &str,
        metric: &str,
        [baseline, lower_limit, upper_limit]: [Option<f64>; 3],
    ) -> Value {
        json!({
            "threshold": threshold(threshold_uuid, test, metric),
            "boundary": {
                "baseline": baseline,
                "lower_limit": lower_limit,
                "upper_limit": upper_limit,
            },
        })
    }

    fn metric(name: &str, value: f64, boundaries: &[Value]) -> Value {
        json!({
            "uuid": "99999999-9999-9999-9999-999999999999",
            "name": name,
            "value": value,
            "boundaries": boundaries,
        })
    }

    /// A version 1 measure: no deprecated fields, so the table reads only `metrics`.
    fn report_measure(slug: &str, metrics: &[Value]) -> Value {
        json!({
            "measure": measure(slug),
            "metrics": metrics,
            "threshold": null,
            "boundary": null,
        })
    }

    fn result(iteration: u32, variant: &str, parameters: &Value, measures: &[Value]) -> Value {
        json!({
            "iteration": iteration,
            "benchmark": benchmark(),
            "variant": { "uuid": variant, "parameters": parameters },
            "measures": measures,
        })
    }

    /// A report of `results`, one array of results per iteration.
    fn report(results: Value) -> JsonReport {
        let mut report = json_report(Visibility::Public);
        report.results = Some(serde_json::from_value(results).unwrap());
        report
    }

    struct Alert<'a> {
        iteration: u32,
        variant: &'a str,
        parameters: Value,
        measure: &'a str,
        metric: &'a str,
        threshold: &'a str,
        value: f64,
        upper_limit: f64,
    }

    impl Alert<'_> {
        /// The report with this alert added, past its upper limit.
        fn on(&self, mut report: JsonReport) -> JsonReport {
            let mut threshold = json!({
                "uuid": self.threshold,
                "project": PROJECT,
                "branch": serde_json::to_value(&report.branch).unwrap(),
                "testbed": serde_json::to_value(&report.testbed).unwrap(),
                "measure": measure(self.measure),
                "model": null,
                "created": date(),
                "modified": date(),
            });
            if self.metric != "value" {
                threshold["metric"] = self.metric.into();
            }
            let alert = json!({
                "uuid": ALERT,
                "report": "00000000-0000-0000-0000-000000000000",
                "iteration": self.iteration,
                "benchmark": benchmark(),
                "variant": {
                    "uuid": self.variant,
                    "benchmark": BENCHMARK,
                    "parameters": self.parameters,
                    "created": date(),
                    "modified": date(),
                    "archived": null,
                },
                "value": self.value,
                "threshold": threshold,
                "boundary": {
                    "baseline": 1.0,
                    "lower_limit": null,
                    "upper_limit": self.upper_limit,
                },
                "limit": "upper",
                "status": "active",
                "created": date(),
                "modified": date(),
            });
            let mut alerts = report.alerts.take().unwrap_or_default();
            alerts.push(serde_json::from_value(alert).unwrap());
            report.alerts = Some(alerts);
            report
        }
    }

    /// Everything after the alerts table: the benchmark tables.
    fn benchmark_tables(html: &str) -> &str {
        html.split_once("Click to view all benchmark results")
            .unwrap_or_else(|| panic!("no benchmark table: {html}"))
            .1
    }

    fn head(table: &str) -> &str {
        let (_, head) = table.split_once("<thead>").unwrap();
        head.split_once("</thead>").unwrap().0
    }

    fn body_rows(table: &str) -> Vec<&str> {
        let (_, body) = table.split_once("<tbody>").unwrap();
        let (body, _) = body.split_once("</tbody>").unwrap();
        body.split("<tr>").skip(1).collect()
    }

    /// Each result line of the run output, up to its link.
    fn result_lines(human: &str) -> Vec<&str> {
        human
            .lines()
            .filter_map(|line| line.split_once("): http"))
            .map(|(line, _)| line)
            .collect()
    }

    fn checked(uuid: &str, metric: &str) -> Vec<Value> {
        vec![boundary(uuid, "t_test", metric, [None, None, None])]
    }

    const MEMORY_GROUPS: [&str; 4] = [
        ">Memory</a></th>",
        ">Memory: median</a></th>",
        ">Memory: mean</a></th>",
        ">Memory: p99</a></th>",
    ];

    /// Measures checked on no metric, on some, and on `value` among others, over two
    /// iterations that check different metrics of `memory`.
    fn default_metrics_comment() -> ReportComment {
        report_comment_for(report(json!([
            [result(
                0,
                VARIANT_A,
                &json!({}),
                &[
                    report_measure(
                        "allocated",
                        &[metric("max", 1.0, &[]), metric("min", 2.0, &[])]
                    ),
                    report_measure(
                        "cycles",
                        &[
                            metric("lower_value", 1.0, &[]),
                            metric("upper_value", 3.0, &[]),
                            metric("value", 2.0, &[]),
                        ]
                    ),
                    report_measure(
                        "instructions",
                        &[
                            metric("median", 5.0, &checked(THRESHOLD_1, "median")),
                            metric("value", 4.0, &[]),
                        ]
                    ),
                    report_measure(
                        "latency",
                        &[
                            metric("mean", 1.0, &[]),
                            metric("median", 2.0, &[]),
                            metric("std_dev", 3.0, &[]),
                        ]
                    ),
                    report_measure(
                        "memory",
                        &[
                            metric("lower_value", 1.0, &[]),
                            metric("mean", 2.0, &[]),
                            metric("median", 3.0, &checked(THRESHOLD_2, "median")),
                            metric("p99", 4.0, &[]),
                            metric("value", 5.0, &checked(THRESHOLD_3, "value")),
                        ]
                    ),
                    report_measure(
                        "throughput",
                        &[metric("mean", 1.0, &[]), metric("p99", 2.0, &[])]
                    ),
                ]
            )],
            [result(
                1,
                VARIANT_A,
                &json!({}),
                &[report_measure(
                    "memory",
                    &[
                        metric(
                            "mean",
                            2.0,
                            &checked("40000000-0000-0000-0000-000000000000", "mean")
                        ),
                        metric("median", 3.0, &[]),
                        metric(
                            "p99",
                            4.0,
                            &checked("50000000-0000-0000-0000-000000000000", "p99")
                        ),
                        metric("value", 5.0, &[]),
                    ]
                ),]
            )],
        ])))
    }

    // Kills default metrics that ignore the thresholds, take them from one iteration, or add
    // an unchecked `value`; a fallback out of the `value`, `median`, `mean`, then name order,
    // which lands on `lower_value` for a version 0 measure; and checked metrics in name order.
    #[test]
    fn default_metrics_are_the_checked_ones_else_one_by_preference() {
        let comment = default_metrics_comment();

        let human = comment.human();
        assert_eq!(
            result_lines(&human),
            [
                "- bench (Allocated: max",
                "- bench (Cycles",
                "- bench (Instructions: median",
                "- bench (Latency: median",
                "- bench (Memory: value, median, mean, p99",
                "- bench (Throughput: mean",
                "- bench (Memory: value, median, mean, p99",
            ],
            "{human}"
        );

        // The first iteration's table draws the metrics only the second one checked.
        let html = comment.html(false, None);
        let (first, _) = benchmark_tables(&html).split_once("</table>").unwrap();
        let found = MEMORY_GROUPS.map(|group| {
            first
                .find(group)
                .unwrap_or_else(|| panic!("{group}: {first}"))
        });
        assert!(found.is_sorted(), "{first}");
        for group in [
            ">Allocated: max</a>",
            ">Cycles</a>",
            ">Throughput: mean</a>",
        ] {
            assert!(first.contains(group), "{group}: {first}");
        }
    }

    // Kills a `--ci-only-thresholds` table that draws a default metric its own iteration did
    // not check.
    #[test]
    fn only_thresholds_draws_what_each_iteration_checked() {
        let groups = MEMORY_GROUPS;
        let html = default_metrics_comment().html(true, None);
        let (first, second) = benchmark_tables(&html).split_once("</table>").unwrap();
        assert_eq!(first.matches("</a></th>").count(), 3, "{first}");
        for group in [">Instructions: median</a>", groups[0], groups[1]] {
            assert!(first.contains(group), "{group}: {first}");
        }
        assert_eq!(second.matches("</a></th>").count(), 2, "{second}");
        for group in [groups[2], groups[3]] {
            assert!(second.contains(group), "{group}: {second}");
        }
    }

    // Kills a column group per measure rather than per checked metric, groups in name order
    // rather than `value`, `median`, `mean`, then name, one unit scale per measure, a group for
    // an unchecked metric beside checked ones, a header that names no metric, and a result
    // that lacks a group's metric drawing anything in that group or skipping its lower
    // boundary cell.
    #[test]
    fn each_checked_metric_is_a_column_group_with_its_own_scale() {
        let limits = |uuid, test, metric, baseline| {
            vec![boundary(
                uuid,
                test,
                metric,
                [Some(baseline), None, Some(3000.0)],
            )]
        };
        let report = report(json!([[
            result(
                0,
                VARIANT_A,
                &json!({}),
                &[report_measure(
                    "latency",
                    &[
                        metric(
                            "mean",
                            2500.0,
                            &limits(THRESHOLD_1, "t_test", "mean", 2000.0)
                        ),
                        metric(
                            "median",
                            2400.0,
                            &limits(THRESHOLD_2, "z_score", "median", 2000.0)
                        ),
                        metric("p99", 4000.0, &[]),
                        metric(
                            "std_dev",
                            120.0,
                            &[boundary(
                                THRESHOLD_3,
                                "static",
                                "std_dev",
                                [Some(100.0), Some(50.0), Some(200.0)]
                            )],
                        ),
                    ]
                )]
            ),
            result(
                0,
                VARIANT_B,
                &json!({}),
                &[report_measure(
                    "latency",
                    &[
                        metric(
                            "mean",
                            2600.0,
                            &limits(THRESHOLD_1, "t_test", "mean", 2000.0)
                        ),
                        metric(
                            "median",
                            2500.0,
                            &limits(THRESHOLD_2, "z_score", "median", 2000.0)
                        ),
                        metric("p99", 4100.0, &[]),
                    ]
                )]
            ),
        ]]));
        let html = report_comment_for(report).html(false, None);
        let table = benchmark_tables(&html);

        let micros = "<th>Benchmark Result<br />microseconds (µs)<br />(Result Δ%)</th><th>Upper Boundary<br />microseconds (µs)<br />(Limit %)</th>";
        let nanos = "<th>Benchmark Result<br />nanoseconds (ns)<br />(Result Δ%)</th><th>Lower Boundary<br />nanoseconds (ns)<br />(Limit %)</th><th>Upper Boundary<br />nanoseconds (ns)<br />(Limit %)</th>";
        let latency = format!("{CONSOLE}/measures/latency");
        assert_eq!(
            head(table),
            format!(
                r#"<tr><th>Benchmark</th><th><a href="{latency}">Latency: median</a></th>{micros}<th><a href="{latency}">Latency: mean</a></th>{micros}<th><a href="{latency}">Latency: std_dev</a></th>{nanos}</tr>"#
            )
        );

        let rows = body_rows(table);
        assert_eq!(rows.len(), 2, "{table}");
        assert!(
            rows[0].contains("<td>2.40 µs<br /><details><summary>(+20.00%)</summary>Baseline: 2.00 µs</details></td><td>3.00 µs<br />(80.00%)</td>"),
            "{}",
            rows[0]
        );
        assert!(
            rows[0].ends_with("<td>120.00 ns<br /><details><summary>(+20.00%)</summary>Baseline: 100.00 ns</details></td><td>50.00 ns<br />(41.67%)</td><td>200.00 ns<br />(60.00%)</td></tr>"),
            "{}",
            rows[0]
        );
        assert!(
            rows[1].ends_with(
                "<td>3.00 µs<br />(86.67%)</td><td></td><td></td><td></td><td></td></tr>"
            ),
            "{}",
            rows[1]
        );
    }

    // Kills a row without its variant's parameters, parameters in other than the console's
    // `key=value` form (a bare string, `""` for an empty one, JavaScript's number spelling),
    // pairs out of key order or sharing one `<code>`, parameters that reach the HTML unescaped,
    // and a parameters line on a variant that has none.
    #[test]
    fn each_variant_is_a_row_named_by_its_parameters() {
        let latency = || [report_measure("latency", &[metric("value", 1.0, &[])])];
        let report = report(json!([[
            result(
                0,
                VARIANT_A,
                &json!({"order": "random", "length": 1024}),
                &latency()
            ),
            result(0, VARIANT_B, &json!({"tag": "a<b&c"}), &latency()),
            result(0, VARIANT_C, &json!({}), &latency()),
            result(
                0,
                "77777777-7777-7777-7777-77777777777d",
                &json!({"warm": true, "empty": "", "big": 1e21}),
                &latency()
            ),
        ]]));
        let html = report_comment_for(report).html(false, None);
        let rows = body_rows(benchmark_tables(&html));
        let bench = format!(r#"<td><a href="{CONSOLE}/benchmarks/bench">bench</a>"#);
        assert_eq!(rows.len(), 4, "{html}");
        for (row, parameters) in rows.iter().zip([
            "<br /><code>length=1024</code> <code>order=random</code></td>",
            "<br /><code>tag=a&lt;b&amp;c</code></td>",
            "</td>",
            "<br /><code>big=1e+21</code> <code>empty=&quot;&quot;</code> <code>warm=true</code></td>",
        ]) {
            assert!(
                row.starts_with(&format!("{bench}{parameters}<td>📈")),
                "{row}"
            );
        }
    }

    /// Two variants, each with CPU Time checked by one threshold and `Latency: median` checked
    /// by two on the first variant and one on the second, and an alert from the second
    /// threshold on the first variant.
    fn two_thresholds_report() -> JsonReport {
        let cpu_time = |value| {
            report_measure(
                "cpu-time",
                &[metric(
                    "value",
                    value,
                    &[boundary(
                        THRESHOLD_1,
                        "t_test",
                        "value",
                        [Some(80.0), None, Some(120.0)],
                    )],
                )],
            )
        };
        let static_median = boundary(
            THRESHOLD_2,
            "static",
            "median",
            [Some(40.0), Some(30.0), Some(60.0)],
        );
        let percentage_median = boundary(
            THRESHOLD_3,
            "percentage",
            "median",
            [Some(45.0), None, Some(55.0)],
        );
        let report = report(json!([[
            result(
                0,
                VARIANT_A,
                &json!({"size": 64}),
                &[
                    cpu_time(100.0),
                    report_measure(
                        "latency",
                        &[metric(
                            "median",
                            58.0,
                            &[static_median.clone(), percentage_median],
                        )]
                    ),
                ]
            ),
            result(
                0,
                VARIANT_B,
                &json!({}),
                &[
                    cpu_time(90.0),
                    report_measure("latency", &[metric("median", 45.0, &[static_median])]),
                ]
            ),
        ]]));
        Alert {
            iteration: 0,
            variant: VARIANT_A,
            parameters: json!({"size": 64}),
            measure: "latency",
            metric: "median",
            threshold: THRESHOLD_3,
            value: 58.0,
            upper_limit: 55.0,
        }
        .on(report)
    }

    // Kills one threshold per cell, a sub-row count taken from the first group rather than the
    // busiest, a benchmark cell that does not span its sub-rows or repeats in one, a later
    // sub-row that repeats a group with fewer thresholds, a sub-row without its threshold's
    // test, a test on a variant with one threshold per group, and an alert drawn on another
    // threshold's sub-row.
    #[test]
    fn two_thresholds_on_one_metric_give_the_variant_a_sub_row_each() {
        let html = report_comment_for(two_thresholds_report()).html(false, None);
        let table = benchmark_tables(&html);
        let rows = body_rows(table);
        assert_eq!(rows.len(), 3, "{table}");
        let threshold = |uuid: &str, label: &str| {
            format!(r#"<a href="{CONSOLE}/thresholds/{uuid}">view threshold{label}</a>"#)
        };

        let bench = format!(r#"<a href="{CONSOLE}/benchmarks/bench">bench</a>"#);
        assert!(
            rows[0].starts_with(&format!(
                r#"<td rowspan="2">{bench}<br /><code>size=64</code></td><td>📈"#
            )),
            "{}",
            rows[0]
        );
        assert!(
            rows[0].contains(&threshold(THRESHOLD_1, " (t_test)")),
            "{}",
            rows[0]
        );
        assert!(
            rows[0].contains(&threshold(THRESHOLD_2, " (static)")),
            "{}",
            rows[0]
        );
        assert!(
            rows[0].ends_with("<td>58.00 ns<br /><details><summary>(+45.00%)</summary>Baseline: 40.00 ns</details></td><td>30.00 ns<br />(51.72%)</td><td>60.00 ns<br />(96.67%)</td></tr>"),
            "{}",
            rows[0]
        );
        assert!(!rows[0].contains("<b>"), "{}", rows[0]);

        assert!(
            rows[1].starts_with("<td></td><td></td><td></td><td>📈"),
            "the second sub-row has no benchmark cell and an empty CPU Time group: {}",
            rows[1]
        );
        assert!(
            rows[1].contains(&threshold(THRESHOLD_3, " (percentage)")),
            "{}",
            rows[1]
        );
        assert!(
            rows[1].contains(&format!(r#"<a href="{CONSOLE}/alerts/{ALERT}">"#)),
            "{}",
            rows[1]
        );
        assert!(
            rows[1].ends_with("<td><b>58.00 ns<br /><details><summary>(+28.89%)</summary>Baseline: 45.00 ns</details></b></td><td></td><td><b>55.00 ns<br />(105.45%)<br /><br /></b></td></tr>"),
            "{}",
            rows[1]
        );

        assert!(
            rows[2].starts_with(&format!("<td>{bench}</td><td>📈")),
            "{}",
            rows[2]
        );
        assert_eq!(
            rows[2].matches(">view threshold</a>").count(),
            2,
            "{}",
            rows[2]
        );
        assert_eq!(table.matches("/alerts/").count(), 1, "{table}");
    }

    // Kills an alert matched by benchmark and threshold alone, which draws it on every
    // iteration and every variant the threshold checked.
    #[test]
    fn an_alert_lands_on_its_own_iteration_and_variant() {
        // A `value` row its bare threshold checked, with the deprecated fields the API fills for
        // it, so a table that reads either shape draws the group and only the match differs.
        let latency = |value: f64| {
            let check = boundary(
                THRESHOLD_1,
                "t_test",
                "value",
                [Some(10.0), None, Some(20.0)],
            );
            json!({
                "measure": measure("latency"),
                "metrics": [metric("value", value, std::slice::from_ref(&check))],
                "metric": {
                    "uuid": "99999999-9999-9999-9999-999999999999",
                    "value": value,
                    "lower_value": null,
                    "upper_value": null,
                },
                "threshold": check["threshold"],
                "boundary": check["boundary"],
            })
        };
        let iteration = |n: u32| {
            json!([
                result(n, VARIANT_A, &json!({"size": 1}), &[latency(11.0)]),
                result(n, VARIANT_B, &json!({"size": 2}), &[latency(30.0)]),
            ])
        };
        let report = Alert {
            iteration: 1,
            variant: VARIANT_B,
            parameters: json!({"size": 2}),
            measure: "latency",
            metric: "value",
            threshold: THRESHOLD_1,
            value: 30.0,
            upper_limit: 20.0,
        }
        .on(report(json!([iteration(0), iteration(1)])));
        let html = report_comment_for(report).html(false, None);
        let tables = benchmark_tables(&html)
            .split("<table>")
            .skip(1)
            .collect::<Vec<_>>();
        assert_eq!(tables.len(), 2, "{html}");
        assert!(!tables[0].contains("/alerts/"), "{}", tables[0]);
        let rows = body_rows(tables[1]);
        assert!(!rows[0].contains("/alerts/"), "{}", rows[0]);
        assert_eq!(rows[1].matches("/alerts/").count(), 1, "{}", rows[1]);
        assert!(rows[1].contains("<b>30.00 ns"), "{}", rows[1]);
    }

    // Kills an alerts table or alert line that names neither the metric nor the variant, so two
    // alerts on one benchmark read the same, and one that names `value`.
    #[test]
    fn alerts_name_their_metric_and_variant() {
        let report = report(json!([[result(
            0,
            VARIANT_A,
            &json!({"size": 64}),
            &[
                report_measure("cpu-time", &[metric("value", 50.0, &[])]),
                report_measure("latency", &[metric("median", 50.0, &[])]),
            ]
        )]]));
        let alert = |measure, metric, threshold| Alert {
            iteration: 0,
            variant: VARIANT_A,
            parameters: json!({"size": 64}),
            measure,
            metric,
            threshold,
            value: 50.0,
            upper_limit: 20.0,
        };
        let report = alert("latency", "median", THRESHOLD_1).on(report);
        let report = alert("cpu-time", "value", THRESHOLD_2).on(report);
        let comment = report_comment_for(report);

        let html = comment.html(false, None);
        let (alerts, _) = html
            .split_once("Click to view all benchmark results")
            .unwrap();
        let bench = format!(
            r#"<td><a href="{CONSOLE}/benchmarks/bench">bench</a><br /><code>size=64</code></td>"#
        );
        for (measure, name) in [("latency", "Latency: median"), ("cpu-time", "CPU Time")] {
            let cells = format!(
                r#"{bench}<td><a href="{CONSOLE}/measures/{measure}">{name}<br />nanoseconds (ns)</a></td>"#
            );
            assert!(alerts.contains(&cells), "{cells}: {alerts}");
        }

        let human = comment.human();
        let (_, lines) = human.split_once("View alerts:").unwrap();
        assert_eq!(
            result_lines(lines),
            [
                "- bench size=64 (Latency: median",
                "- bench size=64 (CPU Time",
            ],
            "{human}"
        );
    }

    // Kills a `--ci-only-thresholds` table that needs a limit, which posts an empty table on
    // a threshold's first runs at either version, and a gate that reads otherwise than the table.
    #[test]
    fn only_thresholds_draws_a_threshold_before_its_limits() {
        for (metric_name, header) in [("median", "Latency: median"), ("value", "Latency")] {
            let report = report(json!([[result(
                0,
                VARIANT_A,
                &json!({}),
                &[
                    report_measure("cpu-time", &[metric("mean", 7.0, &[])]),
                    report_measure(
                        "latency",
                        &[metric(
                            metric_name,
                            12.0,
                            &checked(THRESHOLD_1, metric_name),
                        )]
                    ),
                ]
            )]]));
            let comment = report_comment_for(report);
            assert!(comment.has_threshold(), "{metric_name}");
            let html = comment.html(true, None);
            let table = benchmark_tables(&html);
            assert_eq!(
                head(table),
                format!(
                    r#"<tr><th>Benchmark</th><th><a href="{CONSOLE}/measures/latency">{header}</a></th><th>nanoseconds (ns)</th></tr>"#
                ),
                "{html}"
            );
            let threshold =
                format!(r#"<a href="{CONSOLE}/thresholds/{THRESHOLD_1}">view threshold</a>"#);
            assert!(
                body_rows(table)[0].ends_with(&format!("{threshold}</td><td>12.00 ns</td></tr>")),
                "{table}"
            );
        }
    }

    // Kills a run output line without its variant or its default metrics, one that names
    // `value`, and pairs out of key order or joined by other than a space.
    #[test]
    fn run_output_names_the_variant_and_the_default_metrics() {
        let report = report(json!([[
            result(
                0,
                VARIANT_A,
                &json!({"size": 64, "mode": "fast"}),
                &[
                    report_measure("cpu-time", &[metric("value", 1.0, &[])]),
                    report_measure(
                        "latency",
                        &[
                            metric("mean", 2.0, &checked(THRESHOLD_1, "mean")),
                            metric("median", 3.0, &checked(THRESHOLD_2, "median")),
                            metric("std_dev", 1.0, &[]),
                        ]
                    ),
                ]
            ),
            result(
                0,
                VARIANT_B,
                &json!({}),
                &[report_measure(
                    "latency",
                    &[metric("mean", 2.0, &[]), metric("median", 3.0, &[]),]
                )]
            ),
        ]]));
        let human = report_comment_for(report).human();
        assert_eq!(
            result_lines(&human),
            [
                "- bench mode=fast size=64 (CPU Time",
                "- bench mode=fast size=64 (Latency: median, mean",
                "- bench (Latency: median, mean",
            ],
            "{human}"
        );
    }
}
