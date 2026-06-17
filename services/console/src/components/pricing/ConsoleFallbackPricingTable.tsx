import Pricing from "../console/billing/plan/Pricing";

export const FREE_TEXT = "Stick with Free";
export const PRO_TEXT = "Go with Pro";
export const ENTERPRISE_TEXT = "Go with Enterprise";

const ConsoleFallbackPricingTable = (props: { hideFree: boolean }) => {
	return (
		<Pricing
			themeColor="is-light"
			freeText={FREE_TEXT}
			handleFree={() => {}}
			hideFree={props.hideFree}
			proText={PRO_TEXT}
			handlePro={() => {}}
			enterpriseText={ENTERPRISE_TEXT}
			handleEnterprise={() => {}}
		/>
	);
};

export default ConsoleFallbackPricingTable;
