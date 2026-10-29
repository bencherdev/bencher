import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import { type QueryClient, useQueryClient } from "@tanstack/solid-query";
import { Show, createEffect, createMemo, createSignal, on } from "solid-js";
import {
	type JsonConsoleProject,
	type JsonProject,
	Visibility,
} from "../../types/bencher";
import type { Api } from "../api";
import { ORGANIZATIONS_PATH } from "../paths";
import { useProject, useProjectNavigate } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import type SlugDialog from "../SlugDialog";
import { lazyView } from "../shell/lazy";
import { applyPatch, forgetSlug, settleProject } from "./data";
import DeleteProject from "./DeleteProject";
import {
	type Draft,
	type Field,
	type Patch,
	type Refusal,
	draftOf,
	patchOf,
	problemsOf,
	refusalOf,
	webUrl,
} from "./general";
import Head from "./Head";

const FIELDS: readonly Field[] = ["name", "slug", "url"];

/** The project's name, slug, and URL, and the Danger section with its visibility. */
const General = () => {
	const { api, slug } = useProject();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	return (
		<>
			<Head
				slug={slug()}
				title="General"
				readOnly={bootstrap().data?.permissions.edit === false}
			/>
			<Show when={bootstrap().data} fallback={<Skeleton size="card" />}>
				{(data) => (
					<>
						<Show
							when={data().permissions.edit}
							fallback={<ProjectFacts project={data().project} />}
						>
							<ProjectForm data={data()} />
						</Show>
						<Show when={data().permissions.edit || data().permissions.manage}>
							<Danger data={data()} />
						</Show>
					</>
				)}
			</Show>
		</>
	);
};

export default General;

const slugDialog = lazyView(() => import("../SlugDialog"));

