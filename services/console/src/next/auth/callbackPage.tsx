import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import { Match, Show, Switch, createSignal, onMount } from "solid-js";
import { render } from "solid-js/web";
import type { JsonProviderSignIn } from "../../types/bencher";
import { takeAuthReturn } from "../astro/authReturn";
import { handOff } from "../handoff";
import { SIGN_IN_FAILED, providerFailure, refusedBy } from "./copy";
import type { Provider } from "./Door";
import Frame, { GitHubMark, GoogleMark } from "./Frame";
import { finish } from "./land";
import {
	type Started,
	doorPath,
	keepPending,
	pendingOf,
	takeStarted,
} from "./pending";
import { signalProved, takeFreshWindow } from "./proved";
import { ask } from "./request";

const NAMES: Record<Provider, string> = { github: "GitHub", google: "Google" };

const boot = (mount: HTMLElement) => {
	const provider: Provider =
		mount.dataset.provider === "google" ? "google" : "github";
	const { code, state } = takeAuthReturn();
	if (!state) {
		location.replace("/auth/login");
		return;
	}
	const started = takeStarted(sessionStorage);
	const fresh = takeFreshWindow(sessionStorage);
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<Callback
						provider={provider}
						code={code}
						state={state}
						started={started}
						fresh={fresh}
					/>
				</Frame>
			),
			mount,
		),
	);
};

/**
 * The provider's answer, finished in the browser that started the sign-in:
 * signed in, or on to Check your inbox for an account that proves its address
 * first. A provider that sends no code back refused the sign-in itself. In a
 * window opened to sign in again, a proof tells the dialog that asked, and the
 * window closes.
 */
const Callback = (props: {
	provider: Provider;
	code: string | undefined;
	state: string;
	started: Started;
	/** Opened by a dialog to sign in again. */
	fresh: boolean;
}) => {
	const name = NAMES[props.provider];
	const [refused, setRefused] = createSignal<string | undefined>(
		props.code ? undefined : refusedBy(name),
	);
	const [proved, setProved] = createSignal(false);
	onMount(() => {
		const { code, provider, started, state } = props;
		if (!code) {
			return;
		}
		ask<JsonProviderSignIn>("POST", `/v0/auth/providers/${provider}/finish`, {
			code,
			state,
		}).then((answer) => {
			if (!answer.ok) {
				setRefused(providerFailure(answer.refused));
				return;
			}
			const { signed_in, inbox } = answer.data;
			if (answer.data.proved) {
				signalProved(localStorage);
				setProved(true);
				window.close();
			} else if (signed_in) {
				finish(signed_in);
			} else if (inbox) {
				keepPending(sessionStorage, {
					...pendingOf(inbox, started.door, started.carry, Date.now()),
					who: { provider, handle: inbox.handle },
				});
				location.replace("/auth/confirm");
			} else {
				setRefused(SIGN_IN_FAILED.text);
			}
		});
	});
	const Mark = props.provider === "github" ? GitHubMark : GoogleMark;
	return (
		<>
			<Heading level={1} size="xl">
				{props.fresh ? "Sign in again" : "Sign in to Bencher"}
			</Heading>
			<Switch
				fallback={
					<p class="au-busy" role="status">
						<Mark />
						Signing in with {name}
					</p>
				}
			>
				<Match when={proved()}>
					<p class="au-busy" role="status">
						<Icon name="check" />
						Signed in again
					</p>
				</Match>
				<Match when={refused()}>
					{(text) => (
						<>
							<Banner status="error" role="alert">
								{text()}
							</Banner>
							<Show
								when={props.fresh}
								fallback={
									<ButtonLink
										variant="primary"
										size="lg"
										full
										href={doorPath(props.started.door, props.started.carry)}
									>
										Back to sign in
									</ButtonLink>
								}
							>
								<Button
									variant="primary"
									size="lg"
									full
									onClick={() => window.close()}
								>
									Close
								</Button>
							</Show>
						</>
					)}
				</Match>
			</Switch>
		</>
	);
};

const mount = document.getElementById("auth");
if (mount) {
	boot(mount);
}
