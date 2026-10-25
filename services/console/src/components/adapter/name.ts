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
		case Adapter.PythonAsv:
			return "airspeed velocity";
		case Adapter.PythonPytest:
			return "pytest-benchmark";
		case Adapter.RubyBenchmark:
			return "Benchmark";
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
			return `bencher run${host} "cargo bench"`;
		case Adapter.CppGoogle:
			return `bencher run${host} "make benchmarks --benchmark_format=json"`;
		case Adapter.CppCatch2:
		case Adapter.CppCatch2Xml:
		case Adapter.CppCatch2Text:
			return `bencher run${host} "make benchmarks"`;
		case Adapter.DartBenchmarkHarness:
			return `bencher run${host} --adapter dart_benchmark_harness "dart run benchmark/benchmark.dart"`;
		case Adapter.GoBench:
		case Adapter.GoBenchJson:
		case Adapter.GoBenchText:
			return `bencher run${host} "go test -bench"`;
		case Adapter.JavaJmh:
			return `bencher run${host} --file results.json "java -jar benchmarks.jar -rf json -rff results.json"`;
		case Adapter.CSharpDotNet:
			return `bencher run${host} "dotnet run -c Release"`;
		case Adapter.JsBenchmark:
		case Adapter.JsTime:
			return `bencher run${host} "node benchmark.js"`;
		case Adapter.JsVitest:
		case Adapter.JsVitestV1:
			return `bencher run${host} --file results.json "vitest bench --run --outputJson results.json"`;
		case Adapter.JsVitestV5:
			return `bencher run${host} --file results.json "vitest bench --run --reporter=json --outputFile results.json"`;
		case Adapter.PythonAsv:
			return `bencher run${host} "asv run"`;
		case Adapter.PythonPytest:
			return `bencher run${host} --file results.json "pytest --benchmark-json results.json benchmarks.py"`;
		case Adapter.RubyBenchmark:
			return `bencher run${host} "ruby benchmarks.rb"`;
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
