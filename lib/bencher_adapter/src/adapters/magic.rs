use crate::{
    results::adapter_results::AdapterResults, Adaptable, AdapterCSharp, AdapterCpp, AdapterGo,
    AdapterJava, AdapterJs, AdapterJson, AdapterPython, AdapterRuby, AdapterRust, AdapterShell,
    Settings,
};

pub struct AdapterMagic;

impl Adaptable for AdapterMagic {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJson::parse(input, settings)
            .or_else(|| AdapterCSharp::parse(input, settings))
            .or_else(|| AdapterCpp::parse(input, settings))
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
    use crate::adapters::{
        c_sharp::{dot_net::test_c_sharp_dot_net, AdapterCSharp},
        cpp::{catch2::test_cpp_catch2, google::test_cpp_google},
        go::bench::test_go_bench,
        java::jmh::test_java_jmh,
        js::{benchmark::test_js_benchmark, time::test_js_time},
        json::test_json,
        python::{asv::test_python_asv, pytest::test_python_pytest},
        ruby::benchmark::test_ruby_benchmark,
        rust::{
            bench::test_rust_bench, criterion::test_rust_criterion, iai::test_rust_iai,
            iai_callgrind::test_rust_iai_callgrind,
        },
        shell::hyperfine::test_shell_hyperfine,
        test_util::convert_file_path,
    };

    #[test]
    fn test_adapter_magic_json_latency() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_latency.json");
        test_json::validate_adapter_json_latency(&results);
    }

    #[test]
    fn test_adapter_magic_json_dhat() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_dhat.json");
        test_json::validate_adapter_json_dhat(&results);
    }

    #[test]
    fn test_adapter_magic_c_sharp_dot_net() {
        let results = convert_file_path::<AdapterCSharp>("./tool_output/c_sharp/dot_net/two.json");
        test_c_sharp_dot_net::validate_adapter_c_sharp_dot_net(&results);
    }

    #[test]
    fn test_adapter_magic_cpp_google() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/cpp/google/two.txt");
        test_cpp_google::validate_adapter_cpp_google(&results);
    }

    #[test]
    fn test_adapter_magic_cpp_catch2() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/cpp/catch2/four.txt");
        test_cpp_catch2::validate_adapter_cpp_catch2(&results);
    }

    #[test]
    fn test_adapter_magic_go_bench() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/go/bench/five.txt");
        test_go_bench::validate_adapter_go_bench(&results);
    }

    #[test]
    fn test_adapter_magic_java_jmh() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/java/jmh/six.json");
        test_java_jmh::validate_adapter_java_jmh(&results);
    }

    #[test]
    fn test_adapter_magic_js_benchmark() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/js/benchmark/three.txt");
        test_js_benchmark::validate_adapter_js_benchmark(&results);
    }

    #[test]
    fn test_adapter_magic_js_time() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/js/time/four.txt");
        test_js_time::validate_adapter_js_time(&results);
    }

    #[test]
    fn test_adapter_python_asv() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/python/asv/six.txt");
        test_python_asv::validate_adapter_python_asv(&results);
    }

    #[test]
    fn test_adapter_python_pytest() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/python/pytest/four.json");
        test_python_pytest::validate_adapter_python_pytest(&results);
    }

    #[test]
    fn test_adapter_ruby_benchmark() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/ruby/benchmark/five.txt");
        test_ruby_benchmark::validate_adapter_ruby_benchmark(&results);
    }

    #[test]
    fn test_adapter_magic_rust_bench() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/bench/many.txt");
        test_rust_bench::validate_adapter_rust_bench(&results);
    }

    #[test]
    fn test_adapter_magic_rust_criterion() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/criterion/many.txt");
        test_rust_criterion::validate_adapter_rust_criterion(&results);
    }

    #[test]
    fn test_adapter_magic_rust_iai() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/iai/two.txt");
        test_rust_iai::validate_adapter_rust_iai(&results);
    }

    #[test]
    fn test_adapter_magic_rust_iai_callgrind() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/iai_callgrind/two.txt");
        test_rust_iai_callgrind::validate_adapter_rust_iai_callgrind(&results);
    }

    #[test]
    fn test_adapter_magic_shell_hyperfine() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/shell/hyperfine/two.json");
        test_shell_hyperfine::validate_adapter_shell_hyperfine(&results);
    }
}
