import type { GitHubLinkStatus, JsonConsoleProject } from "../types/bencher";
import { PROJECTS_PATH } from "./paths";
import { commitOf, repositoryOf } from "./commit";

/** In the console: the linked repository, else a GitHub project URL, and the ask for a Maintainer. */
export const consoleGitHub = (
	bootstrap: () => JsonConsoleProject | undefined,
): GitHubLinks => ({
	commit: (hash) => {
		const project = bootstrap();
		return commitOf(
			project?.github_repository ?? repositoryOf(project?.project.url),
			hash,
		);
	},
	ask: () => {
		const project = bootstrap();
		const link = project?.project.github_link;
		const label = link && ASKS[link];
		return project?.permissions.manage && label
			? { label, href: connectionsPath(project.project.slug) }
			: undefined;
	},
});

/** Where a hash opens on GitHub, and what asks a Maintainer to link the project. */
export interface GitHubLinks {
	commit: (hash: string) => string | undefined;
	ask?: () => GitHubAsk | undefined;
}

export interface GitHubAsk {
	label: "Link GitHub" | "Reconnect GitHub";
	href: string;
}

/** A project's Connections settings, where it links GitHub. */
export const connectionsPath = (project: string) =>
	`${PROJECTS_PATH}/${encodeURIComponent(project)}/connections`;

const ASKS: Record<GitHubLinkStatus, GitHubAsk["label"] | undefined> = {
	unlinked: "Link GitHub",
	connected: undefined,
	disconnected: "Reconnect GitHub",
};
