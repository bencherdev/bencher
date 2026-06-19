import type { Params } from "astro";
import { type Resource, Show, createEffect, createMemo } from "solid-js";
import {
	type JsonAuthUser,
	type JsonUsage,
	PlanLevel,
} from "../../../../types/bencher";
import { fmtDate } from "../../../../util/convert";
import { BENCHER_CALENDLY_URL } from "../../../../util/ext";
import { useSearchParams } from "../../../../util/url";
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
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlanLevel = (planLevel: PlanLevel) => {
		setSearchParams({ [PLAN_PARAM]: planLevel });
	};
	const plan = createMemo(() => searchParams[PLAN_PARAM] as PlanLevel);

	createEffect(() => {
		if (!props.bencher_valid()) {
			return;
		}
		if (!validPlanLevel(searchParams[PLAN_PARAM])) {
			setPlanLevel(props.onboard ? PlanLevel.Pro : PlanLevel.Free);
		} else if (props.onboard && plan() === PlanLevel.Free) {
			setPlanLevel(PlanLevel.Pro);
		}
	});

	return (
		<>
			<InnerPricingTable
				hideFree={props.onboard}
				handleFree={() => setPlanLevel(PlanLevel.Free)}
				handlePro={() => setPlanLevel(PlanLevel.Pro)}
				handleEnterprise={() =>
					window.open(BENCHER_CALENDLY_URL, "_blank", "noreferrer")
				}
			/>
			<Show when={plan() === PlanLevel.Pro}>
				<Checkout
					apiUrl={props.apiUrl}
					params={props.params}
					onboard={props.onboard}
					bencher_valid={props.bencher_valid}
					user={props.user}
					organization={props.usage()?.organization}
					plan={plan}
					entitlements={() => null}
					organizationUuid={() => null}
					organizationUuidValid={() => true}
					handleRefresh={props.handleRefresh}
				/>
			</Show>
			<Show when={plan() === PlanLevel.Free}>
				<FreeUsage usage={props.usage} />
			</Show>
		</>
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
