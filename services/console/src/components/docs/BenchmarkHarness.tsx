import LinkFragment from "../site/LinkFragment.tsx";

const LANGUAGES: string[][] = [
	["devicon-cplusplus-line", "devicon-python-plain"],
	["devicon-java-plain", "devicon-javascript-plain"],
	["devicon-ruby-plain", "devicon-csharp-line"],
	["devicon-go-original-wordmark", "devicon-rust-plain"],
];

const BenchmarkHarness = () => (
	<div class="content">
		<h2 class="title is-3">
			Select your Benchmark Harness
			<LinkFragment fragment="select-your-benchmark-harness" />
		</h2>
		<br />
		<div class="columns is-centered is-vcentered">
			{LANGUAGES.map(([left_icon, right_icon]) => (
				<div class="column is-one-quarter">
					<div class="columns is-mobile">
						<div class="column is-half">
							<span class="icon has-text-primary is-large">
								<i class={`${left_icon} fa-5x`} />
							</span>
						</div>
						<br />
						<div class="column is-half">
							<span class="icon has-text-primary is-large">
								<i class={`${right_icon} fa-5x`} />
							</span>
						</div>
					</div>
				</div>
			))}
		</div>
	</div>
);

export default BenchmarkHarness;
