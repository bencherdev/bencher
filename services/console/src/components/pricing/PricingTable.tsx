import { PlanLevel } from "../../types/bencher";
import { BENCHER_CALENDLY_URL } from "../../util/ext";
import { useNavigate } from "../../util/url";
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
			handlePro={() => {
				navigate(url(PlanLevel.Pro));
			}}
			handleEnterprise={() => {
				// Enterprise is "Contact us" (custom hardware), not self-serve.
				navigate(BENCHER_CALENDLY_URL);
			}}
		/>
	);
};

export default PricingTable;
