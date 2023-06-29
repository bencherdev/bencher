import { useSearchParams } from "solid-app-router";
import { createMemo, createSignal } from "solid-js";
import {
	BENCHER_API_URL,
	PLAN_PARAM,
	validate_plan_level,
} from "../../../site/util";
import PaymentCard, { cardForm } from "./PaymentCard";
import { PlanLevel } from "../../../../types/bencher";
import Pricing from "./Pricing";

const Billing = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlanLevel = (plan_level: PlanLevel) => {
		setSearchParams({ [PLAN_PARAM]: plan_level });
	};
	if (!validate_plan_level(searchParams[PLAN_PARAM])) {
		setPlanLevel(PlanLevel.Free);
	}
	const plan = createMemo(() => searchParams[PLAN_PARAM]);

	const [form, setForm] = createSignal(cardForm());

	return (
		<div class="columns is-centered">
			<div class="column">
				<Pricing
					active={plan()}
					free_text="Stick with Free"
					handleFree={(e) => {
						e.preventDefault();
						setPlanLevel(PlanLevel.Free);
					}}
					team_text="Go with Team"
					handleTeam={(e) => {
						e.preventDefault();
						setPlanLevel(PlanLevel.Team);
					}}
					enterprise_text="Go with Enterprise"
					handleEnterprise={(e) => {
						e.preventDefault();
						setPlanLevel(PlanLevel.Enterprise);
					}}
				/>
				<br />
				{plan() !== PlanLevel.Free && (
					<PaymentCard
						user={props.user}
						url={`${BENCHER_API_URL()}/v0/organizations/${props.organization_slug()}/plan`}
						plan={plan}
						form={form}
						handleForm={setForm}
						handleRefresh={props.handleRefresh}
					/>
				)}
			</div>
		</div>
	);
};

export default Billing;
