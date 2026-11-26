import { type Language, defaultLang } from "../../i18n/ui";
import { Adapter } from "../../types/bencher";

export const adapterName = (adapter: Adapter): string => {
	switch (adapter) {
		case Adapter.Magic:
			return "Magic";
		case Adapter.Json:
		case Adapter.JsonV0:
		case Adapter.JsonV1:
			return "JSON";
		case Adapter.RustBench:
		case Adapter.RustBenchJson:
		case Adapter.RustBenchText:
			return "libtest bench";
		case Adapter.RustCriterion:
		case Adapter.RustCriterionJson:
		case Adapter.RustCriterionText:
			return "Criterion";
		case Adapter.RustIai:
			return "Iai";
		case Adapter.RustGungraun:
		case Adapter.RustGungraunStdout:
		case Adapter.RustGungraunJson:
			return "Gungraun";
		case Adapter.RustDivan:
			return "Divan";
		case Adapter.CppGoogle:
			return "Google Benchmark";
		case Adapter.CppCatch2:
		case Adapter.CppCatch2Xml:
		case Adapter.CppCatch2Text:
			return "Catch2";
		case Adapter.DartBenchmarkHarness:
			return "benchmark_harness";
		case Adapter.GoBench:
		case Adapter.GoBenchJson:
		case Adapter.GoBenchText:
			return "go test -bench";
		case Adapter.JavaJmh:
			return "Java Microbenchmark Harness (JMH)";
		case Adapter.CSharpDotNet:
			return "BenchmarkDotNet";
		case Adapter.JsBenchmark:
			return "Benchmark.js";
		case Adapter.JsTime:
			return "console.time/console.timeEnd";
		case Adapter.JsVitest:
		case Adapter.JsVitestV1:
		case Adapter.JsVitestV5:
			return "Vitest";
		case Adapter.JsDeno:
			return "Deno bench";
		case Adapter.JsMitata:
			return "mitata";
		case Adapter.JsTinybench:
			return "tinybench";
		case Adapter.PythonAsv:
		case Adapter.PythonAsvJson:
		case Adapter.PythonAsvText:
			return "airspeed velocity";
		case Adapter.PythonPytest:
			return "pytest-benchmark";
		case Adapter.RubyBenchmark:
			return "Benchmark";
		case Adapter.RubyBenchmarkIps:
			return "benchmark-ips";
		case Adapter.ShellHyperfine:
		case Adapter.ShellHyperfineV1:
		case Adapter.ShellHyperfineV2:
			return "Hyperfine";
	}
};

export const adapterCommand = (isConsole: boolean, adapter: null | Adapter) => {
	const host = isConsole ? "" : " --host http://localhost:6610";

	switch (adapter) {
		case Adapter.RustBench:
		case Adapter.RustBenchJson:
		case Adapter.RustBenchText:
			return `bencher run${host} "cargo +nightly bench"`;
		case Adapter.RustCriterion:
		case Adapter.RustCriterionJson:
		case Adapter.RustCriterionText:
		case Adapter.RustIai:
		case Adapter.RustGungraun:
		case Adapter.RustGungraunStdout:
		case Adapter.RustGungraunJson:
		case Adapter.RustDivan:
			return `bencher run${host} "cargo bench"`;
		case Adapter.CppGoogle:
			return `bencher run${host} "./benchmarks --benchmark_format=json"`;
		case Adapter.CppCatch2:
		case Adapter.CppCatch2Xml:
		case Adapter.CppCatch2Text:
			return `bencher run${host} "./benchmarks -r xml"`;
		case Adapter.DartBenchmarkHarness:
			return `bencher run${host} --adapter dart_benchmark_harness "dart run benchmark/benchmark.dart"`;
		case Adapter.GoBench:
		case Adapter.GoBenchJson:
		case Adapter.GoBenchText:
			return `bencher run${host} "go test -bench ."`;
		case Adapter.JavaJmh:
			return `bencher run${host} --file results.json "java -jar benchmarks.jar -rf json -rff results.json"`;
		case Adapter.CSharpDotNet:
			return `bencher run${host} --file BenchmarkDotNet.Artifacts/results/Benchmarks-report-full-compressed.json "dotnet run -c Release -- --exporters json --filter '*'"`;
		case Adapter.JsBenchmark:
		case Adapter.JsTime:
			return `bencher run${host} "node benchmark.js"`;
		case Adapter.JsVitest:
		case Adapter.JsVitestV5:
			return `bencher run${host} --file results.json "vitest bench --run --reporter=json --outputFile=results.json"`;
		case Adapter.JsVitestV1:
			return `bencher run${host} --file results.json "vitest bench --run --outputJson results.json"`;
		case Adapter.JsDeno:
			return `bencher run${host} "deno bench --json"`;
		case Adapter.JsMitata:
			return `bencher run${host} "node benchmarks.mjs"`;
		case Adapter.JsTinybench:
			return `bencher run${host} "node benchmarks.js"`;
		case Adapter.PythonAsv:
		case Adapter.PythonAsvJson:
		case Adapter.PythonAsvText:
			return `bencher run${host} "asv run"`;
		case Adapter.PythonPytest:
			return `bencher run${host} --file results.json "pytest --benchmark-max-time 0.02 --benchmark-json results.json benchmarks.py"`;
		case Adapter.RubyBenchmark:
			return `bencher run${host} "ruby benchmarks.rb"`;
		case Adapter.RubyBenchmarkIps:
			return `bencher run${host} --file results.json "ruby benchmarks.rb"`;
		case Adapter.ShellHyperfine:
		case Adapter.ShellHyperfineV1:
		case Adapter.ShellHyperfineV2:
			return `bencher run${host} --file results.json "hyperfine --export-json results.json 'sleep 0.1'"`;
		// biome-ignore lint/complexity/noUselessSwitchCase: code as docs
		case Adapter.Json:
		// biome-ignore lint/complexity/noUselessSwitchCase: code as docs
		case Adapter.JsonV0:
		// biome-ignore lint/complexity/noUselessSwitchCase: code as docs
		case Adapter.JsonV1:
		default:
			return `bencher run${host} "bencher mock"`;
	}
};

