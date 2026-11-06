import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import { Match, Switch, createSignal, onMount } from "solid-js";
import { render } from "solid-js/web";
import { takeSignIn } from "../../components/auth/signIn";
import type { JsonSignedIn } from "../../types/bencher";
import { takeAuthReturn } from "../astro/authReturn";
import { handOff } from "../handoff";
import type { Provider } from "./Door";
import Frame, { GitHubMark, GoogleMark } from "./Frame";
import { finish } from "./land";
import { ask } from "./request";

const NAMES: Record<Provider, string> = { github: "GitHub", google: "Google" };

const boot = (mount: HTMLElement) => {
	const provider: Provider =
		mount.dataset.provider === "google" ? "google" : "github";
	const { code, state } = takeAuthReturn();
	if (!code || !state) {
		location.replace("/auth/login");
		return;
	}
	const started = takeSignIn(state);
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<Callback
						provider={provider}
						code={code}
						state={state}
						started={started}
					/>
				</Frame>
			),
			mount,
		),
	);
};

/**
 * The provider's answer, exchanged for a session: at once for the sign-in this
 * tab started, and only once asked for any other.
 */
const Callback = (props: {
	provider: Provider;
	code: string;
	state: string;
	started: boolean;
}) => {
	const [phase, setPhase] = createSignal<"asking" | "working" | "refused">(
		props.started ? "working" : "asking",
	);
	const name = NAMES[props.provider];
	const exchange = () => {
		setPhase("working");
		ask<JsonSignedIn>("POST", `/v0/auth/${props.provider}`, {
			code: props.code,
			state: props.state,
		}).then((answer) => {
			if (answer.ok) {
				finish(answer.data);
			} else {
				setPhase("refused");
			}
		});
	};
	onMount(() => {
		if (props.started) {
			exchange();
		}
	});
	const Mark = props.provider === "github" ? GitHubMark : GoogleMark;
	return (
		<>
			<Heading level={1} size="xl">
				Sign in to Bencher
			</Heading>
			<Switch>
				<Match when={phase() === "asking"}>
					<div class="au-stack">
						<Button variant="primary" size="lg" full onClick={exchange}>
							<Mark />
							Continue with {name}
						</Button>
						<ButtonLink size="lg" full href="/auth/login">
							Cancel
						</ButtonLink>
					</div>
				</Match>
				<Match when={phase() === "working"}>
					<p class="au-busy" role="status">
						<Mark />
						Signing in with {name}
					</p>
				</Match>
				<Match when={phase() === "refused"}>
					<Banner status="error" role="alert">
						Refused by {name}
					</Banner>
					<ButtonLink variant="primary" size="lg" full href="/auth/login">
						Back to sign in
					</ButtonLink>
				</Match>
			</Switch>
		</>
	);
};

const mount = document.getElementById("auth");
if (mount) {
	boot(mount);
}
