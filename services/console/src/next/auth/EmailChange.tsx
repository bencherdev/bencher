import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import ButtonLink from "@bencherdev/ui/ButtonLink";
import Heading from "@bencherdev/ui/Heading";
import { Match, Show, Switch, createSignal, onMount } from "solid-js";
import type { JsonEmailChange } from "../../types/bencher";
import { type Answer, type Refused, ask } from "./request";

/** Where Open Profile goes: the reader's own Profile, or the door on the way to it. */
export const PROFILE = "/console/users/settings";

/** A secret is 32 bytes, as unpadded base64url. */
const SECRET = /^[\w-]{43}$/;

export interface EmailChangeEffects {
	read: (secret: string) => Promise<Answer<JsonEmailChange>>;
	confirm: (secret: string) => Promise<Answer<JsonEmailChange>>;
	/** Go to Profile, past a console cache that still holds the old email. */
	leave: () => void;
}

export const emailChangeEffects = (
	leave: EmailChangeEffects["leave"],
): EmailChangeEffects => ({
	read: (secret) => ask("POST", "/v0/auth/email/change", { secret }),
	confirm: (secret) => ask("POST", "/v0/auth/email/change/confirm", { secret }),
	leave,
});

type Ended = "expired" | "used" | "taken" | "locked";

type State =
	| { kind: "reading" }
	| { kind: "unread" }
	| { kind: "live"; change: JsonEmailChange; busy: boolean; failed: boolean }
	| { kind: Ended; newEmail?: string };

const BANNERS: Record<Ended, string> = {
	expired: "Expired",
	used: "Already used",
	taken: "Address taken",
	locked: "This account is locked",
};

/** Why the API refused the link, if it said. */
const endedBy = ({ status, code }: Refused): Ended | undefined => {
	switch (code) {
		case "email_change_expired":
			return "expired";
		case "email_change_used":
			return "used";
		case "locked":
			return "locked";
		default:
			return status === 409 ? "taken" : undefined;
	}
};

/**
 * An email change's link, as board 05 draws it: both addresses and one press,
 * or why the link no longer works. Nothing changes before the press.
 */
const EmailChange = (props: {
	secret: string | undefined;
	effects: EmailChangeEffects;
}) => {
	const secret = props.secret && SECRET.test(props.secret) ? props.secret : "";
	const [state, setState] = createSignal<State>(
		secret ? { kind: "reading" } : { kind: "expired" },
	);
	const live = () => {
		const now = state();
		return now.kind === "live" ? now : undefined;
	};
	const ended = () => {
		const now = state();
		return now.kind === "expired" ||
			now.kind === "used" ||
			now.kind === "taken" ||
			now.kind === "locked"
			? now
			: undefined;
	};

	const read = async () => {
		setState({ kind: "reading" });
		const answer = await props.effects.read(secret);
		if (answer.ok) {
			setState({
				kind: "live",
				change: answer.data,
				busy: false,
				failed: false,
			});
			return;
		}
		const kind = endedBy(answer.refused);
		setState(kind ? { kind } : { kind: "unread" });
	};
	onMount(() => {
		if (secret) {
			read();
		}
	});

	const confirm = async (change: JsonEmailChange) => {
		setState({ kind: "live", change, busy: true, failed: false });
		const answer = await props.effects.confirm(secret);
		if (answer.ok) {
			props.effects.leave();
			return;
		}
		const kind = endedBy(answer.refused);
		setState(
			kind
				? { kind, newEmail: change.new_email }
				: { kind: "live", change, busy: false, failed: true },
		);
	};

	return (
		<>
			<Heading level={1} size="xl">
				Change email
			</Heading>
			<Switch>
				<Match when={live()}>
					{(now) => (
						<>
							<dl>
								<div class="kv">
									<dt>from</dt>
									<dd>{now().change.email}</dd>
								</div>
								<div class="kv">
									<dt>to</dt>
									<dd>{now().change.new_email}</dd>
								</div>
							</dl>
							<Show when={now().failed}>
								<Banner status="error" role="alert">
									Not changed
								</Banner>
							</Show>
							<Button
								variant="primary"
								size="lg"
								full
								disabled={now().busy}
								onClick={() => confirm(now().change)}
							>
								Change email
							</Button>
						</>
					)}
				</Match>
				<Match when={ended()}>
					{(now) => (
						<>
							<Show when={now().kind === "taken" && now().newEmail}>
								{(newEmail) => (
									<dl>
										<div class="kv">
											<dt>to</dt>
											<dd>{newEmail()}</dd>
										</div>
									</dl>
								)}
							</Show>
							<Banner status="error" role="alert">
								{BANNERS[now().kind]}
							</Banner>
							<ButtonLink size="lg" full href={PROFILE}>
								Open Profile
							</ButtonLink>
						</>
					)}
				</Match>
				<Match when={state().kind === "unread"}>
					<Banner status="error" role="alert">
						Did not load
					</Banner>
					<Button size="lg" full onClick={read}>
						Try again
					</Button>
				</Match>
			</Switch>
		</>
	);
};

export default EmailChange;
