import { TURNSTILE_SITE_KEY } from "astro:env/client";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import { Match, Show, Switch, createSignal, onMount } from "solid-js";
import { render } from "solid-js/web";
import type {
	JsonSignedIn,
	JsonSignInAs,
	JsonSignInEmail,
	PlanLevel,
} from "../../types/bencher";
import { takeAuthReturn } from "../astro/authReturn";
import { handOff } from "../handoff";
import { READER_KEY, readReader } from "../reader";
import { type Failure, inboxFailure } from "./copy";
import Frame from "./Frame";
import Inbox, { type InboxEffects, type Status } from "./Inbox";
import { finish, landing } from "./land";
import { tokenEmail } from "./params";
import {
	type Carry,
	type Door,
	type Pending,
	type Who,
	doorPath,
	forgetPending,
	keepPending,
	keepPrefill,
	pendingOf,
	readPending,
} from "./pending";
import { type Answer, ask } from "./request";
import SignInAs from "./SignInAs";
import { loadTurnstile, turnstileToken } from "./turnstile";

const boot = (mount: HTMLElement) => {
	// A link pasted into this tab changes only the fragment, which loads no page.
	window.addEventListener("hashchange", () => location.reload());
	const { secret, token } = takeAuthReturn();
	const pending = readPending(sessionStorage);
	if (!secret && !token && !pending) {
		location.replace("/auth/login");
		return;
	}
	// A link's page loads no third party, and needs no Turnstile.
	if (!secret && TURNSTILE_SITE_KEY) {
		loadTurnstile().catch(() => {});
	}
	if (pending) {
		moveOnWithAnotherTab(pending.carry);
	}
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<Switch>
						<Match when={secret}>
							{(secret) => <LinkApp secret={secret()} pending={pending} />}
						</Match>
						<Match when={token}>
							{(token) => (
								<InboxApp
									email={tokenEmail(token())}
									door="login"
									carry={{}}
									status={{ kind: "expired" }}
								/>
							)}
						</Match>
						<Match when={pending}>
							{(pending) => (
								<InboxApp
									email={pending().email}
									door={pending().door}
									carry={pending().carry}
									resendAt={pending().resendAt}
									cappedUntil={pending().cappedUntil}
									who={pending().who}
								/>
							)}
						</Match>
					</Switch>
				</Frame>
			),
			mount,
		),
	);
};

/** A tab left here moves on when this browser signs in in another tab, which writes the hint. */
const moveOnWithAnotherTab = ({ authorization, plan }: Carry) =>
	window.addEventListener("storage", (event) => {
		if (
			(event.key === READER_KEY || event.key === null) &&
			readReader(localStorage)
		) {
			forgetPending(sessionStorage);
			location.replace(landing({ authorization, plan: plan as PlanLevel }));
		}
	});

/** A token for a request that may send an email, where the server asks for one. */
const verified = async (): Promise<
	{ turnstile?: string } | { refused: Answer<never> }
> => {
	if (!TURNSTILE_SITE_KEY) {
		return {};
	}
	try {
		return { turnstile: await turnstileToken(TURNSTILE_SITE_KEY) };
	} catch {
		return {
			refused: {
				ok: false,
				refused: {
					status: undefined,
					code: "turnstile_failed",
					retryAfter: undefined,
					triesLeft: undefined,
				},
			},
		};
	}
};

