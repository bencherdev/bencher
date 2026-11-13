import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import TextInput from "@bencherdev/ui/TextInput";
import { useNavigate, useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createEffect, createMemo, createSignal, on } from "solid-js";
import type { JsonUser } from "../../types/bencher";
import { useAccount } from "../account/context";
import Frame from "../account/Frame";
import {
	type Draft,
	type Field,
	draftOf,
	failureOf,
	legacyTokens,
	legacyTokensQuery,
	patchOf,
	problemsOf,
} from "../account/profile";
import { accountPath } from "../account/paths";
import { userQuery } from "../queries";
import { useQueryResult } from "../query";

const FIELDS: readonly Field[] = ["name", "slug", "email"];

/**
 * The reader's name, slug, and email, and a link to any legacy tokens; to a
 * server admin, any user's, with their Admin and Locked switches.
 */
const Profile = () => {
	const { api, reader, keepUser } = useAccount();
	const params = useParams();
	const slug = () => params.user ?? "";
	const user = useQueryResult(() => userQuery(api, slug()));
	const tokens = useQueryResult(() => legacyTokensQuery(api, slug()));
	// The reader's own account says whether they are a server admin, which the hint follows.
	createEffect(() => {
		const own = user().data;
		if (own && own.uuid === reader().uuid && own.admin !== !!reader().admin) {
			keepUser(own);
		}
	});
	return (
		<Frame user={slug()} section="profile">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Account<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						Profile
					</Heading>
				</div>
			</div>
			<Show
				when={user().data}
				fallback={
					<Show when={user().error} fallback={<Skeleton size="card" />}>
						<Failed what="Profile" retry={() => user().refetch()} />
					</Show>
				}
			>
				{(data) => (
					<>
						<ProfileForm user={data()} tokens={tokens().data} />
						{/* Only a server admin reads another user, and the reader's own account says whether they are one. */}
						<Show when={data().uuid !== reader().uuid || data().admin}>
							<ServerCard user={data()} />
						</Show>
					</>
				)}
			</Show>
			<Show when={tokens().error}>
				<Failed what="Legacy tokens" retry={() => tokens().refetch()} />
			</Show>
			<Show when={tokens().data}>
				{(count) => (
					<p class="notice">
						<Icon name="key" />
						<a href={accountPath(slug(), "tokens")}>{legacyTokens(count())}</a>
					</p>
				)}
			</Show>
		</Frame>
	);
};

export default Profile;

// Account pages share Profile's chunk rather than splitting a module each into its own.
export { default as Frame } from "../account/Frame";
export { failureOf, legacyTokensQuery } from "../account/profile";

const Failed = (props: { what: string; retry: () => void }) => (
	<Banner status="error" role="alert" class="load-error">
		<span class="grow">{props.what} did not load</span>
		<Button size="sm" onClick={props.retry}>
			Retry
		</Button>
	</Banner>
);

