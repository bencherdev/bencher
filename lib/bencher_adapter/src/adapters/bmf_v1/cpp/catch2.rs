use bencher_json::{
    ParameterSet,
    project::measure::built_in::{BuiltInMeasure as _, default::Latency},
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::{catch2_text::AdapterCppCatch2Text, catch2_xml::AdapterCppCatch2Xml},
        leaf::{LeafResults, Variant},
    },
    results::adapter_results::AdapterResults,
};

/// Catch2 at version 1: its XML reporter, then its console reporter.
pub struct AdapterCppCatch2V1;

impl Adaptable for AdapterCppCatch2V1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterCppCatch2Xml::parse(input, settings)
            .or_else(|| AdapterCppCatch2Text::parse(input, settings))
    }
}

/// What Catch2 reports of one benchmark in nanoseconds: the mean and the standard deviation,
/// each with its bootstrap interval, and the intervals' level when the output states it.
pub(super) struct Statistics<'a> {
    pub mean: [f64; 3],
    /// `None` when the run skipped the analysis, which leaves the mean alone.
    pub std_dev: Option<[f64; 3]>,
    pub level: Option<&'a str>,
}

/// A benchmark named `Case / Section / Bench`, with its latency, or skipped and counted when
/// Catch2 gives it no name.
pub(super) fn insert(
    results: &mut LeafResults,
    path: &[String],
    benchmark: &str,
    statistics: &Statistics<'_>,
) {
    if benchmark.is_empty() {
        results.skip_benchmark();
        return;
    }
    let name = path
        .iter()
        .map(String::as_str)
        .chain([benchmark])
        .collect::<Vec<_>>()
        .join(" / ");
    let Ok(benchmark_name) = name.parse() else {
        results.skip_benchmark();
        return;
    };
    let mut variant = results.variant(benchmark_name, ParameterSet::default());
    let Statistics {
        mean,
        std_dev,
        level,
    } = *statistics;
    if let Some(std_dev) = std_dev {
        insert_estimate(&mut variant, "mean", mean, level);
        insert_estimate(&mut variant, "std_dev", std_dev, level);
    } else {
        let [mean, _, _] = mean;
        variant.insert(Latency::name_id(), "mean", mean);
    }
}

fn insert_estimate(
    variant: &mut Variant<'_>,
    statistic: &str,
    [estimate, lower, upper]: [f64; 3],
    level: Option<&str>,
) {
    let interval = format!("{statistic}_ci{}", level.unwrap_or_default());
    variant.insert(Latency::name_id(), statistic, estimate);
    variant.insert(Latency::name_id(), &format!("{interval}_lower"), lower);
    variant.insert(Latency::name_id(), &format!("{interval}_upper"), upper);
}

