import { useSearchParams } from "solid-app-router";
import { createMemo, createSignal } from "solid-js";
import { PLAN_PARAM } from "../../../auth/AuthForm";
import { validate_plan } from "../../../site/util";
import Pricing, { Plan } from "./Pricing";

const Billing = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlan = (plan: Plan) => {
		setSearchParams({ [PLAN_PARAM]: plan });
	};
	if (!validate_plan(searchParams[PLAN_PARAM])) {
		setPlan(Plan.TEAM);
	}
	const plan = createMemo(() => searchParams[PLAN_PARAM]);

	return (
		<div class="content has-text-centered">
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
	);
};

export default Billing;
