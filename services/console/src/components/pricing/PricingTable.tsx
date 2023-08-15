import { useNavigate } from "../../util/url";
import { PlanLevel } from "../../types/bencher";
import Pricing from "../console/billing/plan/Pricing";

const PricingTable = () => {
	const navigate = useNavigate();

	const url = (plan: PlanLevel) => {
		return `/auth/signup?plan=${plan}`;
	};

	return (
		<Pricing
			plan={PlanLevel.Team}
			freeText="Sign up for free"
			handleFree={() => {
				navigate(url(PlanLevel.Free));
			}}
			teamText="Continue with Team"
			handleTeam={() => {
				navigate(url(PlanLevel.Team));
			}}
			enterpriseText="Continue with Enterprise"
			handleEnterprise={() => {
				navigate(url(PlanLevel.Enterprise));
			}}
		/>
	);
};

export default PricingTable;
