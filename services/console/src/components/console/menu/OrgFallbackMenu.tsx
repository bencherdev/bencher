import OrgMenuInner from "./OrgMenuInner";

const OrgFallbackMenu = () => {
	return (
		<OrgMenuInner
			organization={() => undefined}
			billing={{
				state: "ready",
				loading: false,
				error: undefined,
				latest: true,
			}}
		/>
	);
};

export default OrgFallbackMenu;
