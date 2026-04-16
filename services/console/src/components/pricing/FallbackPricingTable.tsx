import InnerPricingTable from "./InnerPricingTable";

const FallbackPricingTable = () => {
	return (
		<InnerPricingTable
			handleFree={() => {}}
			handleTeam={() => {}}
			handleEnterprise={() => {}}
		/>
	);
};

export default FallbackPricingTable;
