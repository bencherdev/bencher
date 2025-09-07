import { Show, createMemo } from "solid-js";
import { httpGet } from "../../util/http";
import { useNavigate, useSearchParams } from "../../util/url";
import { CLAIM_PARAM, INVITE_PARAM, PLAN_PARAM } from "./auth";

interface Props {
	apiUrl: string;
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
				<button
					type="button"
					class="button is-fullwidth"
					style="margin-top: 1rem;"
					onMouseDown={() => googlePath(props.apiUrl, searchParams)}
				>
					<span class="icon">
						<i class="fab fa-google" />
					</span>
					<span>{props.newUser ? "Sign up" : "Log in"} with Google</span>
				</button>
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

const googlePath = async (
	apiUrl: string,
	searchParams: Record<string, string>,
) => {
	let path = "/v0/auth/google";
	const invite = searchParams[INVITE_PARAM];
	const claim = searchParams[CLAIM_PARAM];
	const plan = searchParams[PLAN_PARAM];
	if (invite) {
		path += `&invite=${invite}`;
	} else if (claim) {
		path += `&claim=${claim}`;
	} else if (plan) {
		path += `&plan=${plan}`;
	}
	await httpGet(apiUrl, path, null)
		.then((resp) => {
			const navigate = useNavigate();
			console.log(resp.data.url);
			navigate(resp.data.url);
		})
		.catch((error) => {
			console.error(error);
			return;
		});
};

export default OAuthForm;
