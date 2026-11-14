use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            c_sharp::dot_net::AdapterCSharpDotNetV1,
            cpp::{catch2::AdapterCppCatch2V1, google::AdapterCppGoogleV1},
            dart::benchmark_harness::AdapterDartBenchmarkHarnessV1,
            go::bench::AdapterGoBenchV1,
            java::jmh::AdapterJavaJmhV1,
            js::{
                benchmark::AdapterJsBenchmarkV1, time::AdapterJsTimeV1, vitest::AdapterJsVitestV1,
            },
            python::{asv::AdapterPythonAsvV1, pytest::AdapterPythonPytestV1},
            ruby::benchmark::AdapterRubyBenchmarkV1,
            rust::{
                bench::AdapterRustBenchV1, criterion::AdapterRustCriterionV1,
                gungraun::AdapterRustGungraunV1, iai::AdapterRustIaiV1,
            },
            shell::hyperfine::AdapterShellHyperfineV1,
        },
        json::v1::AdapterJsonV1,
    },
    results::adapter_results::AdapterResults,
};

/// The version 1 `magic`: BMF, then each harness in the version 0 `magic`'s order, each through
/// its structured format before its text.
pub struct AdapterMagicV1;

impl Adaptable for AdapterMagicV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJsonV1::parse(input, settings)
            .or_else(|| AdapterCSharpDotNetV1::parse(input, settings))
            .or_else(|| AdapterCppCatch2V1::parse(input, settings))
            .or_else(|| AdapterCppGoogleV1::parse(input, settings))
            .or_else(|| AdapterDartBenchmarkHarnessV1::parse(input, settings))
            .or_else(|| AdapterGoBenchV1::parse(input, settings))
            .or_else(|| AdapterJavaJmhV1::parse(input, settings))
            .or_else(|| AdapterJsBenchmarkV1::parse(input, settings))
            .or_else(|| AdapterJsTimeV1::parse(input, settings))
            .or_else(|| AdapterJsVitestV1::parse(input, settings))
            .or_else(|| AdapterPythonAsvV1::parse(input, settings))
            .or_else(|| AdapterPythonPytestV1::parse(input, settings))
            .or_else(|| AdapterRubyBenchmarkV1::parse(input, settings))
            .or_else(|| AdapterRustBenchV1::parse(input, settings))
            .or_else(|| AdapterRustCriterionV1::parse(input, settings))
            .or_else(|| AdapterRustIaiV1::parse(input, settings))
            .or_else(|| AdapterRustGungraunV1::parse(input, settings))
            .or_else(|| AdapterShellHyperfineV1::parse(input, settings))
    }
}

#[cfg(test)]
mod test_magic_v1 {
    use std::{
        ffi::OsStr,
        path::{Path, PathBuf},
    };

    use bencher_json::{BmfVersion, project::report::Adapter};
    use pretty_assertions::assert_eq;

    use crate::{Adaptable as _, Settings};

    const FIXTURES: &str = "./tool_output";

    const LEAVES: [Adapter; 26] = [
        Adapter::JsonV1,
        Adapter::CSharpDotNet,
        Adapter::CppCatch2Xml,
        Adapter::CppCatch2Text,
        Adapter::CppGoogle,
        Adapter::DartBenchmarkHarness,
        Adapter::GoBenchJson,
        Adapter::GoBenchText,
        Adapter::JavaJmh,
        Adapter::JsBenchmark,
        Adapter::JsTime,
        Adapter::JsVitestV5,
        Adapter::JsVitestV1,
        Adapter::PythonAsvJson,
        Adapter::PythonAsvText,
        Adapter::PythonPytest,
        Adapter::RubyBenchmark,
        Adapter::RustBenchJson,
        Adapter::RustBenchText,
        Adapter::RustCriterionJson,
        Adapter::RustCriterionText,
        Adapter::RustIai,
        Adapter::RustGungraunJson,
        Adapter::RustGungraunStdout,
        Adapter::ShellHyperfineV2,
        Adapter::ShellHyperfineV1,
    ];

    /// The fixtures the directory and name rules do not decide, each with the leaf that owns it,
    /// or `None` when no leaf may claim it.
    const EXCEPTIONS: [(&str, Option<Adapter>); 8] = [
        // criterion's `--output-format bencher` prints libtest's bench lines.
        (
            "rust/criterion/bencher_format.txt",
            Some(Adapter::RustBenchText),
        ),
        // asv's benchmark list holds no results.
        ("python/asv/benchmarks.json", None),
        // BMF version 0.
        ("json/report_bmf_mixed.json", None),
        ("json/report_dhat.json", None),
        ("json/report_latency.json", None),
        // Neither BMF version: it mixes the two.
        ("json/report_mixed_versions.json", None),
        // libtest ran no benchmark.
        ("rust/bench/zero.txt", None),
        // libtest ran no benchmark, and gungraun's summary is of a schema no leaf reads.
        ("rust/gungraun/json_not_v6.txt", None),
    ];