const ProjectForm = (props: { data: JsonConsoleProject }) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const navigate = useProjectNavigate();

	const saved = createMemo(() => draftOf(props.data.project), undefined, {
		equals: (a, b) => patchOf(a, b) === undefined,
	});
	const [draft, setDraft] = createSignal<Draft>(saved());
	const [refusal, setRefusal] = createSignal<Refusal>();
	const [justSaved, setJustSaved] = createSignal(false);
	const [moved, setMoved] = createSignal<string>();

	const [saving, setSaving] = createSignal(false);
	const [confirming, setConfirming] = createSignal<typeof SlugDialog>();
	const save = async (patch: Patch) => {
		const from = slug();
		setConfirming(undefined);
		holdFocus();
		setSaving(true);
		setRefusal(undefined);
		try {
			const project = await sendPatch(api, client, from, patch);
			setDraft(draftOf(project));
			setJustSaved(true);
			if (project.slug !== from) {
				setMoved(from);
				// Relative to the router's base.
				navigate(`/${project.slug}/settings`, { replace: true });
			}
		} catch (error) {
			setRefusal(refusalOf(error, patch));
		} finally {
			setSaving(false);
		}
	};

	// The old slug's cache goes once the page has left it, so nothing still
	// watching it asks the API for a project that is no longer there.
	createEffect(() => {
		const from = moved();
		if (from && slug() !== from) {
			forgetSlug(client, localStorage, from);
			setMoved(undefined);
		}
	});

	// A change made elsewhere reaches a form the reader has not touched.
	createEffect(
		on(saved, (next, previous) => {
			if (
				previous &&
				!saving() &&
				!refusal() &&
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
		setRefusal(undefined);
	};
	const refused = (field: Refusal["field"]) =>
		refusal()?.field === field ? refusal()?.message : undefined;
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

	return (
		<>
			<form class="setcard" aria-labelledby="sec-project" onSubmit={submit}>
				<div class="setcard-head">
					<h2
						class="seclabel"
						id="sec-project"
						tabindex="-1"
						ref={(element) => {
							heading = element;
						}}
					>
						Project
					</h2>
				</div>
				<div class="formrows">
					<div class="formrow">
						<div class="fl">
							<label for="f-name">Name</label>
						</div>
						<div class="field">
							<TextInput
								id="f-name"
								value={draft().name}
								onInput={(event) => edit({ name: event.currentTarget.value })}
								aria-describedby="e-name"
								aria-invalid={problem("name") ? true : undefined}
								autocomplete="off"
							/>
							<FieldError id="e-name" message={problem("name")} />
						</div>
					</div>

					<div class="formrow">
						<div class="fl">
							<label for="f-slug">Slug</label>
						</div>
						<div class="field">
							<TextInput
								id="f-slug"
								class="mono"
								value={draft().slug}
								onInput={(event) => edit({ slug: event.currentTarget.value })}
								aria-describedby="e-slug r-slug"
								aria-invalid={
									problem("slug") || refused("slug") ? true : undefined
								}
								slug
							/>
							<FieldError id="e-slug" message={problem("slug")} />
							<Show when={refused("slug")}>
								{(message) => (
									<p class="ferror" id="r-slug" role="alert">
										<Truncate text={message()} reveal />
									</p>
								)}
							</Show>
						</div>
					</div>

					<div class="formrow">
						<div class="fl">
							<label for="f-url">URL</label>
						</div>
						<div class="field">
							<TextInput
								id="f-url"
								type="url"
								value={draft().url}
								onInput={(event) => edit({ url: event.currentTarget.value })}
								aria-describedby="e-url"
								aria-invalid={problem("url") ? true : undefined}
								autocomplete="off"
							/>
							<FieldError id="e-url" message={problem("url")} />
						</div>
					</div>

					<div class="formrow">
						<div class="fl">
							<span>BMF version</span>
						</div>
						<div class="fvalue mono">{props.data.project.bmf_version}</div>
					</div>
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
					<Show when={refused("form")}>
						{(message) => (
							<p class="ferror" role="alert">
								<Truncate text={message()} reveal />
							</p>
						)}
					</Show>
					<span class="spacer" />
					<Show when={patch()}>
						<Button
							onClick={() => {
								holdFocus();
								setDraft(saved());
								setRefusal(undefined);
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
							noun="project"
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

/** What a reader who cannot edit the project sees in place of the form. */
const ProjectFacts = (props: { project: JsonProject }) => (
	<section class="setcard" aria-labelledby="sec-project">
		<div class="setcard-head">
			<h2 class="seclabel" id="sec-project">
				Project
			</h2>
		</div>
		<dl class="formrows">
			<div class="formrow">
				<dt class="fl">
					<span>Name</span>
				</dt>
				<dd class="fvalue">{props.project.name}</dd>
			</div>
			<div class="formrow">
				<dt class="fl">
					<span>Slug</span>
				</dt>
				<dd class="fvalue mono">{props.project.slug}</dd>
			</div>
			<div class="formrow">
				<dt class="fl">
					<span>URL</span>
				</dt>
				<dd class="fvalue">
					<Show
						when={props.project.url}
						fallback={<span class="muted">None</span>}
					>
						{(url) => (
							<Show when={webUrl(url())} fallback={<span>{url()}</span>}>
								{(link) => (
									<a href={link()} rel="noopener noreferrer nofollow">
										{link()}
									</a>
								)}
							</Show>
						)}
					</Show>
				</dd>
			</div>
			<div class="formrow">
				<dt class="fl">
					<span>Visibility</span>
				</dt>
				<dd class="fvalue">
					{props.project.visibility === Visibility.Private
						? "Private"
						: "Public"}
				</dd>
			</div>
			<div class="formrow">
				<dt class="fl">
					<span>BMF version</span>
				</dt>
				<dd class="fvalue">
					<span class="mono">{props.project.bmf_version}</span>
				</dd>
			</div>
		</dl>
	</section>
);

const FieldError = (props: { id: string; message: string | undefined }) => (
	<Show when={props.message}>
		{(message) => (
			<p class="ferror" id={props.id}>
				{message()}
			</p>
		)}
	</Show>
);

const billing = (data: JsonConsoleProject) =>
	`${ORGANIZATIONS_PATH}/${data.organization.slug}/billing`;

const Danger = (props: { data: JsonConsoleProject }) => {
	const [changing, setChanging] = createSignal(false);
	const [deleting, setDeleting] = createSignal(false);
	return (
		<section class="setcard danger" aria-labelledby="sec-danger">
			<div class="setcard-head">
				<h2 class="seclabel danger" id="sec-danger">
					Danger
				</h2>
			</div>
			<div class="setcard-body">
				<Show when={props.data.permissions.edit}>
					<div class="danger-row">
						<div class="grow">
							<h3>Change project visibility</h3>
							<p id="dz-vis" class="muted">
								This project is currently {props.data.project.visibility}.
							</p>
						</div>
						<Button
							aria-haspopup="dialog"
							aria-describedby="dz-vis"
							onClick={() => setChanging(true)}
						>
							Change visibility
						</Button>
					</div>
				</Show>
				<Show when={props.data.permissions.manage}>
					<div class="danger-row">
						<div class="grow">
							<h3>Delete this project</h3>
							<p id="dz-del" class="muted">
								Once you delete a project, there is no going back. Please be
								certain.
							</p>
						</div>
						<Button
							variant="danger"
							aria-haspopup="dialog"
							aria-describedby="dz-del"
							onClick={() => setDeleting(true)}
						>
							Delete project
						</Button>
					</div>
				</Show>
			</div>
			<Show when={changing()}>
				<ChangeVisibility
					data={props.data}
					onDismiss={() => setChanging(false)}
				/>
			</Show>
			<Show when={deleting()}>
				<DeleteProject
					project={props.data.project}
					onDismiss={() => setDeleting(false)}
				/>
			</Show>
		</section>
	);
};

/** Asks, then changes the project to the other visibility with the form's patch. */
const ChangeVisibility = (props: {
	data: JsonConsoleProject;
	onDismiss: () => void;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	// Held from the opening: the change shows at once, under the dialog.
	const to =
		props.data.project.visibility === Visibility.Private
			? Visibility.Public
			: Visibility.Private;
	const [sending, setSending] = createSignal(false);
	const [refusal, setRefusal] = createSignal<Refusal>();
	// The button stays enabled while sending, so the focus on it is never dropped.
	const change = async () => {
		if (sending()) {
			return;
		}
		const patch = { visibility: to };
		setSending(true);
		setRefusal(undefined);
		try {
			await sendPatch(api, client, slug(), patch);
			props.onDismiss();
		} catch (error) {
			setRefusal(refusalOf(error, patch));
		} finally {
			setSending(false);
		}
	};
	return (
		<Dialog
			role="alertdialog"
			aria-labelledby="vis-title"
			onDismiss={props.onDismiss}
		>
			<div class="ui-dialog-head">
				<h2 id="vis-title">
					Make {props.data.project.slug} {to}?
				</h2>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Close"
					onClick={props.onDismiss}
				>
					<Icon name="close" />
				</Button>
			</div>
			<Show when={refusal()}>
				{(refused) => (
					<div class="ui-dialog-body">
						<p class="ferror" role="alert">
							<Icon name="warning" />
							<span>
								<Show
									when={refused().field === "visibility"}
									fallback={<Truncate text={refused().message} reveal />}
								>
									{refused().message}{" "}
									<a href={billing(props.data)}>See plans</a>
								</Show>
							</span>
						</p>
					</div>
				)}
			</Show>
			<div class="ui-dialog-foot">
				<Button onClick={props.onDismiss}>Cancel</Button>
				<Button variant="primary" onClick={change}>
					Make {to}
				</Button>
			</div>
		</Dialog>
	);
};

/**
 * Show `patch` at once and send it, taking it back if the API refuses; the
 * project as the API kept it.
 */
const sendPatch = async (
	api: Api,
	client: QueryClient,
	slug: string,
	patch: Patch,
) => {
	// A read in flight would land over the change.
	await client.cancelQueries({ queryKey: ["console", "project", slug] });
	const undo = applyPatch(client, slug, patch);
	try {
		const { data: project } = await api.send<JsonProject>(
			"PATCH",
			`/v0/projects/${encodeURIComponent(slug)}`,
			patch,
		);
		settleProject(client, localStorage, slug, project);
		return project;
	} catch (error) {
		undo();
		throw error;
	}
};
