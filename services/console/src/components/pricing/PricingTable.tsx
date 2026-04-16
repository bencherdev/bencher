import { useNavigate } from "../../util/url";
import { PlanLevel } from "../../types/bencher";
import InnerPricingTable from "./InnerPricingTable";

const PricingTable = () => {
	const navigate = useNavigate();

	const URL = "/auth/signup";
	const url = (plan: PlanLevel) => {
		return `${URL}?plan=${plan}`;
	};

	return (
		<InnerPricingTable
			handleFree={() => {
				navigate(URL);
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
