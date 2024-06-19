import { For, Show } from "solid-js";
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
	themeColor: string;
	plan?: PlanLevel;
	freeText: string;
	handleFree: () => void;
	hideFree?: undefined | boolean;
	teamText: string;
	handleTeam: () => void;
	enterpriseText: string;
	handleEnterprise: () => void;
}

const Pricing = (props: Props) => {
	const Free = (
		<PlanPanel
			themeColor={props.themeColor}
			active={props.plan}
			plan={PlanLevel.Free}
			buttonText={props.freeText}
			handlePlanLevel={props.handleFree}
		/>
	);
	const Team = (
		<PlanPanel
			themeColor={props.themeColor}
			active={props.plan}
			plan={PlanLevel.Team}
			buttonText={props.teamText}
			handlePlanLevel={props.handleTeam}
		/>
	);
	const Enterprise = (
		<PlanPanel
			themeColor={props.themeColor}
			active={props.plan}
			plan={PlanLevel.Enterprise}
			buttonText={props.enterpriseText}
			handlePlanLevel={props.handleEnterprise}
		/>
	);
	return (
		<div class="columns is-centered">
			<Show
				when={props.hideFree}
				fallback={
					<>
						<div class="column is-3">{Free}</div>
						<div class="column is-3">{Team}</div>
						<div class="column is-3">{Enterprise}</div>
					</>
				}
			>
				<div class="column is-6">{Team}</div>
				<div class="column is-6">{Enterprise}</div>
			</Show>
		</div>
	);
};

const NO_FEATURE = "\xa0";

const PlanCopy = {
	[PlanLevel.Free]: {
		title: "Free",
		price: "$0",
		features: [
			"Public Projects",
			NO_FEATURE,
			"Community Support",
			NO_FEATURE,
			NO_FEATURE,
		],
	},
	[PlanLevel.Team]: {
		title: "Team",
		price: "$0.01",
		features: [
			"Public Projects",
			"Private Projects",
			"Customer Support",
			NO_FEATURE,
			NO_FEATURE,
		],
	},
	[PlanLevel.Enterprise]: {
		title: "Enterprise",
		price: "$0.05",
		features: [
			"Public Projects",
			"Private Projects",
			"Priority Support",
			"Single Sign-On (SSO)",
			"Dedicated Onboarding",
		],
	},
};

const PlanPanel = (props: {
	themeColor: string;
	active: undefined | PlanLevel;
	plan: PlanLevel;
	buttonText: string;
	handlePlanLevel: () => void;
}) => {
	const plan = PlanCopy[props.plan];
	return (
		<div class={`panel ${props.themeColor}`}>
			<div class="panel-heading">
				<div class="content has-text-centered">
					<h2 class="title is-2">{plan.title}</h2>
					<h3 class="subtitle is-1">{plan.price}</h3>
					<br />
					<sup>per benchmark result</sup>
				</div>
			</div>
			<For each={plan.features}>
				{(feature) => (
					<Show
						when={feature === NO_FEATURE}
						fallback={
							<div class="panel-block">
								<span class="panel-icon">
									<i class="fas fa-check" />
								</span>
								{feature}
							</div>
						}
					>
						<div class="panel-block">{feature}</div>
					</Show>
				)}
			</For>
			<div class="panel-block">
				<button
					class={`button is-fullwidth ${
						props.plan === props.active && "is-primary"
					}`}
					type="button"
					onMouseDown={(e) => {
						e.preventDefault();
						props.handlePlanLevel();
					}}
				>
					{props.buttonText}
				</button>
			</div>
		</div>
	);
};

export default Pricing;
