import { describe, expect, test } from "vitest";
import {
	GitHubLinkStatus,
	type JsonConsoleProject,
	Visibility,
} from "../types/bencher";
import { consoleGitHub } from "./github";
import { publicGitHub } from "./commit";

const HASH = "9c1f2e4b5a6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f";

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
		expect(publicGitHub(() => url).commit(HASH)).toBe(
			repository && `https://github.com/${repository}/commit/${HASH}`,
		);
	});
});

describe("consoleGitHub", () => {
	// Kills a project URL that wins over the linked repository, a link the
	// console ignores, and a link that hides a GitHub project URL when absent.
	test("links a hash through the linked repository, else a GitHub project URL", () => {
		const commit = (bootstrap: JsonConsoleProject) =>
			consoleGitHub(() => bootstrap).commit(HASH);
		expect(
			commit(
				bootstrapOf({
					repository: "pompeii-llc/rosti",
					url: "https://github.com/pompeii-llc/hashbrown",
				}),
			),
		).toBe(`https://github.com/pompeii-llc/rosti/commit/${HASH}`);
		expect(
			commit(
				bootstrapOf({
					repository: "pompeii-llc/rosti",
					url: "https://hashbrown.example",
				}),
			),
		).toBe(`https://github.com/pompeii-llc/rosti/commit/${HASH}`);
		expect(
			commit(bootstrapOf({ url: "https://github.com/pompeii-llc/hashbrown/" })),
		).toBe(`https://github.com/pompeii-llc/hashbrown/commit/${HASH}`);
		expect(
			commit(bootstrapOf({ url: "https://hashbrown.example" })),
		).toBeUndefined();
		expect(commit(bootstrapOf({}))).toBeUndefined();
		expect(consoleGitHub(() => undefined).commit(HASH)).toBeUndefined();
	});

	// Kills an ask for a reader who may not manage the project, one for a
	// connected link or a server that cannot link, and the wrong words or place.
	test("asks a Maintainer to link an unlinked project and reconnect a disconnected one", () => {
		const ask = (bootstrap: JsonConsoleProject) =>
			consoleGitHub(() => bootstrap).ask?.();
		const connections = "/console/projects/hashbrown/connections";
		expect(ask(bootstrapOf({ link: GitHubLinkStatus.Unlinked }))).toEqual({
			label: "Link GitHub",
			href: connections,
		});
		expect(ask(bootstrapOf({ link: GitHubLinkStatus.Disconnected }))).toEqual({
			label: "Reconnect GitHub",
			href: connections,
		});
		expect(
			ask(bootstrapOf({ link: GitHubLinkStatus.Connected })),
		).toBeUndefined();
		expect(ask(bootstrapOf({}))).toBeUndefined();
		for (const link of [
			GitHubLinkStatus.Unlinked,
			GitHubLinkStatus.Disconnected,
		]) {
			expect(ask(bootstrapOf({ link, manage: false }))).toBeUndefined();
		}
		expect(consoleGitHub(() => undefined).ask?.()).toBeUndefined();
	});
});
