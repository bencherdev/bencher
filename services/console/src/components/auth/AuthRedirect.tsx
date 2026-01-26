import * as Sentry from "@sentry/astro";
import { Show, createMemo, createResource } from "solid-js";
import type {
	JsonAccept,
	JsonAuthAck,
	JsonAuthUser,
	JsonOrganization,
} from "../../types/bencher";
import { authUser } from "../../util/auth";
import { httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { useSearchParams } from "../../util/url";
import Redirect from "../site/Redirect";
import { CLAIM_PARAM, INVITE_PARAM, PLAN_PARAM } from "./auth";

const AuthRedirect = (props: { apiUrl?: string; path: string }) => {
	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();

	const acceptFetcher = createMemo(() => {
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
					props.path,
					[PLAN_PARAM],
					null,
				);
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				navigateNotify(
					NotifyKind.ERROR,
					"Invalid invitation. Please, try again.",
					null,
					[PLAN_PARAM],
					null,
				);
			});
	};
	const [_jsonAuth] = createResource<JsonAuthAck>(acceptFetcher, acceptInvite);

	const claimFetcher = createMemo(() => {
		return {
			user: user,
			organization_uuid: searchParams[CLAIM_PARAM],
		};
	});
	const claimProject = async (fetcher: {
		user: JsonAuthUser;
		organization_uuid: undefined | string;
	}) => {
		const token = fetcher.user?.token;
		const organization_uuid = fetcher.organization_uuid;
		if (!props.apiUrl || !token || !organization_uuid) {
			return null;
		}
		const path = `/v0/organizations/${organization_uuid}/claim`;
		return await httpPost(props.apiUrl, path, token, {})
			.then((_resp) => {
				navigateNotify(
					NotifyKind.OK,
					"Project claimed!",
					"/console",
					null,
					null,
				);
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				navigateNotify(
					NotifyKind.ERROR,
					`Invalid claim attempt: ${error}`,
					null,
					null,
					null,
				);
			});
	};
	const [_jsonOrganization] = createResource<JsonOrganization>(
		claimFetcher,
		claimProject,
	);

	return (
		<Show
			when={
				authUser()?.token &&
				!searchParams[INVITE_PARAM] &&
				!searchParams[CLAIM_PARAM]
			}
		>
			<Redirect path={props.path} />
		</Show>
	);
};

export default AuthRedirect;
