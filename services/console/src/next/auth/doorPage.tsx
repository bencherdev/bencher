import { TURNSTILE_SITE_KEY } from "astro:env/client";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { planParam } from "../../components/auth/auth";
import { keepBack } from "../../components/auth/back";
import type {
	JsonOAuthUrl,
	JsonProject,
	JsonSignInEmail,
	PlanLevel,
} from "../../types/bencher";
import { handOff } from "../handoff";
import { forgetReader } from "../memory";
import { projectPath } from "../paths";
import { readReader } from "../reader";
import {
	type Failure,
	SIGN_IN_FAILED,
	VERIFICATION_FAILED,
	doorFailure,
} from "./copy";
import Door, { type Provider } from "./Door";
import Frame from "./Frame";
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
import { ask } from "./request";
import { loadTurnstile, turnstileToken } from "./turnstile";

const boot = async (mount: HTMLElement) => {
	const door: DoorName = mount.dataset.door === "signup" ? "signup" : "login";
	const query = new URLSearchParams(location.search);
	const carry = carryOf(query);
	keepBack(query.get("back"));
	if (readReader(localStorage)) {
		if (await signedIn(carry)) {
			return;
		}
		// The session the hint named has ended.
		forgetReader(localStorage);
	}
	if (TURNSTILE_SITE_KEY) {
		loadTurnstile().catch(() => {});
	}
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
						email={prefill ?? typed ?? mount.dataset.email}
						select={prefill !== undefined}
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
 * A signed in reader goes where a finished sign-in goes, accepting the invite
 * or claiming the organization the door's address carries; `false` when the
 * session the hint names has ended, which would otherwise send the reader
 * back here from wherever the door sends them.
 */
const signedIn = async (carry: Carry): Promise<boolean> => {
	if (carry.invite) {
		const accepted = await ask("POST", "/v0/auth/accept", {
			invite: carry.invite,
		});
		if (!accepted.ok && accepted.refused.status === 401) {
			return false;
		}
		location.replace(`/console${planParam(carry.plan as PlanLevel)}`);
		return true;
	}
	if (carry.claim) {
		const organization = `/v0/organizations/${carry.claim}`;
		const claimed = await ask("POST", `${organization}/claim`, {});
		if (!claimed.ok && claimed.refused.status === 401) {
			return false;
		}
		// An unclaimed organization holds the one project its run made.
		const projects = await ask<JsonProject[]>(
			"GET",
			`${organization}/projects`,
		);
		const [project] = projects.ok ? projects.data : [];
		location.replace(project ? projectPath(project.slug) : "/console");
		return true;
	}
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
	email: string | undefined;
	select: boolean;
}) => {
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<Failure>();
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
				...props.carry,
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
				props.carry,
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
