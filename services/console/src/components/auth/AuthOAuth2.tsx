import * as Sentry from "@sentry/astro";
import { createMemo, createResource } from "solid-js";
import type {
	JsonAuthUser,
	JsonOAuth,
	Jwt,
	PlanLevel,
	Uuid,
} from "../../types/bencher";
import { authUser, setUser } from "../../util/auth";
import { httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { useNavigate, useSearchParams } from "../../util/url";
import {
	type InitValid,
	init_valid,
	validJwt,
	validOptionUuid,
	validPlanLevel,
} from "../../util/valid";
import { PLAN_PARAM } from "./auth";

const CODE_PARAM = "code";
const STATE_PARAM = "state";

interface Props {
	apiUrl: string;
	pathname: string;
}

const AuthOAuth2 = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);

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
		bencher_valid: InitValid;
		code: undefined | string;
		state: undefined | string;
	}) => {
		if (fetcher.user?.token) {
			navigate("/console", { replace: true });
		}
		if (!fetcher.bencher_valid || !fetcher.code || !props.pathname) {
			return null;
		}
		const oauth = {
			code: fetcher.code,
		} as JsonOAuth;
		const state = fetcher.state;
		const setParams: [string, string][] = [];
		if (validJwt(state)) {
			oauth.invite = state as Jwt;
		} else if (validOptionUuid(state)) {
			oauth.claim = state as Uuid;
		} else if (validPlanLevel(state)) {
			oauth.plan = state as PlanLevel;
			setParams.push([PLAN_PARAM, state as PlanLevel]);
		}
		return await httpPost(props.apiUrl, props.pathname, null, oauth)
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
				Sentry.captureException(error);
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

export default AuthOAuth2;
