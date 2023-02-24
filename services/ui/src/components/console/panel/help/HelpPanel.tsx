import Help from "./Help";
import HelpHeader from "./HelpHeader";

const HelpPanel = (props) => {
	return (
		<>
			<HelpHeader config={props.config?.header} />
			<Help
				user={props.user}
				config={props.config?.form}
				path_params={props.path_params}
			/>
		</>
	);
};

export default HelpPanel;
