import { useNavigate } from "../../util/url";
import { PlanLevel } from "../../types/bencher";
import InnerPricingTable from "./InnerPricingTable";
import { getThemeColor } from "../navbar/theme/util";

const PricingTable = () => {
	const navigate = useNavigate();
	const themeColor = getThemeColor;

	const URL = "/auth/signup";
	const url = (plan: PlanLevel) => {
		return `${URL}?plan=${plan}`;
	};

	return (
		<InnerPricingTable
			themeColor={themeColor()}
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
