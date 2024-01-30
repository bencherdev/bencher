import { Show, createMemo, createResource } from "solid-js";
import Redirect from "../site/Redirect";
import { authUser } from "../../util/auth";
import { useSearchParams } from "../../util/url";
import { INVITE_PARAM, PLAN_PARAM } from "./auth";
import type {
	JsonAuthAck,
	JsonAccept,
	JsonAuthUser,
} from "../../types/bencher";
import { httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";

const AuthRedirect = (props: { apiUrl?: string; path: string }) => {
	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();

	const fetcher = createMemo(() => {
		return {
			user: user,
			invite: searchParams[INVITE_PARAM],
		};
	});
	const acceptInvite = async (fetcher: {
		user: JsonAuthUser;
		invite: undefined | string;
	}) => {
		const token = fetcher.user?.token;
		const invite = fetcher.invite;
		if (!props.apiUrl || !token || !invite) {
			return null;
		}
		const accept = {
			invite,
		} as JsonAccept;
		return await httpPost(props.apiUrl, "/v0/auth/accept", token, accept)
			.then((_resp) => {
				navigateNotify(
					NotifyKind.OK,
					"Invitation accepted!",
					"/console",
					[PLAN_PARAM],
					null,
				);
			})
			.catch((error) => {
				console.error(error);
				navigateNotify(
					NotifyKind.ERROR,
					"Invalid invitation. Please, try again.",
					null,
					[PLAN_PARAM],
					null,
				);
			});
	};
	const [_jsonAuth] = createResource<JsonAuthAck>(fetcher, acceptInvite);

	return (
		<Show when={authUser()?.token && !searchParams[INVITE_PARAM]}>
			<Redirect path={props.path} />
		</Show>
	);
};

export default AuthRedirect;
