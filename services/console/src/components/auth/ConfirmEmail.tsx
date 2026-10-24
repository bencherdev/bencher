import * as Sentry from "@sentry/astro";
import { Match, Show, Switch, createResource, createSignal } from "solid-js";
import type { JsonAuthUser, JsonConfirm, Jwt } from "../../types/bencher";
import { getUserRaw, setUser } from "../../util/auth";
import { httpPut } from "../../util/http";
import { deleteCache } from "../../next/idb";
import { profilePath } from "../../next/paths";
import { hiddenRedirect, useSearchParams } from "../../util/url";
import { init_valid, validJwt, validUuid } from "../../util/valid";
import { TOKEN_PARAM, USER_PARAM } from "./auth";

export interface Props {
	apiUrl: string;
}

const ConfirmEmail = (props: Props) => {
	const [searchParams] = useSearchParams();
	const user = searchParams[USER_PARAM];
	const token = searchParams[TOKEN_PARAM];

	const [intent, setIntent] = createSignal(false);

	const [bencher_valid] = createResource(init_valid);
	const valid = () =>
		!!bencher_valid() && !!user && validUuid(user) && validJwt(token);
	const [state] = createResource(
		() => valid() && intent(),
		async () => {
			const confirm: JsonConfirm = { token: token as Jwt };
			return await httpPut(
				props.apiUrl,
				`/v0/users/${user}/email`,
				null,
				confirm,
			)
				.then((resp) => {
					const authUser: JsonAuthUser = resp.data;
					const stored = getUserRaw();
					if (stored?.token && stored.user?.uuid !== authUser.user.uuid) {
						return EmailState.SignIn;
					}
					if (!setUser(authUser)) {
						return EmailState.Failed;
					}
					// The console's saved responses still hold the email just replaced.
					deleteCache();
					hiddenRedirect(profilePath(authUser.user.slug));
					return EmailState.Confirming;
				})
				.catch((error) => {
					switch (error?.response?.status) {
						case 401:
							return EmailState.Expired;
						case 403:
							return EmailState.Used;
						case 409:
							return EmailState.Taken;
						default:
							console.error(error);
							Sentry.captureException(error);
							return EmailState.Failed;
					}
				});
		},
	);

	return (
		<div class="content has-text-centered">
			<Switch>
				<Match when={bencher_valid() && !valid()}>
					<h1 class="title is-3">This link is not valid</h1>
				</Match>
				<Match when={!intent()}>
					<h1 class="title is-3">Confirm your new email</h1>
					<Show when={valid()}>
						<div class="buttons is-centered">
							<button
								class="button is-primary"
								type="button"
								onClick={() => setIntent(true)}
							>
								Change email
							</button>
							<a class="button" href="/auth/login">
								Cancel
							</a>
						</div>
					</Show>
				</Match>
			</Switch>
			<div role="status">
				<Show when={intent()}>
					<Switch
						fallback={<h1 class="title is-3">Confirming your new email</h1>}
					>
						<Match when={state() === EmailState.Expired}>
							<h1 class="title is-3">This link has expired</h1>
						</Match>
						<Match when={state() === EmailState.Used}>
							<h1 class="title is-3">This link is no longer valid</h1>
						</Match>
						<Match when={state() === EmailState.Taken}>
							<h1 class="title is-3">That email is already in use</h1>
						</Match>
						<Match when={state() === EmailState.SignIn}>
							<h1 class="title is-3">Your email has changed</h1>
						</Match>
						<Match when={state() === EmailState.Failed}>
							<h1 class="title is-3">Failed to confirm your new email</h1>
						</Match>
					</Switch>
				</Show>
			</div>
		</div>
	);
};

enum EmailState {
	Confirming = "confirming",
	Expired = "expired",
	Used = "used",
	Taken = "taken",
	SignIn = "sign_in",
	Failed = "failed",
}

export default ConfirmEmail;
