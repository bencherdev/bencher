import { Link, useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import Pricing, { PlanLevel } from "../../console/panel/billing/Pricing";
import { BENCHER_CALENDLY_URL, pageTitle, validate_jwt } from "../util";

const PricingPage = (props) => {
	const navigate = useNavigate();

	createEffect(() => {
		if (validate_jwt(props.user.token)) {
			navigate("/console?plan=free");
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
				<Pricing
					active={PlanLevel.TEAM}
					free_text="Sign up for free"
					handleFree={(e) => {
						e.preventDefault();
						navigate(plan_signup(PlanLevel.FREE));
					}}
					team_text="Continue with Team"
					handleTeam={(e) => {
						e.preventDefault();
						navigate(plan_signup(PlanLevel.TEAM));
					}}
					enterprise_text="Continue with Enterprise"
					handleEnterprise={(e) => {
						e.preventDefault();
						navigate(plan_signup(PlanLevel.ENTERPRISE));
					}}
				/>
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
							<h3 class="subtitle">What is a Public Project?</h3>
							<p>
								A Public Project is visible to anyone who has access to your
								Bencher instance.
							</p>
							<p>
								On Bencher Cloud,{" "}
								<a href="/perf" target="_blank" rel="noreferrer">
									all Public Projects are available here
								</a>
								. If you are using Bencher Self-Hosted, then anyone with access
								to your virtual private cloud (VPC) will likewise be able to see
								all Public Projects.
							</p>
						</div>
					</div>
					<div class="box">
						<div class="content">
							<h3 class="subtitle">What is a Private Project?</h3>
							<p>
								A Private Project is only visible to members of your
								organization.
							</p>
							<p>
								In order to create and use Private Projects, your organization
								must have an active Bencher Plus plan.
							</p>
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
								For example, if you create 5,280 Metrics in a particular
								calendar month then you would be billed for 5,280 Metrics that
								month.
							</p>
							{/* <p>
								Bencher Self-Hosted Metrics are billed annually, grouped by the
								thousand.
							</p>
							<p>
								For example, if you create at most 5,280 Metrics within any
								given 30 day period (rolling window) then you would need to have
								a Self-Hosted limit of at least 6,000 Metrics/month.
							</p> */}
						</div>
					</div>
					<div class="box">
						<div class="content">
							<h3 class="subtitle">Are Metrics for Public Projects counted?</h3>
							<p>
								Yes, if you have a valid Bencher Plus plan then Metrics from
								both your Public Projects and Private Projects are counted.
							</p>
						</div>
					</div>
					{/* <div class="box">
						<div class="content">
							<h3 class="subtitle">
								What happens if I reach my Bencher Self-Hosted limit for a 30
								day period?
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
								Do excess Bencher Self-Hosted Metrics rollover to the next 30
								day period?
							</h3>
							<p>
								Bencher Self-Hosted Metrics are calculated based on a 30 day
								rolling window. Therefore, there's no rollover.
							</p>
						</div>
					</div> */}
				</div>
			</section>
		</div>
	);
};

const plan_signup = (plan: PlanLevel) => {
	return `/auth/signup?plan=${plan}`;
};

export default PricingPage;
