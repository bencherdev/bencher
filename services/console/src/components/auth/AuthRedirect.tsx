import { Show } from "solid-js";
import { authUser } from "../../util/auth";
import { hiddenRedirect, useSearchParams } from "../../util/url";
import Redirect from "../site/Redirect";
import { CLAIM_PARAM, INVITE_PARAM } from "./auth";
import { AUTHORIZATION_PARAM, consentPath } from "./authorize/path";

/**
 * A signed in reader goes on to `path`, or to an app's pending consent, unless
 * the page carries an invitation, in its fragment, or a claim.
 */
const AuthRedirect = (props: { path: string }) => {
	const [searchParams, _setSearchParams] = useSearchParams();
	const invited = new URLSearchParams(location.hash.slice(1)).has(INVITE_PARAM);
	return (
		<Show when={authUser() && !invited && !searchParams[CLAIM_PARAM]}>
			<Show
				when={consentPath(searchParams[AUTHORIZATION_PARAM])}
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
