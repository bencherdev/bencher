pub mod asv;
pub mod pytest;

#[cfg(test)]
mod test_python_ruby_shell_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings};

    /// Each Python, Ruby, and shell leaf, the directory of its harness's fixtures, and the fixtures
    /// there of its harness's other format.
    const LEAVES: [(Adapter, &str, &[&str]); 6] = [
        (Adapter::PythonPytest, "python/pytest", &[]),
        (
            Adapter::PythonAsvJson,
            "python/asv",
            &[
                "benchmarks.json",
                "commits.txt",
                "empty.txt",
                "params.txt",
                "six.txt",
            ],
        ),
        (
            Adapter::PythonAsvText,
            "python/asv",
            &[
                "benchmarks.json",
                "empty.json",
                "kinds.json",
                "results.json",
                "samples.json",
                "shapes.json",
            ],
        ),
        (Adapter::RubyBenchmark, "ruby/benchmark", &[]),
        (
            Adapter::ShellHyperfineV2,
            "shell/hyperfine",
            &[
                "one.json",
                "two.json",
                "v1_one_run.json",
                "v1_same_name.json",
                "v1_scan.json",
            ],
        ),
        (
            Adapter::ShellHyperfineV1,
            "shell/hyperfine",
            &[
                "v2_counters.json",
                "v2_env.json",
                "v2_env_clash.json",
                "v2_env_named.json",
                "v2_env_quoted.json",
                "v2_env_scan.json",
                "v2_failure.json",
                "v2_named.json",
                "v2_one.json",
                "v2_one_run.json",
                "v2_parameters.json",
                "v2_same_name.json",
                "v2_scan_one_run.json",
                "v2_two.json",
                "v2_two_lists.json",
            ],
        ),
    ];

    /// Kills a leaf that claims another Python, Ruby, or shell harness's output, or a hyperfine
    /// leaf that claims the other hyperfine format.
    #[test]
    fn python_ruby_shell_v1_leaves_refuse_other_formats() {
        let settings = Settings::new(None, BmfVersion::V1);
        for (leaf, directory, others) in LEAVES {
            let mut inputs = Vec::new();
            for (_, other, _) in LEAVES.iter().filter(|(_, other, _)| *other != directory) {
                for entry in std::fs::read_dir(format!("./tool_output/{other}"))
                    .expect("Failed to read the fixtures")
                {
                    inputs.push(entry.expect("Failed to read a fixture").path());
                }
            }
            inputs.extend(
                others
                    .iter()
                    .map(|other| format!("./tool_output/{directory}/{other}").into()),
            );
            for path in inputs {
                let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
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
