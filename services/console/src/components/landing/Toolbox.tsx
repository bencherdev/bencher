import { For } from "solid-js";

const Toolbox = (_props: {}) => {
	const LANGUAGES: string[][] = [
		["devicon-cplusplus-line", "devicon-python-plain"],
		["devicon-java-plain", "devicon-javascript-plain"],
		["devicon-ruby-plain", "devicon-csharp-line"],
		["devicon-go-original-wordmark", "devicon-rust-plain"],
	];

	return (
		<section class="section" style="margin-top: 8rem;">
			<div class="content has-text-centered">
				<h2 class="title is-2">Use Your Favorite Benchmark Harness</h2>
				<br />
				<br />
				<div class="columns is-centered is-vcentered">
					<For each={LANGUAGES}>
						{([left_icon, right_icon]) => (
							<div class="column is-one-quarter">
								<div class="columns is-mobile">
									<LanguageIcon icon={left_icon} />
									<br />
									<LanguageIcon icon={right_icon} />
								</div>
							</div>
						)}
					</For>
				</div>
			</div>
		</section>
	);
};

const LanguageIcon = (props: { icon: undefined | string }) => {
	return (
		<div class="column is-half">
			<span class="icon has-text-primary is-large">
				<i class={`${props.icon} fa-5x`} aria-hidden="true" />
			</span>
		</div>
	);
};

export default Toolbox;
