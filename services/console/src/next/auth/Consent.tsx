import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Field from "@bencherdev/ui/Field";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import TextInput from "@bencherdev/ui/TextInput";
import {
	For,
	Match,
	Show,
	Switch,
	createResource,
	createSignal,
	onMount,
} from "solid-js";
import {
	Credential,
	PastedKey,
	Refusal,
	answerUrl,
	pastedKey,
	refusal,
	refusalText,
} from "../../components/auth/authorize/consent";
import { signInPath } from "../../components/auth/authorize/path";
import { scopeText } from "../../components/auth/authorize/scope";
import type {
	JsonOAuthAuthorization,
	JsonOAuthDecision,
	JsonOAuthRedirect,
	JsonSession,
} from "../../types/bencher";

/** What the page asks the API, so a test can stand in for it. */
export interface ConsentEffects {
	view: (uuid: string) => Promise<Loaded>;
	/** The reader this browser is signed in as, if any. */
	session: () => Promise<JsonSession | undefined>;
	decide: (
		uuid: string,
		decision: JsonOAuthDecision,
		credential: Credential | undefined,
	) => Promise<Answer>;
	/** End the session, forgetting the reader, for Use a different account. */
	signOut: () => Promise<void>;
	leave: (to: string) => void;
}

export type Loaded =
	| { authorization: JsonOAuthAuthorization }
	| { refusal: Refusal };
export type Answer = { to: string } | { refusal: Refusal };

/** An app's request to connect: approved with the session or a pasted user key, or denied. */
const Consent = (props: { uuid: string; effects: ConsentEffects }) => {
	const [loaded] = createResource(() =>
		Promise.all([
			props.effects.view(props.uuid),
			props.effects.session().catch(() => undefined),
		]),
	);
	// Signed out here, by a session that ended or Use a different account.
	const [signedOut, setSignedOut] = createSignal(false);
	const signedIn = () => (signedOut() ? undefined : loaded()?.[1]?.user.email);
	const [ended, setEnded] = createSignal<Refusal>();
	const [leaving, setLeaving] = createSignal<string>();
	const [answering, setAnswering] = createSignal(false);
	const [notice, setNotice] = createSignal<string>();
	const [key, setKey] = createSignal("");
	let signInHeading: HTMLHeadingElement | undefined;

	const answer = async (
		destination: string,
		decision: JsonOAuthDecision,
		credential: Credential | undefined,
	) => {
		setAnswering(true);
		setNotice(undefined);
		const result = await props.effects.decide(props.uuid, decision, credential);
		if ("to" in result) {
			// A filled password field gone at navigation would prompt to save the key.
			setKey("");
			setLeaving(destination);
			props.effects.leave(result.to);
			return;
		}
		setAnswering(false);
		switch (result.refusal) {
			case Refusal.Gone:
			case Refusal.Answered:
				setEnded(result.refusal);
				return;
			case Refusal.SessionEnded:
				setSignedOut(true);
				queueMicrotask(() => signInHeading?.focus());
				break;
			case Refusal.KeyRefused:
				setKey("");
				break;
		}
		setNotice(refusalText(result.refusal));
	};

	const approveWithKey = (destination: string) => {
		const pasted = key().trim();
		switch (pastedKey(pasted)) {
			case PastedKey.User:
				void answer(
					destination,
					{ approve: true, key: pasted },
					Credential.Key,
				);
				return;
			case PastedKey.Project:
				setNotice(
					"A project API key (bencher_run_) cannot approve a request to connect. Use a user API key (bencher_user_) instead.",
				);
				return;
			case PastedKey.Other:
				setNotice("Paste a user API key. It starts with bencher_user_");
				return;
		}
	};

	const useDifferentAccount = async () => {
		setAnswering(true);
		await props.effects.signOut();
		setAnswering(false);
		setSignedOut(true);
		setNotice(undefined);
		queueMicrotask(() => signInHeading?.focus());
	};

	const request = (authorization: JsonOAuthAuthorization) => {
		const destination = authorization.redirect.host;
		const deny = () => void answer(destination, { approve: false }, undefined);
		return (
			<>
				<Heading level={1} size="xl">
					Connect <Host host={authorization.client.host} /> to Bencher
				</Heading>
				{/* The design system's token, by its classes: importing `Token` would split it out of the chunk Connections loads. */}
				<Show when={authorization.client.verified}>
					<span class="ui-token au-who" data-tone="green">
						<Icon name="check" />
						Verified
					</span>
				</Show>
				<dl>
					<div class="kv">
						<dt>calls itself</dt>
						<dd>{authorization.client.name}</dd>
					</div>
					<div class="kv">
						<dt>can</dt>
						<dd>
							<ul>
								<For each={authorization.scopes}>
									{(scope) => <li>{scopeText(scope)}</li>}
								</For>
							</ul>
						</dd>
					</div>
					<div class="kv">
						<dt>returns to</dt>
						<dd>
							<span class="mono">
								<Host host={destination} />
							</span>
						</dd>
					</div>
					<Show when={signedIn()}>
						{(email) => (
							<div class="kv">
								<dt>signed in as</dt>
								<dd>{email()}</dd>
							</div>
						)}
					</Show>
				</dl>
				<Show when={notice()}>
					{(text) => (
						<Banner status="error" role="alert">
							{text()}
						</Banner>
					)}
				</Show>
				<Show
					when={signedIn()}
					fallback={
						<>
							<Heading level={2} size="lg" tabIndex={-1} ref={signInHeading}>
								Sign in to approve
							</Heading>
							<div class="au-row">
								<ButtonLink
									variant="primary"
									size="lg"
									href={signInPath("/auth/login", props.uuid)}
								>
									Sign in
								</ButtonLink>
								<ButtonLink
									size="lg"
									href={signInPath("/auth/signup", props.uuid)}
								>
									Sign up
								</ButtonLink>
							</div>
							<form
								class="au-stack"
								onSubmit={(event) => {
									event.preventDefault();
									approveWithKey(destination);
								}}
							>
								<Field
									label="Or approve with a user API key"
									for="user-api-key"
								>
									<TextInput
										id="user-api-key"
										type="password"
										size="lg"
										autocomplete="off"
										spellcheck={false}
										placeholder="bencher_user_"
										value={key()}
										onInput={(event) => setKey(event.currentTarget.value)}
									/>
								</Field>
								<Button
									type="submit"
									size="lg"
									full
									disabled={answering() || !key().trim()}
								>
									Approve with key
								</Button>
							</form>
							<Button size="lg" full disabled={answering()} onClick={deny}>
								Deny
							</Button>
						</>
					}
				>
					<div class="au-row">
						<Button size="lg" disabled={answering()} onClick={deny}>
							Deny
						</Button>
						<Button
							variant="primary"
							size="lg"
							disabled={answering()}
							onClick={() =>
								void answer(destination, { approve: true }, Credential.Session)
							}
						>
							Approve
						</Button>
					</div>
					<div>
						<button
							type="button"
							class="lnk"
							disabled={answering()}
							onClick={() => void useDifferentAccount()}
						>
							Use a different account
						</button>
					</div>
				</Show>
			</>
		);
	};

	return (
		<Switch>
			<Match when={leaving()}>
				{(destination) => (
					<p class="au-busy" role="status">
						Returning you to {destination()}
					</p>
				)}
			</Match>
			<Match when={ended()}>
				{(refused) => <Ended text={refusalText(refused())} focus />}
			</Match>
			<Match when={loaded()}>
				{(loaded) => {
					const [view] = loaded();
					return "authorization" in view ? (
						request(view.authorization)
					) : (
						<Ended text={refusalText(view.refusal)} />
					);
				}}
			</Match>
		</Switch>
	);
};

