import * as Sentry from "@sentry/astro";
import { Show, createMemo, createResource, createSignal } from "solid-js";
import type {
	JsonAuthUser,
	JsonOAuth,
	JsonOAuthUser,
} from "../../types/bencher";
import { authUser, setUser } from "../../util/auth";
import { httpPost } from "../../util/http";
import { NotifyKind, navigateNotify } from "../../util/notify";
import { hiddenRedirect, useNavigate, useSearchParams } from "../../util/url";
import { type InitValid, init_valid } from "../../util/valid";
import SignInAs from "./SignInAs";
import { PLAN_PARAM } from "./auth";
import { signInPath } from "./authorize/path";
import { signedInPath } from "./back";
import { takeSignIn } from "./signIn";

const CODE_PARAM = "code";
const STATE_PARAM = "state";

interface Props {
	apiUrl: string;
	pathname: string;
}

const AuthOAuth = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);

	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();
	const navigate = useNavigate();
	// Signed in with the provider, but not by the sign in this tab started, so not signed in until asked.
	const [pending, setPending] = createSignal<JsonOAuthUser>();

	const signIn = (oauthUser: JsonOAuthUser) => {
		// Taken before the reader is stored, so a tab that sees them first finds it gone.
		const to = signedInPath(oauthUser?.authorization);
		if (!setUser(oauthUser?.user)) {
			navigateNotify(
				NotifyKind.ERROR,
				"Invalid user. Please, try again.",
				"/auth/login",
				null,
				null,
			);
			return;
		}
		const plan = oauthUser.plan;
		if (to) {
			hiddenRedirect(to);
		} else {
			navigateNotify(
				NotifyKind.OK,
				"Hoppy to git to see you!",
				"/console",
				null,
				plan ? [[PLAN_PARAM, plan]] : null,
			);
		}
	};

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
			navigate(signedInPath(undefined) ?? "/console", { replace: true });
		}
		if (!fetcher.bencher_valid || !fetcher.code) {
			return null;
		}
		const json = {
			state: fetcher.state,
			code: fetcher.code,
		} as JsonOAuth;
		return await httpPost(props.apiUrl, props.pathname, null, json)
			.then((resp) => {
				if (takeSignIn(fetcher.state)) {
					signIn(resp.data);
				} else {
					setPending(resp.data);
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
	const [_jsonAuthUser] = createResource<
		// biome-ignore lint/suspicious/noConfusingVoidType: getAuthUser navigates and returns nothing
		void | null,
		ReturnType<typeof fetcher>
	>(fetcher, getAuthUser);

	return (
		<Show when={pending()}>
			{(oauthUser) => (
				<section class="section">
					<div class="container">
						<div class="columns is-centered">
							<div class="column is-half">
								<SignInAs
									user={oauthUser().user}
									cancel={signInPath("/auth/login", oauthUser().authorization)}
									onSignIn={() => signIn(oauthUser())}
								/>
							</div>
						</div>
					</div>
				</section>
			)}
		</Show>
	);
};

export default AuthOAuth;
