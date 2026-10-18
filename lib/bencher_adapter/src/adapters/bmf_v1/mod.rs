use bencher_json::project::report::Adapter;

use crate::{
    Adaptable as _, Settings, adapters::json::v1::AdapterJsonV1, convert_v0,
    results::adapter_results::AdapterResults,
};

mod magic;

use magic::AdapterMagicV1;

/// The BMF version 1 adapter tree.
///
/// An adapter it does not hold yet parses with the version 0 tree, whose results
/// the declared version then refuses, exactly as before this tree existed.
pub(crate) fn convert(adapter: Adapter, input: &str, settings: Settings) -> Option<AdapterResults> {
    match adapter {
        Adapter::Magic => AdapterMagicV1::parse(input, settings),
        Adapter::Json | Adapter::JsonV1 => AdapterJsonV1::parse(input, settings),
        Adapter::JsonV0
        | Adapter::Rust
        | Adapter::RustBench
        | Adapter::RustCriterion
        | Adapter::RustIai
        | Adapter::RustGungraun
        | Adapter::RustGungraunStdout
        | Adapter::RustGungraunJson
        | Adapter::Cpp
        | Adapter::CppGoogle
        | Adapter::CppCatch2
        | Adapter::Go
        | Adapter::GoBench
        | Adapter::Java
        | Adapter::JavaJmh
        | Adapter::CSharp
        | Adapter::CSharpDotNet
        | Adapter::Js
        | Adapter::JsBenchmark
        | Adapter::JsTime
        | Adapter::JsVitest
        | Adapter::Python
        | Adapter::PythonAsv
        | Adapter::PythonPytest
        | Adapter::Ruby
        | Adapter::RubyBenchmark
        | Adapter::Shell
        | Adapter::ShellHyperfine
        | Adapter::ShellHyperfineV1
        | Adapter::ShellHyperfineV2
        | Adapter::Dart
        | Adapter::DartBenchmarkHarness => convert_v0(adapter, input, settings),
    }
}

#[cfg(test)]
mod test_bmf_v1 {
    use bencher_json::{BmfVersion, project::report::Adapter};
    use std::path::{Path, PathBuf};

    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::json::{
            test_json::{V0_FIXTURES, V1_FIXTURES, fixture_path, version_settings},
            v1::AdapterJsonV1,
        },
        convert_v0,
    };

    const ADAPTERS: [Adapter; 35] = [
        Adapter::Magic,
        Adapter::Json,
        Adapter::JsonV0,
        Adapter::JsonV1,
        Adapter::Rust,
        Adapter::RustBench,
        Adapter::RustCriterion,
        Adapter::RustIai,
        Adapter::RustGungraun,
        Adapter::RustGungraunStdout,
        Adapter::RustGungraunJson,
        Adapter::Cpp,
        Adapter::CppGoogle,
        Adapter::CppCatch2,
        Adapter::Go,
        Adapter::GoBench,
        Adapter::Java,
        Adapter::JavaJmh,
        Adapter::CSharp,
        Adapter::CSharpDotNet,
        Adapter::Js,
        Adapter::JsBenchmark,
        Adapter::JsTime,
        Adapter::JsVitest,
        Adapter::Python,
        Adapter::PythonAsv,
        Adapter::PythonPytest,
        Adapter::Ruby,
        Adapter::RubyBenchmark,
        Adapter::Shell,
        Adapter::ShellHyperfine,
        Adapter::ShellHyperfineV1,
        Adapter::ShellHyperfineV2,
        Adapter::Dart,
        Adapter::DartBenchmarkHarness,
    ];

    /// Every adapter that parses a v1 payload at version 1 resolves it through
    /// the `json_v1` leaf, and records that leaf.
    #[test]
    fn bmf_v1_json_payload_resolves_to_json_v1() {
        for adapter in [Adapter::Magic, Adapter::Json, Adapter::JsonV1] {
            for suffix in V1_FIXTURES {
                let input = read(Path::new(&fixture_path(suffix)));
                let results = adapter
                    .convert(&input, version_settings(BmfVersion::V1))
                    .unwrap_or_else(|| panic!("expected {adapter} to parse {suffix}"));
                assert_eq!(
                    results.resolved_adapter,
                    Some(Adapter::JsonV1),
                    "{adapter} {suffix}"
                );
                assert_eq!(
                    Some(results),
                    AdapterJsonV1::parse(&input, Settings::default()),
                    "{adapter} {suffix}"
                );
            }
        }
    }

    /// Version 0 parses with the version 0 tree and records no leaf, so a v1
    /// payload is refused there and a v0 payload parses as it always has.
    #[test]
    fn bmf_v1_version_0_records_no_leaf() {
        let settings = version_settings(BmfVersion::V0);
        for adapter in [Adapter::Magic, Adapter::Json] {
            for suffix in V1_FIXTURES {
                let input = read(Path::new(&fixture_path(suffix)));
                assert_eq!(
                    adapter.convert(&input, settings),
                    None,
                    "{adapter} {suffix}"
                );
            }
            for suffix in V0_FIXTURES {
                let input = read(Path::new(&fixture_path(suffix)));
                let results = adapter
                    .convert(&input, settings)
                    .unwrap_or_else(|| panic!("expected {adapter} to parse {suffix}"));
                assert_eq!(results.resolved_adapter, None, "{adapter} {suffix}");
            }
        }
    }

    /// Every fixture through every adapter parses at either version exactly as
    /// the version 0 tree parses it, because the version 1 tree holds only the
    /// `json_v1` leaf, which the version 0 tree reaches at version 1 too.
    #[test]
    fn bmf_v1_every_payload_parses_as_the_version_0_tree() {
        let fixtures = fixtures(Path::new("./tool_output"));
        assert!(fixtures.len() > 64, "found {} fixtures", fixtures.len());
        for path in &fixtures {
            let input = read(path);
            for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
                let settings = version_settings(bmf_version);
                for adapter in ADAPTERS {
                    assert_eq!(
                        adapter.convert(&input, settings),
                        convert_v0(adapter, &input, settings),
                        "{adapter} at version {bmf_version}: {}",
                        path.display()
                    );
                }
            }
        }
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()))
    }

    fn fixtures(dir: &Path) -> Vec<PathBuf> {
        let mut fixtures = Vec::new();
        for entry in std::fs::read_dir(dir).expect("Failed to read the fixtures") {
            let path = entry.expect("Failed to read a fixture").path();
            if path.is_dir() {
                fixtures.extend(self::fixtures(&path));
            } else {
                fixtures.push(path);
            }
        }
        fixtures
    }
}
