import { createEffect, createMemo, createResource } from "solid-js";
import { authUser } from "../../util/auth";
import { useNavigate, useSearchParams } from "../../util/url";
import type { JsonAuth, JsonAuthUser, JsonOAuth } from "../../types/bencher";
import { httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { PLAN_PARAM } from "./auth";

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

	const fetcher = createMemo(() => {
		return {
			code: searchParams[CODE_PARAM],
			state: searchParams[STATE_PARAM],
		};
	});
	const getAuthUser = async (fetcher: {
		code: undefined | string;
		state: undefined | string;
	}) => {
		if (!fetcher.code) {
			return null;
		}
		const oauth = {
			code: fetcher.code,
			invite: fetcher.state,
		} as JsonOAuth;
		return await httpPost(props.apiUrl, "/v0/auth/github", null, oauth)
			.then((resp) => {
				navigateNotify(
					NotifyKind.OK,
					`Hoppy to git to see you!`,
					"/console",
					[PLAN_PARAM],
					null,
				);
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [jsonAuthUser] = createResource<JsonAuthUser>(fetcher, getAuthUser);

	createEffect(() => {
		if (user?.token) {
			navigate("/console", { replace: true });
		}
		const code = searchParams[CODE_PARAM];
		const state = searchParams[STATE_PARAM];
		if (!code) {
			return;
		}
		// todo send over code and state -> invite to server to get auth creds then save to local storage
		console.log("code", code);
		console.log("state", state);
	});

	return <></>;
};

export default AuthGitHub;
