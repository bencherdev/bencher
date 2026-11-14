pub mod bench;
pub mod criterion;
pub mod criterion_json;
pub mod criterion_text;
pub mod gungraun;
pub mod gungraun_json;
pub mod gungraun_stdout;
pub mod iai;

#[cfg(test)]
mod test_rust_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings, adapters::bmf_v1::test_util::convert_v1};

    /// Each harness's leaves, and the directory of its fixtures.
    const HARNESSES: [(&[Adapter], &str); 4] = [
        (&[Adapter::RustBenchJson, Adapter::RustBenchText], "bench"),
        (
            &[Adapter::RustCriterionJson, Adapter::RustCriterionText],
            "criterion",
        ),
        (&[Adapter::RustIai], "iai"),
        (
            &[Adapter::RustGungraunJson, Adapter::RustGungraunStdout],
            "gungraun",
        ),
    ];

    /// criterion's `--output-format bencher` prints libtest's bench lines.
    const LIBTEST_SHAPED: &str = "bencher_format.txt";

    /// Kills a leaf that claims another harness's output.
    #[test]
    fn rust_v1_leaves_refuse_other_harnesses() {
        let settings = Settings::new(None, BmfVersion::V1);
        for (leaves, directory) in HARNESSES {
            for (_, other) in HARNESSES.iter().filter(|(_, other)| *other != directory) {
                for entry in std::fs::read_dir(format!("./tool_output/rust/{other}"))
                    .expect("Failed to read the fixtures")
                {
                    let path = entry.expect("Failed to read a fixture").path();
                    if path.ends_with(LIBTEST_SHAPED) {
                        continue;
                    }
                    let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
                    for leaf in leaves {
                        assert_eq!(
                            leaf.convert(&input, settings),
                            None,
                            "{leaf} claimed {}",
                            path.display()
                        );
                    }
                }
            }
        }
    }

    /// Kills a criterion leaf that claims libtest's lines, which libtest's text leaf reads.
    #[test]
    fn rust_v1_libtest_lines_resolve_to_libtest() {
        let suffix = format!("rust/criterion/{LIBTEST_SHAPED}");
        let results = convert_v1(Adapter::RustBench, &suffix);
        assert_eq!(results.resolved_adapter, Some(Adapter::RustBenchText));
        let input = std::fs::read_to_string(format!("./tool_output/{suffix}"))
            .expect("Failed to read a fixture");
        assert_eq!(
            Adapter::RustCriterion.convert(&input, Settings::new(None, BmfVersion::V1)),
            None
        );
    }
}
