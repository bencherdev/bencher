import InnerPricingTable from "./InnerPricingTable";

const FallbackPricingTable = () => {
	return (
		<InnerPricingTable
			handleFree={() => {}}
			handlePro={() => {}}
			handleEnterprise={() => {}}
		/>
	);
};

export default FallbackPricingTable;
