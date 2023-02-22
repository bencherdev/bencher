import Pricing, { PlanLevel } from "./Pricing";

const Plan = (props) => {
	console.log(props.plan());

	const format_date_time = (date_str: string) => {
		const date_ms = Date.parse(date_str);
		if (date_ms) {
			const date = new Date(date_ms);
			if (date) {
				return date.toDateString();
			}
		}

		return null;
	};
	return (
		<div class="columns">
			<div class="column">
				<PlanLevelButtons level={props.plan()?.level} />
				<h4 class="title">Current Billing Period</h4>
				<h4 class="subtitle">
					{format_date_time(props.plan()?.current_period_start)} -{" "}
					{format_date_time(props.plan()?.current_period_end)}
				</h4>
				<p>Per Metric Rate: ${props.plan()?.unit_amount / 100}</p>
				<p>Estimated Usage: x</p>
				<p>Current Estimated Cost: y</p>
			</div>
		</div>
	);
};

export default Plan;

const PlanLevelButtons = (props: { level: PlanLevel }) => {
	return (
		<div class="buttons has-addons is-centered">
			<button class="button" disabled={true}>
				<span
					class={`icon is-small ${
						props.level === PlanLevel.TEAM && "has-text-primary"
					}`}
				>
					<i class="fas fa-users" aria-hidden="true" />
				</span>
				<span>Team</span>
			</button>
			<button class="button" disabled={true}>
				<span
					class={`icon is-small ${
						props.level === PlanLevel.ENTERPRISE && "has-text-primary"
					}`}
				>
					<i class="far fa-building" aria-hidden="true" />
				</span>
				<span>Enterprise</span>
			</button>
		</div>
	);
};
