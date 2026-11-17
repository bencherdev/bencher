import Icon from "@bencherdev/ui/Icon";
import Truncate from "@bencherdev/ui/Truncate";
import { type JSX, Show } from "solid-js";
import type { CommitRow, GitHubAsk, GitHubLinks } from "./github";

/** The GitHub logo, then a commit's title or View commit, an ask, or muted words, on one line; links only where `links`, as a pinned readout and a page have them. */
const CommitLine = (props: { row: CommitRow; links: boolean }) => {
	const askOf = (ask: GitHubAsk) => (
		<Linked href={props.links ? ask.href : undefined} class="cm-ask">
			{ask.label}
		</Linked>
	);
	const content = (): JSX.Element => {
		const row = props.row;
		switch (row.kind) {
			case "commit":
				return (
					<>
						<Linked
							href={props.links ? row.href : undefined}
							class="cm-text"
							away
						>
							<Truncate text={row.text} />
						</Linked>
						<Show when={row.ask}>
							{(ask) => (
								<>
									<span class="cm-dot" aria-hidden="true">
										·
									</span>
									{askOf(ask())}
								</>
							)}
						</Show>
					</>
				);
			case "ask":
				return askOf(row.ask);
			case "muted":
				return (
					<span class="cm-text">
						<Truncate text={row.text} />
					</span>
				);
		}
	};
	return (
		<span
			class="cm-row"
			data-muted={props.row.kind === "muted" ? "" : undefined}
		>
			<Icon name="github" />
			{content()}
		</span>
	);
};

export default CommitLine;

/** A link where there is somewhere to go, else the same words as text. */
const Linked = (props: {
	href: string | undefined;
	class: string;
	/** Opens in a new tab, as GitHub does. */
	away?: boolean;
	children: JSX.Element;
}) => (
	<Show
		when={props.href}
		fallback={<span class={props.class}>{props.children}</span>}
	>
		{(href) => (
			<a
				class={`lnk ${props.class}`}
				href={href()}
				{...(props.away
					? { target: "_blank", rel: "noopener noreferrer" }
					: {})}
			>
				{props.children}
			</a>
		)}
	</Show>
);

/** On the public plot and its embed, which read no bootstrap: the project URL alone, and no ask. */
export const publicGitHub = (url: () => string | undefined): GitHubLinks => ({
	row: (hash, title) =>
		commitRow(title, commitOf(repositoryOf(url()), hash), undefined),
});

/** The title or View commit, once there is either to show. */
export const commitRow = (
	title: string | undefined,
	href: string | undefined,
	ask: GitHubAsk | undefined,
): CommitRow | undefined =>
	title || href
		? { kind: "commit", text: title || "View commit", href, ask }
		: undefined;

export const commitOf = (repository: string | undefined, hash: string) =>
	repository && `https://github.com/${repository}/commit/${hash}`;

const REPOSITORY_URL =
	/^https:\/\/github\.com\/([A-Za-z0-9-]+\/[A-Za-z0-9._-]+)\/?$/;

/** The `owner/repo` of a URL in the one shape `https://github.com/{owner}/{repo}`, with an optional trailing slash. */
export const repositoryOf = (url: string | undefined) => {
	const repository = url?.match(REPOSITORY_URL)?.[1];
	const name = repository?.split("/")[1];
	return name && name !== "." && name !== ".." && !name.endsWith(".git")
		? repository
		: undefined;
};