/** Check your inbox for the address this tab emailed, or an old link's, whose sign-in has ended. */
const InboxApp = (props: {
	email: string | undefined;
	door: Door;
	carry: Carry;
	resendAt?: number | undefined;
	cappedUntil?: number | undefined;
	status?: Status;
	who?: Who | undefined;
}) => {
	// A sign-in begun again by email carries no provider account to pin.
	const [who, setWho] = createSignal(props.who);
	const keep = (dates: Pick<Pending, "resendAt" | "cappedUntil">) => {
		if (props.email) {
			const pinning = who();
			keepPending(sessionStorage, {
				email: props.email,
				door: props.door,
				carry: props.carry,
				...dates,
				...(pinning ? { who: pinning } : {}),
			});
		}
	};
	const restart = async (): Promise<Answer<JsonSignInEmail>> => {
		const check = await verified();
		if ("refused" in check) {
			return check.refused;
		}
		return ask<JsonSignInEmail>("POST", "/v0/auth/email", {
			email: props.email,
			...props.carry,
			...check,
		});
	};
	const effects: InboxEffects = {
		prove: (code) => ask<JsonSignedIn>("POST", "/v0/auth/code", { code }),
		resend: async () => {
			const check = await verified();
			if ("refused" in check) {
				return check.refused;
			}
			const answer = await ask<JsonSignInEmail>(
				"POST",
				"/v0/auth/email/resend",
				check,
			);
			// A sign-in that ended, by its tries or its time, starts again for the same address.
			const ended = !answer.ok && answer.refused.code === "sign_in_ended";
			const sent = ended ? await restart() : answer;
			if (ended && sent.ok) {
				setWho(undefined);
			}
			if (sent.ok) {
				keep(pendingOf(sent.data, props.door, props.carry, Date.now()));
			}
			return sent;
		},
		another: () => {
			ask("DELETE", "/v0/auth/email").finally(() => {
				if (props.email) {
					keepPrefill(sessionStorage, props.email);
				}
				forgetPending(sessionStorage);
				location.assign(doorPath(props.door, props.carry));
			});
		},
		signedIn: finish,
		dated: keep,
	};
	return (
		<Inbox
			email={props.email}
			who={who()}
			resendAt={props.resendAt}
			cappedUntil={props.cappedUntil}
			status={props.status}
			effects={effects}
		/>
	);
};

type Link =
	| { kind: "redeeming" }
	| { kind: "asking"; email: string }
	| { kind: "ended" }
	| { kind: "failed"; failure: Failure };

/** A link from the email: it signs in the browser that asked, and asks first in any other. */
const LinkApp = (props: { secret: string; pending: Pending | undefined }) => {
	const [state, setState] = createSignal<Link>({ kind: "redeeming" });
	const [busy, setBusy] = createSignal(false);
	let retry = () => {};
	const redeem = (confirm: boolean) => {
		retry = () => redeem(confirm);
		setBusy(true);
		ask<JsonSignInAs>("POST", "/v0/auth/link", {
			secret: props.secret,
			confirm,
		}).then((answer) => {
			if (answer.ok) {
				if (answer.data.signed_in) {
					finish(answer.data.signed_in);
					return;
				}
				setBusy(false);
				setState({ kind: "asking", email: answer.data.email });
				return;
			}
			setBusy(false);
			setState(
				answer.refused.code === "sign_in_ended"
					? { kind: "ended" }
					: { kind: "failed", failure: inboxFailure(answer.refused) },
			);
		});
	};
	onMount(() => redeem(false));
	const asking = () => {
		const now = state();
		return now.kind === "asking" ? now.email : undefined;
	};
	const failure = () => {
		const now = state();
		return now.kind === "failed" ? now.failure : undefined;
	};
	return (
		<Switch>
			<Match when={asking()}>
				{(email) => (
					<SignInAs
						email={email()}
						busy={busy()}
						onSignIn={() => redeem(true)}
						cancel="/auth/login"
					/>
				)}
			</Match>
			<Match when={state().kind === "ended"}>
				<InboxApp
					email={props.pending?.email}
					door={props.pending?.door ?? "login"}
					carry={props.pending?.carry ?? {}}
					status={{ kind: "expired" }}
				/>
			</Match>
			<Match when={failure()}>
				{(failed) => (
					<>
						<Heading level={1} size="xl">
							Sign in to Bencher
						</Heading>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">{failed().text}</span>
							<Show when={failed().retry}>
								<Button size="sm" disabled={busy()} onClick={() => retry()}>
									Try again
								</Button>
							</Show>
						</Banner>
					</>
				)}
			</Match>
		</Switch>
	);
};

const mount = document.getElementById("auth");
if (mount) {
	boot(mount);
}
