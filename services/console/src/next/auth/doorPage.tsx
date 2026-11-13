import { TURNSTILE_SITE_KEY } from "astro:env/client";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { keepBack } from "../../components/auth/back";
import type {
	JsonInvitation,
	JsonOAuthUrl,
	JsonSignInEmail,
	PlanLevel,
} from "../../types/bencher";
import { takeInvite } from "../astro/inviteReturn";
import { handOff } from "../handoff";
import { forgetReader } from "../memory";
import { readReader } from "../reader";
import {
	type Failure,
	SIGN_IN_FAILED,
	VERIFICATION_FAILED,
	doorFailure,
} from "./copy";
import Door, { type Provider } from "./Door";
import Frame from "./Frame";
import { readInvitation } from "./invitation";
import { landing } from "./land";
import { carryOf } from "./params";
import {
	type Carry,
	type Door as DoorName,
	keepPending,
	keepStarted,
	pendingOf,
	takePrefill,
} from "./pending";
import { type Answer, ask } from "./request";
import { loadTurnstile, turnstileToken } from "./turnstile";

const boot = async (mount: HTMLElement) => {
	// An invitation's link opened in this tab changes only the fragment, which loads no page.
	window.addEventListener("hashchange", () => location.reload());
	const door: DoorName = mount.dataset.door === "signup" ? "signup" : "login";
	const query = new URLSearchParams(location.search);
	const carry = carryOf(query);
	const invite = takeInvite();
	keepBack(query.get("back"));
	const show = (read?: Answer<JsonInvitation>) =>
		showDoor(mount, door, carry, invite, read);
	if (readReader(localStorage)) {
		if (invite !== undefined || carry.claim) {
			const { press } = await import("./signedIn");
			if (
				await press(mount, {
					secret: invite,
					claim: carry.claim,
					signedOut: show,
				})
			) {
				return;
			}
		} else if (await signedIn(carry)) {
			return;
		}
		// The session the hint named has ended.
		forgetReader(localStorage);
	}
	await show();
};

/**
 * The door, with the address of the invitation `invite` names filled in, or
 * its refusal; `read` is the invitation, if the page has read it already.
 */
const showDoor = async (
	mount: HTMLElement,
	door: DoorName,
	carry: Carry,
	invite: string | undefined,
	read?: Answer<JsonInvitation>,
) => {
	if (TURNSTILE_SITE_KEY) {
		loadTurnstile().catch(() => {});
	}
	const invitation =
		invite === undefined ? undefined : (read ?? (await readInvitation(invite)));
	const prefill = takePrefill(sessionStorage);
	// What the reader typed before this script ran stays, with the focus.
	const field = mount.querySelector<HTMLInputElement>("input[name=email]");
	const typed = field?.value || undefined;
	const typing = field !== null && document.activeElement === field;
	handOff(mount, () =>
		render(
			() => (
				<Frame>
					<DoorApp
						door={door}
						github={mount.dataset.github !== undefined}
						google={mount.dataset.google !== undefined}
						carry={carry}
						invite={invitation?.ok ? invite : undefined}
						email={
							prefill ??
							typed ??
							(invitation?.ok ? invitation.data.email : undefined)
						}
						select={prefill !== undefined}
						failure={
							invitation && !invitation.ok
								? doorFailure(invitation.refused)
								: undefined
						}
					/>
				</Frame>
			),
			mount,
		),
	);
	if (typing) {
		mount.querySelector<HTMLInputElement>("input[name=email]")?.focus();
	}
};

/**
 * A signed in reader goes where a finished sign-in goes; `false` when the
 * session the hint names has ended, which would otherwise send the reader
 * back here from wherever the door sends them.
 */
const signedIn = async (carry: Carry): Promise<boolean> => {
	const session = await ask("GET", "/v0/auth/session");
	if (!session.ok && session.refused.status === 401) {
		return false;
	}
	location.replace(
		landing({
			authorization: carry.authorization,
			plan: carry.plan as PlanLevel,
		}),
	);
	return true;
};

const DoorApp = (props: {
	door: DoorName;
	github: boolean;
	google: boolean;
	carry: Carry;
	/** The secret of the invitation the sign-in carries, which no tab keeps. */
	invite: string | undefined;
	email: string | undefined;
	select: boolean;
	failure: Failure | undefined;
}) => {
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<Failure | undefined>(
		props.failure,
	);
	const carried = () => ({
		...props.carry,
		...(props.invite ? { invite: props.invite } : {}),
	});
	let retry = () => {};
	/** Run `work`, which leaves the page or answers why it could not. */
	const attempt = (work: () => Promise<Failure | undefined>) => {
		retry = () => attempt(work);
		setBusy(true);
		setFailure(undefined);
		work().then(
			(failed) => {
				if (failed) {
					setFailure(failed);
					setBusy(false);
				}
			},
			() => {
				setFailure(SIGN_IN_FAILED);
				setBusy(false);
			},
		);
	};

	const withEmail = (email: string) =>
		attempt(async () => {
			let turnstile: string | undefined;
			if (TURNSTILE_SITE_KEY) {
				try {
					turnstile = await turnstileToken(TURNSTILE_SITE_KEY);
				} catch {
					return VERIFICATION_FAILED;
				}
			}
			const answer = await ask<JsonSignInEmail>("POST", "/v0/auth/email", {
				email,
				...carried(),
				...(turnstile ? { turnstile } : {}),
			});
			if (!answer.ok) {
				return doorFailure(answer.refused);
			}
			keepPending(
				sessionStorage,
				pendingOf(answer.data, props.door, props.carry, Date.now()),
			);
			location.assign("/auth/confirm");
			return undefined;
		});

	const withProvider = (provider: Provider) =>
		attempt(async () => {
			const answer = await ask<JsonOAuthUrl>(
				"POST",
				`/v0/auth/providers/${provider}/start`,
				carried(),
			);
			if (!answer.ok) {
				return doorFailure(answer.refused);
			}
			keepStarted(sessionStorage, { door: props.door, carry: props.carry });
			location.assign(answer.data.url);
			return undefined;
		});

	return (
		<Door
			door={props.door}
			github={props.github}
			google={props.google}
			email={props.email}
			select={props.select}
			busy={busy()}
			failure={failure()}
			onEmail={withEmail}
			onProvider={withProvider}
			onRetry={() => retry()}
		/>
	);
};

const mount = document.getElementById("auth");
if (mount) {
	boot(mount);
}
