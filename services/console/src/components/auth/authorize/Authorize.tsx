import * as Sentry from "@sentry/astro";
import {
	For,
	Match,
	Show,
	Switch,
	createResource,
	createSignal,
	onMount,
} from "solid-js";
import type {
	JsonAuthUser,
	JsonOAuthAuthorization,
	JsonOAuthDecision,
	JsonOAuthRedirect,
} from "../../../types/bencher";
import { getUserRaw } from "../../../util/auth";
import { apiUrl } from "../../../util/http";
import { scrubber } from "../../../util/scrub";
import { signOut } from "../signOut";
import {
	type ConsentRequest,
	Credential,
	PastedKey,
	Refusal,
	answerUrl,
	consentErrorText,
	pastedKey,
	refusal,
	refusalText,
} from "./consent";
import { signInPath } from "./path";
import { scopeText } from "./scope";

export interface Props {
	apiUrl: string;
	request: ConsentRequest;
}

const Authorize = (props: Props) => {
	const request = props.request;
	// The error and trace events this page sends leave out API keys, which a user
	// may paste here, and the request's UUID, which is all it takes to answer it.
	Sentry.addEventProcessor(scrubber("uuid" in request ? [request.uuid] : []));
	if ("error" in request) {
		return <Ended text={consentErrorText(request.error)} />;
	}
	return <Consent apiUrl={props.apiUrl} uuid={request.uuid} />;
};

