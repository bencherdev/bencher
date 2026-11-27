import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import {
	Match,
	Show,
	Switch,
	createMemo,
	createSignal,
	onCleanup,
	onMount,
} from "solid-js";
import type { JsonSignedIn, JsonSignInEmail } from "../../types/bencher";
import Code, { emptyCode } from "./Code";
import {
	CODES_PAUSED,
	EXPIRED,
	type Failure,
	NO_TRIES_LEFT,
	WRONG_CODE,
	capped,
	inboxFailure,
	resendIn,
} from "./copy";
import { GitHubMark, GoogleMark } from "./Frame";
import type { Who } from "./pending";
import type { Answer } from "./request";

/** What Check your inbox shows under the code. */
export type Status =
	| { kind: "entering" }
	| { kind: "wrong" }
	| { kind: "no_tries" }
	| { kind: "expired" }
	| { kind: "paused" }
	| { kind: "capped" }
	| { kind: "failed"; failure: Failure };

/** What the page does for the reader, so a test can stand in for the API. */
export interface InboxEffects {
	prove: (code: string) => Promise<Answer<JsonSignedIn>>;
	resend: () => Promise<Answer<JsonSignInEmail>>;
	another: () => void;
	signedIn: (signedIn: JsonSignedIn) => void;
	/** Each time the page dates Resend or the cap, so the open tab reads the same after a reload. */
	dated?: (dates: { resendAt?: number; cappedUntil?: number }) => void;
}

export interface InboxProps {
	/** Unknown for a link opened in a tab that did not ask for it. */
	email?: string | undefined;
	/** The provider account that the proof pins, above the heading so the reader sees which asks. */
	who?: Who | undefined;
	resendAt?: number | undefined;
	cappedUntil?: number | undefined;
	status?: Status | undefined;
	effects: InboxEffects;
	now?: () => number;
}