    /// Every fixture is claimed by its own leaf alone, and `magic` reads it exactly as that leaf.
    ///
    /// Kills: a leaf that claims another harness's output or refuses its own, a leaf that records
    /// another leaf or another version, and a `magic` that skips a harness, reads with another
    /// leaf, or claims what every leaf refuses.
    #[test]
    fn magic_v1_every_fixture_is_claimed_by_its_owner_alone() {
        let settings = Settings::new(None, BmfVersion::V1);
        let mut fixtures = fixtures(Path::new(FIXTURES));
        fixtures.sort();
        for (exception, _) in EXCEPTIONS {
            assert!(
                fixtures.iter().any(|(fixture, _)| fixture == exception),
                "no fixture {exception}"
            );
        }
        for (fixture, path) in &fixtures {
            let input = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Failed to read {fixture}: {e}"));
            let owner = owner(fixture);
            let claimants: Vec<Adapter> = LEAVES
                .into_iter()
                .filter(|leaf| leaf.convert(&input, settings).is_some())
                .collect();
            assert_eq!(claimants, Vec::from_iter(owner), "{fixture}");
            let read = owner.and_then(|owner| owner.convert(&input, settings));
            if let Some(results) = &read {
                assert_eq!(
                    (results.resolved_adapter, results.version),
                    (owner, BmfVersion::V1),
                    "{fixture}"
                );
            }
            assert_eq!(
                Adapter::Magic.convert(&input, settings),
                read,
                "magic {fixture}"
            );
        }
    }

    /// criterion's `--output-format bencher` prints libtest's lines, which the libtest node reads
    /// and the criterion node refuses.
    ///
    /// Kills: a criterion node that falls back to libtest's text leaf.
    #[test]
    fn magic_v1_libtest_lines_are_libtest_nodes() {
        let settings = Settings::new(None, BmfVersion::V1);
        let input =
            std::fs::read_to_string(format!("{FIXTURES}/rust/criterion/bencher_format.txt"))
                .expect("Failed to read a fixture");
        assert_eq!(Adapter::RustCriterion.convert(&input, settings), None);
        assert_eq!(
            Adapter::RustBench
                .convert(&input, settings)
                .and_then(|results| results.resolved_adapter),
            Some(Adapter::RustBenchText)
        );
    }

    /// The leaf that owns a fixture, by its directory and name.
    fn owner(fixture: &str) -> Option<Adapter> {
        if let Some((_, owner)) = EXCEPTIONS
            .iter()
            .find(|(exception, _)| *exception == fixture)
        {
            return *owner;
        }
        let (directory, name) = fixture
            .rsplit_once('/')
            .unwrap_or_else(|| panic!("no directory for {fixture}"));
        let owner = rule(directory, name);
        assert!(
            owner.is_some(),
            "no owner for {fixture}: give it a rule, or list it with a reason"
        );
        owner
    }

    /// The leaf that owns a fixture in `directory` named `name`, if a rule decides it.
    fn rule(directory: &str, name: &str) -> Option<Adapter> {
        let json = name.starts_with("json");
        let extension = Path::new(name).extension().and_then(OsStr::to_str);
        let (structured, (structured_leaf, text_leaf)) = match directory {
            "c_sharp/dot_net" => return Some(Adapter::CSharpDotNet),
            "cpp/google" => return Some(Adapter::CppGoogle),
            "dart/benchmark_harness" => return Some(Adapter::DartBenchmarkHarness),
            "java/jmh" => return Some(Adapter::JavaJmh),
            "js/benchmark" => return Some(Adapter::JsBenchmark),
            "js/time" => return Some(Adapter::JsTime),
            "python/pytest" => return Some(Adapter::PythonPytest),
            "ruby/benchmark" => return Some(Adapter::RubyBenchmark),
            "rust/iai" => return Some(Adapter::RustIai),
            "json" => return name.starts_with("report_v1_").then_some(Adapter::JsonV1),
            "cpp/catch2" => (
                extension == Some("xml"),
                (Adapter::CppCatch2Xml, Adapter::CppCatch2Text),
            ),
            "go/bench" => (json, (Adapter::GoBenchJson, Adapter::GoBenchText)),
            "js/vitest" => (
                name.starts_with("v5"),
                (Adapter::JsVitestV5, Adapter::JsVitestV1),
            ),
            "python/asv" => (
                extension == Some("json"),
                (Adapter::PythonAsvJson, Adapter::PythonAsvText),
            ),
            "rust/bench" => (json, (Adapter::RustBenchJson, Adapter::RustBenchText)),
            "rust/criterion" => (
                json,
                (Adapter::RustCriterionJson, Adapter::RustCriterionText),
            ),
            "rust/gungraun" => (
                json,
                (Adapter::RustGungraunJson, Adapter::RustGungraunStdout),
            ),
            "shell/hyperfine" => (
                name.starts_with("v2"),
                (Adapter::ShellHyperfineV2, Adapter::ShellHyperfineV1),
            ),
            _ => return None,
        };
        Some(if structured {
            structured_leaf
        } else {
            text_leaf
        })
    }

    /// Every fixture under `directory`, named by its path below the fixtures' root.
    fn fixtures(directory: &Path) -> Vec<(String, PathBuf)> {
        let mut fixtures = Vec::new();
        for entry in std::fs::read_dir(directory).expect("Failed to read the fixtures") {
            let path = entry.expect("Failed to read a fixture").path();
            if path.is_dir() {
                fixtures.extend(self::fixtures(&path));
            } else {
                let fixture = path
                    .strip_prefix(FIXTURES)
                    .expect("Failed to name a fixture")
                    .to_str()
                    .expect("Failed to name a fixture")
                    .to_owned();
                fixtures.push((fixture, path));
            }
        }
        fixtures
    }
}