#[cfg(test)]
pub(super) mod test_cpp_catch2_v1 {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Latency},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{Expected, assert_variant, convert_v1},
        results::adapter_results::AdapterResults,
    };

    /// The metric names of a mean and a standard deviation, each with its interval.
    pub(crate) type Names = [&'static str; 6];

    pub(crate) const GENERIC: Names = [
        "mean",
        "mean_ci_lower",
        "mean_ci_upper",
        "std_dev",
        "std_dev_ci_lower",
        "std_dev_ci_upper",
    ];

    pub(crate) fn latency(names: Names, values: [f64; 6]) -> Expected {
        (Latency::SLUG_STR, names.into_iter().zip(values).collect())
    }

    pub(crate) fn mean(mean: f64) -> Expected {
        (Latency::SLUG_STR, vec![("mean", mean)])
    }

    /// The benchmarks both reporters print for a v3 run, which wrap no name in the XML.
    pub(crate) fn assert_v3(results: &AdapterResults, names: Names, values: [[f64; 6]; 5]) {
        assert_eq!(results.inner.len(), 5);
        assert_eq!(results.skipped.benchmarks, 1);
        let [long, push_16, push_64, fib_10, fib_20] = values;
        assert_variant(
            results,
            "A test case with a name long enough that the console reporter has to wrap it / \
             A benchmark with a name long enough that the console reporter wraps it",
            "{}",
            &[latency(names, long)],
        );
        assert_variant(
            results,
            "Containers / vector / small / push 16",
            "{}",
            &[latency(names, push_16)],
        );
        assert_variant(
            results,
            "Containers / vector / push 64",
            "{}",
            &[latency(names, push_64)],
        );
        assert_variant(
            results,
            "Fibonacci / Fibonacci 10",
            "{}",
            &[latency(names, fib_10)],
        );
        assert_variant(
            results,
            "Fibonacci / Fibonacci 20",
            "{}",
            &[latency(names, fib_20)],
        );
    }

    /// Kills a text leaf that reads the table header Catch2 prints again after a failure as a
    /// test case header, counts a failure twice, loses a benchmark uncounted, or skips one that
    /// Catch2 printed shifted: after a failure inside the samples, Catch2's console reporter prints
    /// later rows one column to the right, in the analysis table until the next such failure and
    /// in the mean table for the rest of the run. Each text and XML pair is two runs, so only
    /// their benchmarks agree.
    #[test]
    fn adapter_cpp_catch2_v1_failed() {
        for (fixture, kept, failed) in [
            ("failed", 2, 1),
            ("failed_samples", 4, 1),
            ("failed_first_no_analysis", 4, 1),
            ("wrapped_failures", 6, 2),
            ("wrapped_failures_no_analysis", 6, 2),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_eq!(
                (text.inner.len(), text.skipped.benchmarks),
                (kept, failed),
                "{fixture}"
            );
        }
        let text = convert_v1(Adapter::CppCatch2Text, "cpp/catch2/failed_samples.txt");
        assert_variant(
            &text,
            "After / Fibonacci 15",
            "{}",
            &[latency(
                GENERIC,
                [406.337, 402.845, 415.575, 26.7559, 11.7677, 52.5085],
            )],
        );
    }

    /// Kills a text leaf that reads Catch2's table by line shapes rather than by its printer's
    /// column, which on these real runs fails a passing run whose durations have exponents,
    /// misreads or skips a benchmark Catch2 printed shifted, reads the header Catch2 prints again
    /// after a failed `CHECK` as a row, takes an unnamed benchmark's samples for its name, stores a
    /// benchmark with no name as `Case / `, or loses a benchmark uncounted; and an XML leaf that
    /// refuses a run whose every benchmark failed or one with an unnamed benchmark. Each text and
    /// XML pair is one run's two reports.
    #[test]
    fn adapter_cpp_catch2_v1_corpus() {
        for (fixture, kept, skipped) in [
            ("corpus_pass_s3_d", 3, 0),
            ("corpus_sections_s3", 3, 0),
            ("corpus_earlyafter_decl", 3, 3),
            ("corpus_throwshift_decl", 2, 2),
            ("corpus_softcheck_decl", 6, 0),
            ("corpus_softcheck_decl_na", 6, 0),
            ("corpus_softcheck_rand11", 6, 0),
            ("corpus_shapes_decl_na", 3, 2),
            ("corpus_shapes_rand6", 3, 2),
            ("corpus_wrap3_rand6_na", 6, 1),
            ("corpus_allfail_decl", 0, 1),
            ("corpus_width_120_wrapfail_decl", 6, 2),
            ("corpus_width_120_softcheck_decl", 6, 0),
            ("corpus_emptyname_decl", 1, 1),
            ("corpus_emptyname_decl_na", 1, 1),
            ("corpus_longfail_decl", 2, 4),
            ("corpus_longfail_decl_na", 2, 4),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_same_values(&text, &xml, fixture);
            assert_eq!(
                (text.inner.len(), text.skipped.benchmarks),
                (kept, skipped),
                "{fixture}"
            );
        }
        let (shapes, _) = pair("corpus_shapes_rand6");
        assert_variant(
            &shapes,
            "Checks inside / Inner section with a long name that wraps across the console width / \
             checked with a rather long name that wraps around",
            "{}",
            &[latency(
                GENERIC,
                [12.7394, 12.6381, 12.8991, 0.639_149, 0.440_761, 0.894_583],
            )],
        );
        let (wrap3, _) = pair("corpus_wrap3_rand6_na");
        assert_variant(&wrap3, "Last / Fibonacci 11", "{}", &[mean(53.1703)]);
    }

    /// Kills a text leaf that reads no duration in seconds or in minutes, or reads one in minutes
    /// as seconds, cuts a name at its column's width in bytes where Catch2 pads it by bytes and
    /// wraps it by characters, takes a name that starts like a failure for one, ends a failure at
    /// its first line where its message has more, at a line of its message that ends in `) `, or
    /// before its last line where Catch2 padded that line to the first column's width, reads the
    /// program's own output after a row as a close, joins a name with a space where Catch2 broke
    /// it at punctuation, keeps the hyphen Catch2 adds where it split a word or drops one Catch2
    /// wrote, as one that ends a full line of a name with spaces, splits a wrapped test case name,
    /// or splits a header that Catch2 wraps past its `: `.
    #[test]
    fn adapter_cpp_catch2_v1_corpus_edges() {
        for (fixture, kept, skipped) in [
            ("corpus_minutes_decl", 5, 0),
            ("corpus_edgenames_decl", 30, 1),
            ("corpus_edgenames_decl_na", 30, 1),
            ("corpus_fakefail_decl", 2, 0),
            ("corpus_breaks_decl", 12, 1),
            ("corpus_casewrap_decl", 3, 0),
            ("corpus_minstat_s2", 2, 0),
            ("corpus_failmix_decl", 5, 2),
            ("corpus_bdd_decl", 4, 0),
            ("corpus_names3_decl", 5, 0),
            ("corpus_failreal_decl", 3, 5),
            ("corpus_failreal_decl_na", 3, 5),
            ("corpus_width_120_failreal_decl", 3, 5),
            ("corpus_width_60_minfail_s3", 2, 1),
            ("corpus_hyphen_decl", 2, 0),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_same_values(&text, &xml, fixture);
            assert_eq!(
                (text.inner.len(), text.skipped.benchmarks),
                (kept, skipped),
                "{fixture}"
            );
        }
    }

    /// Kills a text leaf that takes a line that starts with `benchmark name` for a table's
    /// header, or finds the samples column's title inside a name, where Catch2's header is its
    /// exact column titles above a rule: a benchmark named like the header reads as a benchmark
    /// and a test case named so as a test case, on Catch2 v3 at two widths and on v2.
    #[test]
    fn adapter_cpp_catch2_v1_header_names() {
        for (fixture, kept) in [
            ("corpus_cbsamples_decl", 6),
            ("corpus_ctname_decl", 5),
            ("corpus_cbname_decl", 6),
            ("corpus_cbname_decl_na", 6),
            ("corpus_cbname_s3", 6),
            ("corpus_cbname_succ_na", 6),
            ("corpus_width_60_cbname_decl", 6),
            ("corpus_width_60_cbname_decl_na", 6),
            ("corpus_width_60_cbname_s3", 6),
            ("corpus_width_60_cbname_succ_na", 6),
            ("corpus_width_60_cbsamples_decl", 6),
            ("corpus_width_60_cbsamples_decl_na", 6),
            ("corpus_width_60_cbsamples_s3", 6),
            ("corpus_width_60_cbsamples_succ_na", 6),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_same_values(&text, &xml, fixture);
            assert_eq!(
                (text.inner.len(), text.skipped.benchmarks),
                (kept, 0),
                "{fixture}"
            );
        }
        for fixture in [
            "corpus_v2_13_10_cbname_decl",
            "corpus_v2_13_10_cbname_decl_na",
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_eq!(text.inner.len(), 6, "{fixture}");
        }
    }

    /// Kills a text leaf that reads the indented blank line of an assertion's message between
    /// tables as a row, or ends a failure's cell at a blank line of its message, where Catch2 ends
    /// it at the `)` that closes the message: v3 then prints the message again under the
    /// failure's assertion, and v2 goes on with the next benchmark's cells, after the padding of
    /// a message shorter than the first column.
    #[test]
    fn adapter_cpp_catch2_v1_messages() {
        for (fixture, kept, skipped) in [
            ("corpus_cmsg_decl", 5, 0),
            ("corpus_cfailmsg_decl", 3, 3),
            ("corpus_cfailmsg_rand7", 3, 3),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_same_values(&text, &xml, fixture);
            assert_eq!(
                (text.inner.len(), text.skipped.benchmarks),
                (kept, skipped),
                "{fixture}"
            );
        }
        for (fixture, kept) in [
            ("corpus_v2_13_10_cfailmsg_decl", 6),
            ("corpus_v2_13_10_shortlate_decl", 3),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_eq!(text.inner.len(), kept, "{fixture}");
        }
    }

    /// Kills a text leaf that, on Catch2 v2, closes the table after a failure, where v2 goes on
    /// with the next benchmark in the same table, reads past an assertion inside the samples,
    /// after which v2 prints no failure, into the next test case, splits a header that v2 wraps
    /// past its `: `, splits a word by characters where v2 splits by bytes, ends a failure at a
    /// line of its message that ends in `) `, or runs one into the first line of the next
    /// benchmark's wrapped name. Each text and XML pair is two runs, and v2's XML
    /// names the benchmarks after a failed test case under that test case.
    #[test]
    fn adapter_cpp_catch2_v1_text_v2_failures() {
        for (fixture, kept) in [
            ("corpus_v2_13_10_shapes_decl", 5),
            ("corpus_v2_11_0_shapes_decl", 5),
            ("corpus_v2_13_10_bdd_decl", 4),
            ("corpus_v2_11_0_bdd_decl", 4),
            ("corpus_v2_13_10_names3_decl", 5),
            ("corpus_v2_13_10_failreal_decl", 3),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_eq!(text.inner.len(), kept, "{fixture}");
        }
        // The wrapped name's first line ends in a `)`, after which the join adds no space.
        let (text, xml) = pair("corpus_v2_13_10_wrapafter_decl");
        assert_eq!(text.skipped, xml.skipped);
        assert!(names(&text).contains(
            "Failure before a wrapped name / Wrapping benchmark name of fib(10)with more"
        ));
        for fixture in ["corpus_v2_13_10_fail2_decl", "corpus_v2_11_0_fail2_decl"] {
            let (text, xml) = pair(fixture);
            assert_eq!(
                names(&text),
                [
                    "After / Fibonacci 12",
                    "After / Fibonacci 15",
                    "Before / Fibonacci 10",
                    "Later / Inner / Fibonacci 11",
                ]
                .map(ToOwned::to_owned)
                .into(),
                "{fixture}"
            );
            assert_eq!(text.skipped, xml.skipped, "{fixture}");
        }
    }

    /// Kills a text leaf that, where the program prints inside Catch2's table, loses a benchmark
    /// uncounted, as one whose row a benchmark that lost step took for a print after Catch2
    /// opened the table again, counts two that prints reach as one, loses a failure, one on
    /// lines, a row, or a blank name's row that a print put out of its table, keeps one the
    /// program printed, opens a table at a printed header with no rule, takes rows into a header
    /// after a printed rule, places a rule with a print before it, starts a failure at a print
    /// of one after padding, reads a printed failure on v3 as one of v2's over lines, counts a
    /// printed failure out of a table, reads a print in a cell Catch2 leaves empty, takes the
    /// blank line that closes a table for the program's own, keeps a value of the program's own,
    /// or takes a value cell of Catch2's for a print because a count the program printed follows
    /// it: each run's text keeps only the XML's benchmarks, with their values where one run
    /// wrote both, and counts every other one, as many as each print reaches, and a print after
    /// the last table as one of its own.
    #[test]
    fn adapter_cpp_catch2_v1_prints() {
        const NONE: &[&str] = &[];
        for (fixture, kept, skipped, one_run, printed) in [
            ("corpus_stray_decl", 4, 1, true, NONE),
            ("corpus_strayearly_decl", 4, 1, true, NONE),
            ("corpus_strayearly_decl_na", 4, 1, true, NONE),
            ("corpus_straysc_decl", 10, 5, true, NONE),
            ("corpus_straysc_decl_na", 10, 5, true, NONE),
            ("corpus_width_60_straysc_decl_na", 10, 5, true, NONE),
            ("corpus_straysp_decl", 15, 1, true, NONE),
            ("corpus_failprint_decl", 10, 7, true, NONE),
            ("corpus_lastcol_decl_na", 11, 8, true, NONE),
            ("corpus_lastcol_cout_decl_na", 19, 4, true, NONE),
            ("corpus_naprint_decl", 12, 7, true, NONE),
            ("corpus_naprint_decl_na", 12, 7, true, NONE),
            ("corpus_valprint_decl_na", 12, 9, true, NONE),
            ("corpus_secprint_decl", 12, 4, true, NONE),
            ("corpus_rowprint_decl", 9, 7, true, NONE),
            (
                "corpus_rowprint_decl_na",
                10,
                5,
                true,
                &["Rows between / fake before"][..],
            ),
            ("corpus_secprint_cout_decl", 14, 2, true, NONE),
            ("corpus_rowprint_cout_decl", 14, 6, true, NONE),
            ("corpus_valprint_cout_decl", 21, 6, true, NONE),
            ("corpus_distfail_decl", 3, 2, true, NONE),
            ("corpus_blankfail_decl", 4, 1, true, NONE),
            ("corpus_blankun_decl_na", 5, 1, true, NONE),
            ("corpus_absorb_decl", 4, 2, true, NONE),
            ("corpus_unnamed_decl_na", 4, 2, true, NONE),
            ("corpus_afterprint_decl", 3, 1, true, NONE),
            ("corpus_afterprint_decl_na", 3, 1, true, NONE),
            ("corpus_width_120_valprint_decl", 12, 9, true, NONE),
            ("corpus_blankfailmsg_decl", 4, 3, true, NONE),
            ("corpus_fakeheader_decl", 2, 1, true, NONE),
            ("corpus_crossing_decl", 4, 4, true, NONE),
            ("corpus_sepline_decl", 10, 3, true, NONE),
            ("corpus_sepline_decl_na", 10, 3, true, NONE),
            ("corpus_failtext_decl", 4, 5, true, NONE),
            ("corpus_failvalue_decl", 4, 2, true, NONE),
            ("corpus_failvalue_decl_na", 4, 2, true, NONE),
            ("corpus_v2_13_10_failtext_decl", 6, 5, false, NONE),
            ("corpus_v2_13_10_straysc_decl_na", 10, 5, false, NONE),
            ("corpus_v2_13_10_naprint_decl_na", 12, 7, false, NONE),
        ] {
            let (text, xml) = pair(fixture);
            assert_eq!(
                (text.inner.len(), text.skipped.benchmarks),
                (kept, skipped),
                "{fixture}"
            );
            let (mut text_names, xml_names) = (names(&text), names(&xml));
            // A row the program prints byte for byte as Catch2 would reads as Catch2's.
            for name in printed {
                assert!(text_names.remove(*name), "{fixture}: {name}");
            }
            assert!(text_names.is_subset(&xml_names), "{fixture}");
            let missing = xml_names.difference(&text_names).count();
            assert!(
                text.skipped.benchmarks >= xml.skipped.benchmarks + missing,
                "{fixture}"
            );
            if one_run {
                let xml_metrics = metrics(&xml);
                let printed_metrics =
                    |metric: &String| printed.iter().any(|name| metric.starts_with(name));
                for (metric, value) in metrics(&text)
                    .into_iter()
                    .filter(|(metric, _)| !printed_metrics(metric))
                {
                    let expected = xml_metrics.get(&metric).copied().unwrap_or(f64::NAN);
                    assert_close(fixture, &metric, value, expected);
                }
            }
        }
    }

    /// Kills a text leaf that reads Catch2's colour codes as text, where `--colour-mode ansi`
    /// prints the same tables as a plain run, in both table modes.
    #[test]
    fn adapter_cpp_catch2_v1_text_ansi() {
        for (fixture, plain) in [
            ("corpus_ansi_decl", "corpus_narrow_decl"),
            ("corpus_ansi_decl_na", "corpus_narrow_decl_na"),
        ] {
            let (text, xml) = pair(fixture);
            assert_agree(&text, &xml, fixture);
            assert_same_values(&text, &xml, fixture);
            let (plain, _) = pair(plain);
            assert_eq!(names(&text), names(&plain), "{fixture}");
            assert_eq!(text.skipped, plain.skipped, "{fixture}");
        }
    }

    /// Kills a text leaf that loses a benchmark where a console narrower than 59 columns wraps
    /// the table's title, or at 46 columns leaves it blank, or that keeps one there, as a
    /// one-letter name it reads cleanly, or that counts one more than once where Catch2 v2 goes
    /// on with the next benchmark on a failure's line: every benchmark is skipped and counted.
    #[test]
    fn adapter_cpp_catch2_v1_text_narrow() {
        for fixture in [
            "corpus_width_58_narrow_decl",
            "corpus_width_58_narrow_decl_na",
            "corpus_width_46_shapes_decl_na",
            "corpus_width_46_letter_decl",
            "corpus_v2_11_0_width_46_shapes_decl",
        ] {
            let (text, xml) = pair(fixture);
            assert!(text.inner.is_empty(), "{fixture}");
            assert_eq!(
                text.skipped.benchmarks,
                xml.inner.len() + xml.skipped.benchmarks,
                "{fixture}"
            );
        }
    }

    /// Kills a text leaf that keeps a benchmark, or counts fewer skips than the run has
    /// benchmarks, where Catch2 v2 at 46 columns prints a soft check's failure inside a table.
    #[test]
    fn adapter_cpp_catch2_v1_text_narrow_soft_check() {
        const FIXTURE: &str = "corpus_v2_13_10_width_46_softcheck_decl";
        let (text, xml) = pair(FIXTURE);
        assert!(text.inner.is_empty(), "{FIXTURE}");
        assert!(
            text.skipped.benchmarks >= xml.inner.len() + xml.skipped.benchmarks,
            "{FIXTURE}"
        );
    }

    /// Kills a text leaf whose reading of these Catch2 v2 runs changes: one that places a row the
    /// program printed onto when a failure follows it, or loses the benchmarks of the test case
    /// after a `REQUIRE` failed inside the samples.
    #[test]
    fn adapter_cpp_catch2_v1_text_v2_counts() {
        const NONE: &[(&str, f64)] = &[];
        for (fixture, kept, skipped) in [
            ("corpus_v2_13_10_glueprint_na", NONE, 2),
            (
                "corpus_v2_13_10_requirenext",
                &[
                    ("Next case / next first", 56.0165),
                    ("Next case / next second", 55.7877),
                ][..],
                1,
            ),
        ] {
            let text = convert_v1(Adapter::CppCatch2Text, &format!("cpp/catch2/{fixture}.txt"));
            assert_eq!(text.skipped.benchmarks, skipped, "{fixture}");
            assert_eq!(
                names(&text),
                kept.iter()
                    .map(|(name, _)| (*name).to_owned())
                    .collect::<BTreeSet<_>>(),
                "{fixture}"
            );
            let metrics = metrics(&text);
            for (name, mean) in kept {
                let metric = format!("{name} {} mean", Latency::SLUG_STR);
                assert_close(fixture, &metric, metrics[&metric], *mean);
            }
        }
    }

    /// Kills a text leaf that ends a failure's message at a line that only ends in a space or
    /// only in `)`, which loses the benchmarks Catch2 v2 printed after a failure inside the
    /// samples whose message runs over lines: each benchmark kept reads at the mean Catch2
    /// printed, and each one not kept is counted with the failure.
    #[test]
    fn adapter_cpp_catch2_v1_text_sample_print() {
        const FIXTURE: &str = "corpus_v2_13_10_width_60_sampleprint";
        let printed = BTreeMap::from([
            ("Sample multiline throw / abcdefghijkl", 55.825),
            ("Sample multiline throw / abcdefghijklm", 56.0481),
            ("Sample multiline throw / abcdefghijklmn", 57.8011),
            ("Sample multiline throw / printing after", 56.7244),
        ]);
        let text = convert_v1(Adapter::CppCatch2Text, &format!("cpp/catch2/{FIXTURE}.txt"));
        let kept = names(&text);
        for name in [
            "Sample multiline throw / abcdefghijkl",
            "Sample multiline throw / abcdefghijklm",
        ] {
            assert!(kept.contains(name), "{name}");
        }
        let metrics = metrics(&text);
        for name in &kept {
            let mean = printed
                .get(name.as_str())
                .unwrap_or_else(|| panic!("{FIXTURE}: {name}"));
            let metric = format!("{name} {} mean", Latency::SLUG_STR);
            assert_close(FIXTURE, &metric, metrics[&metric], *mean);
        }
        let lost = printed.len() - kept.len();
        assert!(text.skipped.benchmarks > lost, "{FIXTURE}");
    }

    /// Kills a text leaf that, in output with no banner, takes a line that only ends in dashes,
    /// or an empty one, for a rule, which claims Ruby's `benchmark` output, or that takes a
    /// failure after any text for one in a row, which claims Catch2's own XML of a benchmark
    /// named like a failure.
    #[test]
    fn adapter_cpp_catch2_v1_text_refuses_other_output() {
        for (adapter, suffix) in [
            (Adapter::CppCatch2, "ruby/benchmark/two.txt"),
            (
                Adapter::CppCatch2Text,
                "cpp/catch2/corpus_fakefail_decl.xml",
            ),
        ] {
            let input = std::fs::read_to_string(format!("./tool_output/{suffix}"))
                .expect("Failed to read a fixture");
            assert_eq!(
                adapter.convert(&input, Settings::new(None, BmfVersion::V1)),
                None,
                "{adapter} {suffix}"
            );
        }
    }

    /// Kills a text leaf that, after a benchmark whose values do not read, as under a locale
    /// with a decimal comma, loses the rest of its table rather than skipping and counting that
    /// benchmark alone, or judges the values before it has read all of the cells.
    #[test]
    fn adapter_cpp_catch2_v1_locale() {
        let (text, xml) = pair("corpus_locale_decl");
        assert_agree(&text, &xml, "corpus_locale_decl");
        assert_eq!((text.inner.len(), text.skipped.benchmarks), (0, 4));
    }

    /// Kills a text leaf that takes whitespace at a name's start for a place `TextFlow` breaks,
    /// which keeps the hyphen it added.
    #[test]
    fn adapter_cpp_catch2_v1_text_leading_split() {
        let (text, xml) = pair("corpus_shapes9_decl");
        for name in [
            "Leading spaces split /    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            "Leading spaces split /  yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy",
        ] {
            assert!(names(&text).contains(name), "{name}");
            assert!(names(&xml).contains(name), "{name}");
        }
    }

    /// Kills a text leaf that trims the leading spaces of a name in the first column, which are
    /// the name's own.
    #[test]
    fn adapter_cpp_catch2_v1_text_leading_spaces() {
        let (text, xml) = pair("corpus_names2_decl");
        let name = "Spaces at the ends /   leading spaces";
        assert!(names(&text).contains(name));
        assert!(names(&xml).contains(name));
    }

    fn pair(fixture: &str) -> (AdapterResults, AdapterResults) {
        (
            convert_v1(Adapter::CppCatch2Text, &format!("cpp/catch2/{fixture}.txt")),
            convert_v1(Adapter::CppCatch2Xml, &format!("cpp/catch2/{fixture}.xml")),
        )
    }

    /// The text leaf keeps the benchmarks the XML leaf keeps and counts the same failures.
    fn assert_agree(text: &AdapterResults, xml: &AdapterResults, fixture: &str) {
        assert_eq!(names(text), names(xml), "{fixture}");
        assert_eq!(text.skipped, xml.skipped, "{fixture}");
    }

    /// Every metric of one run's two reports agrees to the six significant digits Catch2 prints
    /// in each, by statistic, as the text states no level.
    fn assert_same_values(text: &AdapterResults, xml: &AdapterResults, fixture: &str) {
        let (text, xml) = (metrics(text), metrics(xml));
        assert_eq!(
            text.keys().collect::<Vec<_>>(),
            xml.keys().collect::<Vec<_>>(),
            "{fixture}"
        );
        for ((metric, text), xml) in text.iter().zip(xml.values()) {
            assert_close(fixture, metric, *text, *xml);
        }
    }

    fn assert_close(fixture: &str, metric: &str, text: f64, xml: f64) {
        let digit = 10f64.powf(text.abs().max(xml.abs()).log10().floor() - 5.0);
        assert!(
            (text - xml).abs() <= digit,
            "{fixture} {metric}: {text} != {xml}"
        );
    }

    /// Each metric by benchmark, measure, and statistic, its interval's level left out.
    fn metrics(results: &AdapterResults) -> BTreeMap<String, f64> {
        let mut metrics = BTreeMap::new();
        for (benchmark, variants) in &results.inner {
            for (measure, metric) in variants.values().flat_map(|variant| &variant.inner) {
                for (name, value) in &metric.inner {
                    let statistic = statistic(name.as_ref());
                    metrics.insert(
                        format!("{benchmark} {measure} {statistic}"),
                        value.into_inner(),
                    );
                }
            }
        }
        metrics
    }

    /// A metric's name without its interval's level, as in `mean_ci95_lower`.
    fn statistic(name: &str) -> String {
        let Some((statistic, rest)) = name.split_once("_ci") else {
            return name.to_owned();
        };
        let bound = rest.split_once('_').map_or(rest, |(_, bound)| bound);
        format!("{statistic}_ci_{bound}")
    }

    /// Kills a text leaf that keeps a pending name across a table's header, which glues the
    /// section times `-d yes` prints onto the next benchmark's name.
    #[test]
    fn adapter_cpp_catch2_v1_durations() {
        let text = convert_v1(Adapter::CppCatch2Text, "cpp/catch2/durations.txt");
        let xml = convert_v1(Adapter::CppCatch2Xml, "cpp/catch2/durations.xml");
        assert_eq!(
            names(&text),
            [
                "Plain / Fibonacci 12",
                "Sections / First / Fibonacci 10",
                "Sections / Second / Fibonacci 11",
            ]
            .map(ToOwned::to_owned)
            .into()
        );
        assert_eq!(names(&text), names(&xml));
    }

    fn names(results: &AdapterResults) -> BTreeSet<String> {
        results.inner.keys().map(ToString::to_string).collect()
    }

    /// Kills a node that tries the text leaf before the XML leaf.
    #[test]
    fn adapter_cpp_catch2_v1_tries_xml_first() {
        let input = ["v3.xml", "v3.txt"]
            .map(|fixture| {
                std::fs::read_to_string(format!("./tool_output/cpp/catch2/{fixture}"))
                    .expect("Failed to read a fixture")
            })
            .join("\n");
        let results = Adapter::CppCatch2
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse");
        assert_eq!(results.resolved_adapter, Some(Adapter::CppCatch2Xml));
    }
}
