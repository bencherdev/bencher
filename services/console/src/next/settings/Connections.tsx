import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import EmptyState from "@bencherdev/ui/EmptyState";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Timestamp from "@bencherdev/ui/Timestamp";
import Token from "@bencherdev/ui/Token";
import { type QueryClient, useQueryClient } from "@tanstack/solid-query";
import { For, Show, createSignal } from "solid-js";
// Types alone: an enum's object would join the types every console page loads.
import type {
	JsonGitHubLinkUrls,
	JsonGitHubRepositories,
	JsonGitHubRepository,
	JsonLinkedRepository,
	JsonProjectGitHub,
	LinkedRepositoryReason,
} from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { apiMessage, failureOf } from "./failure";
import Head from "./Head";
import { formatDay } from "./keys";

/** What the project is connected to: GitHub first, with later integrations beside it. */
const Connections = () => {
	const { api, slug } = useProject();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	// The API reads and changes a connection only for a reader with manage.
	const manage = () => bootstrap().data?.permissions.manage;
	return (
		<>
			<Head slug={slug()} title="Connections" readOnly={manage() === false} />
			<GitHub api={api} slug={slug()} manage={manage()} />
		</>
	);
};

export default Connections;

/**
 * The link starts beside the shell's request, not a round after it. A
 * refusal, such as a server with no app, is the card's answer until the
 * reader retries, rather than a second round.
 */
export const prefetch = async (client: QueryClient, api: Api, slug: string) => {
	const query = githubQuery(api, slug);
	if (client.getQueryState(query.queryKey)?.status !== "error") {
		await client.prefetchQuery(query);
	}
};

const GitHub = (props: {
	api: Api;
	slug: string;
	manage: boolean | undefined;
}) => {
	const client = useQueryClient();
	const link = useQueryResult(() => ({
		...githubQuery(props.api, props.slug),
		enabled: props.manage !== false,
	}));
	const listed = () => {
		const repositories = returned();
		return repositories?.project === props.slug
			? repositories.repositories
			: undefined;
	};
	const [busy, setBusy] = createSignal(false);
	const [unlinking, setUnlinking] = createSignal<JsonLinkedRepository>();
	const [failure, setFailure] = createSignal("");
	let heading: HTMLHeadingElement | undefined;

	const leave = async (to: keyof JsonGitHubLinkUrls) => {
		setBusy(true);
		setFailure("");
		try {
			const { data } = await props.api.get<JsonGitHubLinkUrls>(
				`${githubPath(props.slug)}/urls`,
			);
			away.to(data[to]);
		} catch (error) {
			setFailure(failureOf(error, "Bencher did not start the link"));
		} finally {
			// A Back that restores this page from the browser's cache finds it ready.
			setBusy(false);
		}
	};

	const linkRepository = async (
		repository: JsonGitHubRepository,
		grant: string,
	) => {
		const key = githubQuery(props.api, props.slug).queryKey;
		setBusy(true);
		setFailure("");
		try {
			const { data } = await props.api.send<JsonLinkedRepository>(
				"PUT",
				githubPath(props.slug),
				{ grant },
			);
			// A read in flight would land over the link.
			await client.cancelQueries({ queryKey: key });
			client.setQueryData<JsonProjectGitHub>(key, { linked_repository: data });
		} catch (error) {
			setFailure(
				failureOf(error, `Bencher did not link ${repository.full_name}`),
			);
		} finally {
			// Linked or refused, the list is spent: a refusal starts over at Link repository.
			hold(undefined);
			setBusy(false);
			// The row whose Link held focus is gone.
			heading?.focus();
		}
	};

	const unlink = async (linked: JsonLinkedRepository) => {
		const key = githubQuery(props.api, props.slug).queryKey;
		setUnlinking(undefined);
		setFailure("");
		await client.cancelQueries({ queryKey: key });
		const before = client.getQueryData<JsonProjectGitHub>(key);
		client.setQueryData<JsonProjectGitHub>(key, {});
		heading?.focus();
		try {
			await props.api.send("DELETE", githubPath(props.slug));
		} catch (error) {
			client.setQueryData(key, before);
			setFailure(
				failureOf(error, `Bencher did not unlink ${linked.full_name}`),
			);
		}
	};

	const linked = () => link().data?.linked_repository;
	const disconnected = () => linked()?.status === "disconnected";
	const loaded = () => props.manage === true && link().data !== undefined;

	return (
		<section class="setcard" aria-labelledby="sec-github">
			<div class="setcard-head">
				<h2
					class="seclabel"
					id="sec-github"
					tabindex="-1"
					ref={(element) => {
						heading = element;
					}}
				>
					GitHub
				</h2>
			</div>
			<Show when={props.manage !== false}>
				<Show
					when={link().data}
					fallback={
						<LinkFailure error={link().error} retry={() => link().refetch()} />
					}
				>
					<dl class="formrows">
						<div class="formrow">
							<dt class="fl">
								<span>Repository</span>
							</dt>
							<dd class="fvalue">
								<Show
									when={linked()}
									fallback={<span class="muted">None</span>}
								>
									{(repository) => (
										<span class="mono">{repository().full_name}</span>
									)}
								</Show>
							</dd>
						</div>
						<Show when={linked()}>
							{(repository) => (
								<>
									<div class="formrow">
										<dt class="fl">
											<span>Status</span>
										</dt>
										<dd class="fvalue">
											<Show
												when={disconnected()}
												fallback={
													<Token tone="green" dot>
														Connected
													</Token>
												}
											>
												<Token tone="red" dot>
													Disconnected
												</Token>
												<Show when={repository().reason}>
													{(reason) => (
														<span class="muted">{REASONS[reason()]}</span>
													)}
												</Show>
											</Show>
										</dd>
									</div>
									<div class="formrow">
										<dt class="fl">
											<span>Linked by</span>
										</dt>
										<dd class="fvalue">
											<span>{repository().user.name}</span>
											<Timestamp datetime={repository().linked}>
												{formatDay(Date.parse(repository().linked))}
											</Timestamp>
										</dd>
									</div>
								</>
							)}
						</Show>
					</dl>
				</Show>
			</Show>
			<Show when={loaded() && listed()}>
				{(repositories) => (
					<Repositories
						repositories={repositories()}
						busy={busy()}
						onLink={linkRepository}
					/>
				)}
			</Show>
			<Show when={loaded()}>
				<div class="setcard-foot">
					<Show when={failure()}>
						<p class="ferror" role="alert">
							{failure()}
						</p>
					</Show>
					<span class="spacer" />
					<Show when={listed() || disconnected()}>
						<Button disabled={busy()} onClick={() => leave("install")}>
							Install on GitHub
						</Button>
					</Show>
					<Show
						when={linked()}
						fallback={
							<Show when={!listed()}>
								<Button
									variant="primary"
									disabled={busy()}
									onClick={() => leave("authorize")}
								>
									Link repository
								</Button>
							</Show>
						}
					>
						{(repository) => (
							<Button
								aria-haspopup="dialog"
								onClick={() => setUnlinking(repository())}
							>
								Unlink
							</Button>
						)}
					</Show>
				</div>
			</Show>
			<Show when={unlinking()} keyed>
				{(target) => (
					<Unlink
						target={target}
						onDismiss={() => setUnlinking(undefined)}
						onUnlink={() => unlink(target)}
					/>
				)}
			</Show>
		</section>
	);
};

