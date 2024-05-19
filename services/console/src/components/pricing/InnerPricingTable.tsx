import Pricing from "../console/billing/plan/Pricing";

interface Props {
	themeColor: string;
	handleFree: () => void;
	handleTeam: () => void;
	handleEnterprise: () => void;
}

const InnerPricingTable = (props: Props) => {
	return (
		<Pricing
			themeColor={props.themeColor}
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
