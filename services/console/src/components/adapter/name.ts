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
