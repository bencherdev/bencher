import * as Sentry from "@sentry/astro";
import { Show, createMemo, createResource } from "solid-js";
import { projectPath } from "../../next/paths";
import type {
	JsonAccept,
	JsonAuthUser,
	JsonProject,
} from "../../types/bencher";
import { authUser } from "../../util/auth";
import { httpGet, httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { BACK_PARAM, hiddenRedirect, useSearchParams } from "../../util/url";
import Redirect from "../site/Redirect";
import { CLAIM_PARAM, INVITE_PARAM, PLAN_PARAM } from "./auth";
import { AUTHORIZATION_PARAM, consentPath } from "./authorize/path";
import { keepBack, signedInPath } from "./back";

const AuthRedirect = (props: {
	/** Accepts the invite or the claim the address carries. */
	accept?: boolean;
	path: string;
	/** A sign in page keeps its way back; the confirm page only returns to one. */
	back?: "keep" | "take";
}) => {
	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();
	if (props.back === "keep") {
		keepBack(searchParams[BACK_PARAM]);
	}

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
		if (!props.accept || !token || !invite) {
			return null;
		}
		const accept = {
			invite,
		} as JsonAccept;
		return await httpPost("/v0/auth/accept", token, accept)
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
	const [_jsonAuth] = createResource<
		// biome-ignore lint/suspicious/noConfusingVoidType: acceptInvite navigates and returns nothing
		void | null,
		ReturnType<typeof acceptFetcher>
	>(acceptFetcher, acceptInvite);

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
		if (!props.accept || !token || !organization_uuid) {
			return null;
		}
		const path = `/v0/organizations/${organization_uuid}/claim`;
		return await httpPost(path, token, {})
			.then(async () => {
				hiddenRedirect(
					(await claimedProject(organization_uuid, token)) ?? "/console",
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
	const [_jsonOrganization] = createResource<
		// biome-ignore lint/suspicious/noConfusingVoidType: claimProject navigates and returns nothing
		void | null,
		ReturnType<typeof claimFetcher>
	>(claimFetcher, claimProject);

	return (
		<Show
			when={
				authUser()?.token &&
				!searchParams[INVITE_PARAM] &&
				!searchParams[CLAIM_PARAM]
			}
		>
			<Show
				when={(props.back ? signedInPath : consentPath)(
					searchParams[AUTHORIZATION_PARAM],
				)}
				fallback={<Redirect path={props.path} />}
			>
				{(consent) => {
					hiddenRedirect(consent());
					return null;
				}}
			</Show>
		</Show>
	);
};

export default AuthRedirect;

// An unclaimed organization holds the one project its run made; `projectAt` here would split its module out of every console page's entry.
const claimedProject = (organization: string, token: string) =>
	httpGet(`/v0/organizations/${organization}/projects`, token)
		.then((resp) => {
			const [project]: JsonProject[] = resp?.data ?? [];
			return project && projectPath(project.slug);
		})
		.catch(() => undefined);
