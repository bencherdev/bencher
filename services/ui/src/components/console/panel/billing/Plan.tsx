import axios from "axios";
import { createMemo, createResource, For } from "solid-js";
import {
	BENCHER_API_URL,
	get_options,
	usd_formatter,
	validate_jwt,
} from "../../../site/util";
import { PlanLevel } from "../../../../types/bencher";

export enum PlanStatus {
	ACTIVE = "active",
	CANCELED = "canceled",
	INCOMPLETE = "incomplete",
	INCOMPLETE_EXPIRED = "incomplete_expired",
	PAST_DUE = "past_due",
	PAUSED = "paused",
	TRIALING = "trialing",
	UNPAID = "unpaid",
}

export enum CardBrand {
	AMEX = "amex",
	DINERS = "diners",
	DISCOVER = "discover",
	JCB = "jcb",
	MASTERCARD = "mastercard",
	UNIONPAY = "unionpay",
	VISA = "visa",
	UNKNOWN = "unknown",
}

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

const plan_level = (level: PlanLevel) => {
	switch (level) {
		case PlanLevel.Team: {
			return "Team";
		}
		case PlanLevel.Enterprise: {
			return "Enterprise";
		}
		default:
			return "---";
	}
};

const Plan = (props) => {
	const fetchUsage = async (organization_slug: string) => {
		const EMPTY_OBJECT = {};
		const token = props.user?.token;
		if (!validate_jwt(props.user?.token)) {
			return EMPTY_OBJECT;
		}
		const start = date_time_millis(props.plan()?.current_period_start);
		const end = date_time_millis(props.plan()?.current_period_end);
		const url = `${BENCHER_API_URL()}/v0/organizations/${organization_slug}/usage?start=${start}&end=${end}`;
		return await axios(get_options(url, token))
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_OBJECT;
			});
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

	const customer = createMemo(() => props.plan()?.customer);
	const card = createMemo(() => props.plan()?.card);

	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<h4 class="title">Status</h4>
						<FmtPlanStatus status={props.plan()?.status} />
						<br />

						<h4 class="title">Current Billing Period</h4>
						<h4 class="subtitle">
							{format_date_time(props.plan()?.current_period_start)} -{" "}
							{format_date_time(props.plan()?.current_period_end)}
						</h4>
						<p>Plan Level: {plan_level(props.plan().level)}</p>
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
						{/* TODO if plan_level === PlanLevel.Team then Upgrade Plan button */}
						<br />

						<h4 class="title">Payment Method</h4>
						<FmtCardBrand brand={card()?.brand} />
						<p>Name: {customer()?.name}</p>
						<p>Last Four: {card()?.last_four}</p>
						<p>
							Expiration: {card()?.exp_month}/{card()?.exp_year}
						</p>
						<br />

						<For each={[...Array(5).keys()]}>{(_k, _i) => <br />}</For>
						<p>
							To update or cancel your subscription please email{" "}
							<a href="mailto:everett@bencher.dev">everett@bencher.dev</a>
						</p>
					</div>
				</div>
			</div>
		</section>
	);
};

const FmtPlanStatus = (props) => {
	switch (props.status) {
		case PlanStatus.ACTIVE: {
			return <OkStatus status="Active" />;
		}
		case PlanStatus.CANCELED: {
			return <ErrStatus status="Canceled" />;
		}
		case PlanStatus.INCOMPLETE: {
			return <ErrStatus status="Incomplete" />;
		}
		case PlanStatus.INCOMPLETE_EXPIRED: {
			return <ErrStatus status="Incomplete Expired" />;
		}
		case PlanStatus.PAST_DUE: {
			return <ErrStatus status="Past Due" />;
		}
		case PlanStatus.PAUSED: {
			return <ErrStatus status="Paused" />;
		}
		case PlanStatus.TRIALING: {
			return <OkStatus status="Trialing" />;
		}
		case PlanStatus.UNPAID: {
			return <ErrStatus status="Unpaid" />;
		}
		default:
			return <WarnStatus status="---" />;
	}
};

const OkStatus = (props) => {
	return <h4 class="subtitle has-text-success">{props.status}</h4>;
};

const WarnStatus = (props) => {
	return <h4 class="subtitle has-text-warning">{props.status}</h4>;
};

const ErrStatus = (props) => {
	return <h4 class="subtitle has-text-danger">{props.status}</h4>;
};

const FmtCardBrand = (props) => {
	switch (props.brand) {
		case CardBrand.AMEX: {
			return (
				<FmtCardBrandInner
					brand={branded_card("cc-amex")}
					name="American Express"
				/>
			);
		}
		case CardBrand.DINERS: {
			return (
				<FmtCardBrandInner
					brand={branded_card("cc-diners-club")}
					name="Diners Club"
				/>
			);
		}
		case CardBrand.DISCOVER: {
			return (
				<FmtCardBrandInner
					brand={branded_card("cc-discover")}
					name="Discover"
				/>
			);
		}
		case CardBrand.JCB: {
			return <FmtCardBrandInner brand={branded_card("cc-jcb")} name="JCB" />;
		}
		case CardBrand.MASTERCARD: {
			return (
				<FmtCardBrandInner
					brand={branded_card("cc-mastercard")}
					name="Mastercard"
				/>
			);
		}
		case CardBrand.UNIONPAY: {
			return <FmtCardBrandInner brand={generic_card()} name="Unionpay" />;
		}
		case CardBrand.VISA: {
			return <FmtCardBrandInner brand={branded_card("visa")} name="Visa" />;
		}
		case CardBrand.UNKNOWN: {
			return <FmtCardBrandInner brand={generic_card()} name="Credit Card" />;
		}
		default:
			return <FmtCardBrandInner brand={generic_card()} />;
	}
};

const FmtCardBrandInner = (props) => {
	return (
		<h4 class="subtitle">
			<span class="icon-text">
				<span class="icon">
					<i class={props.brand} aria-hidden="true" />
				</span>
				<span>{props.name}</span>
			</span>
		</h4>
	);
};

const branded_card = (brand) => {
	return `fab fa-${brand}`;
};

const generic_card = () => {
	return "fas fa-credit-card";
};

export default Plan;
