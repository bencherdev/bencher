import { createEffect } from "solid-js";
import { useNavigate } from "solid-app-router";
import Help from "../../console/panel/help/Help";
import { validate_jwt } from "../util";

const HelpPage = (props) => {
	const navigate = useNavigate();

	createEffect(() => {
		if (validate_jwt(props.user?.token)) {
			navigate(`/console/users/${props.user?.user?.slug}/help`);
		}
	});
	return <Help user={props.user} />;
};

export default HelpPage;