export const githubQuery = (api: Api, slug: string) => ({
	queryKey: ["console", "github", slug],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(await api.get<JsonProjectGitHub>(githubPath(slug), signal)).data,
	retryOnMount: false,
});

// A GitHub return's grants live in memory alone, never in the cache that
// persists, so a reload drops them.
const [returned, setReturned] = createSignal<JsonGitHubRepositories>();

/** Keep the repositories a GitHub return listed, for its project's card, or forget them. */
export const hold = (repositories: JsonGitHubRepositories | undefined) =>
	setReturned(repositories);

/** Where the reader leaves for GitHub. */
export const away = { to: (url: string) => location.assign(url) };

const githubPath = (slug: string) =>
	`/v0/projects/${encodeURIComponent(slug)}/github`;

const REASONS: Record<LinkedRepositoryReason, string> = {
	uninstalled: "App uninstalled",
	suspended: "Installation suspended",
	removed: "Removed from the installation",
};

/** A server with no app answers with why; anything else may come back. */
const LinkFailure = (props: { error: Error | null; retry: () => void }) => (
	<Show when={props.error} fallback={<Skeleton size="card" />}>
		{(error) => (
			<Show
				when={(() => {
					const refused = error();
					return refused instanceof ApiError && refused.kind === "not_found"
						? refused
						: undefined;
				})()}
				fallback={
					<div class="setcard-body">
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">GitHub did not load</span>
							<Button size="sm" onClick={props.retry}>
								Retry
							</Button>
						</Banner>
					</div>
				}
			>
				{(refused) => (
					<div class="setcard-body">
						<p class="text2">{apiMessage(refused())}</p>
					</div>
				)}
			</Show>
		)}
	</Show>
);

/** What a GitHub return listed: a Link for each repository with a grant. */
const Repositories = (props: {
	repositories: JsonGitHubRepository[];
	busy: boolean;
	onLink: (repository: JsonGitHubRepository, grant: string) => void;
}) => (
	<div class="setcard-body">
		<Show
			when={props.repositories.length > 0}
			fallback={<EmptyState title="No repositories" />}
		>
			<ul class="ui-list repolist" aria-label="Repositories">
				<For each={props.repositories}>
					{(repository) => (
						<li class="ui-list-item">
							<span class="mono spacer">{repository.full_name}</span>
							<Show
								when={repository.grant}
								fallback={
									<>
										<Token tone="yellow">Approval required</Token>
										<a
											href={repository.installation_url}
											rel="noopener noreferrer"
											aria-label={`Approve on GitHub: ${repository.full_name}`}
										>
											Approve on GitHub
										</a>
									</>
								}
							>
								{(grant) => (
									<Button
										size="sm"
										disabled={props.busy}
										aria-label={`Link ${repository.full_name}`}
										onClick={() => props.onLink(repository, grant())}
									>
										Link
									</Button>
								)}
							</Show>
						</li>
					)}
				</For>
			</ul>
		</Show>
	</div>
);

const Unlink = (props: {
	target: JsonLinkedRepository;
	onDismiss: () => void;
	onUnlink: () => void;
}) => (
	<Dialog
		role="alertdialog"
		aria-labelledby="ul-title"
		onDismiss={props.onDismiss}
	>
		<div class="ui-dialog-head">
			<h2 id="ul-title">Unlink {props.target.full_name}?</h2>
			<Button
				variant="ghost"
				iconOnly
				aria-label="Close"
				onClick={props.onDismiss}
			>
				<Icon name="close" />
			</Button>
		</div>
		<div class="ui-dialog-foot">
			<Button onClick={props.onDismiss}>Cancel</Button>
			<Button variant="destructive" onClick={props.onUnlink}>
				Unlink
			</Button>
		</div>
	</Dialog>
);
