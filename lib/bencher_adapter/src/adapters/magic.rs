use crate::{
    Adaptable, AdapterCSharp, AdapterCpp, AdapterDart, AdapterGo, AdapterJava, AdapterJs,
    AdapterJson, AdapterPython, AdapterRuby, AdapterRust, AdapterShell, Settings,
    results::adapter_results::AdapterResults,
};

pub struct AdapterMagic;

impl Adaptable for AdapterMagic {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJson::parse(input, settings)
            .or_else(|| AdapterCSharp::parse(input, settings))
            .or_else(|| AdapterCpp::parse(input, settings))
            .or_else(|| AdapterDart::parse(input, settings))
            .or_else(|| AdapterGo::parse(input, settings))
            .or_else(|| AdapterJava::parse(input, settings))
            .or_else(|| AdapterJs::parse(input, settings))
            .or_else(|| AdapterPython::parse(input, settings))
            .or_else(|| AdapterRuby::parse(input, settings))
            .or_else(|| AdapterRust::parse(input, settings))
            .or_else(|| AdapterShell::parse(input, settings))
    }
}

#[cfg(test)]
mod test_magic {
    use super::AdapterMagic;
    use crate::Settings;
    use crate::adapters::{
        c_sharp::{AdapterCSharp, dot_net::test_c_sharp_dot_net},
        cpp::{catch2::test_cpp_catch2, google::test_cpp_google},
        dart::benchmark_harness::test_dart_benchmark_harness,
        go::bench::test_go_bench,
        java::jmh::test_java_jmh,
        js::{benchmark::test_js_benchmark, time::test_js_time, vitest::test_js_vitest},
        json::{test_json, v0::test_json_v0, v1::test_json_v1},
        python::{asv::test_python_asv, pytest::test_python_pytest},
        ruby::benchmark::test_ruby_benchmark,
        rust::{
            bench::test_rust_bench, criterion::test_rust_criterion,
            gungraun_json::test_rust_gungraun_json, gungraun_stdout::test_rust_gungraun_stdout,
            iai::test_rust_iai,
        },
        shell::hyperfine::test_shell_hyperfine,
        test_util::{convert_file_path, opt_convert_file_path},
    };

