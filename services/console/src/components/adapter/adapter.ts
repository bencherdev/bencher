import { Adapter } from "../../types/bencher";

export const BENCHER_ADAPTER_KEY = "BENCHER_ADAPTER";

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

export const getAdapter = () => {
	if (typeof localStorage !== "undefined") {
		const adapter = localStorage.getItem(BENCHER_ADAPTER_KEY);
		switch (adapter) {
			case Adapter.Json:
			case Adapter.RustBench:
			case Adapter.RustCriterion:
			case Adapter.RustIai:
			case Adapter.RustIaiCallgrind:
			case Adapter.CppGoogle:
			case Adapter.CppCatch2:
			case Adapter.GoBench:
			case Adapter.JavaJmh:
			case Adapter.CSharpDotNet:
			case Adapter.JsBenchmark:
			case Adapter.JsTime:
			case Adapter.PythonAsv:
			case Adapter.PythonPytest:
			case Adapter.RubyBenchmark:
			case Adapter.ShellHyperfine:
				return adapter;
			case null:
				return null;
			default:
				localStorage.removeItem(BENCHER_ADAPTER_KEY);
		}
	}
	return null;
};

export const storeAdapter = (adapter: Adapter) =>
	window.localStorage.setItem(BENCHER_ADAPTER_KEY, adapter);
