import { Show, createMemo } from "solid-js";
import { useSearchParams } from "../../util/url";
import { CLAIM_PARAM, INVITE_PARAM, PLAN_PARAM } from "./auth";

interface Props {
	newUser: boolean;
	githubClientId: undefined | string;
}

const OAuthForm = (props: Props) => {
	const [searchParams, _setSearchParams] = useSearchParams();

	const githubPath = createMemo(() => {
		let path = `https://github.com/login/oauth/authorize?client_id=${props.githubClientId}`;
		const invite = searchParams[INVITE_PARAM];
		const claim = searchParams[CLAIM_PARAM];
		const plan = searchParams[PLAN_PARAM];
		if (invite) {
			path += `&state=${invite}`;
		} else if (claim) {
			path += `&state=${claim}`;
		} else if (plan) {
			path += `&state=${plan}`;
		}
		return path;
	});

	return (
		<Show when={props.githubClientId}>
			<a
				class="button is-fullwidth"
				href={githubPath()}
				style="margin-top: 3rem;"
			>
				<span class="icon">
					<i class="fab fa-github" />
				</span>
				<span>{props.newUser ? "Sign up" : "Log in"} with GitHub</span>
			</a>
		</Show>
	);
};

export default OAuthForm;