    #[test]
    fn adapter_magic_json_latency() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_latency.json");
        test_json::validate_adapter_json_latency(&results);
    }

    #[test]
    fn adapter_magic_json_dhat() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_dhat.json");
        test_json::validate_adapter_json_dhat(&results);
    }

    #[test]
    fn adapter_magic_json_bmf_mixed() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_bmf_mixed.json");
        test_json_v0::validate_adapter_json_bmf_mixed(&results);
    }

    #[test]
    fn adapter_magic_json_v1_latency() {
        let results =
            convert_file_path::<AdapterMagic>("./tool_output/json/report_v1_latency.json");
        test_json_v1::validate_adapter_json_v1_latency(&results);
    }

    #[test]
    fn adapter_magic_json_v1_parameters() {
        let results =
            convert_file_path::<AdapterMagic>("./tool_output/json/report_v1_parameters.json");
        test_json_v1::validate_adapter_json_v1_parameters(&results);
    }

    #[test]
    fn adapter_magic_json_v1_named() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_v1_named.json");
        test_json_v1::validate_adapter_json_v1_named(&results);
    }

    #[test]
    fn adapter_magic_json_v1_cap() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_v1_cap.json");
        test_json_v1::validate_adapter_json_v1_cap(&results);
    }

    #[test]
    fn adapter_magic_json_v1_canonical() {
        let results =
            convert_file_path::<AdapterMagic>("./tool_output/json/report_v1_canonical.json");
        test_json_v1::validate_adapter_json_v1_canonical(&results);
    }

    /// A payload that mixes the two BMF shapes fails the json node,
    /// and no other adapter claims it either.
    #[test]
    fn adapter_magic_json_mixed_versions_fails() {
        assert!(
            opt_convert_file_path::<AdapterMagic>(
                "./tool_output/json/report_mixed_versions.json",
                Settings::default()
            )
            .is_none(),
            "expected a mixed version payload to fail magic"
        );
    }

    /// An out of bounds parameter set fails magic outright: the json node rejects
    /// it and no other adapter claims it either.
    #[test]
    fn adapter_magic_json_out_of_bounds_parameters_fails() {
        assert!(
            opt_convert_file_path::<AdapterMagic>(
                "./tool_output/json/report_v1_bad_parameters.json",
                Settings::default()
            )
            .is_none(),
            "expected an out of bounds parameter set to fail magic"
        );
    }

    #[test]
    fn adapter_magic_c_sharp_dot_net() {
        let results = convert_file_path::<AdapterCSharp>("./tool_output/c_sharp/dot_net/two.json");
        test_c_sharp_dot_net::validate_adapter_c_sharp_dot_net(&results);
    }

    #[test]
    fn adapter_magic_cpp_google() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/cpp/google/two.txt");
        test_cpp_google::validate_adapter_cpp_google(&results);
    }

    #[test]
    fn adapter_magic_cpp_catch2() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/cpp/catch2/four.txt");
        test_cpp_catch2::validate_adapter_cpp_catch2(&results);
    }

    #[test]
    fn adapter_magic_dart_benchmark_harness() {
        let results =
            convert_file_path::<AdapterMagic>("./tool_output/dart/benchmark_harness/two.txt");
        test_dart_benchmark_harness::validate_adapter_dart_benchmark_harness(&results);
    }

    #[test]
    fn adapter_magic_go_bench() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/go/bench/five.txt");
        test_go_bench::validate_adapter_go_bench(&results);
    }

    #[test]
    fn adapter_magic_java_jmh() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/java/jmh/six.json");
        test_java_jmh::validate_adapter_java_jmh(&results);
    }

    #[test]
    fn adapter_magic_js_benchmark() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/js/benchmark/four.txt");
        test_js_benchmark::validate_adapter_js_benchmark(&results);
    }

    #[test]
    fn adapter_magic_js_time() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/js/time/four.txt");
        test_js_time::validate_adapter_js_time(&results);
    }

    #[test]
    fn adapter_magic_js_vitest() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/js/vitest/four.json");
        test_js_vitest::validate_adapter_js_vitest(&results);
    }

    #[test]
    fn adapter_python_asv() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/python/asv/six.txt");
        test_python_asv::validate_adapter_python_asv(&results);
    }

    #[test]
    fn adapter_python_pytest() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/python/pytest/four.json");
        test_python_pytest::validate_adapter_python_pytest(&results);
    }

    #[test]
    fn adapter_ruby_benchmark() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/ruby/benchmark/five.txt");
        test_ruby_benchmark::validate_adapter_ruby_benchmark(&results);
    }

    #[test]
    fn adapter_magic_rust_bench() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/bench/many.txt");
        test_rust_bench::validate_adapter_rust_bench(&results);
    }

    #[test]
    fn adapter_magic_rust_criterion() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/criterion/many.txt");
        test_rust_criterion::validate_adapter_rust_criterion(&results);
    }

    #[test]
    fn adapter_magic_rust_iai() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/iai/two.txt");
        test_rust_iai::validate_adapter_rust_iai(&results);
    }

    #[test]
    fn adapter_magic_rust_gungraun_stdout() {
        let results = convert_file_path::<AdapterMagic>(
            "./tool_output/rust/gungraun/without-optional-metrics.txt",
        );

        test_rust_gungraun_stdout::validate_adapter_rust_gungraun_stdout(
            &results,
            &test_rust_gungraun_stdout::OptionalMetrics::default(),
        );
    }

    #[test]
    fn adapter_magic_rust_gungraun_json() {
        {
            let results = convert_file_path::<AdapterMagic>(
                "./tool_output/rust/gungraun/json_pretty_one_callgrind.txt",
            );

            test_rust_gungraun_json::validate_adapter_rust_gungraun_json(&results);
        }

        {
            let results = convert_file_path::<AdapterMagic>(
                "./tool_output/rust/gungraun/json_one_callgrind_diff.txt",
            );

            test_rust_gungraun_json::validate_adapter_rust_gungraun_json(&results);
        }
    }

    #[test]
    fn adapter_magic_shell_hyperfine() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/shell/hyperfine/two.json");
        test_shell_hyperfine::validate_adapter_shell_hyperfine(&results);
    }
}
