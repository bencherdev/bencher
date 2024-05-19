import InnerPricingTable from "./InnerPricingTable";

const FallbackPricingTable = () => {
	return (
		<InnerPricingTable
			themeColor="is-light"
			handleFree={() => {}}
			handleTeam={() => {}}
			handleEnterprise={() => {}}
		/>
	);
};

export default FallbackPricingTable;
