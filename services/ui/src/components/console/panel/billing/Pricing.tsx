import { Show } from "solid-js";
import { PlanLevel } from "../../../../types/bencher";

export const per_metric_cost = (plan: PlanLevel) => {
	switch (plan) {
		case PlanLevel.Free:
			return 0;
		case PlanLevel.Team:
			return 1;
		case PlanLevel.Enterprise:
			return 5;
	}
};

const Pricing = (props: {
	active: string;
	free_text: string;
	handleFree: Function;
	team_text: string;
	handleTeam: Function;
	enterprise_text: string;
	handleEnterprise: Function;
}) => {
	return (
		<div class="pricing-table is-comparative">
			<div
				class={`pricing-plan ${props.active === PlanLevel.Free && "is-active"}`}
			>
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
					<div class="plan-item">Public Projects</div>
					<div class="plan-item" data-feature="Private Projects">
						⎯⎯⎯
					</div>
					<div class="plan-item">Community Support</div>
				</div>
				<Footer
					active={props.active}
					plan={PlanLevel.Free}
					button_text={props.free_text}
					handlePlanLevel={props.handleFree}
				/>
			</div>

			<div
				class={`pricing-plan ${props.active === PlanLevel.Team && "is-active"}`}
			>
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
					<div class="plan-item">Public Projects</div>
					<div class="plan-item">Private Projects</div>
					<div class="plan-item">Customer Support</div>
				</div>
				<Footer
					active={props.active}
					plan={PlanLevel.Team}
					button_text={props.team_text}
					handlePlanLevel={props.handleTeam}
				/>
			</div>

			<div
				class={`pricing-plan ${
					props.active === PlanLevel.Enterprise && "is-active"
				}`}
			>
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
					<div class="plan-item">Public Projects</div>
					<div class="plan-item">Private Projects</div>
					<div class="plan-item">Priority Support</div>
				</div>
				<Footer
					active={props.active}
					plan={PlanLevel.ENTERPRISE}
					button_text={props.enterprise_text}
					handlePlanLevel={props.handleEnterprise}
				/>
			</div>
		</div>
	);
};

const Footer = (props: {
	active: string;
	plan: string;
	button_text: string;
	handlePlanLevel: Function;
}) => {
	return (
		<div class="plan-footer">
			<Show
				when={props.plan === props.active}
				fallback={
					<div class="columns is-centered">
						<div class="column is-four-fifths">
							<button
								class="button is-fullwidth"
								onClick={props.handlePlanLevel}
							>
								{props.button_text}
							</button>
						</div>
					</div>
				}
			>
				<button class="button is-fullwidth" onClick={props.handlePlanLevel}>
					{props.button_text}
				</button>
			</Show>
		</div>
	);
};

export default Pricing;
