import LinkFragment from "../site/LinkFragment.tsx";

const BenchmarkHarness = () => (
	<div class="content">
		<h2 class="title is-3">
			Select your Benchmark Harness
			<LinkFragment fragment="select-your-benchmark-harness" />
		</h2>
		<br />
		<div class="columns">
			<div class="column is-half">
				<LanguageBox icon="devicon-csharp-line" name="C#" />
				<LanguageBox icon="devicon-cplusplus-plain" name="C++" />
				<LanguageBox icon="devicon-go-original-wordmark" name="Go" />
				<LanguageBox icon="devicon-java-plain" name="Java" />
				<LanguageBox icon="devicon-javascript-plain" name="JavaScript" />
			</div>
			<div class="column is-half">
				<LanguageBox icon="devicon-python-plain" name="Python" />
				<LanguageBox icon="devicon-ruby-plain" name="Ruby" />
				<LanguageBox icon="devicon-rust-plain" name="Rust" />
				<LanguageBox icon="devicon-bash-plain" name="Shell" />
				<LanguageBox icon="devicon-json-plain" name="JSON" />
			</div>
		</div>
	</div>
);

const LanguageBox = (props: { icon: string; name: string }) => (
	<div class="box columns is-mobile is-vcentered is-gapless">
		<div class="column is-narrow">
			<span class="icon has-text-primary is-large">
				<i class={`${props.icon} fa-2x`} />
			</span>
		</div>
		<div class="column is-narrow">
			<div>{props.name}</div>
		</div>
		<div class="column is-fullwidth">
			<div class="content has-text-right">
				<span class="icon">
					<i class="fas fa-caret-right" />
				</span>
			</div>
		</div>
	</div>
);

export default BenchmarkHarness;
