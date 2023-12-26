import { createEffect } from "solid-js";
import { authUser } from "../../util/auth";
import { useNavigate, useSearchParams } from "../../util/url";

const CODE_PARAM = "code";
const STATE_PARAM = "state";
// const INSTALLATION_ID_PARAM = "installation_id";
// const SETUP_ACTION_PARAM = "setup_action";

interface Props {
	apiUrl: string;
}

const AuthGitHub = (props: Props) => {
	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();
	const navigate = useNavigate();

	createEffect(() => {
		if (user?.token) {
			navigate("/console", { replace: true });
		}
		const code = searchParams[CODE_PARAM];
		const state = searchParams[STATE_PARAM];
		if (!code) {
			return;
		}
		console.log("code", code);
		console.log("state", state);
	});

	return <></>;
};

export default AuthGitHub;
