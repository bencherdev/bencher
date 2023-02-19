import { useSearchParams } from "solid-app-router";
import { createMemo, createSignal } from "solid-js";
import { PLAN_PARAM } from "../../../auth/AuthForm";
import { validate_plan } from "../../../site/util";
import Pricing, { Plan } from "./Pricing";

const HOST_PARAM = "host";

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

export default Billing;
