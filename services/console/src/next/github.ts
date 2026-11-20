import type { GitHubLinkStatus, JsonConsoleProject } from "../types/bencher";
import { commitOf, commitRow, repositoryUrlOf } from "./commit";
import { connectionsPath } from "./paths";

/** In the console: the linked repository on the server's GitHub, else a github.com project URL; the ask for a Maintainer; for a reader who cannot link, the ask of a Maintainer; and on a server without the GitHub App, the ask of an admin. */
export const consoleGitHub = (
	bootstrap: () => JsonConsoleProject | undefined,
): GitHubLinks => ({
	row: (hash, title) => {
		const project = bootstrap();
		if (!project) {
			return undefined;
		}
		const link = project.project.github_link;
		const label = link && ASKS[link];
		const ask =
			project.permissions.manage && label
				? { label, href: connectionsPath(project.project.slug) }
				: undefined;
		const href = commitOf(
			project.github_repository_url ?? repositoryUrlOf(project.project.url),
			hash,
		);
		const row = commitRow(title, href, ask);
		if (row) {
			return row;
		}
		if (ask) {
			return { kind: "ask", ask };
		}
		if (link === undefined) {
			return { kind: "muted", text: "Ask an admin to add the GitHub App" };
		}
		return label === "Link GitHub"
			? { kind: "muted", text: "Ask a Maintainer to link GitHub" }
			: undefined;
	},
});

/** What a hash's commit row shows, from its commit's title; none before the console knows the project, or on a public plot with no title or repository. */
export interface GitHubLinks {
	row: (hash: string, title: string | undefined) => CommitRow | undefined;
}

/** The title or View commit, opening the commit where its repository is known, with any ask at the end; an ask alone; or muted words that link nowhere. */
export type CommitRow =
	| {
			kind: "commit";
			text: string;
			href: string | undefined;
			ask: GitHubAsk | undefined;
	  }
	| { kind: "ask"; ask: GitHubAsk }
	| { kind: "muted"; text: MutedText };

/** What a row says where it has nothing to open and no ask the reader can follow. */
export type MutedText =
	| "No commit"
	| "Ask a Maintainer to link GitHub"
	| "Ask an admin to add the GitHub App";

export interface GitHubAsk {
	label: "Link GitHub" | "Reconnect GitHub";
	href: string;
}

const ASKS: Record<GitHubLinkStatus, GitHubAsk["label"] | undefined> = {
	unlinked: "Link GitHub",
	connected: undefined,
	disconnected: "Reconnect GitHub",
};
