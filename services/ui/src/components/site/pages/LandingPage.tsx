import { Link, useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { BENCHER_TITLE, pageTitle, validate_jwt } from "../util";

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
						<div class="column is-half">
							<button
								class="button is-primary is-large is-responsive is-fullwidth"
								onClick={(e) => {
									e.preventDefault();
									navigate("/auth/signup");
								}}
							>
								Start Now
							</button>
						</div>
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
											tools. The <code>bencher</code> CLI simply wraps your
											existing benchmarking framework and stores its results.
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
									<Link href="/docs/how-to/quick-start">
										<button class="button is-fullwidth">Learn More</button>
									</Link>
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
										<span class="icon has-text-primary is-disabled">
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
									<Link href="/docs/how-to/quick-start">
										<button class="button is-fullwidth">Learn More</button>
									</Link>
								</div>
							</div>
						</div>
					</div>
				</div>
			</section>
		</section>
	);
};

export default LandingPage;
