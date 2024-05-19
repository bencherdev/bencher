import Pricing from "../console/billing/plan/Pricing";

export const FREE_TEXT = "Stick with Free";
export const TEAM_TEXT = "Go with Team";
export const ENTERPRISE_TEXT = "Go with Enterprise";

const ConsoleFallbackPricingTable = (props: { hideFree: boolean }) => {
	return (
		<Pricing
			themeColor="is-light"
			freeText={FREE_TEXT}
			handleFree={() => {}}
			hideFree={props.hideFree}
			teamText={TEAM_TEXT}
			handleTeam={() => {}}
			enterpriseText={ENTERPRISE_TEXT}
			handleEnterprise={() => {}}
		/>
	);
};

export default ConsoleFallbackPricingTable;