/** The address, six code boxes, Resend, and Use another email. */
const Inbox = (props: InboxProps) => {
	const now = props.now ?? Date.now;
	const [clock, setClock] = createSignal(now());
	const tick = setInterval(() => setClock(now()), 1_000);
	onCleanup(() => clearInterval(tick));

	const [digits, setDigits] = createSignal(emptyCode());
	const [resendAt, setResendAt] = createSignal(props.resendAt);
	const [cappedUntil, setCappedUntil] = createSignal(props.cappedUntil);
	// Until the refusal's `Retry-After` passes, or for good when it gave none.
	const [pausedUntil, setPausedUntil] = createSignal<number>();
	const [status, setStatus] = createSignal<Status>(
		props.status ??
			((props.cappedUntil ?? 0) > now()
				? { kind: "capped" }
				: { kind: "entering" }),
	);
	const [sent, setSent] = createSignal(false);
	const [busy, setBusy] = createSignal(false);
	let retry = () => {};

	const failure = createMemo(() => {
		const shown = status();
		return shown.kind === "failed" ? shown.failure : undefined;
	});
	const waitFor = (until: number | undefined) =>
		until === undefined ? 0 : Math.ceil((until - clock()) / 1_000);
	const paused = () =>
		status().kind === "paused" &&
		(pausedUntil() === undefined || waitFor(pausedUntil()) > 0);
	const locked = createMemo(() => {
		const kind = status().kind;
		return kind === "no_tries" || kind === "expired" || paused();
	});
	const resendWait = () => waitFor(resendAt());
	const capWait = () => waitFor(cappedUntil());
	const resendOpen = () =>
		!busy() && props.email !== undefined && resendWait() <= 0 && capWait() <= 0;

	let group: HTMLDivElement | undefined;
	const focusFirst = () => {
		const first = group?.querySelector<HTMLInputElement>(
			"input:not(:disabled)",
		);
		first?.focus();
	};
	onMount(focusFirst);

	const prove = (code: string) => {
		retry = () => prove(code);
		setBusy(true);
		setSent(false);
		props.effects.prove(code).then((answer) => {
			if (answer.ok) {
				props.effects.signedIn(answer.data);
				return;
			}
			setBusy(false);
			const { refused } = answer;
			switch (refused.code) {
				case "wrong_code":
					setStatus(
						refused.triesLeft === 0 ? { kind: "no_tries" } : { kind: "wrong" },
					);
					break;
				case "no_tries_left":
					setStatus({ kind: "no_tries" });
					break;
				case "sign_in_ended":
					setStatus({ kind: "expired" });
					break;
				case "codes_paused":
					setPausedUntil(
						refused.retryAfter === undefined
							? undefined
							: now() + refused.retryAfter * 1_000,
					);
					setStatus({ kind: "paused" });
					break;
				default:
					setStatus({ kind: "failed", failure: inboxFailure(refused) });
					return;
			}
			if (status().kind === "wrong") {
				focusFirst();
			} else {
				setDigits(emptyCode());
			}
		});
	};

	const resend = () => {
		retry = resend;
		setBusy(true);
		props.effects.resend().then((answer) => {
			setBusy(false);
			if (answer.ok) {
				const at = now();
				setClock(at);
				if (answer.data.wait !== undefined) {
					const until = at + answer.data.wait * 1_000;
					setCappedUntil(until);
					props.effects.dated?.({ cappedUntil: until });
					if (!paused()) {
						setStatus({ kind: "capped" });
					}
					return;
				}
				const opens =
					answer.data.resend_after === undefined
						? undefined
						: at + answer.data.resend_after * 1_000;
				setResendAt(opens);
				props.effects.dated?.({ resendAt: opens });
				setSent(true);
				// A new email does not resume codes the address has paused.
				if (paused()) {
					return;
				}
				setDigits(emptyCode());
				setStatus({ kind: "entering" });
				queueMicrotask(focusFirst);
				return;
			}
			const { refused } = answer;
			if (
				refused.code === "resend_waiting" &&
				refused.retryAfter !== undefined
			) {
				const opens = now() + refused.retryAfter * 1_000;
				setResendAt(opens);
				props.effects.dated?.({ resendAt: opens });
				return;
			}
			setStatus({ kind: "failed", failure: inboxFailure(refused) });
		});
	};

	const resendLabel = () =>
		resendWait() > 0 ? resendIn(resendWait()) : "Resend";
	const primaryResend = () =>
		props.email !== undefined &&
		(status().kind === "no_tries" || status().kind === "expired");

	return (
		<>
			{/* The design system's token, by its classes, as consent draws one. */}
			<Show when={props.who}>
				{(who) => (
					<span class="ui-token au-who">
						{who().provider === "github" ? <GitHubMark /> : <GoogleMark />}
						{who().handle}
					</span>
				)}
			</Show>
			<Heading level={1} size="xl">
				Check your inbox
			</Heading>
			<Show when={props.email}>
				{(email) => <p class="au-addr">{email()}</p>}
			</Show>
			<div ref={group}>
				<Code
					digits={digits()}
					disabled={locked() || busy()}
					invalid={status().kind === "wrong"}
					onDigits={(next) => {
						setDigits(next);
						if (status().kind === "wrong" || status().kind === "failed") {
							setStatus({ kind: "entering" });
						}
					}}
					onCode={prove}
				/>
			</div>
			<Switch>
				<Match when={status().kind === "wrong"}>
					<p class="ferror" role="alert">
						{WRONG_CODE}
					</p>
				</Match>
				<Match when={status().kind === "no_tries"}>
					<p class="ferror" role="alert">
						{NO_TRIES_LEFT}
					</p>
				</Match>
				<Match when={status().kind === "expired"}>
					<p class="ferror" role="alert">
						{EXPIRED}
					</p>
				</Match>
				<Match when={paused()}>
					<p class="ferror" role="alert">
						{CODES_PAUSED}
					</p>
				</Match>
				<Match when={status().kind === "capped" && capWait() > 0}>
					<p class="ferror" role="alert">
						{capped(capWait())}
					</p>
				</Match>
				<Match when={failure()}>
					{(failed) => (
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">{failed().text}</span>
							<Show when={failed().retry}>
								<Button size="sm" disabled={busy()} onClick={() => retry()}>
									Try again
								</Button>
							</Show>
						</Banner>
					)}
				</Match>
				<Match when={sent()}>
					<span class="set-status" role="status">
						<Icon name="check" />
						Sent
					</span>
				</Match>
			</Switch>
			<Show when={primaryResend()}>
				<Button
					variant="primary"
					size="lg"
					full
					disabled={!resendOpen()}
					onClick={resend}
				>
					{resendLabel()}
				</Button>
			</Show>
			<div class="au-links">
				<Show when={!primaryResend() && props.email !== undefined}>
					<button
						type="button"
						class="lnk"
						disabled={!resendOpen()}
						onClick={resend}
					>
						{resendLabel()}
					</button>
				</Show>
				<button
					type="button"
					class="lnk"
					onClick={() => props.effects.another()}
				>
					Use another email
				</button>
			</div>
		</>
	);
};

export default Inbox;
