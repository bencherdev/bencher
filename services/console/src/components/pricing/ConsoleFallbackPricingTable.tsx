import InnerPricingTable from "./InnerPricingTable";

const ConsoleFallbackPricingTable = (props: { freeCtaText: string }) => {
	return (
		<InnerPricingTable
			freeCtaText={props.freeCtaText}
			handleFree={() => {}}
			handlePro={() => {}}
			handleEnterprise={() => {}}
		/>
	);
};

export default ConsoleFallbackPricingTable;
