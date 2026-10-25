pub mod benchmark;
mod rate;
pub mod time;
pub(crate) mod tinybench;
pub mod vitest;

#[cfg(test)]
mod test_js_dart_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings};

    /// Each JavaScript and Dart harness's leaves, and the directory of its fixtures.
    const HARNESSES: [(&[Adapter], &str); 4] = [
        (&[Adapter::JsBenchmark], "js/benchmark"),
        (&[Adapter::JsTime], "js/time"),
        (&[Adapter::JsVitestV5, Adapter::JsVitestV1], "js/vitest"),
        (&[Adapter::DartBenchmarkHarness], "dart/benchmark_harness"),
    ];

    /// Kills a leaf that claims another JavaScript or Dart harness's output.
    #[test]
    fn js_dart_v1_leaves_refuse_other_harnesses() {
        let settings = Settings::new(None, BmfVersion::V1);
        for (leaves, directory) in HARNESSES {
            for (_, other) in HARNESSES.iter().filter(|(_, other)| *other != directory) {
                for entry in std::fs::read_dir(format!("./tool_output/{other}"))
                    .expect("Failed to read the fixtures")
                {
                    let path = entry.expect("Failed to read a fixture").path();
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
}
