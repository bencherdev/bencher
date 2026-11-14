import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import {
	For,
	Match,
	Show,
	Switch,
	createSignal,
	onCleanup,
	onMount,
} from "solid-js";
import type {
	JsonFreshSignIn,
	JsonProved,
	JsonSignInEmail,
} from "../../types/bencher";
import { type Failure, SIGN_IN_FAILED, inboxFailure } from "../auth/copy";
import type { Provider } from "../auth/Door";
import { GitHubMark, GoogleMark } from "../auth/Frame";
import { CodeStep, type Status } from "../auth/Inbox";
import type { Answer } from "../auth/request";

/** What the dialog does for the reader, so a test can stand in for the API and the browser. */
export interface FreshEffects {
	/** The ways the reader can sign in again. */
	methods: () => Promise<Answer<JsonFreshSignIn>>;
	/** Email the reader a code and a link. */
	email: () => Promise<Answer<JsonSignInEmail>>;
	resend: () => Promise<Answer<JsonSignInEmail>>;
	prove: (code: string) => Promise<Answer<JsonProved>>;
	/** Sign in again with `provider` in a window of its own, opened from the reader's press. */
	provider: (provider: Provider) => Promise<Failure | undefined>;
	/** Call `listener` once another tab proves this session again; returns its end. */
	onProved: (listener: () => void) => () => void;
}

const NAMES: Record<Provider, string> = { github: "GitHub", google: "Google" };

type Step =
	| { kind: "methods" }
	| {
			kind: "code";
			email: string;
			resendAt?: number;
			cappedUntil?: number;
			status?: Status;
	  };

/**
 * Sign in again, over the page: each provider pinned to the reader, and an
 * email with a code, whose six boxes go back to the ways to sign in again. A
 * proof here or in another tab ends it with `true`, and Cancel with `false`.
 */
const FreshDialog = (props: {
	effects: FreshEffects;
	onDone: (proved: boolean) => void;
}) => {
	const [providers, setProviders] = createSignal<readonly Provider[]>();
	const [step, setStep] = createSignal<Step>({ kind: "methods" });
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<Failure>();
	let retry = () => {};
	onMount(() => {
		props.effects.methods().then((answer) => {
			// A server built without the providers answers none.
			const pinned: readonly string[] = answer.ok
				? (answer.data.providers ?? [])
				: [];
			setProviders(pinned.filter(isProvider));
		});
	});
	onCleanup(props.effects.onProved(() => props.onDone(true)));

	/** Run `work`, which answers why it could not, or nothing once it has moved on. */
	const attempt = (work: () => Promise<Failure | undefined>) => {
		retry = () => attempt(work);
		setBusy(true);
		setFailure(undefined);
		work().then(
			(failed) => {
				setFailure(failed);
				setBusy(false);
			},
			() => {
				setFailure(SIGN_IN_FAILED);
				setBusy(false);
			},
		);
	};

	const emailed = (answer: Answer<JsonSignInEmail>, now = Date.now()) => {
		if (!answer.ok) {
			return inboxFailure(answer.refused);
		}
		const { email, resend_after, wait } = answer.data;
		setStep({
			kind: "code",
			email,
			...(resend_after === undefined
				? {}
				: { resendAt: now + resend_after * 1_000 }),
			...(wait === undefined
				? {}
				: {
						cappedUntil: now + wait * 1_000,
						status: { kind: "capped" } as const,
					}),
		});
		return undefined;
	};

	const code = () => {
		const now = step();
		return now.kind === "code" ? now : undefined;
	};

	return (
		<Dialog
			aria-labelledby="fresh-title"
			class="au-fresh"
			onDismiss={() => props.onDone(false)}
		>
			<div class="ui-dialog-head">
				<h2 id="fresh-title">Sign in again</h2>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Close"
					onClick={() => props.onDone(false)}
				>
					<Icon name="close" />
				</Button>
			</div>
			<div class="ui-dialog-body">
				<Switch>
					<Match when={code()} keyed>
						{(shown) => (
							<CodeStep
								email={shown.email}
								resendAt={shown.resendAt}
								cappedUntil={shown.cappedUntil}
								status={shown.status}
								effects={{
									prove: (entered) => props.effects.prove(entered),
									resend: async () => {
										const answer = await props.effects.resend();
										// One that ended, by its tries or its time, starts again.
										return !answer.ok && answer.refused.code === "sign_in_ended"
											? props.effects.email()
											: answer;
									},
									proved: () => props.onDone(true),
								}}
								leave="Back"
								onLeave={() => setStep({ kind: "methods" })}
							/>
						)}
					</Match>
					<Match when={providers()}>
						{(pinned) => (
							<div class="au-stack">
								<For each={pinned()}>
									{(provider) => (
										<Button
											size="lg"
											full
											disabled={busy()}
											onClick={() =>
												attempt(() => props.effects.provider(provider))
											}
										>
											{provider === "github" ? <GitHubMark /> : <GoogleMark />}
											Continue with {NAMES[provider]}
										</Button>
									)}
								</For>
								<Button
									size="lg"
									full
									disabled={busy()}
									onClick={() =>
										attempt(async () => emailed(await props.effects.email()))
									}
								>
									Email a code
								</Button>
								<Show when={failure()}>
									{(failed) => (
										<Banner status="error" role="alert" class="load-error">
											<span class="grow">{failed().text}</span>
											<Show when={failed().retry}>
												<Button
													size="sm"
													disabled={busy()}
													onClick={() => retry()}
												>
													Try again
												</Button>
											</Show>
										</Banner>
									)}
								</Show>
							</div>
						)}
					</Match>
				</Switch>
			</div>
			<div class="ui-dialog-foot">
				<Button onClick={() => props.onDone(false)}>Cancel</Button>
			</div>
		</Dialog>
	);
};

export default FreshDialog;

const isProvider = (name: string): name is Provider =>
	name === "github" || name === "google";
