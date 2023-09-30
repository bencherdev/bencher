import type { InitOutput } from "bencher_valid";
import { createMemo, createResource, For, type Resource } from "solid-js";
import {
	CardBrand,
	type JsonAuthUser,
	type JsonPlan,
	type JsonUsage,
	PlanLevel,
	PlanStatus,
} from "../../../../types/bencher";
import type { Params } from "astro";
import { validJwt } from "../../../../util/valid";
import { httpGet } from "../../../../util/http";
import { dateTimeMillis } from "../../../../util/convert";

const fmtDateTime = (date_str: undefined | string) => {
	if (date_str === undefined) {
		return null;
	}
	const date_ms = Date.parse(date_str);
	if (date_ms) {
		const date = new Date(date_ms);
		if (date) {
			return date.toDateString();
		}
	}
	return null;
};

const planLevel = (level: undefined | PlanLevel) => {
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

export const fmtUsd = (usd: undefined | number) => {
	const numberFmd = new Intl.NumberFormat("en-US", {
		style: "currency",
		currency: "USD",
	});
	return numberFmd.format(usd ?? 0);
};

interface Props {
	apiUrl: string;
	params: Params;
	bencher_valid: Resource<InitOutput>;
	user: JsonAuthUser;
	plan: Resource<JsonPlan>;
}

const Plan = (props: Props) => {
	const fetcher = createMemo(() => {
		return {
			bencher_valid: props.bencher_valid(),
			organization_slug: props.params.organization,
			token: props.user?.token,
		};
	});
	const fetchUsage = async (fetcher: {
		bencher_valid: InitOutput;
		organization_slug: string;
		token: string;
	}): Promise<null | JsonUsage> => {
		if (!fetcher.bencher_valid) {
			return null;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		const start = dateTimeMillis(props.plan()?.current_period_start);
		const end = dateTimeMillis(props.plan()?.current_period_end);
		const path = `/v0/organizations/${fetcher.organization_slug}/usage?start=${start}&end=${end}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [usage] = createResource<null | JsonUsage>(fetcher, fetchUsage);

	const perMetricRate = createMemo(() => {
		const unitAmount = props.plan()?.unit_amount;
		if (unitAmount === undefined) {
			return 1;
		} else {
			return unitAmount / 100;
		}
	});
	const estCost = createMemo(() => {
		const metricsUsed = usage()?.metrics_used;
		if (metricsUsed === undefined) {
			return 0;
		}
		if (!Number.isInteger(metricsUsed)) {
			return 0;
		}
		return metricsUsed * perMetricRate();
	});

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
							{fmtDateTime(props.plan()?.current_period_start)} -{" "}
							{fmtDateTime(props.plan()?.current_period_end)}
						</h4>
						<p>Plan Level: {planLevel(props.plan()?.level)}</p>
						<p>Per Metric Rate: {fmtUsd(perMetricRate())}</p>
						<p>
							Estimated Usage:{" "}
							{Number.isInteger(usage()?.metrics_used)
								? usage()?.metrics_used
								: "---"}
						</p>
						<p>
							Current Estimated Cost:{" "}
							{estCost() === null ? "---" : fmtUsd(estCost())}
						</p>
						{/* TODO if planLevel === PlanLevel.Team then Upgrade Plan button */}
						<br />

						<h4 class="title">Payment Method</h4>
						<FmtCardBrand brand={props.plan()?.card?.brand} />
						<p>Name: {props.plan()?.customer?.name}</p>
						<p>Last Four: {props.plan()?.card?.last_four}</p>
						<p>
							Expiration: {props.plan()?.card?.exp_month}/
							{props.plan()?.card?.exp_year}
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

const FmtPlanStatus = (props: {
	status: undefined | PlanStatus;
}) => {
	switch (props.status) {
		case PlanStatus.Active: {
			return <OkStatus status="Active" />;
		}
		case PlanStatus.Canceled: {
			return <ErrStatus status="Canceled" />;
		}
		case PlanStatus.Incomplete: {
			return <ErrStatus status="Incomplete" />;
		}
		case PlanStatus.IncompleteExpired: {
			return <ErrStatus status="Incomplete Expired" />;
		}
		case PlanStatus.PastDue: {
			return <ErrStatus status="Past Due" />;
		}
		case PlanStatus.Paused: {
			return <ErrStatus status="Paused" />;
		}
		case PlanStatus.Trialing: {
			return <OkStatus status="Trialing" />;
		}
		case PlanStatus.Unpaid: {
			return <ErrStatus status="Unpaid" />;
		}
		default:
			return <WarnStatus status="---" />;
	}
};

const OkStatus = (props: {
	status: string;
}) => {
	return <h4 class="subtitle has-text-success">{props.status}</h4>;
};

const WarnStatus = (props: {
	status: string;
}) => {
	return <h4 class="subtitle has-text-warning">{props.status}</h4>;
};

const ErrStatus = (props: {
	status: string;
}) => {
	return <h4 class="subtitle has-text-danger">{props.status}</h4>;
};

const FmtCardBrand = (props: {
	brand: undefined | CardBrand;
}) => {
	switch (props.brand) {
		case CardBrand.Amex: {
			return (
				<FmtCardBrandInner
					brand={brandedCard("cc-amex")}
					name="American Express"
				/>
			);
		}
		case CardBrand.Diners: {
			return (
				<FmtCardBrandInner
					brand={brandedCard("cc-diners-club")}
					name="Diners Club"
				/>
			);
		}
		case CardBrand.Discover: {
			return (
				<FmtCardBrandInner brand={brandedCard("cc-discover")} name="Discover" />
			);
		}
		case CardBrand.Jcb: {
			return <FmtCardBrandInner brand={brandedCard("cc-jcb")} name="JCB" />;
		}
		case CardBrand.Mastercard: {
			return (
				<FmtCardBrandInner
					brand={brandedCard("cc-mastercard")}
					name="Mastercard"
				/>
			);
		}
		case CardBrand.Unionpay: {
			return <FmtCardBrandInner brand={genericCard()} name="Unionpay" />;
		}
		case CardBrand.Visa: {
			return <FmtCardBrandInner brand={brandedCard("visa")} name="Visa" />;
		}
		case CardBrand.Unknown: {
			return <FmtCardBrandInner brand={genericCard()} name="Credit Card" />;
		}
		default:
			return <FmtCardBrandInner brand={genericCard()} />;
	}
};

const FmtCardBrandInner = (props: {
	brand: string;
	name?: string;
}) => {
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

const brandedCard = (brand: string) => {
	return `fab fa-${brand}`;
};

const genericCard = () => {
	return "fas fa-credit-card";
};

export default Plan;