/** The adapter's section on the adapters page, at the heading id the pull request comment links. */
export const adapterDocsUrl = (lang: Language, adapter: Adapter) => {
	const prefix = lang === defaultLang ? "" : `/${lang}`;
	return `${prefix}/docs/explanation/adapters/#${adapterFragment(adapter)}`;
};

const adapterFragment = (adapter: Adapter) => {
	switch (adapter) {
		case Adapter.Json:
		case Adapter.JsonV0:
		case Adapter.JsonV1:
			return "-json";
		case Adapter.RustBench:
		case Adapter.RustBenchJson:
		case Adapter.RustBenchText:
			return "-rust-bench";
		case Adapter.RustCriterion:
		case Adapter.RustCriterionJson:
		case Adapter.RustCriterionText:
			return "-rust-criterion";
		case Adapter.RustIai:
			return "-rust-iai";
		case Adapter.RustGungraun:
		case Adapter.RustGungraunStdout:
		case Adapter.RustGungraunJson:
			return "-rust-gungraun";
		case Adapter.RustDivan:
			return "-rust-divan";
		case Adapter.CppGoogle:
			return "-c-google";
		case Adapter.CppCatch2:
		case Adapter.CppCatch2Xml:
		case Adapter.CppCatch2Text:
			return "-c-catch2";
		case Adapter.GoBench:
		case Adapter.GoBenchJson:
		case Adapter.GoBenchText:
			return "-go-bench";
		case Adapter.JavaJmh:
			return "%EF%B8%8F-java-jmh";
		case Adapter.CSharpDotNet:
			return "%EF%B8%8F%E2%83%A3-c-dotnet";
		case Adapter.JsBenchmark:
			return "-javascript-benchmark";
		case Adapter.JsTime:
			return "-javascript-time";
		case Adapter.JsVitest:
		case Adapter.JsVitestV1:
		case Adapter.JsVitestV5:
			return "-javascript-vitest";
		case Adapter.JsDeno:
			return "-javascript-deno";
		case Adapter.JsMitata:
			return "-javascript-mitata";
		case Adapter.JsTinybench:
			return "-javascript-tinybench";
		case Adapter.PythonAsv:
		case Adapter.PythonAsvJson:
		case Adapter.PythonAsvText:
			return "-python-asv";
		case Adapter.PythonPytest:
			return "-python-pytest";
		case Adapter.RubyBenchmark:
			return "%EF%B8%8F-ruby-benchmark";
		case Adapter.RubyBenchmarkIps:
			return "%EF%B8%8F-ruby-benchmark-ips";
		case Adapter.ShellHyperfine:
		case Adapter.ShellHyperfineV1:
		case Adapter.ShellHyperfineV2:
			return "_%EF%B8%8F-shell-hyperfine";
		case Adapter.DartBenchmarkHarness:
			return "-dart-benchmark_harness";
		case Adapter.Magic:
		case Adapter.Rust:
		case Adapter.Cpp:
		case Adapter.Go:
		case Adapter.Java:
		case Adapter.CSharp:
		case Adapter.Js:
		case Adapter.Python:
		case Adapter.Ruby:
		case Adapter.Shell:
		case Adapter.Dart:
			return "-magic-default";
	}
};
