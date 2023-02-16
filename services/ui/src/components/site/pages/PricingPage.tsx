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
							<br />
							<small>billed never</small>
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
											navigate("/auth/signup");
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
							<br />
							<small>billed annually</small>
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
									navigate("/auth/signup");
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
							<br />
							<small>billed annually</small>
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
											navigate("/auth/signup");
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
		</div>
	);
};

export default PricingPage;
