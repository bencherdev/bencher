import Help from "./Help";
import HelpHeader from "./HelpHeader";

const HelpPanel = (props) => {
	return (
		<>
			<HelpHeader config={props.config?.header} />
			<Help user={props.user} />
		</>
	);
};

export default HelpPanel;
