import Icon from "@bencherdev/ui/Icon";
import { Show } from "solid-js";
import type { GitHubLinks } from "./github";

/** Opens a hash's commit on GitHub, beside the hash, when the project has a repository. */
const CommitLink = (props: {
	href: string | undefined;
	/** How it sits in its line, by default reaching into the space around it. */
	style?: Record<string, string>;
}) => (
	<Show when={props.href}>
		{(href) => (
			<a
				class="ui-button"
				style={props.style ?? IN_LINE}
				data-variant="ghost"
				data-size="sm"
				data-icon-only
				href={href()}
				target="_blank"
				rel="noopener noreferrer"
				aria-label="Open commit on GitHub"
			>
				<Icon name="github" />
			</a>
		)}
	</Show>
);

export default CommitLink;

/** A small button that reaches into the space around its line, so the line keeps its height. */
const IN_LINE = {
	"margin-block": "calc(8px - var(--ui-button-size) / 2)",
	"vertical-align": "middle",
};

/** On the public plot and its embed, which read no bootstrap: the project URL alone, and no ask. */
export const publicGitHub = (url: () => string | undefined): GitHubLinks => ({
	commit: (hash) => commitOf(repositoryOf(url()), hash),
});

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
