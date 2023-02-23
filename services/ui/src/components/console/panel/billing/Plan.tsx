import axios from "axios";
import { createMemo, createResource } from "solid-js";
import {
	BENCHER_API_URL,
	get_options,
	usd_formatter,
	validate_jwt,
} from "../../../site/util";
import { PlanLevel } from "./Pricing";

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

const date_time_millis = (date_str: string) => {
	const date_ms = Date.parse(date_str);
	if (date_ms) {
		const date = new Date(date_ms);
		if (date) {
			return date.getTime();
		}
	}

	return null;
};

const Plan = (props) => {
	console.log(props.plan());

	const plan_level = createMemo(() => {
		switch (props.plan()?.level) {
			case PlanLevel.TEAM: {
				return "Team";
			}
			case PlanLevel.ENTERPRISE: {
				return "Enterprise";
			}
			default:
				return "---";
		}
	});

	const fetchUsage = async (organization_slug: string) => {
		const EMPTY_OBJECT = {};
		try {
			const token = props.user?.token;
			if (!validate_jwt(props.user?.token)) {
				return EMPTY_OBJECT;
			}
			const start = date_time_millis(props.plan()?.current_period_start);
			const end = date_time_millis(props.plan()?.current_period_end);
			const url = `${BENCHER_API_URL()}/v0/organizations/${organization_slug}/usage?start=${start}&end=${end}`;
			const resp = await axios(get_options(url, token));
			props.handleRefresh();
			return resp?.data;
		} catch (error) {
			console.error(error);
			return EMPTY_OBJECT;
		}
	};

	const [usage] = createResource(props.organization_slug, fetchUsage);

	const per_metric_rate = createMemo(() => props.plan()?.unit_amount / 100);
	const estimated_cost = createMemo(() => {
		const metrics_used = usage()?.metrics_used;
		if (!Number.isInteger(metrics_used)) {
			return null;
		}

		return metrics_used * per_metric_rate();
	});

	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<h4 class="title">Current Billing Period</h4>
						<h4 class="subtitle">
							{format_date_time(props.plan()?.current_period_start)} -{" "}
							{format_date_time(props.plan()?.current_period_end)}
						</h4>
						<p>Plan Level: {plan_level()}</p>
						<p>Per Metric Rate: {usd_formatter.format(per_metric_rate())}</p>
						<p>
							Estimated Usage:{" "}
							{Number.isInteger(usage()?.metrics_used)
								? usage()?.metrics_used
								: "---"}
						</p>
						<p>
							Current Estimated Cost:{" "}
							{estimated_cost() === null
								? "---"
								: usd_formatter.format(estimated_cost())}
						</p>
					</div>
				</div>
			</div>
		</section>
	);
};

export default Plan;
