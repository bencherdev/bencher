import { Match, Show, Switch, createResource } from "solid-js";
import { Adapter } from "../../types/bencher";
import { useBash } from "../os/operating_system";
import { adapter } from "./util";

const BencherRunInner = (props) => {
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
			<Match when={adapter() === Adapter.RustDivan}>
				<Show when={bash()} fallback={props.rust_divan_powershell}>
					{props.rust_divan_bash}
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
			<Match when={adapter() === Adapter.RustGungraun}>
				<Show when={bash()} fallback={props.rust_gungraun_powershell}>
					{props.rust_gungraun_bash}
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
			<Match when={adapter() === Adapter.DartBenchmarkHarness}>
				<Show when={bash()} fallback={props.dart_benchmark_harness_powershell}>
					{props.dart_benchmark_harness_bash}
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
			<Match when={adapter() === Adapter.JsVitest}>
				<Show when={bash()} fallback={props.js_vitest_powershell}>
					{props.js_vitest_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.JsDeno}>
				<Show when={bash()} fallback={props.js_deno_powershell}>
					{props.js_deno_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.JsMitata}>
				<Show when={bash()} fallback={props.js_mitata_powershell}>
					{props.js_mitata_bash}
				</Show>
			</Match>
			<Match when={adapter() === Adapter.JsTinybench}>
				<Show when={bash()} fallback={props.js_tinybench_powershell}>
					{props.js_tinybench_bash}
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
			<Match when={adapter() === Adapter.RubyBenchmarkIps}>
				<Show when={bash()} fallback={props.ruby_benchmark_ips_powershell}>
					{props.ruby_benchmark_ips_bash}
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

export default BencherRunInner;
