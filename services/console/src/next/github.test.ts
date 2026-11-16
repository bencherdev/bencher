import { describe, expect, test } from "vitest";
import {
	GitHubLinkStatus,
	type JsonConsoleProject,
	Visibility,
} from "../types/bencher";
import { consoleGitHub } from "./github";
import { publicGitHub } from "./commit";

const HASH = "9c1f2e4b5a6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f";
const TITLE = "Fold the hash browns into the rosti";

const bootstrapOf = (fields: {
	repository?: string;
	url?: string;
	link?: GitHubLinkStatus;
	manage?: boolean;
}): JsonConsoleProject => ({
	project: {
		uuid: "p",
		organization: "o",
		name: "Hashbrown",
		slug: "hashbrown",
		...(fields.url === undefined ? {} : { url: fields.url }),
		...(fields.link === undefined ? {} : { github_link: fields.link }),
		visibility: Visibility.Public,
		bmf_version: 1,
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	},
	...(fields.repository === undefined
		? {}
		: { github_repository: fields.repository }),
	organization: { uuid: "o", name: "Pompeii LLC", slug: "pompeii-llc" },
	permissions: {
		view: true,
		create: true,
		edit: true,
		delete: true,
		manage: fields.manage ?? true,
	},
	active_alerts: 0,
	plots: 0,
});

describe("publicGitHub", () => {
	// Kills a URL taken for a repository in any shape but `https://github.com/{owner}/{repo}`,
	// with or without a trailing slash, which would link a hash to a page that is no commit.
	test.each([
		["https://github.com/pompeii-llc/hashbrown", "pompeii-llc/hashbrown"],
		["https://github.com/pompeii-llc/hash.browns/", "pompeii-llc/hash.browns"],
		[
			"https://github.com/Pompeii-LLC/hash_browns-2",
			"Pompeii-LLC/hash_browns-2",
		],
		["http://github.com/pompeii-llc/hashbrown", undefined],
		["https://www.github.com/pompeii-llc/hashbrown", undefined],
		["https://gitlab.com/pompeii-llc/hashbrown", undefined],
		["https://github.com.example/pompeii-llc/hashbrown", undefined],
		["https://github.com:443/pompeii-llc/hashbrown", undefined],
		["https://muriel@github.com/pompeii-llc/hashbrown", undefined],
		["https://github.com/pompeii-llc", undefined],
		["https://github.com/pompeii-llc/", undefined],
		["https://github.com/pompeii-llc/hashbrown//", undefined],
		["https://github.com/pompeii-llc/hashbrown.git", undefined],
		["https://github.com/pompeii-llc/hashbrown/tree/main", undefined],
		["https://github.com/pompeii-llc/hashbrown?tab=readme", undefined],
		["https://github.com/pompeii-llc/hashbrown#readme", undefined],
		["https://github.com/pompeii-llc/..", undefined],
		[" https://github.com/pompeii-llc/hashbrown", undefined],
		[undefined, undefined],
	])("%s names %s", (url, repository) => {
		expect(publicGitHub(() => url).row(HASH, undefined)).toEqual(
			repository && {
				kind: "commit",
				text: "View commit",
				href: `https://github.com/${repository}/commit/${HASH}`,
				ask: undefined,
			},
		);
	});

	// Kills a public plot that drops a title, links a title it has no
	// repository for, or shows a call to action or the ask of a Maintainer.
	test("shows the title or View commit, and never an ask", () => {
		const row = (url: string | undefined, title: string | undefined) =>
			publicGitHub(() => url).row(HASH, title);
		const repository = "https://github.com/pompeii-llc/hashbrown";
		expect(row(repository, TITLE)).toEqual({
			kind: "commit",
			text: TITLE,
			href: `https://github.com/pompeii-llc/hashbrown/commit/${HASH}`,
			ask: undefined,
		});
		expect(row(undefined, TITLE)).toEqual({
			kind: "commit",
			text: TITLE,
			href: undefined,
			ask: undefined,
		});
		expect(row("https://hashbrown.example", undefined)).toBeUndefined();
		expect(row(undefined, undefined)).toBeUndefined();
	});
});