const ProfileForm = (props: {
	user: JsonUser;
	/** Unknown until the count arrives, which an email change waits for. */
	tokens: number | undefined;
}) => {
	const { api, reader, keepUser } = useAccount();
	const client = useQueryClient();
	const navigate = useNavigate();

	const saved = createMemo(() => draftOf(props.user), undefined, {
		equals: (a, b) => patchOf(a, b) === undefined,
	});
	const [draft, setDraft] = createSignal<Draft>(saved());
	const [failure, setFailure] = createSignal<string>();
	const [justSaved, setJustSaved] = createSignal(false);
	/** A confirm link went out for an email change that waits on it. */
	const [linkSent, setLinkSent] = createSignal(false);
	const [confirming, setConfirming] = createSignal(false);
	const [saving, setSaving] = createSignal(false);
	/** The reader's own account, as opposed to another user's a server admin edits. */
	const own = () => props.user.uuid === reader().uuid;

	// A change made elsewhere reaches a form the reader has not touched.
	createEffect(
		on(saved, (next, previous) => {
			if (
				previous &&
				!saving() &&
				!failure() &&
				patchOf(previous, draft()) === undefined
			) {
				setDraft(next);
			}
		}),
	);

	const problems = createMemo(() => problemsOf(draft()));
	const patch = createMemo(() => patchOf(saved(), draft()));
	const edit = (change: Partial<Draft>) => {
		setDraft((current) => ({ ...current, ...change }));
		setJustSaved(false);
		setLinkSent(false);
		setFailure(undefined);
	};
	// Only a field the reader changed can hold them back.
	const problem = (field: Field) =>
		draft()[field] === saved()[field] ? undefined : problems()[field];
	const blocked = () =>
		!patch() ||
		saving() ||
		FIELDS.some((field) => problem(field)) ||
		(patch()?.email !== undefined && props.tokens === undefined);

	let heading: HTMLHeadingElement | undefined;
	let foot: HTMLDivElement | undefined;
	// Save disables itself and Discard goes away, so focus on either moves to
	// the form's heading first rather than falling to the page.
	const holdFocus = () => {
		if (foot?.contains(document.activeElement)) {
			heading?.focus();
		}
	};

	const save = async (changes: Partial<Draft>) => {
		setConfirming(false);
		setSaving(true);
		setFailure(undefined);
		try {
			const { data: user } = await api.send<JsonUser>(
				"PATCH",
				`/v0/users/${props.user.uuid}`,
				changes,
			);
			// The reader's own new email takes effect from the confirm link the API mails it.
			const waiting =
				own() &&
				changes.email !== undefined &&
				!sameEmail(user.email, changes.email);
			client.setQueryData(userQuery(api, user.slug).queryKey, user);
			client.setQueryData(
				legacyTokensQuery(api, user.slug).queryKey,
				props.tokens,
			);
			if (own()) {
				keepUser(user);
			}
			setDraft(draftOf(user));
			setJustSaved(!waiting);
			setLinkSent(waiting);
			if (user.slug !== props.user.slug) {
				// Relative to the router's base.
				navigate(`/users/${user.slug}/settings`, { replace: true });
			}
		} catch (error) {
			setFailure(failureOf(error));
		} finally {
			setSaving(false);
		}
	};

	const submit = (event: SubmitEvent) => {
		event.preventDefault();
		const changes = patch();
		if (!changes || blocked()) {
			return;
		}
		// Emails differ by more than case before any legacy token or another user's session is at stake.
		if (
			changes.email !== undefined &&
			!sameEmail(changes.email, saved().email) &&
			(props.tokens || !own())
		) {
			setConfirming(true);
			return;
		}
		holdFocus();
		save(changes);
	};

	const field = (
		id: Field,
		label: string,
		input: { type?: string; class?: string; autocomplete: string },
	) => (
		<div class="formrow">
			<div class="fl">
				<label for={`f-${id}`}>{label}</label>
			</div>
			<div class="field">
				<TextInput
					id={`f-${id}`}
					type={input.type ?? "text"}
					class={input.class}
					value={draft()[id]}
					onInput={(event) => edit({ [id]: event.currentTarget.value })}
					aria-describedby={`e-${id}`}
					aria-invalid={problem(id) ? true : undefined}
					autocomplete={input.autocomplete}
					spellcheck={false}
					slug={id === "slug"}
				/>
				<Show when={problem(id)}>
					{(message) => (
						<p class="ferror" id={`e-${id}`}>
							{message()}
						</p>
					)}
				</Show>
			</div>
		</div>
	);

	return (
		<>
			<form class="setcard" aria-labelledby="sec-you" onSubmit={submit}>
				<div class="setcard-head">
					<h2
						class="seclabel"
						id="sec-you"
						tabindex="-1"
						ref={(element) => {
							heading = element;
						}}
					>
						You
					</h2>
				</div>
				<div class="formrows">
					{field("name", "Name", { autocomplete: "name" })}
					{field("slug", "Slug", { class: "mono", autocomplete: "off" })}
					{field("email", "Email", { type: "email", autocomplete: "email" })}
				</div>
				<div
					class="setcard-foot"
					ref={(element) => {
						foot = element;
					}}
				>
					<span class="set-status" role="status">
						<Show when={justSaved()}>
							<Icon name="check" />
							Saved
						</Show>
						<Show when={linkSent()}>Confirm link sent</Show>
					</span>
					<Show when={failure()}>
						{(message) => (
							<p class="ferror" role="alert">
								<span>{message()}</span>
							</p>
						)}
					</Show>
					<span class="spacer" />
					<Show when={patch()}>
						<Button
							onClick={() => {
								holdFocus();
								setDraft(saved());
								setFailure(undefined);
							}}
						>
							Discard
						</Button>
					</Show>
					<Button type="submit" variant="primary" disabled={blocked()}>
						Save changes
					</Button>
				</div>
			</form>
			<Show when={confirming()}>
				<Dialog
					role="alertdialog"
					aria-labelledby="em-title"
					aria-describedby="em-desc"
					onDismiss={() => setConfirming(false)}
				>
					<div class="ui-dialog-head">
						<h2 id="em-title">
							<Show
								when={own()}
								fallback={`Change ${props.user.name}'s email to ${draft().email.trim()}?`}
							>
								Change your email to {draft().email.trim()}?
							</Show>
						</h2>
						<Button
							variant="ghost"
							iconOnly
							aria-label="Close"
							onClick={() => setConfirming(false)}
						>
							<Icon name="close" />
						</Button>
					</div>
					<div class="ui-dialog-body">
						<p id="em-desc">
							<Show
								when={own()}
								fallback={
									props.tokens
										? `Every session of ${props.user.name} ends, and their ${legacyTokens(props.tokens)} are revoked.`
										: `Every session of ${props.user.name} ends.`
								}
							>
								Your {legacyTokens(props.tokens ?? 0)} will be revoked.
							</Show>
						</p>
					</div>
					<div class="ui-dialog-foot">
						<Button onClick={() => setConfirming(false)}>Cancel</Button>
						<Button
							variant="destructive"
							onClick={() => {
								const changes = patch();
								if (changes) {
									save(changes);
								}
							}}
						>
							Change email
						</Button>
					</div>
				</Dialog>
			</Show>
		</>
	);
};