const Consent = (props: { apiUrl: string; uuid: string }) => {
	const url = apiUrl(props.apiUrl, `/v0/oauth/authorizations/${props.uuid}`);
	const [loaded] = createResource(() => view(url));
	const [user, setUser] = createSignal(signedInUser());
	const [ended, setEnded] = createSignal<Refusal>();
	const [leaving, setLeaving] = createSignal<string>();
	const [answering, setAnswering] = createSignal(false);
	const [notice, setNotice] = createSignal<string>();
	const [key, setKey] = createSignal("");
	let signInHeading: undefined | HTMLHeadingElement;

	const answer = async (
		destination: string,
		approve: boolean,
		credential: undefined | Credential,
		bearer: undefined | string,
	) => {
		setAnswering(true);
		setNotice(undefined);
		const result = await decide(url, approve, credential, bearer);
		if ("to" in result) {
			// A filled password field gone at navigation would prompt to save the key.
			setKey("");
			setLeaving(destination);
			window.location.replace(result.to);
			return;
		}
		setAnswering(false);
		switch (result.refusal) {
			case Refusal.Gone:
			case Refusal.Answered:
				setEnded(result.refusal);
				return;
			case Refusal.SessionEnded:
				signOut();
				setUser(undefined);
				signInHeading?.focus();
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
				void answer(destination, true, Credential.Key, pasted);
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

	const useDifferentAccount = () => {
		signOut();
		setUser(undefined);
		setNotice(undefined);
		signInHeading?.focus();
	};

	const request = (authorization: JsonOAuthAuthorization) => {
		const host = authorization.client.host;
		const destination = authorization.redirect.host;
		return (
			<div class="box">
				<h1 class="title is-3">Connect {host} to Bencher</h1>
				<p class="subtitle is-6">
					It calls itself “{authorization.client.name}”.{" "}
					<Show when={authorization.client.verified}>
						<span class="tag is-success">Verified</span>{" "}
						<small>Bencher knows this app.</small>
					</Show>
				</p>
				<Show when={user()}>
					{(user) => (
						<p>
							You are signed in as <strong>{user().user.email}</strong>.
						</p>
					)}
				</Show>
				<div class="content mt-4">
					<p>{host} will be able to:</p>
					<ul>
						<For each={authorization.scopes}>
							{(scope) => <li>{scopeText(scope)}</li>}
						</For>
					</ul>
					<p>
						After you answer, you go to <strong>{destination}</strong>.
					</p>
				</div>
				<Show when={authorization.redirect.loopback}>
					<div class="notification is-warning">
						Your answer goes to a program on this computer, not a website. Only
						approve if you just started connecting from a program you trust.
					</div>
				</Show>
				<Show when={notice()}>
					{(notice) => (
						<div class="notification is-danger" role="alert">
							{notice()}
						</div>
					)}
				</Show>
				<Show
					when={user()}
					fallback={
						<>
							<h2 class="title is-5 mb-3" tabIndex={-1} ref={signInHeading}>
								Log in to approve
							</h2>
							<div class="buttons">
								<a
									class="button is-primary"
									href={signInPath("/auth/login", props.uuid)}
								>
									Log in
								</a>
								<a class="button" href={signInPath("/auth/signup", props.uuid)}>
									Sign up
								</a>
							</div>
							<form
								class="mt-5"
								onSubmit={(event) => {
									event.preventDefault();
									approveWithKey(destination);
								}}
							>
								<div class="field">
									<label class="label" for="user-api-key">
										Or approve with a user API key
									</label>
									<div class="control">
										<input
											id="user-api-key"
											class="input"
											type="password"
											autocomplete="off"
											spellcheck={false}
											placeholder="bencher_user_"
											aria-describedby="user-api-key-help"
											value={key()}
											onInput={(event) => setKey(event.currentTarget.value)}
										/>
									</div>
									<p class="help" id="user-api-key-help">
										The key is sent once to approve and never stored. The
										connection ends when the key is revoked or expires.
									</p>
								</div>
								<button
									class="button is-primary"
									type="submit"
									disabled={answering() || !key().trim()}
								>
									Approve with key
								</button>
							</form>
							<hr />
							<button
								class="button"
								type="button"
								disabled={answering()}
								onClick={() =>
									void answer(destination, false, undefined, undefined)
								}
							>
								Deny
							</button>
						</>
					}
				>
					{(user) => (
						<>
							<div class="buttons">
								<button
									class="button is-primary"
									type="button"
									disabled={answering()}
									onClick={() =>
										void answer(
											destination,
											true,
											Credential.Session,
											user().token,
										)
									}
								>
									Approve
								</button>
								<button
									class="button"
									type="button"
									disabled={answering()}
									onClick={() =>
										void answer(destination, false, undefined, undefined)
									}
								>
									Deny
								</button>
							</div>
							<button
								class="button is-text"
								type="button"
								disabled={answering()}
								onClick={useDifferentAccount}
							>
								Use a different account
							</button>
						</>
					)}
				</Show>
			</div>
		);
	};

	return (
		<Switch fallback={<p class="has-text-centered">Loading the request</p>}>
			<Match when={leaving()}>
				{(destination) => (
					<p class="has-text-centered">Returning you to {destination()}</p>
				)}
			</Match>
			<Match when={ended()}>
				{(refusal) => <Ended text={refusalText(refusal())} focus />}
			</Match>
			<Match when={loaded()}>
				{(loaded) => {
					const result = loaded();
					return "authorization" in result ? (
						request(result.authorization)
					) : (
						<Ended text={refusalText(result.refusal)} />
					);
				}}
			</Match>
		</Switch>
	);
};

/** The page once there is nothing to answer, taking focus when it replaces the request. */
const Ended = (props: { text: string; focus?: boolean }) => {
	let heading: undefined | HTMLHeadingElement;
	onMount(() => {
		if (props.focus) {
			heading?.focus();
		}
	});
	return (
		<div class="box">
			<h1 class="title is-4" tabIndex={-1} ref={heading}>
				Connect an app to Bencher
			</h1>
			<div class="notification is-warning" role="alert">
				{props.text}
			</div>
			<p>Go back to the app and try connecting again.</p>
		</div>
	);
};

/** The user signed in to this browser, while their session lasts. */
const signedInUser = (): undefined | JsonAuthUser => {
	try {
		const user = getUserRaw();
		return user.token &&
			user.user?.email &&
			Date.parse(user.expiration) > Date.now()
			? user
			: undefined;
	} catch {
		return undefined;
	}
};

type Loaded = { authorization: JsonOAuthAuthorization } | { refusal: Refusal };
type Answer = { to: string } | { refusal: Refusal };

// No cookies, no cache, and no referrer: a request's UUID is all it takes to answer it.
const REQUEST: RequestInit = {
	credentials: "omit",
	cache: "no-store",
	referrerPolicy: "no-referrer",
};

const view = async (url: string): Promise<Loaded> => {
	try {
		const response = await fetch(url, REQUEST);
		if (!response.ok) {
			return { refusal: refusal(response.status, undefined) };
		}
		return {
			authorization: (await response.json()) as JsonOAuthAuthorization,
		};
	} catch {
		return { refusal: Refusal.Failed };
	}
};

/**
 * Send the user's answer, approving with `bearer` when given. A failure is
 * never logged or reported, since the bearer may be a pasted key.
 */
const decide = async (
	url: string,
	approve: boolean,
	credential: undefined | Credential,
	bearer: undefined | string,
): Promise<Answer> => {
	const headers: Record<string, string> = {
		"Content-Type": "application/json",
	};
	if (bearer) {
		headers.Authorization = `Bearer ${bearer}`;
	}
	const decision: JsonOAuthDecision = { approve };
	try {
		const response = await fetch(url, {
			...REQUEST,
			method: "POST",
			headers,
			body: JSON.stringify(decision),
		});
		if (!response.ok) {
			return { refusal: refusal(response.status, credential) };
		}
		const to = answerUrl(((await response.json()) as JsonOAuthRedirect).url);
		return to ? { to } : { refusal: Refusal.Failed };
	} catch {
		return { refusal: Refusal.Failed };
	}
};

export default Authorize;
