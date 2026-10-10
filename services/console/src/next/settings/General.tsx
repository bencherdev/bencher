import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import Skeleton from "@bencherdev/ui/Skeleton";
import TextInput from "@bencherdev/ui/TextInput";
import { useNavigate } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createEffect, createMemo, createSignal, on } from "solid-js";
import {
	type JsonConsoleProject,
	type JsonProject,
	Visibility,
} from "../../types/bencher";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
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

const VISIBILITIES = [
	{ value: Visibility.Public, label: "Public" },
	{ value: Visibility.Private, label: "Private" },
] as const;

/** The project's name, slug, URL, and visibility, and the Danger section. */
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
						<Show when={data().permissions.manage}>
							<Danger data={data()} />
						</Show>
					</>
				)}
			</Show>
		</>
	);
};

export default General;

const ProjectForm = (props: { data: JsonConsoleProject }) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const navigate = useNavigate();

	const saved = createMemo(() => draftOf(props.data.project), undefined, {
		equals: (a, b) => patchOf(a, b) === undefined,
	});
	const [draft, setDraft] = createSignal<Draft>(saved());
	const [refusal, setRefusal] = createSignal<Refusal>();
	const [justSaved, setJustSaved] = createSignal(false);
	const [moved, setMoved] = createSignal<string>();

	const [saving, setSaving] = createSignal(false);
	const save = async (patch: Patch) => {
		const from = slug();
		setSaving(true);
		setRefusal(undefined);
		// A read in flight would land over the change.
		await client.cancelQueries({ queryKey: ["console", "project", from] });
		const undo = applyPatch(client, from, patch);
		try {
			const { data: project } = await api.send<JsonProject>(
				"PATCH",
				`/v0/projects/${encodeURIComponent(from)}`,
				patch,
			);
			settleProject(client, localStorage, from, project);
			setDraft(draftOf(project));
			setJustSaved(true);
			if (project.slug !== from) {
				setMoved(from);
				// Relative to the router's base.
				navigate(`/${project.slug}/settings`, { replace: true });
			}
		} catch (error) {
			setRefusal(refusalOf(error, patch));
			undo();
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
	const submit = (event: SubmitEvent) => {
		event.preventDefault();
		const changes = patch();
		if (changes && !blocked()) {
			holdFocus();
			save(changes);
		}
	};

	return (
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
						<span class="fhelp" id="h-name">
							Shown on every page of the project.
						</span>
					</div>
					<div class="field">
						<TextInput
							id="f-name"
							value={draft().name}
							onInput={(event) => edit({ name: event.currentTarget.value })}
							aria-describedby="h-name e-name"
							aria-invalid={problem("name") ? true : undefined}
							autocomplete="off"
						/>
						<FieldError id="e-name" message={problem("name")} />
					</div>
				</div>

				<div class="formrow">
					<div class="fl">
						<label for="f-slug">Slug</label>
						<span class="fhelp">
							The project's name in links and in{" "}
							<span class="mono">bencher run --project</span>.
						</span>
					</div>
					<div class="field">
						<TextInput
							id="f-slug"
							class="mono"
							value={draft().slug}
							onInput={(event) => edit({ slug: event.currentTarget.value })}
							aria-describedby="w-slug e-slug r-slug"
							aria-invalid={
								problem("slug") || refused("slug") ? true : undefined
							}
							autocomplete="off"
							spellcheck={false}
						/>
						<p class="fwarn" id="w-slug">
							<Icon name="warning" />
							<span>
								Changing the slug breaks every existing link to this project,
								and every run that names it.
							</span>
						</p>
						<FieldError id="e-slug" message={problem("slug")} />
						<Show when={refused("slug")}>
							{(message) => (
								<p class="ferror" id="r-slug" role="alert">
									{message()}
								</p>
							)}
						</Show>
					</div>
				</div>

				<div class="formrow">
					<div class="fl">
						<label for="f-url">URL</label>
						<span class="fhelp" id="h-url">
							Where the project lives, such as its repository.
						</span>
					</div>
					<div class="field">
						<TextInput
							id="f-url"
							type="url"
							value={draft().url}
							onInput={(event) => edit({ url: event.currentTarget.value })}
							aria-describedby="h-url e-url"
							aria-invalid={problem("url") ? true : undefined}
							autocomplete="off"
						/>
						<FieldError id="e-url" message={problem("url")} />
					</div>
				</div>

				<div class="formrow">
					<div class="fl">
						<span id="lab-vis">Visibility</span>
						<span class="fhelp">Who can open the public plot.</span>
					</div>
					<div class="field">
						<Segmented
							name="visibility"
							aria-labelledby="lab-vis"
							options={VISIBILITIES}
							value={draft().visibility}
							onChange={(visibility) => edit({ visibility })}
							style={{ "align-self": "flex-start" }}
						/>
						<p class="fhelp">
							{draft().visibility === Visibility.Public
								? "Anyone can open the public plot. Every other page needs a login and a seat on the project."
								: "Only the project's members can see it, and the public plot is gone."}
						</p>
						<Show
							when={refused("visibility")}
							fallback={
								<Show when={saved().visibility === Visibility.Public}>
									<p class="fhelp">
										Private needs a paid plan.{" "}
										<a href={billing(props.data)}>See plans</a>
									</p>
								</Show>
							}
						>
							{(message) => (
								<p class="ferror" role="alert">
									<Icon name="warning" />
									<span>
										{message()} <a href={billing(props.data)}>See plans</a>
									</span>
								</p>
							)}
						</Show>
					</div>
				</div>

				<div class="formrow">
					<div class="fl">
						<span>BMF version</span>
						<span class="fhelp" id="h-bmf">
							The version a report gets when it names none.
						</span>
					</div>
					<BmfVersion version={props.data.project.bmf_version} />
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
							setRefusal(undefined);
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

const BmfVersion = (props: { version: number }) => (
	<div class="fvalue">
		<span class="mono">{props.version}</span>
		<span class="set-status">
			<Icon name="read-only" />
			Read only
		</span>
	</div>
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
	`/console/organizations/${data.organization.slug}/billing`;

const Danger = (props: { data: JsonConsoleProject }) => {
	const [deleting, setDeleting] = createSignal(false);
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
						<b>Delete this project</b>
						<span class="muted sm">
							Every report, threshold, alert, and pinned plot goes with it, and
							members lose access at once. It cannot be undone.
						</span>
					</div>
					<Button
						variant="danger"
						aria-haspopup="dialog"
						onClick={() => setDeleting(true)}
					>
						Delete project
					</Button>
				</div>
			</div>
			<Show when={deleting()}>
				<DeleteProject
					project={props.data.project}
					organization={props.data.organization.uuid}
					onDismiss={() => setDeleting(false)}
				/>
			</Show>
		</section>
	);
};