const sameEmail = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** A server admin's switches for the user: whether they are a server admin, and whether they are locked out. */
const ServerCard = (props: { user: JsonUser }) => {
	const { api, reader, keepUser } = useAccount();
	const client = useQueryClient();
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const flip = async (field: "admin" | "locked") => {
		setBusy(true);
		setFailure(undefined);
		try {
			const { data: user } = await api.send<JsonUser>(
				"PATCH",
				`/v0/users/${props.user.uuid}`,
				{ [field]: !props.user[field] },
			);
			client.setQueryData(userQuery(api, user.slug).queryKey, user);
			if (user.uuid === reader().uuid) {
				keepUser(user);
			}
		} catch (error) {
			setFailure(failureOf(error));
		} finally {
			setBusy(false);
		}
	};
	return (
		<section class="setcard" aria-labelledby="sec-server">
			<div class="setcard-head">
				<h2 class="seclabel" id="sec-server">
					Server
				</h2>
			</div>
			<div class="setcard-body">
				<div class="toolbar">
					<Button
						selected={props.user.admin}
						disabled={busy()}
						onClick={() => flip("admin")}
					>
						Admin
					</Button>
					<Button
						selected={props.user.locked}
						disabled={busy()}
						onClick={() => flip("locked")}
					>
						Locked
					</Button>
				</div>
				<Show when={failure()}>
					{(message) => (
						<p class="ferror" role="alert">
							<span>{message()}</span>
						</p>
					)}
				</Show>
			</div>
		</section>
	);
};
