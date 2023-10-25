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

interface Props {
	plan: PlanLevel;
	freeText: string;
	handleFree: () => void;
	teamText: string;
	handleTeam: () => void;
	enterpriseText: string;
	handleEnterprise: () => void;
}

const Pricing = (props: Props) => {
	return (
		<div class="pricing-table is-comparative">
			<div
				class={`pricing-plan ${props.plan === PlanLevel.Free && "is-active"}`}
			>
				<div class="plan-header">
					<h2 class="title">Free</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0</span>
				</div>
				<div class="content has-text-centered">
					<small>/metric</small>
				</div>
				<div class="plan-items">
					<div class="plan-item">Public Projects</div>
					<div class="plan-item" data-feature="Private Projects">
						⎯⎯⎯
					</div>
					<div class="plan-item">Community Support</div>
				</div>
				<Footer
					active={props.plan}
					plan={PlanLevel.Free}
					text={props.freeText}
					handlePlanLevel={props.handleFree}
				/>
			</div>

			<div
				class={`pricing-plan ${props.plan === PlanLevel.Team && "is-active"}`}
			>
				<div class="plan-header">
					<h2 class="title">Team</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0.01</span>
				</div>
				<div class="content has-text-centered">
					<small>/metric</small>
				</div>
				<div class="plan-items">
					<div class="plan-item">Public Projects</div>
					<div class="plan-item">Private Projects</div>
					<div class="plan-item">Customer Support</div>
				</div>
				<Footer
					active={props.plan}
					plan={PlanLevel.Team}
					text={props.teamText}
					handlePlanLevel={props.handleTeam}
				/>
			</div>

			<div
				class={`pricing-plan ${
					props.plan === PlanLevel.Enterprise && "is-active"
				}`}
			>
				<div class="plan-header">
					<h2 class="title">Enterprise</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0.05</span>
				</div>
				<div class="content has-text-centered">
					<small>/metric</small>
				</div>
				<div class="plan-items">
					<div class="plan-item">Public Projects</div>
					<div class="plan-item">Private Projects</div>
					<div class="plan-item">Priority Support</div>
				</div>
				<Footer
					active={props.plan}
					plan={PlanLevel.Enterprise}
					text={props.enterpriseText}
					handlePlanLevel={props.handleEnterprise}
				/>
			</div>
		</div>
	);
};

const Footer = (props: {
	active: string;
	plan: string;
	text: string;
	handlePlanLevel: () => void;
}) => {
	return (
		<div class="plan-footer">
			<Show
				when={props.active === props.plan}
				fallback={
					<div class="columns is-centered">
						<div class="column is-11">
							<button
								class="button is-fullwidth"
								onClick={(e) => {
									e.preventDefault();
									props.handlePlanLevel();
								}}
							>
								{props.text}
							</button>
						</div>
					</div>
				}
			>
				<button
					class="button is-fullwidth"
					onClick={(e) => {
						e.preventDefault();
						props.handlePlanLevel();
					}}
				>
					{props.text}
				</button>
			</Show>
		</div>
	);
};

export default Pricing;