describe("consoleGitHub", () => {
	const hrefOf = (bootstrap: JsonConsoleProject | undefined) => {
		const row = consoleGitHub(() => bootstrap).row(HASH, undefined);
		return row?.kind === "commit" ? row.href : undefined;
	};

	// Kills a project URL that wins over the linked repository, a link the
	// console ignores, and a link that hides a GitHub project URL when absent.
	test("links a hash through the linked repository, else a GitHub project URL", () => {
		expect(
			hrefOf(
				bootstrapOf({
					repository: "pompeii-llc/rosti",
					url: "https://github.com/pompeii-llc/hashbrown",
				}),
			),
		).toBe(`https://github.com/pompeii-llc/rosti/commit/${HASH}`);
		expect(
			hrefOf(
				bootstrapOf({
					repository: "pompeii-llc/rosti",
					url: "https://hashbrown.example",
				}),
			),
		).toBe(`https://github.com/pompeii-llc/rosti/commit/${HASH}`);
		expect(
			hrefOf(bootstrapOf({ url: "https://github.com/pompeii-llc/hashbrown/" })),
		).toBe(`https://github.com/pompeii-llc/hashbrown/commit/${HASH}`);
		expect(hrefOf(bootstrapOf({ url: "https://hashbrown.example" }))).toBe(
			undefined,
		);
		expect(hrefOf(bootstrapOf({}))).toBeUndefined();
		expect(hrefOf(undefined)).toBeUndefined();
	});

	const connections = "/console/projects/hashbrown/connections";
	const COMMIT = `https://github.com/pompeii-llc/rosti/commit/${HASH}`;
	const LINK = { label: "Link GitHub", href: connections };
	const RECONNECT = { label: "Reconnect GitHub", href: connections };

	// Kills a row that drops the title or links it nowhere, an ask for a reader
	// who may not manage the project or for a connected link, an ask in place
	// of a title, a muted ask for a Maintainer or a known repository, a call to
	// action from a server that cannot link GitHub, and a server without the
	// GitHub App that asks no admin.
	test.each([
		[
			"a title, connected",
			{ repository: "pompeii-llc/rosti", link: GitHubLinkStatus.Connected },
			TITLE,
			{ kind: "commit", text: TITLE, href: COMMIT, ask: undefined },
		],
		[
			"a title, connected, without manage",
			{
				repository: "pompeii-llc/rosti",
				link: GitHubLinkStatus.Connected,
				manage: false,
			},
			TITLE,
			{ kind: "commit", text: TITLE, href: COMMIT, ask: undefined },
		],
		[
			"no title, connected",
			{ repository: "pompeii-llc/rosti", link: GitHubLinkStatus.Connected },
			undefined,
			{ kind: "commit", text: "View commit", href: COMMIT, ask: undefined },
		],
		[
			"unlinked, with manage",
			{ link: GitHubLinkStatus.Unlinked },
			undefined,
			{ kind: "ask", ask: LINK },
		],
		[
			"unlinked, a project URL, with manage",
			{
				url: "https://github.com/pompeii-llc/rosti",
				link: GitHubLinkStatus.Unlinked,
			},
			undefined,
			{ kind: "commit", text: "View commit", href: COMMIT, ask: LINK },
		],
		[
			"a title, disconnected, with manage",
			{ repository: "pompeii-llc/rosti", link: GitHubLinkStatus.Disconnected },
			TITLE,
			{ kind: "commit", text: TITLE, href: COMMIT, ask: RECONNECT },
		],
		[
			"no title, disconnected, with manage",
			{ repository: "pompeii-llc/rosti", link: GitHubLinkStatus.Disconnected },
			undefined,
			{ kind: "commit", text: "View commit", href: COMMIT, ask: RECONNECT },
		],
		[
			"no title, disconnected, without manage",
			{
				repository: "pompeii-llc/rosti",
				link: GitHubLinkStatus.Disconnected,
				manage: false,
			},
			undefined,
			{ kind: "commit", text: "View commit", href: COMMIT, ask: undefined },
		],
		[
			"unlinked, no repository, without manage",
			{ link: GitHubLinkStatus.Unlinked, manage: false },
			undefined,
			{ kind: "muted", text: "Ask a Maintainer to link GitHub" },
		],
		[
			"a server without the GitHub App, a title",
			{},
			TITLE,
			{ kind: "commit", text: TITLE, href: undefined, ask: undefined },
		],
		[
			"a server without the GitHub App, a project URL",
			{ url: "https://github.com/pompeii-llc/rosti" },
			undefined,
			{ kind: "commit", text: "View commit", href: COMMIT, ask: undefined },
		],
		[
			"a server without the GitHub App, no title or repository",
			{},
			undefined,
			{ kind: "muted", text: "Ask an admin to add the GitHub App" },
		],
		[
			"a server without the GitHub App, without manage",
			{ manage: false },
			undefined,
			{ kind: "muted", text: "Ask an admin to add the GitHub App" },
		],
	] as const)("%s", (_, fields, title, row) => {
		expect(consoleGitHub(() => bootstrapOf(fields)).row(HASH, title)).toEqual(
			row,
		);
	});

	// Kills a row drawn before the console knows the project.
	test("shows no row before the bootstrap", () => {
		expect(consoleGitHub(() => undefined).row(HASH, TITLE)).toBeUndefined();
	});
});
