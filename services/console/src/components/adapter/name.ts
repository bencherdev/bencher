import { Adapter } from "../../types/bencher";

export const adapterName = (adapter: Adapter): string => {
	switch (adapter) {
		case Adapter.Magic:
			return "Magic";
		case Adapter.Json:
			return "JSON";
		case Adapter.Rust:
			return "Rust";
		case Adapter.RustBench:
			return "libtest bench";
		case Adapter.RustCriterion:
			return "Criterion";
		case Adapter.RustIai:
			return "Iai";
		case Adapter.RustIaiCallgrind:
			return "Iai-Callgrind";
		case Adapter.Cpp:
			return "C++";
		case Adapter.CppGoogle:
			return "Google Benchmark";
		case Adapter.CppCatch2:
			return "Catch2";
		case Adapter.Go:
			return "Go";
		case Adapter.GoBench:
			return "go test -bench";
		case Adapter.Java:
			return "Java";
		case Adapter.JavaJmh:
			return "Java Microbenchmark Harness (JMH)";
		case Adapter.CSharp:
			return "C#";
		case Adapter.CSharpDotNet:
			return "BenchmarkDotNet";
		case Adapter.Js:
			return "JavaScript";
		case Adapter.JsBenchmark:
			return "Benchmark.js";
		case Adapter.JsTime:
			return "console.time/console.timeEnd";
		case Adapter.Python:
			return "Python";
		case Adapter.PythonAsv:
			return "airspeed velocity";
		case Adapter.PythonPytest:
			return "pytest-benchmark";
		case Adapter.Ruby:
			return "Ruby";
		case Adapter.RubyBenchmark:
			return "Benchmark";
		case Adapter.Shell:
			return "Shell";
		case Adapter.ShellHyperfine:
			return "Hyperfine";
	}
};

export const adapterCommand = (isConsole: boolean, adapter: null | Adapter) => {
	const host = isConsole ? "" : " --host https://localhost:61016";

	switch (adapter) {
		case Adapter.RustBench:
			return `bencher run${host} "cargo +nightly bench"`;
		case Adapter.RustCriterion:
		case Adapter.RustIai:
		case Adapter.RustIaiCallgrind:
			return `bencher run${host} "cargo bench"`;
		case Adapter.CppGoogle:
			return `bencher run${host} "make benchmarks --benchmark_format=json"`;
		case Adapter.CppCatch2:
			return `bencher run${host} "make benchmarks"`;
		case Adapter.GoBench:
			return `bencher run${host} "go test -bench"`;
		case Adapter.JavaJmh:
			return `bencher run${host} --file results.json "java -jar benchmarks.jar -rf json -rff results.json"`;
		case Adapter.CSharpDotNet:
			return `bencher run${host} "dotnet run -c Release"`;
		case Adapter.JsBenchmark:
		case Adapter.JsTime:
			return `bencher run${host} "node benchmark.js"`;
		case Adapter.PythonAsv:
			return `bencher run${host} "asv run"`;
		case Adapter.PythonPytest:
			return `bencher run${host} --file results.json "pytest --benchmark-json results.json benchmarks.py"`;
		case Adapter.RubyBenchmark:
			return `bencher run${host} "ruby benchmarks.rb"`;
		case Adapter.ShellHyperfine:
			return `bencher run${host} --file results.json "hyperfine --export-json results.json 'sleep 0.1'"`;
		// biome-ignore lint/complexity/noUselessSwitchCase: code as docs
		case Adapter.Json:
		default:
			return `bencher run${host} "bencher mock"`;
	}
};
