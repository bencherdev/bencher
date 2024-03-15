import { PlanLevel } from "../../types/bencher";
import Pricing from "../console/billing/plan/Pricing";

interface Props {
	handleFree: () => void;
	handleTeam: () => void;
	handleEnterprise: () => void;
}

const InnerPricingTable = (props: Props) => {
	return (
		<Pricing
			plan={PlanLevel.Team}
			freeText="Sign up for free"
			handleFree={props.handleFree}
			teamText="Continue with Team"
			handleTeam={props.handleTeam}
			enterpriseText="Continue with Enterprise"
			handleEnterprise={props.handleEnterprise}
		/>
	);
};

export default InnerPricingTable;