export default Consent;

/** The page once there is nothing to answer, taking focus when it replaces the request. */
export const Ended = (props: { text: string; focus?: boolean }) => {
	let heading: HTMLHeadingElement | undefined;
	onMount(() => {
		if (props.focus) {
			heading?.focus();
		}
	});
	return (
		<>
			<Heading level={1} size="xl" tabIndex={-1} ref={heading}>
				Connect an app to Bencher
			</Heading>
			<Banner status="warning" role="alert">
				{props.text}
			</Banner>
			<p>Go back to the app and try connecting again.</p>
		</>
	);
};

/** A host that breaks only at its dots. */
const Host = (props: { host: string }) => (
	<span class="au-host">
		<For each={props.host.split(".")}>
			{(part, index) => (
				<>
					{index() > 0 ? (
						<>
							<wbr />.
						</>
					) : null}
					{part}
				</>
			)}
		</For>
	</span>
);

/**
 * Ask the API through the relay, reading every answer: an unread `no-store`
 * body holds its connection. A session the API answers 401 for is `forget`ten.
 */
export const consentEffects = (
	signOut: () => Promise<void>,
	forget: () => void,
	fetch = globalThis.fetch.bind(globalThis),
): ConsentEffects => {
	const path = (uuid: string) => `/api/v0/oauth/authorizations/${uuid}`;
	return {
		view: async (uuid) => {
			try {
				const response = await fetch(path(uuid), REQUEST);
				const text = await response.text();
				return response.ok
					? { authorization: JSON.parse(text) as JsonOAuthAuthorization }
					: { refusal: refusal(response.status, undefined) };
			} catch {
				return { refusal: Refusal.Failed };
			}
		},
		session: async () => {
			try {
				const response = await fetch("/api/v0/auth/session", {
					cache: "no-store",
				});
				const text = await response.text();
				if (response.status === 401) {
					forget();
				}
				return response.ok ? (JSON.parse(text) as JsonSession) : undefined;
			} catch {
				return undefined;
			}
		},
		decide: async (uuid, decision, credential) => {
			try {
				const response = await fetch(path(uuid), {
					...REQUEST,
					// Only the session's approval rides on its cookie.
					credentials:
						credential === Credential.Session ? "same-origin" : "omit",
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify(decision),
				});
				const text = await response.text();
				if (!response.ok) {
					const refused = refusal(response.status, credential);
					if (refused === Refusal.SessionEnded) {
						forget();
					}
					return { refusal: refused };
				}
				const to = answerUrl((JSON.parse(text) as JsonOAuthRedirect).url);
				return to ? { to } : { refusal: Refusal.Failed };
			} catch {
				return { refusal: Refusal.Failed };
			}
		},
		signOut,
		leave: (to) => window.location.replace(to),
	};
};

// No cache and no referrer: a request's UUID is all it takes to answer it.
const REQUEST: RequestInit = {
	credentials: "omit",
	cache: "no-store",
	referrerPolicy: "no-referrer",
};
