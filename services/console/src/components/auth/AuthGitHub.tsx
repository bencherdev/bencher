import { createEffect } from "solid-js";
import { authUser } from "../../util/auth";
import { useNavigate, useSearchParams } from "../../util/url";

const CODE_PARAM = "code";
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
		if (!code) {
			return;
		}
		console.log("code", code);
	});

	return <></>;
};

export default AuthGitHub;
