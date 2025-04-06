import { createSignal, For, Show } from "solid-js";
import { Adapter } from "../../types/bencher.ts";
import { adapterName } from "./name.ts";

const BenchmarkHarnessFallback = () => (
	<div class="columns">
		<div class="column is-half">
			<LanguageBox
				icon="devicon-csharp-line"
				name="C#"
				adapters={[Adapter.CSharpDotNet]}
			/>
			<LanguageBox
				icon="devicon-cplusplus-plain"
				name="C++"
				adapters={[Adapter.CppGoogle, Adapter.CppCatch2]}
			/>
			<LanguageBox
				icon="devicon-go-original-wordmark"
				name="Go"
				adapters={[Adapter.GoBench]}
			/>
			<LanguageBox
				icon="devicon-java-plain"
				name="Java"
				adapters={[Adapter.JavaJmh]}
			/>
			<LanguageBox
				icon="devicon-javascript-plain"
				name="JavaScript"
				adapters={[Adapter.JsBenchmark, Adapter.JsTime]}
			/>
		</div>
		<div class="column is-half">
			<LanguageBox
				icon="devicon-python-plain"
				name="Python"
				adapters={[Adapter.PythonAsv, Adapter.PythonPytest]}
			/>
			<LanguageBox
				icon="devicon-ruby-plain"
				name="Ruby"
				adapters={[Adapter.RubyBenchmark]}
			/>
			<LanguageBox
				icon="devicon-rust-plain"
				name="Rust"
				adapters={[
					Adapter.RustBench,
					Adapter.RustCriterion,
					Adapter.RustIai,
					Adapter.RustIaiCallgrind,
				]}
			/>
			<LanguageBox
				icon="devicon-bash-plain"
				name="Shell"
				adapters={[Adapter.ShellHyperfine]}
			/>
			<LanguageBox
				icon="devicon-json-plain"
				name="JSON"
				adapters={[Adapter.Json]}
			/>
		</div>
	</div>
);

const LanguageBox = (props: {
	icon: string;
	name: string;
	adapters: Adapter[];
}) => {
	const [active, setActive] = createSignal(false);

	return (
		<div class="card">
			<header
				class="card-header"
				style={{ cursor: "pointer" }}
				onMouseDown={(e) => {
					e.preventDefault();
					setActive(!active());
				}}
			>
				<div class="card-header-title">
					<div class="columns is-mobile is-vcentered is-gapless">
						<div class="column is-narrow">
							<span class="icon has-text-primary is-large">
								<i class={`${props.icon} fa-2x`} />
							</span>
						</div>
						<div class="column is-narrow">
							<div>{props.name}</div>
						</div>
					</div>
				</div>
				<button class="card-header-icon" type="button">
					<span class="icon">
						<Show when={active()} fallback={<i class="fas fa-angle-right" />}>
							<i class="fas fa-angle-down" />
						</Show>
					</span>
				</button>
			</header>
			<Show when={active()}>
				<div class="card-content">
					<div class="content">
						<ul>
							<For
								each={props.adapters}
								fallback={<li>No adapters available</li>}
							>
								{(adapter) => (
									<li style={{ cursor: "pointer" }}>{adapterName(adapter)}</li>
								)}
							</For>
						</ul>
					</div>
				</div>
			</Show>
		</div>
	);
};

export default BenchmarkHarnessFallback;
