import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import TextInput from "@bencherdev/ui/TextInput";
import { useNavigate, useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createEffect, createMemo, createSignal, on } from "solid-js";
import type {
	JsonConsoleOrganization,
	JsonConsoleUser,
	JsonOrganization,
} from "../../types/bencher";
import { useAccount } from "../account/context";
import {
	allowedQuery,
	organizationIn,
	organizationQuery,
	withOrganization,
} from "../organization/data";
import DeleteOrganization from "../organization/DeleteOrganization";
import Frame from "../organization/Frame";
import {
	type Draft,
	type Field,
	draftOf,
	patchOf,
	problemsOf,
	saveFailure,
} from "../organization/general";
import { CLASSIC_ORGANIZATION } from "../memory";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import type SlugDialog from "../SlugDialog";
import { lazyView } from "../shell/lazy";

const FIELDS: readonly Field[] = ["name", "slug"];

/** The organization's name, slug, and UUID, and the Danger box that deletes it. */
const OrgGeneral = () => {
	const { api, reader } = useAccount();
	const params = useParams();
	const key = () => params.organization ?? "";
	const organization = useQueryResult(() => organizationQuery(api, key()));
	const allowed = useQueryResult(() => allowedQuery(api, key()));
	// Only read: an organization the reader has seen paints at once.
	const bootstrap = useQueryResult(() => ({
		...consoleUserQuery(api, reader().user.uuid),
		enabled: false,
	}));
	const known = () =>
		organization().data ?? organizationIn(bootstrap().data, key());
	const ready = () => {
		const shown = known();
		const may = allowed().data;
		return shown && may ? { organization: shown, allowed: may } : undefined;
	};
	return (
		<Frame organization={key()} name={known()?.name} section="general">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Settings<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						General
					</Heading>
				</div>
			</div>
			<Show
				when={ready()}
				fallback={
					<Show
						when={organization().error ?? allowed().error}
						fallback={<Skeleton size="card" />}
					>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">General did not load</span>
							<Button
								size="sm"
								onClick={() => {
									organization().refetch();
									allowed().refetch();
								}}
							>
								Retry
							</Button>
						</Banner>
					</Show>
				}
			>
				{(shown) => (
					<>
						<Show
							when={shown().allowed.edit}
							fallback={<Facts organization={shown().organization} />}
						>
							<OrganizationForm organization={shown().organization} />
						</Show>
						<Show when={shown().allowed.delete}>
							<Danger organization={shown().organization} />
						</Show>
					</>
				)}
			</Show>
		</Frame>
	);
};

export default OrgGeneral;

type Shown = JsonOrganization | JsonConsoleOrganization;

const slugDialog = lazyView(() => import("../SlugDialog"));

const OrganizationForm = (props: { organization: Shown }) => {
	const { api, reader } = useAccount();
	const client = useQueryClient();
	const navigate = useNavigate();
	const params = useParams();

	const saved = createMemo(() => draftOf(props.organization), undefined, {
		equals: (a, b) => patchOf(a, b) === undefined,
	});
	const [draft, setDraft] = createSignal<Draft>(saved());
	const [failure, setFailure] = createSignal<string>();
	const [justSaved, setJustSaved] = createSignal(false);
	const [confirming, setConfirming] = createSignal<typeof SlugDialog>();
	const [saving, setSaving] = createSignal(false);
	const [moved, setMoved] = createSignal<string>();

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

	// The old slug's cache goes once the page has left it, so nothing still
	// watching it asks the API for an organization that is no longer there.
	createEffect(() => {
		const from = moved();
		if (from && params.organization !== from) {
			client.removeQueries({ queryKey: ["organization", from] });
			setMoved(undefined);
		}
	});

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
		!patch() || saving() || FIELDS.some((field) => problem(field));

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
		const from = params.organization ?? "";
		setConfirming(undefined);
		holdFocus();
		setSaving(true);
		setFailure(undefined);
		try {
			const { data: organization } = await api.send<JsonOrganization>(
				"PATCH",
				`/v0/organizations/${props.organization.uuid}`,
				changes,
			);
			const { uuid, name, slug } = organization;
			const to = slug === props.organization.slug ? from : slug;
			client.setQueryData(organizationQuery(api, to).queryKey, organization);
			client.setQueryData(
				allowedQuery(api, to).queryKey,
				client.getQueryData(allowedQuery(api, from).queryKey),
			);
			client.setQueryData<JsonConsoleUser>(
				consoleUserQuery(api, reader().user.uuid).queryKey,
				(user) => user && withOrganization(user, uuid, { uuid, name, slug }),
			);
			setDraft(draftOf(organization));
			setJustSaved(true);
			if (to !== from) {
				localStorage.removeItem(CLASSIC_ORGANIZATION);
				setMoved(from);
				// Relative to the router's base.
				navigate(`/organizations/${slug}/settings`, { replace: true });
			}
		} catch (error) {
			setFailure(saveFailure(error, changes));
		} finally {
			setSaving(false);
		}
	};

	// The confirmation's code loads once a save would need it.
	const preload = () => {
		if (patch()?.slug !== undefined) {
			slugDialog.preload();
		}
	};
	const submit = (event: SubmitEvent) => {
		event.preventDefault();
		const changes = patch();
		if (!changes || blocked()) {
			return;
		}
		if (changes.slug !== undefined) {
			slugDialog
				.load()
				.then((view) => {
					// A change undone while the code loaded no longer needs it.
					if (patch()?.slug !== undefined) {
						setConfirming(() => view);
					}
				})
				.catch(() => {});
			return;
		}
		holdFocus();
		save(changes);
	};

	const field = (id: Field, label: string, mono: boolean) => (
		<div class="formrow">
			<div class="fl">
				<label for={`f-${id}`}>{label}</label>
			</div>
			<div class="field">
				<TextInput
					id={`f-${id}`}
					class={mono ? "mono" : undefined}
					value={draft()[id]}
					onInput={(event) => edit({ [id]: event.currentTarget.value })}
					aria-describedby={`e-${id}`}
					aria-invalid={problem(id) ? true : undefined}
					autocomplete="off"
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
			<form class="setcard" aria-labelledby="sec-org" onSubmit={submit}>
				<div class="setcard-head">
					<h2
						class="seclabel"
						id="sec-org"
						tabindex="-1"
						ref={(element) => {
							heading = element;
						}}
					>
						Organization
					</h2>
				</div>
				<div class="formrows">
					{field("name", "Name", false)}
					{field("slug", "Slug", true)}
					<UuidRow uuid={props.organization.uuid} />
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
					<Button
						type="submit"
						variant="primary"
						disabled={blocked()}
						onPointerEnter={preload}
						onFocus={preload}
					>
						Save changes
					</Button>
				</div>
			</form>
			<Show when={confirming()}>
				{(view) => {
					const Confirm = view();
					return (
						<Confirm
							slug={draft().slug}
							noun="organization"
							onCancel={() => setConfirming(undefined)}
							onConfirm={() => {
								const changes = patch();
								if (changes) {
									save(changes);
								}
							}}
						/>
					);
				}}
			</Show>
		</>
	);
};

