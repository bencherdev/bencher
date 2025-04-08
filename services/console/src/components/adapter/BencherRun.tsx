import { adapter } from "./util";
import { createResource, Match, Show, Switch } from "solid-js";
import { Adapter } from "../../types/bencher";
import { useBash } from "../os/operating_system";

const BencherRun = (props) => {
	const [bash] = createResource(useBash);

	return (
		<Switch
			fallback={
				<Show when={bash()} fallback={props.json_powershell}>
					{props.json_bash}
				</Show>
			}
		>
			<Match when={adapter() === Adapter.RustBench}>
				<Show when={bash()} fallback={props.rust_bench_powershell}>
					{props.rust_bench_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.RustCriterion}>
				<Show when={bash()} fallback={props.rust_criterion_powershell}>
					{props.rust_criterion_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.RustIai}>
				<Show when={bash()} fallback={props.rust_iai_powershell}>
					{props.rust_iai_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.RustIaiCallgrind}>
				<Show when={bash()} fallback={props.rust_iai_callgrind_powershell}>
					{props.rust_iai_callgrind_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.CppGoogle}>
				<Show when={bash()} fallback={props.cpp_google_powershell}>
					{props.cpp_google_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.CppCatch2}>
				<Show when={bash()} fallback={props.cpp_catch2_powershell}>
					{props.cpp_catch2_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.GoBench}>
				<Show when={bash()} fallback={props.go_bench_powershell}>
					{props.go_bench_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.JavaJmh}>
				<Show when={bash()} fallback={props.java_jmh_powershell}>
					{props.java_jmh_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.CSharpDotNet}>
				<Show when={bash()} fallback={props.csharp_dotnet_powershell}>
					{props.csharp_dotnet_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.JsBenchmark}>
				<Show when={bash()} fallback={props.js_benchmark_powershell}>
					{props.js_benchmark_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.JsTime}>
				<Show when={bash()} fallback={props.js_time_powershell}>
					{props.js_time_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.PythonAsv}>
				<Show when={bash()} fallback={props.python_asv_powershell}>
					{props.python_asv_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.PythonPytest}>
				<Show when={bash()} fallback={props.python_pytest_powershell}>
					{props.python_pytest_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.RubyBenchmark}>
				<Show when={bash()} fallback={props.ruby_benchmark_powershell}>
					{props.ruby_benchmark_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.ShellHyperfine}>
				<Show when={bash()} fallback={props.shell_hyperfine_powershell}>
					{props.shell_hyperfine_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.Json}>
				<Show when={bash()} fallback={props.json_powershell}>
					{props.json_bash}
				</Show>
			</Match>
		</Switch>
	);
};

export default BencherRun;
