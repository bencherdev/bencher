import { useNavigate } from "../../util/url";
import { PlanLevel } from "../../types/bencher";
import InnerPricingTable from "./InnerPricingTable";

const PricingTable = () => {
	const navigate = useNavigate();

	const url = (plan: PlanLevel) => {
		return `/auth/signup?plan=${plan}`;
	};

	return (
		<InnerPricingTable
			handleFree={() => {
				navigate(url(PlanLevel.Free));
			}}
			handleTeam={() => {
				navigate(url(PlanLevel.Team));
			}}
			handleEnterprise={() => {
				navigate(url(PlanLevel.Enterprise));
			}}
		/>
	);
};

export default PricingTable;
