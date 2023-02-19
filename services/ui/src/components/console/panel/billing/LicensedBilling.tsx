import { useSearchParams } from "solid-app-router";
import { createMemo, createSignal } from "solid-js";
import { PLAN_PARAM } from "../../../auth/AuthForm";
import { validate_plan } from "../../../site/util";
import Pricing, { per_metric_cost, Plan } from "./Pricing";

const HOST_PARAM = "host";
const QUANTITY = "quantity";

enum Host {
	SelfHosted = "self_hosted",
	BencherCloud = "bencher_cloud",
}

const Billing = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlan = (plan: Plan) => {
		setSearchParams({ [PLAN_PARAM]: plan });
	};
	if (!validate_plan(searchParams[PLAN_PARAM])) {
		setPlan(Plan.TEAM);
	}
	const plan = createMemo(() => searchParams[PLAN_PARAM]);

	const setHost = (host: Host) => {
		setSearchParams({ [HOST_PARAM]: host });
	};
	if (
		!(
			searchParams[HOST_PARAM] === Host.SelfHosted ||
			searchParams[HOST_PARAM] === Host.BencherCloud
		)
	) {
		setHost(Host.BencherCloud);
	}
	const host = createMemo(() => searchParams[HOST_PARAM]);

	const is_valid_quantity = (quantity: any) => {
		let number = Number(quantity);
		return number > 0 && Number.isInteger(number);
	};
	const setQuantity = (quantity: null | number) => {
		setSearchParams({ [QUANTITY]: quantity });
	};
	// if (!is_valid_quantity(searchParams[QUANTITY])) {
	// 	setQuantity(null);
	// }
	const quantity = createMemo(() => searchParams[QUANTITY]);

	const monthly_total = createMemo(
		() => quantity() * per_metric_cost(plan()) * 10,
	);
	const annual_total = createMemo(() => monthly_total() * 12);

	return (
		<div class="columns is-centered">
			<div class="column">
				<HostButtons host={host()} setHost={setHost} />
				<br />
				<Pricing
					active={plan()}
					free_text="Choose Free"
					handleFree={(e) => {
						e.preventDefault();
						setPlan(Plan.FREE);
					}}
					team_text="Go with Team"
					handleTeam={(e) => {
						e.preventDefault();
						setPlan(Plan.TEAM);
					}}
					enterprise_text="Go with Enterprise"
					handleEnterprise={(e) => {
						e.preventDefault();
						setPlan(Plan.ENTERPRISE);
					}}
				/>
				<br />
				{host() === Host.SelfHosted && (
					<Quantity
						valid={is_valid_quantity(quantity())}
						quantity={quantity()}
						setQuantity={setQuantity}
						monthly_total={monthly_total()}
						annual_total={annual_total()}
					/>
				)}
				<br />
				{((host() === Host.SelfHosted && is_valid_quantity(annual_total())) ||
					(host() === Host.BencherCloud && plan() !== Plan.FREE)) && (
					<div class="box">TODO payment</div>
				)}
			</div>
		</div>
	);
};

const HostButtons = (props: { host: Host; setHost: Function }) => {
	return (
		<div class="buttons has-addons is-centered">
			<button
				class="button"
				onClick={(e) => {
					e.preventDefault();
					props.setHost(Host.SelfHosted);
				}}
			>
				<span
					class={`icon is-small ${
						props.host === Host.SelfHosted && "has-text-primary"
					}`}
				>
					<i class="fas fa-server" aria-hidden="true" />
				</span>
				<span>Self-Hosted</span>
			</button>
			<button
				class="button"
				onClick={(e) => {
					e.preventDefault();
					props.setHost(Host.BencherCloud);
				}}
			>
				<span
					class={`icon is-small ${
						props.host === Host.BencherCloud && "has-text-primary"
					}`}
				>
					<i class="fas fa-cloud" aria-hidden="true" />
				</span>
				<span>Bencher Cloud</span>
			</button>
		</div>
	);
};

const Quantity = (props: {
	valid: boolean;
	quantity: null | number;
	setQuantity: Function;
	monthly_total: number;
	annual_total: number;
}) => {
	return (
		<div class="box">
			<form>
				<p class="control">
					<label class="label">Metrics</label>
				</p>
				<div class="field has-addons">
					<div class="control has-icons-left has-icons-right">
						<span class="icon is-small is-left">
							<i class="fas fa-cubes" />
						</span>
						<input
							class="input"
							type="number"
							placeholder="100"
							value={props.quantity}
							onInput={(event) => props.setQuantity(event.target?.value)}
						/>
						{props.valid && (
							<span class="icon is-small is-right">
								<i class="fas fa-check" />
							</span>
						)}
					</div>
					<p class="control">
						{/* rome-ignore lint/a11y/useValidAnchor: Bulma */}
						<a class="button is-static">x 1,000 / month</a>
					</p>
				</div>
				{!props.valid && (
					<p class="control help is-danger">
						Must be an integer greater than zero
					</p>
				)}
			</form>
			<br />
			<p>Monthly Total: ${props.monthly_total ? props.monthly_total : "0"}</p>
			<p>Total Due: ${props.annual_total ? props.annual_total : "0"}</p>
		</div>
	);
};

export default Billing;
