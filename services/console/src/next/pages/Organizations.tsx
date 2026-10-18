import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import TextInput from "@bencherdev/ui/TextInput";
import { useLocation, useNavigate } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Show, createMemo, createSignal, useContext } from "solid-js";
import type { JsonConsoleUser, JsonOrganization } from "../../types/bencher";
import { useAccount } from "../account/context";
import {
	nameProblem,
	organizationRows,
	slugOf,
} from "../account/organizations";
import { FlushContext } from "../flush";
import { ORGANIZATIONS_PATH } from "../paths";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import { Frame, failureOf } from "./Profile";

const settingsHref = (slug: string) =>
	`/console/organizations/${slug}/settings`;
const newProjectHref = (slug: string) =>
	`${ORGANIZATIONS_PATH}/${slug}/projects/add`;

const counted = (count: number, one: string, many: string) =>
	`${count} ${count === 1 ? one : many}`;

/** The organizations the reader belongs to, each leading to its settings. */
const Organizations = (props: {
	/** Where the browser goes once the new organization is made. */
	leave?: (href: string) => void;
}) => {
	const { api, reader } = useAccount();
	const navigate = useNavigate();
	const here = useLocation();
	const adding = () => here.pathname.endsWith("/organizations/add");
	const bootstrap = useQueryResult(() =>
		consoleUserQuery(api, reader().user.uuid),
	);
	const rows = createMemo(() => {
		const data = bootstrap().data;
		return data && organizationRows(data);
	});
	const [opened, setOpened] = createSignal(false);
	const close = () => {
		setOpened(false);
		if (adding()) {
			navigate("/organizations", { replace: true });
		}
	};
	return (
		<Frame user={reader().user.slug} section="organizations">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Account<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						Organizations
					</Heading>
					<p class="ph-sub">
						{rows()
							? counted(rows()?.length ?? 0, "organization", "organizations")
							: "\u00a0"}
					</p>
				</div>
				<div class="ph-actions">
					<Button aria-haspopup="dialog" onClick={() => setOpened(true)}>
						<Plus />
						New organization
					</Button>
				</div>
			</div>
			<Show
				when={rows()}
				fallback={
					<Show when={bootstrap().error} fallback={<Skeleton size="card" />}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">Organizations did not load</span>
							<Button size="sm" onClick={() => bootstrap().refetch()}>
								Retry
							</Button>
						</Banner>
					</Show>
				}
			>
				{(list) => (
					<Table fold aria-label="Your organizations">
						<thead>
							<tr>
								<th scope="col">Organization</th>
								<th scope="col" data-align="end">
									Projects
								</th>
								<th scope="col">
									<span class="sr-only">Actions</span>
								</th>
							</tr>
						</thead>
						<tbody>
							<For each={list()}>
								{(organization) => (
									<tr class="rowlink">
										<td data-fold="l1">
											<a
												class="namelink rowlink-name"
												href={settingsHref(organization.slug)}
											>
												{organization.name}
											</a>
										</td>
										<td data-fold="n1" data-align="end" class="muted">
											{organization.projects
												? counted(organization.projects, "project", "projects")
												: "No projects"}
										</td>
										<td data-fold="act" class="act">
											<Show when={!organization.projects}>
												<a class="lnk" href={newProjectHref(organization.slug)}>
													New project
												</a>
											</Show>
											<a class="lnk" href={settingsHref(organization.slug)}>
												Settings
											</a>
										</td>
									</tr>
								)}
							</For>
						</tbody>
					</Table>
				)}
			</Show>
			<Show when={opened() || adding()}>
				<NewOrganization
					onDismiss={close}
					leave={props.leave ?? ((href) => navigate(href, { resolve: false }))}
				/>
			</Show>
		</Frame>
	);
};

export default Organizations;

export { nameProblem, slugOf } from "../account/organizations";

const NewOrganization = (props: {
	onDismiss: () => void;
	leave: (href: string) => void;
}) => {
	const { api, reader } = useAccount();
	const client = useQueryClient();
	const flush = useContext(FlushContext);
	const [name, setName] = createSignal("");
	const problem = () => nameProblem(name());
	const [creating, setCreating] = createSignal(false);
	const [failed, setFailed] = createSignal<{ error: unknown }>();
	const create = async () => {
		setCreating(true);
		setFailed(undefined);
		let made: JsonOrganization;
		try {
			made = (
				await api.send<JsonOrganization>("POST", "/v0/organizations", {
					name: name().trim(),
				})
			).data;
		} catch (error) {
			setFailed({ error });
			setCreating(false);
			return;
		}
		const { uuid, name: madeName, slug } = made;
		client.setQueryData<JsonConsoleUser>(
			consoleUserQuery(api, reader().user.uuid).queryKey,
			(user) =>
				user && {
					...user,
					organizations: [
						...user.organizations,
						{ uuid, name: madeName, slug },
					],
				},
		);
		// Leaving the app first would keep the bootstrap without it.
		await flush();
		props.leave(newProjectHref(slug));
	};
	return (
		<Dialog aria-labelledby="no-title" onDismiss={props.onDismiss}>
			<form
				class="ui-dialog-form"
				onSubmit={(event) => {
					event.preventDefault();
					if (name().trim() && !problem()) {
						create();
					}
				}}
			>
				<div class="ui-dialog-head">
					<h2 id="no-title">New organization</h2>
					<Button
						variant="ghost"
						iconOnly
						aria-label="Close"
						onClick={props.onDismiss}
					>
						<Icon name="close" />
					</Button>
				</div>
				<div class="ui-dialog-body">
					<div class="field">
						<label class="flab" for="no-name">
							Name
						</label>
						<TextInput
							id="no-name"
							value={name()}
							onInput={(event) => setName(event.currentTarget.value)}
							autocomplete="organization"
							autofocus
							aria-describedby="no-name-e"
							aria-invalid={problem() ? true : undefined}
						/>
						<Show when={problem()}>
							{(message) => (
								<p class="ferror" id="no-name-e">
									{message()}
								</p>
							)}
						</Show>
					</div>
					<div class="field">
						<span class="flab">Slug</span>
						<span class="derived">{slugOf(name())}</span>
					</div>
					<Show when={failed()}>
						{(failure) => (
							<p class="ferror" role="alert">
								{failureOf(
									failure().error,
									"Bencher did not make the organization",
								)}
							</p>
						)}
					</Show>
				</div>
				<div class="ui-dialog-foot">
					<Button onClick={props.onDismiss}>Cancel</Button>
					<Button
						type="submit"
						variant="primary"
						disabled={!name().trim() || problem() !== undefined || creating()}
					>
						Create organization
					</Button>
				</div>
			</form>
		</Dialog>
	);
};

const Plus = () => (
	<svg
		class="ui-icon"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M12 5v14M5 12h14" />
	</svg>
);