const UuidRow = (props: { uuid: string }) => {
	const [copied, setCopied] = createSignal(false);
	return (
		<div class="formrow">
			<div class="fl">
				<span>UUID</span>
			</div>
			<div class="fvalue">
				<span class="mono">{props.uuid}</span>
				<Button
					size="sm"
					aria-label={copied() ? "UUID copied" : "Copy the UUID"}
					onClick={() =>
						navigator.clipboard.writeText(props.uuid).then(
							() => setCopied(true),
							() => setCopied(false),
						)
					}
				>
					{copied() ? "Copied" : "Copy"}
				</Button>
			</div>
		</div>
	);
};

/** What a reader who cannot edit the organization sees in place of the form. */
const Facts = (props: { organization: Shown }) => (
	<section class="setcard" aria-labelledby="sec-org">
		<div class="setcard-head">
			<h2 class="seclabel" id="sec-org">
				Organization
			</h2>
		</div>
		<div class="formrows">
			<div class="formrow">
				<div class="fl">
					<span>Name</span>
				</div>
				<div class="fvalue">{props.organization.name}</div>
			</div>
			<div class="formrow">
				<div class="fl">
					<span>Slug</span>
				</div>
				<div class="fvalue mono">{props.organization.slug}</div>
			</div>
			<UuidRow uuid={props.organization.uuid} />
		</div>
	</section>
);

const Danger = (props: { organization: Shown }) => {
	const { api, reader } = useAccount();
	const client = useQueryClient();
	const [deleting, setDeleting] = createSignal(false);
	// The confirmation names the organization's projects from the reader's bootstrap.
	const intent = () => {
		client
			.prefetchQuery(consoleUserQuery(api, reader().user.uuid))
			.catch(() => {});
	};
	return (
		<section class="setcard danger" aria-labelledby="sec-danger">
			<div class="setcard-head">
				<h2 class="seclabel danger" id="sec-danger">
					Danger
				</h2>
			</div>
			<div class="setcard-body">
				<div class="danger-row">
					<div class="grow">
						<h3>Delete this organization</h3>
						<p id="dz-del" class="muted">
							Once you delete an organization, there is no going back. Please be
							certain.
						</p>
					</div>
					<Button
						variant="danger"
						aria-haspopup="dialog"
						aria-describedby="dz-del"
						onPointerEnter={intent}
						onFocus={intent}
						onClick={() => setDeleting(true)}
					>
						Delete organization
					</Button>
				</div>
			</div>
			<Show when={deleting()}>
				<DeleteOrganization
					organization={props.organization}
					onDismiss={() => setDeleting(false)}
				/>
			</Show>
		</section>
	);
};

// Members loads this chunk rather than split a shared one.
export { default as Frame } from "../organization/Frame";
export { organizationIn, organizationQuery } from "../organization/data";
export { failureOf } from "../organization/general";
