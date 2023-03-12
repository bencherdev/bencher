import { Link, useNavigate } from "solid-app-router";
import { createEffect, For } from "solid-js";
import {
	BENCHER_CALENDLY_URL,
	BENCHER_TITLE,
	pageTitle,
	validate_jwt,
} from "../util";

const LanguageIcon = (props: { icon: string }) => {
	return (
		<div class="column is-1">
			<span class="icon has-text-primary is-large">
				<i class={`${props.icon} fa-5x`} aria-hidden="true" />
			</span>
		</div>
	);
};

const LandingPage = (props) => {
	const navigate = useNavigate();

	createEffect(() => {
		if (validate_jwt(props.user.token)) {
			navigate("/console");
		}

		pageTitle(BENCHER_TITLE);
	});

	return (
		<section class="section is-medium">
			<div class="container">
				<div class="content has-text-centered">
					<h1 class="title">Catch Performance Regressions in CI</h1>
					<h4 class="subtitle">
						Detect and prevent performance regressions before they make it to
						production
					</h4>
					<div class="columns is-centered">
						<div class="column is-one-fourth" />

						<div class="column is-one-fourth">
							<button
								class="button is-primary is-large is-responsive is-fullwidth"
								onClick={(e) => {
									e.preventDefault();
									navigate("/pricing");
								}}
							>
								Start Now
							</button>
						</div>

						<div class="column is-one-fourth">
							<a
								class="button is-inverted is-large is-responsive is-fullwidth"
								href={BENCHER_CALENDLY_URL}
								target="_blank"
								rel="noreferrer"
							>
								Talk to an engineer
							</a>
						</div>

						<div class="column is-one-fourth" />
					</div>
				</div>
			</div>

			<br />
			<br />
			<br />
			<hr />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column">
							<h2 class="title">How It Works</h2>
						</div>
					</div>
					<br />
					<br />
					<div class="columns is-centered">
						<div class="column">
							<div class="columns">
								<div class="column">
									<div class="content has-text-centered">
										<span class="icon has-text-primary">
											<i
												class="fas fa-tachometer-alt fa-5x"
												aria-hidden="true"
											/>
										</span>
										<h5 class="title">Run your benchmarks</h5>
									</div>
									<div class="content">
										<p>
											Run your benchmarks locally or in CI using your favorite
											benchmarking tools. The <code>bencher</code> CLI simply
											wraps your existing benchmark harness and stores its
											results.
										</p>
										<br />
									</div>
								</div>
							</div>
						</div>
						<br />
						<div class="column">
							<div class="columns">
								<div class="column">
									<div class="content has-text-centered">
										<span class="icon has-text-primary">
											<i class="fas fa-chart-line fa-5x" aria-hidden="true" />
										</span>
										<h5 class="title">Track your benchmarks</h5>
									</div>
									<div class="content">
										<p>
											Track the results of your benchmarks over time. Monitor,
											query, and graph the results using the Bencher web console
											based on the source branch and testbed.
										</p>
									</div>
									<br />
								</div>
							</div>
						</div>
						<br />
						<div class="column">
							<div class="columns">
								<div class="column">
									<div class="content has-text-centered">
										<span class="icon has-text-primary">
											<i class="fas fa-bell fa-5x" aria-hidden="true" />
										</span>
										<h5 class="title">Catch performance regressions</h5>
									</div>
									<div class="content">
										<p>
											Catch performance regressions in CI. Bencher uses state of
											the art, customizable analytics to detect performance
											regressions before they make it to production.
										</p>
									</div>
								</div>
							</div>
						</div>
					</div>
				</div>
			</section>

			<hr />
			<br />
			<div class="box">
				<div class="content has-text-centered">
					<h2>Use Your Favorite Tools</h2>
					<br />
					<div class="columns is-centered is-vcentered is-multiline">
						<For
							each={[
								"devicon-cplusplus-line",
								"devicon-python-plain",
								"devicon-java-plain",
								"devicon-javascript-plain",
								"devicon-ruby-plain",
								"devicon-csharp-line",
								"devicon-go-original-wordmark",
								"devicon-rust-plain",
							]}
						>
							{(icon) => <LanguageIcon icon={icon} />}
						</For>
					</div>
				</div>
			</div>
			<br />
			<hr />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column">
							<h2 class="title">Hosting</h2>
						</div>
					</div>
					<br />
					<br />
					<div class="columns is-centered">
						<div class="column">
							<div class="columns">
								<div class="column">
									<div class="content has-text-centered">
										<h2 class="subtitle">Self-Hosted</h2>
										<br />
										<br />
										<br />
										<span class="icon has-text-primary">
											<i class="fas fa-server fa-10x" aria-hidden="true" />
										</span>
									</div>
									<div class="content">
										<p>
											Run Bencher on-prem or in your own cloud. Bencher can be
											deployed on a standalone server, in a Docker container, or
											as part of a Kubernetes cluster.
										</p>
										<br />
									</div>
									<button
										class="button is-fullwidth"
										onClick={(e) => {
											e.preventDefault();
											navigate("/docs/how-to/quick-start");
										}}
									>
										Learn More
									</button>
								</div>
							</div>
						</div>

						<div class="is-divider-vertical" data-content="OR" />

						<div class="column">
							<div class="columns">
								<div class="column">
									<div class="content has-text-centered">
										<h2 class="subtitle">Bencher Cloud</h2>
										<br />
										<br />
										<br />
										<span class="icon has-text-primary">
											<i class="fas fa-cloud fa-10x" aria-hidden="true" />
										</span>
									</div>
									<div class="content">
										<p>
											It's {new Date().getFullYear()}, who wants to manage yet
											another service‽ Let us take care of that for you. All of
											the same great features with none of the hassle.
										</p>
										<br />
									</div>
									<button
										class="button is-fullwidth"
										onClick={(e) => {
											e.preventDefault();
											navigate("/pricing");
										}}
									>
										Get Started
									</button>
								</div>
							</div>
						</div>
					</div>
				</div>
			</section>
			<br />
			<hr />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column">
							<h2 class="title">Share Your Benchmarks</h2>
						</div>
					</div>
					<div class="columns is-centered">
						<div class="column">
							<div class="content">
								<p>
									All public projects have their own{" "}
									<Link href="/perf">perf page</Link>. These results can easily
									be shared with an auto-updating perf image. Perfect for your
									README!
								</p>
							</div>
						</div>
					</div>
					<div class="columns is-centered">
						<div class="column">
							<div class="content has-text-centered">
								<Link href="https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&start_time=1674950400000">
									<img
										style="border: 0.2em solid #ed6704;"
										src="https://api.bencher.dev/v0/projects/bencher/perf/img?branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&metric_kind=latency&start_time=1674950400000&title=Benchmark+Adapter+Comparison"
										title="Benchmark Adapter Comparison"
										alt="Benchmark Adapter Comparison for Bencher - Bencher"
									/>
								</Link>
							</div>
						</div>
					</div>
				</div>
			</section>
		</section>
	);
};

export default LandingPage;
