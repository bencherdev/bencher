import { Show, createMemo } from "solid-js";
import { useSearchParams } from "../../util/url";
import { CLAIM_PARAM, INVITE_PARAM, PLAN_PARAM } from "./auth";

interface Props {
	newUser: boolean;
	githubClientId: undefined | string;
	googleClientId: undefined | string;
}

const OAuthForm = (props: Props) => {
	const [searchParams, _setSearchParams] = useSearchParams();

	const githubPath = createMemo(() => {
		const path = `https://github.com/login/oauth/authorize?client_id=${props.githubClientId}`;
		return authPath(path, searchParams);
	});

	const googlePath = createMemo(() => {
		const path = `https://accounts.google.com/o/oauth2/v2/auth?client_id=${props.googleClientId}&response_type=code&scope=openid%20email%20profile&access_type=offline&prompt=consent`;
		return authPath(path, searchParams);
	});

	return (
		<>
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
			<Show when={props.googleClientId}>
				<a
					class="button is-fullwidth"
					href={googlePath()}
					style="margin-top: 1rem;"
				>
					<span class="icon">
						<i class="fab fa-google" />
					</span>
					<span>{props.newUser ? "Sign up" : "Log in"} with Google</span>
				</a>
			</Show>
		</>
	);
};

const authPath = (path: string, searchParams: Record<string, string>) => {
	let modifiedPath = path;
	const invite = searchParams[INVITE_PARAM];
	const claim = searchParams[CLAIM_PARAM];
	const plan = searchParams[PLAN_PARAM];
	if (invite) {
		modifiedPath += `&state=${invite}`;
	} else if (claim) {
		modifiedPath += `&state=${claim}`;
	} else if (plan) {
		modifiedPath += `&state=${plan}`;
	}
	return modifiedPath;
};

export default OAuthForm;
