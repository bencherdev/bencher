import { type JSX, Match, Switch, createSignal } from "solid-js";
import { render } from "solid-js/web";
import type {
	JsonInvitation,
	JsonOrganization,
	JsonOrganizationName,
	JsonProject,
} from "../../types/bencher";
import { handOff } from "../handoff";
import { deleteCache } from "../idb";
import { forgetReader } from "../memory";
import { organizationPath, projectPath } from "../paths";
import { endSession } from "../session";
import { doorFailure } from "./copy";
import Frame from "./Frame";
import { readInvitation, roleName } from "./invitation";
import { CONSOLE, Press, Refused } from "./Press";
import { type Answer, type Refused as Refusal, ask } from "./request";

/**
 * A signed in reader who opens the door with an invitation or a claim is
 * asked before anything joins: an invitation for them offers Join, one for
 * another address offers Sign out, and an unclaimed organization offers
 * Claim. `false` when the reader's session has ended before a claim;
 * `signedOut` takes over the page, with the invitation already read, once the
 * reader signs out or their session is found to have ended.
 */
export const press = async (
	mount: HTMLElement,
	{
		secret,
		claim,
		signedOut,
	}: {
		secret: string | undefined;
		claim: string | undefined;
		signedOut: (read: Answer<JsonInvitation>) => void;
	},
): Promise<boolean> => {
	if (secret !== undefined) {
		const read = await readInvitation(secret);
		// Only a live session is told whom the invitation is for.
		if (read.ok && read.data.for_you === undefined) {
			forgetReader(localStorage);
			signedOut(read);
			return true;
		}
		const dispose = paint(mount, () => (
			<Invitation
				read={read}
				secret={secret}
				signedOut={() => {
					dispose();
					signedOut(read);
				}}
			/>
		));
		return true;
	}
	if (claim) {
		const [session, read] = await Promise.all([
			ask("GET", "/v0/auth/session"),
			ask<JsonOrganizationName>("GET", `/v0/auth/claim/${claim}`),
		]);
		if (!session.ok && session.refused.status === 401) {
			return false;
		}
		paint(mount, () => <Claim read={read} claim={claim} />);
		return true;
	}
	return false;
};

/** Draw `app` in the frame in place of the static door, and how to take it down. */
const paint = (mount: HTMLElement, app: () => JSX.Element) => {
	let dispose = () => {};
	handOff(mount, () => {
		dispose = render(() => <Frame>{app()}</Frame>, mount);
	});
	return () => dispose();
};

/** An invitation for the reader to join, one for another address to sign out of, or why neither. */
const Invitation = (props: {
	read: Answer<JsonInvitation>;
	secret: string;
	signedOut: () => void;
}) => {
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const invitation = () => (props.read.ok ? props.read.data : undefined);
	const join = async () => {
		setBusy(true);
		setFailure(undefined);
		const joined = await ask<JsonOrganization>(
			"POST",
			"/v0/auth/invite/accept",
			{ secret: props.secret },
		);
		if (joined.ok) {
			location.replace(organizationPath(joined.data.slug));
			return;
		}
		if (joined.refused.status === 401) {
			forgetReader(localStorage);
			props.signedOut();
			return;
		}
		setFailure(doorFailure(joined.refused).text);
		setBusy(false);
	};
	const signOut = async () => {
		setBusy(true);
		await endSession();
		forgetReader(localStorage);
		deleteCache();
		props.signedOut();
	};
	return (
		<Switch
			fallback={
				<Refused
					heading={
						doorFailure(props.read.ok ? NO_REASON : props.read.refused).text
					}
				/>
			}
		>
			<Match when={invitation()?.for_you === true && invitation()}>
				{(invited) => (
					<Press
						heading={`Join ${invited().organization.name} as ${roleName(invited().role)}`}
						press="Join"
						busy={busy()}
						failure={failure()}
						onPress={join}
					/>
				)}
			</Match>
			<Match when={invitation()?.for_you === false && invitation()}>
				{(invited) => (
					<Press
						heading={`This invite is for ${invited().email}`}
						press="Sign out"
						busy={busy()}
						failure={failure()}
						onPress={signOut}
					/>
				)}
			</Match>
		</Switch>
	);
};

/** No reason the API gave, which the door reads as a failed sign-in. */
const NO_REASON: Refusal = {
	status: undefined,
	code: undefined,
	retryAfter: undefined,
	triesLeft: undefined,
};

/** An unclaimed organization to claim, or why it cannot be. */
const Claim = (props: {
	read: Answer<JsonOrganizationName>;
	claim: string;
}) => {
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const organization = `/v0/organizations/${props.claim}`;
	const claim = async () => {
		setBusy(true);
		setFailure(undefined);
		const claimed = await ask("POST", `${organization}/claim`, {});
		if (claimed.ok) {
			// An unclaimed organization holds the one project its run made.
			const projects = await ask<JsonProject[]>(
				"GET",
				`${organization}/projects`,
			);
			const [project] = projects.ok ? projects.data : [];
			location.replace(project ? projectPath(project.slug) : CONSOLE);
			return;
		}
		if (claimed.refused.status === 401) {
			forgetReader(localStorage);
			location.reload();
			return;
		}
		setFailure(claimRefusal(claimed.refused));
		setBusy(false);
	};
	return (
		<Switch
			fallback={
				<Refused
					heading={claimRefusal(props.read.ok ? NO_REASON : props.read.refused)}
				/>
			}
		>
			<Match when={props.read.ok && props.read.data}>
				{(unclaimed) => (
					<Press
						heading={`Claim ${unclaimed().name}`}
						press="Claim"
						busy={busy()}
						failure={failure()}
						onPress={claim}
					/>
				)}
			</Match>
		</Switch>
	);
};

/** Why an organization cannot be claimed: someone has, it is gone, or the door's reason. */
const claimRefusal = (refused: Refusal) => {
	switch (refused.status) {
		case 409:
			return "This organization is already claimed";
		case 404:
			return doorFailure({ ...refused, code: "unknown_organization" }).text;
		default:
			return doorFailure(refused).text;
	}
};
