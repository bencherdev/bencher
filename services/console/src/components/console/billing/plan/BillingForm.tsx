import type { Params } from "astro";
import {
	type Resource,
	Show,
	createEffect,
	createMemo,
	createSignal,
} from "solid-js";
import {
	type JsonAuthUser,
	type JsonUsage,
	PlanLevel,
} from "../../../../types/bencher";
import { fmtDate } from "../../../../util/convert";
import { BENCHER_CALENDLY_URL } from "../../../../util/ext";
import { useNavigate, useSearchParams } from "../../../../util/url";
import { type InitValid, validPlanLevel } from "../../../../util/valid";
import { PLAN_PARAM } from "../../../auth/auth";
import InnerPricingTable from "../../../pricing/InnerPricingTable";
import Checkout from "./Checkout";

interface Props {
	apiUrl: string;
	params: Params;
	onboard: boolean;
	bencher_valid: Resource<InitValid>;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}

// Only Cloud Pro self-serves. Enterprise (and Self-Hosted) go through a sales
// conversation, so the Enterprise call to action opens "Contact us".
const BillingForm = (props: Props) => {
	const navigate = useNavigate();
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlanLevel = (planLevel: PlanLevel) => {
		setSearchParams({ [PLAN_PARAM]: planLevel });
	};
	const plan = createMemo(() => searchParams[PLAN_PARAM] as PlanLevel);

	// The onboarding plan step and the billing page both auto-activate checkout
	// when arriving with ?plan=pro (e.g. from /pricing). The signal is initialized
	// from the URL so the loader shows right away with no pricing-table flash.
	const [checkingOut, setCheckingOut] = createSignal(
		searchParams[PLAN_PARAM] === PlanLevel.Pro,
	);

	createEffect(() => {
		if (!props.bencher_valid()) {
			return;
		}
		const param = searchParams[PLAN_PARAM];
		// The ?plan=pro deep link starts checkout via the signal above. Drop the
		// param (replace, not push) so returning from Stripe lands on the plan
		// page instead of starting checkout again.
		if (param === PlanLevel.Pro) {
			setSearchParams({ [PLAN_PARAM]: PlanLevel.Free }, { replace: true });
			return;
		}
		// The billing page defaults to the Free view (onboarding shows all plans).
		if (!props.onboard && !validPlanLevel(param)) {
			setPlanLevel(PlanLevel.Free);
		}
	});

	const checkout = (planLevel: () => PlanLevel) => (
		<Checkout
			apiUrl={props.apiUrl}
			params={props.params}
			onboard={props.onboard}
			autoSubmit={true}
			bencher_valid={props.bencher_valid}
			user={props.user}
			organization={props.usage()?.organization}
			plan={planLevel}
			entitlements={() => null}
			organizationUuid={() => null}
			organizationUuidValid={() => true}
			handleRefresh={props.handleRefresh}
		/>
	);

	return (
		<Show
			when={props.onboard}
			fallback={
				<Show
					when={checkingOut()}
					fallback={
						<>
							<InnerPricingTable
								handleFree={() => setPlanLevel(PlanLevel.Free)}
								handlePro={() => setCheckingOut(true)}
								handleEnterprise={() =>
									window.open(BENCHER_CALENDLY_URL, "_blank", "noreferrer")
								}
							/>
							<Show when={plan() === PlanLevel.Free}>
								<FreeUsage usage={props.usage} />
							</Show>
						</>
					}
				>
					{checkout(() => PlanLevel.Pro)}
				</Show>
			}
		>
			{/* Onboarding: ?plan=pro (or choosing Pro) auto-activates checkout;
			    otherwise show all three plans with onboard-specific actions. */}
			<Show
				when={checkingOut()}
				fallback={
					<InnerPricingTable
						freeCtaText="Continue with Free"
						handleFree={() => navigate("/console")}
						handlePro={() => setCheckingOut(true)}
						handleEnterprise={() => {
							window.open(BENCHER_CALENDLY_URL, "_blank", "noreferrer");
							navigate("/console");
						}}
					/>
				}
			>
				{checkout(() => PlanLevel.Pro)}
			</Show>
		</Show>
	);
};

const FreeUsage = (props: { usage: Resource<null | JsonUsage> }) => {
	return (
		<div class="columns">
			<div class="column">
				<div class="content" style="margin-top: 4rem">
					<h2 class="title is-2">Free Tier Usage</h2>
					<h3 class="subtitle is-3">
						{fmtDate(props.usage()?.start_time)} -{" "}
						{fmtDate(props.usage()?.end_time)}
					</h3>
					<h4>Metrics Used: {props.usage()?.metrics?.toLocaleString() ?? 0}</h4>
					<h4>
						Runner Minutes Used:{" "}
						{props.usage()?.runner_minutes?.toLocaleString() ?? 0}
					</h4>
				</div>
			</div>
		</div>
	);
};

export default BillingForm;
