import InnerPricingTable from "./InnerPricingTable";

const ConsoleFallbackPricingTable = (props: { hideFree: boolean }) => {
	return (
		<InnerPricingTable
			hideFree={props.hideFree}
			handleFree={() => {}}
			handlePro={() => {}}
			handleEnterprise={() => {}}
		/>
	);
};

export default ConsoleFallbackPricingTable;
