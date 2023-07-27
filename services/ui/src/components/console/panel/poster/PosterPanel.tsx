import Poster from "./Poster";
import PosterHeader from "./PosterHeader";

const PosterPanel = (props) => {
	return (
		<>
			<PosterHeader config={props.config?.header} />
			<Poster
				user={props.user}
				config={props.config?.form}
				operation={props.config?.operation}
				path_params={props.path_params}
			/>
		</>
	);
};

export default PosterPanel;
