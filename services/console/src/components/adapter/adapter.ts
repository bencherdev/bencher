import { createSignal } from "solid-js";
import { Adapter } from "../../types/bencher";

export const BENCHER_ADAPTER_KEY = "BENCHER_ADAPTER";

const JSON_ICON = "devicon-json-plain";
const C_SHARP_ICON = "devicon-csharp-line";
const CPP_ICON = "devicon-cplusplus-plain";
const GO_ICON = "devicon-go-original-wordmark";
const JAVA_ICON = "devicon-java-plain";
const JS_ICON = "devicon-javascript-plain";
const PYTHON_ICON = "devicon-python-plain";
const RUBY_ICON = "devicon-ruby-plain";
const RUST_ICON = "devicon-rust-plain";
const SHELL_ICON = "devicon-bash-plain";

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

export const adapterIcon = (adapter: Adapter) => {
	switch (adapter) {
		case Adapter.Json:
			return JSON_ICON;
		case Adapter.RustBench:
		case Adapter.RustCriterion:
		case Adapter.RustIai:
		case Adapter.RustIaiCallgrind:
			return RUST_ICON;
		case Adapter.CppGoogle:
		case Adapter.CppCatch2:
			return CPP_ICON;
		case Adapter.GoBench:
			return GO_ICON;
		case Adapter.JavaJmh:
			return JAVA_ICON;
		case Adapter.CSharpDotNet:
			return C_SHARP_ICON;
		case Adapter.JsBenchmark:
		case Adapter.JsTime:
			return JS_ICON;
		case Adapter.PythonAsv:
		case Adapter.PythonPytest:
			return PYTHON_ICON;
		case Adapter.RubyBenchmark:
			return RUBY_ICON;
		case Adapter.ShellHyperfine:
			return SHELL_ICON;
		default:
			console.log(`Unsupported adapter: ${adapter}`);
			return;
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
				removeAdapter();
		}
	}
	return null;
};

export const storeAdapter = (adapter: Adapter) =>
	localStorage.setItem(BENCHER_ADAPTER_KEY, adapter);

export const removeAdapter = () => localStorage.removeItem(BENCHER_ADAPTER_KEY);

const [adapter_inner, setAdapter] = createSignal<Adapter | null>(getAdapter());
setInterval(() => {
	const new_adapter = getAdapter();
	if (new_adapter !== adapter_inner()) {
		setAdapter(new_adapter);
	}
}, 100);

export const adapter = adapter_inner;
