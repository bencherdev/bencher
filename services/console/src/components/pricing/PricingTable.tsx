import { PlanLevel } from "../../types/bencher";
import { authUser } from "../../util/auth";
import { BENCHER_CALENDLY_URL } from "../../util/ext";
import { useNavigate } from "../../util/url";
import { PLAN_PARAM } from "../auth/auth";
import InnerPricingTable from "./InnerPricingTable";

const PricingTable = () => {
	const navigate = useNavigate();

	// Logged-in visitors are routed through the console to their organization
	// billing page to sign up for the plan. Logged-out visitors sign up first.
	const handlePlan = (plan: PlanLevel) => {
		if (authUser()?.token) {
			navigate(`/console?${PLAN_PARAM}=${plan}`);
		} else if (plan === PlanLevel.Free) {
			navigate("/auth/signup");
		} else {
			navigate(`/auth/signup?${PLAN_PARAM}=${plan}`);
		}
	};

	return (
		<InnerPricingTable
			handleFree={() => handlePlan(PlanLevel.Free)}
			handlePro={() => handlePlan(PlanLevel.Pro)}
			handleEnterprise={() => {
				// Enterprise is "Contact us" (custom hardware), not self-serve.
				navigate(BENCHER_CALENDLY_URL);
			}}
		/>
	);
};

export default PricingTable;
