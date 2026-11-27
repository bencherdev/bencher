import Banner from "@bencherdev/ui/Banner";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import { Show, createSignal, onMount } from "solid-js";
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
 * The provider's answer, finished in the browser that started the sign-in:
 * signed in, or on to Check your inbox for an account that proves its address
 * first. A provider that sends no code back refused the sign-in itself.
 */
const Callback = (props: {
	provider: Provider;
	code: string | undefined;
	state: string;
	started: Started;
}) => {
	const name = NAMES[props.provider];
	const [refused, setRefused] = createSignal<string | undefined>(
		props.code ? undefined : refusedBy(name),
	);
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
			if (signed_in) {
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
				Sign in to Bencher
			</Heading>
			<Show
				when={refused()}
				fallback={
					<p class="au-busy" role="status">
						<Mark />
						Signing in with {name}
					</p>
				}
			>
				{(text) => (
					<>
						<Banner status="error" role="alert">
							{text()}
						</Banner>
						<ButtonLink
							variant="primary"
							size="lg"
							full
							href={doorPath(props.started.door, props.started.carry)}
						>
							Back to sign in
						</ButtonLink>
					</>
				)}
			</Show>
		</>
	);
};

const mount = document.getElementById("auth");
if (mount) {
	boot(mount);
}
