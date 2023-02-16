import { Link, useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { BENCHER_CALENDLY_URL, pageTitle, validate_jwt } from "../util";

const PricingPage = (props) => {
	const navigate = useNavigate();

	createEffect(() => {
		if (validate_jwt(props.user.token)) {
			navigate("/console/");
		}

		pageTitle("Pricing");
	});

	return (
		<div>
			<section class="hero">
				<div class="hero-body">
					<div class="container">
						<div class="columns is-mobile">
							<div class="column">
								<div class="content has-text-centered">
									<h1 class="title">Pricing</h1>
									<h3 class="subtitle">
										Start tracking your benchmarks for free
									</h3>
									<a
										href={BENCHER_CALENDLY_URL}
										target="_blank"
										rel="noreferrer"
									>
										🐰 Schedule a free demo
									</a>
								</div>
							</div>
						</div>
					</div>
				</div>
			</section>
			<hr />
			<section class="section">
				<div class="pricing-table is-comparative">
					<div class="pricing-plan">
						<div class="plan-header">
							<h2 class="title">Free</h2>
						</div>
						<div class="plan-price">
							<span class="plan-price-amount">$0</span>
						</div>
						<div class="content has-text-centered">
							<small>per metric/month</small>
						</div>
						<div class="plan-items">
							<div class="plan-item" data-feature="Public Projects">
								Public Projects
							</div>
							<div class="plan-item" data-feature="Team Roles">
								---
							</div>
							<div class="plan-item" data-feature="Private Projects">
								Community Support
							</div>
						</div>
						<div class="plan-footer">
							<div class="columns is-centered">
								<div class="column is-four-fifths">
									<button
										class="button is-fullwidth"
										onClick={(e) => {
											e.preventDefault();
											navigate("/auth/signup?plan=free");
										}}
									>
										Sign up for free
									</button>
								</div>
							</div>
						</div>
					</div>

					<div class="pricing-plan is-active">
						<div class="plan-header">
							<h2 class="title">Team</h2>
						</div>
						<div class="plan-price">
							<span class="plan-price-amount">$0.01</span>
						</div>
						<div class="content has-text-centered">
							<small>per metric/month</small>
						</div>
						<div class="plan-items">
							<div class="plan-item" data-feature="Public Projects">
								Public Projects
							</div>
							<div class="plan-item" data-feature="Private Projects">
								Private Projects
							</div>
							<div class="plan-item" data-feature="Team Roles">
								Customer Support
							</div>
						</div>
						<div class="plan-footer">
							<button
								class="button is-fullwidth"
								onClick={(e) => {
									e.preventDefault();
									navigate("/auth/signup?plan=team");
								}}
							>
								Continue with Team
							</button>
						</div>
					</div>

					<div class="pricing-plan">
						<div class="plan-header">
							<h2 class="title">Enterprise</h2>
						</div>
						<div class="plan-price">
							<span class="plan-price-amount">$0.05</span>
						</div>
						<div class="content has-text-centered">
							<small>per metric/month</small>
						</div>
						<div class="plan-items">
							<div class="plan-item" data-feature="Public Projects">
								Public Projects
							</div>
							<div class="plan-item" data-feature="Private Projects">
								Private Projects
							</div>
							<div class="plan-item" data-feature="Team Roles">
								Priority Support
							</div>
						</div>
						<div class="plan-footer">
							<div class="columns is-centered">
								<div class="column is-four-fifths">
									<button
										class="button is-fullwidth"
										onClick={(e) => {
											e.preventDefault();
											navigate("/auth/signup?plan=enterprise");
										}}
									>
										Continue with Enterprise
									</button>
								</div>
							</div>
						</div>
					</div>
				</div>
			</section>

			<br />
			<br />
			<br />
			<hr />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column">
							<h2 class="title">FAQ</h2>
						</div>
					</div>
					<div class="box">
						<div class="content">
							<h3 class="subtitle">What is a Metric?</h3>
							<p>A Metric is a single, point-in-time benchmark result.</p>
							<p>
								For example, if you have five benchmarks then they would create
								five Metrics each time they run. If you ran your benchmarks ten
								times, you would then have fifty Metrics. (ex: 5 benchmarks x 10
								runs = 50 Metrics)
							</p>
						</div>
					</div>
					<div class="box">
						<div class="content">
							<h3 class="subtitle">How are Metrics billed?</h3>
							<p>
								Bencher Cloud Metrics are billed monthly based on metered usage.
							</p>
							<p>
								For example, if you create 5,280 Metrics in a particular month
								then you would be billed for 5,280 Metrics that month.
							</p>
							<p>
								Bencher Self-Hosted Metrics are billed annually, grouped by the
								thousands.
							</p>
							<p>
								For example, if you create at most 5,280 Metrics in any given
								month then you would need to have a Self-Hosted limit of at
								least 6,000 Metrics/month.
							</p>
						</div>
					</div>
					<div class="box">
						<div class="content">
							<h3 class="subtitle">
								What happens if I reach my Bencher Self-Hosted limit for the
								month?
							</h3>
							<p>
								Once you reach your Self-Hosted limit, no new Metrics will be
								accepted.
							</p>
							<p>
								No need to panic though, you can always increase you limit. It
								is best to give yourself an extra margin when setting your
								limit.
							</p>
						</div>
					</div>
					<div class="box">
						<div class="content">
							<h3 class="subtitle">
								Do excess Bencher Self-Hosted Metrics rollover to the next
								Month?
							</h3>
							<p>They do not.</p>
						</div>
					</div>
				</div>
			</section>
		</div>
	);
};

export default PricingPage;
