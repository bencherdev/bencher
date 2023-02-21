import { useSearchParams } from "solid-app-router";
import { createMemo } from "solid-js";
import { PLAN_PARAM } from "../../../auth/AuthForm";
import { validate_plan_level } from "../../../site/util";
import PaymentCard from "./PaymentCard";
import Pricing, { PlanLevel } from "./Pricing";

const Billing = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlanLevel = (plan_level: PlanLevel) => {
		setSearchParams({ [PLAN_PARAM]: plan_level });
	};
	if (!validate_plan_level(searchParams[PLAN_PARAM])) {
		setPlanLevel(PlanLevel.FREE);
	}
	const plan = createMemo(() => searchParams[PLAN_PARAM]);

	return (
		<div class="columns is-centered">
			<div class="column">
				<Pricing
					active={plan()}
					free_text="Stick with Free"
					handleFree={(e) => {
						e.preventDefault();
						setPlanLevel(PlanLevel.FREE);
					}}
					team_text="Go with Team"
					handleTeam={(e) => {
						e.preventDefault();
						setPlanLevel(PlanLevel.TEAM);
					}}
					enterprise_text="Go with Enterprise"
					handleEnterprise={(e) => {
						e.preventDefault();
						setPlanLevel(PlanLevel.ENTERPRISE);
					}}
				/>
				<br />
				{plan() !== PlanLevel.FREE && <PaymentCard />}
			</div>
		</div>
	);
};

export default Billing;
