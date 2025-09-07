import { Show } from "solid-js";
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

	return (
		<>
			<Show when={props.githubClientId}>
				<button
					type="button"
					class="button is-fullwidth"
					style="margin-top: 3rem;"
					onMouseDown={() =>
						authPath(props.apiUrl, "/v0/auth/github", searchParams)
					}
				>
					<span class="icon">
						<i class="fab fa-github" />
					</span>
					<span>{props.newUser ? "Sign up" : "Log in"} with GitHub</span>
				</button>
			</Show>
			<Show when={props.googleClientId}>
				<button
					type="button"
					class="button is-fullwidth"
					style="margin-top: 1rem;"
					onMouseDown={() =>
						authPath(props.apiUrl, "/v0/auth/google", searchParams)
					}
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

const authPath = async (
	apiUrl: string,
	pathname: string,
	searchParams: Record<string, string>,
) => {
	let path = pathname;
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
