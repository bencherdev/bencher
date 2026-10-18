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
	userQuery,
} from "../account/profile";
import { accountPath } from "../account/paths";
import { useQueryResult } from "../query";

const FIELDS: readonly Field[] = ["name", "slug", "email"];

/** The reader's name, slug, and email, and a link to any legacy tokens. */
const Profile = () => {
	const { api, reader } = useAccount();
	const params = useParams();
	const slug = () => params.user ?? "";
	// The reader's own account is already known, so it paints at once.
	const known = () => {
		const { user } = reader();
		return slug() === user.slug || slug() === user.uuid ? user : undefined;
	};
	const user = useQueryResult(() => userQuery(api, slug()));
	const tokens = useQueryResult(() => legacyTokensQuery(api, slug()));
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
				when={user().data ?? known()}
				fallback={
					<Show when={user().error} fallback={<Skeleton size="card" />}>
						<Failed what="Profile" retry={() => user().refetch()} />
					</Show>
				}
			>
				{(data) => <ProfileForm user={data()} tokens={tokens().data} />}
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
	const { api, reader, keepUser, signOut } = useAccount();
	const client = useQueryClient();
	const navigate = useNavigate();

	const saved = createMemo(() => draftOf(props.user), undefined, {
		equals: (a, b) => patchOf(a, b) === undefined,
	});
	const [draft, setDraft] = createSignal<Draft>(saved());
	const [failure, setFailure] = createSignal<string>();
	const [justSaved, setJustSaved] = createSignal(false);
	const [confirming, setConfirming] = createSignal(false);
	const [saving, setSaving] = createSignal(false);

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
			const own = user.uuid === reader().user.uuid;
			// The reader's session is signed for the email it replaced.
			if (own && changes.email !== undefined) {
				signOut();
				return;
			}
			client.setQueryData(userQuery(api, user.slug).queryKey, user);
			client.setQueryData(
				legacyTokensQuery(api, user.slug).queryKey,
				props.tokens,
			);
			if (own) {
				keepUser(user);
			}
			setDraft(draftOf(user));
			setJustSaved(true);
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
		if (changes.email !== undefined && props.tokens) {
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
					</span>
					<Show when={failure()}>
						{(message) => (
							<p class="ferror" role="alert">
								{message()}
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
						<h2 id="em-title">Change your email to {draft().email.trim()}?</h2>
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
							Your {legacyTokens(props.tokens ?? 0)} will be revoked.
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
