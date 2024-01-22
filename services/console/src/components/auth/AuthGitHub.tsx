import bencher_valid_init, { type InitOutput } from "bencher_valid";
import { createMemo, createResource } from "solid-js";
import { authUser, setUser } from "../../util/auth";
import { useNavigate, useSearchParams } from "../../util/url";
import type { JsonAuthUser, JsonOAuth } from "../../types/bencher";
import { httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { PLAN_PARAM } from "./auth";
import { validJwt, validPlanLevel } from "../../util/valid";

const CODE_PARAM = "code";
const STATE_PARAM = "state";
// const INSTALLATION_ID_PARAM = "installation_id";
// const SETUP_ACTION_PARAM = "setup_action";

interface Props {
	apiUrl: string;
}

const AuthGitHub = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();
	const navigate = useNavigate();

	const fetcher = createMemo(() => {
		return {
			user: user,
			bencher_valid: bencher_valid(),
			code: searchParams[CODE_PARAM],
			state: searchParams[STATE_PARAM],
		};
	});
	const getAuthUser = async (fetcher: {
		user: JsonAuthUser;
		bencher_valid: InitOutput;
		code: undefined | string;
		state: undefined | string;
	}) => {
		if (fetcher.user?.token) {
			navigate("/console", { replace: true });
		}
		if (!fetcher.bencher_valid || !fetcher.code) {
			return null;
		}
		const oauth = {
			code: fetcher.code,
		} as JsonOAuth;
		const state = fetcher.state;
		const setParams = [];
		if (validJwt(state)) {
			oauth.invite = state;
		} else if (validPlanLevel(state)) {
			oauth.plan = state;
			setParams.push([PLAN_PARAM, state]);
		}
		return await httpPost(props.apiUrl, "/v0/auth/github", null, oauth)
			.then((resp) => {
				const user = resp.data;
				if (setUser(user)) {
					navigateNotify(
						NotifyKind.OK,
						"Hoppy to git to see you!",
						"/console",
						null,
						setParams,
					);
				} else {
					navigateNotify(
						NotifyKind.ERROR,
						"Invalid user. Please, try again.",
						"/auth/login",
						null,
						null,
					);
				}
			})
			.catch((error) => {
				console.error(error);
				navigateNotify(
					NotifyKind.ERROR,
					"Invalid user. Please, try again.",
					"/auth/login",
					null,
					null,
				);
			});
	};
	const [_jsonAuthUser] = createResource<JsonAuthUser>(fetcher, getAuthUser);

	return <></>;
};

export default AuthGitHub;
