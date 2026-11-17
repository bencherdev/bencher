import { expect } from "vitest";
import {
	Adapter,
	GitHubLinkStatus,
	type JsonConsoleProject,
	type JsonConsoleRun,
	type JsonConsoleRunJobs,
	type JsonConsoleRunReport,
	type JsonConsoleRunSummary,
	Visibility,
} from "../../types/bencher";

const START = Date.parse("2026-09-13T21:16:00Z");
const HOUR = 60 * 60 * 1_000;

const uuid = (index: number) =>
	`00000000-0000-4000-8000-${String(index).padStart(12, "0")}`;

const branch = {
	uuid: "branch",
	name: "main",
	slug: "main",
	head: "head",
} as JsonConsoleRunSummary["branch"];

/** A run as the list endpoint returns it, the `index`th newest. */
export const runFixture = (
	index: number,
	fields: Fields<JsonConsoleRunSummary> = {},
): JsonConsoleRunSummary =>
	({
		uuid: uuid(index),
		number: 100 - index,
		hash: `9c1f2e4${"0".repeat(33)}`,
		branch,
		start_time: START - index * HOUR,
		end_time: START - index * HOUR + 120_000,
		reports: 1,
		testbeds: ["ubuntu-latest"],
		testbed_count: 1,
		adapters: ["json"],
		adapter_count: 1,
		authors: [{ kind: "user", name: "Muriel Bagge" }],
		author_count: 1,
		lines: 36,
		alerts: { total: 0, active: 0 },
		...fields,
	}) as JsonConsoleRunSummary;

/** A report of a run, the `index`th by start. */
export const runReportFixture = (
	index: number,
	fields: Fields<JsonConsoleRunReport> = {},
): JsonConsoleRunReport =>
	({
		order: index + 1,
		uuid: uuid(1_000 + index),
		testbed: { uuid: "testbed", name: "ubuntu-latest", slug: "ubuntu-latest" },
		adapter: "json",
		start_time: START + index * 1_000,
		end_time: START + index * 1_000 + 120_000,
		lines: 36,
		alerts: { total: 0, active: 0 },
		author: { kind: "user", name: "Muriel Bagge" },
		...fields,
	}) as JsonConsoleRunReport;

/** A run's page as the run endpoint returns it, with `reports` as its page of reports. */
export const runPageFixture = (
	reports: JsonConsoleRunReport[],
	fields: Fields<JsonConsoleRun> = {},
): JsonConsoleRun =>
	({
		uuid: uuid(0),
		number: 248,
		hash: `9c1f2e4${"0".repeat(33)}`,
		branch,
		start_time: START,
		end_time: START + 848_000,
		total: reports.length,
		testbeds: 1,
		lines: 36 * reports.length,
		alerts: { total: 0, active: 0 },
		reports,
		...fields,
	}) as JsonConsoleRun;

/** Some of a response's fields, an optional one left out by `undefined`. */
type Fields<T> = { [K in keyof T]?: T[K] | undefined };

/** A run as wide as a likely one reads: a long branch, adapter, and author, and three-digit alerts. */
export const likelyRun = (
	index: number,
	fields: Fields<JsonConsoleRunSummary> = {},
) =>
	runFixture(index, {
		branch: { ...branch, name: "feature-simd", slug: "feature-simd" },
		adapters: [Adapter.RustCriterion],
		alerts: { total: 456, active: 123 },
		...fields,
	});

/** Likely runs at `now`: one finished 11 months ago, the widest time a row shows, then the longest Running and Failed pills. */
export const likelyRuns = (now: number) => {
	const jobs = (fields: Partial<JsonConsoleRunJobs>) =>
		({
			queued: 0,
			running: 0,
			failed: 0,
			canceled: 0,
			...fields,
		}) as JsonConsoleRunJobs;
	const start_time = now - 30 * HOUR;
	const failed = (index: number, took: number) =>
		likelyRun(index, {
			reports: 2,
			start_time,
			end_time: start_time + took,
			jobs: jobs({ failed: 2 }),
		});
	const old = now - 335 * 24 * HOUR;
	return [
		likelyRun(0, { start_time: old, end_time: old + 120_000 }),
		likelyRun(1, {
			reports: 2,
			jobs: jobs({ running: 2, since: now - 59 * 60_000 }),
		}),
		failed(2, 59 * 60_000 + 59_000),
		failed(3, 25 * HOUR + 59 * 60_000),
	];
};

/** A run's report as wide as a likely one reads: a long adapter and three-digit alerts. */
export const likelyReport = (
	index: number,
	fields: Fields<JsonConsoleRunReport> = {},
) =>
	runReportFixture(index, {
		adapter: Adapter.RustCriterion,
		alerts: { total: 456, active: 123 },
		...fields,
	});

/** How far `cell`'s content, or anything in it, runs past its box. */
export const overrun = (cell: Element) =>
	Math.max(
		0,
		...[cell, ...cell.querySelectorAll("*")].map(
			(element) => element.scrollWidth - element.clientWidth,
		),
	);

/** The text of each of `row`'s cells, in `columns` (by header) if given, that the reader sees cut off. */
export const cutCells = (row: Element, columns?: readonly string[]) => {
	const headers = [
		...(row.closest("table")?.querySelectorAll("thead th") ?? []),
	].map((th) => th.textContent?.trim() ?? "");
	return [...row.querySelectorAll("td")]
		.filter(
			(cell, index) =>
				(!columns || columns.includes(headers[index] ?? "")) &&
				overrun(cell) > 0.5,
		)
		.map((cell) => `${headers[cell.cellIndex]}: ${cell.textContent?.trim()}`);
};

/** Each header of `table` whose text and padding are wider than its box, which a header's own scrollWidth never shows. */
export const cutHeaders = (table: Element) =>
	[...table.querySelectorAll<HTMLElement>("thead th")]
		.filter((th) => {
			const text = document.createRange();
			text.selectNodeContents(th);
			const style = getComputedStyle(th);
			return (
				text.getBoundingClientRect().width +
					Number.parseFloat(style.paddingLeft) +
					Number.parseFloat(style.paddingRight) >
				th.getBoundingClientRect().width + 0.5
			);
		})
		.map((th) => th.textContent?.trim());

/** A commit's title, as GitHub gives its first line. */
export const COMMIT_TITLE = "Fold the hash browns into the rosti";

/** A title longer than any box that shows it. */
export const LONG_COMMIT_TITLE =
	`Fold the hash browns into the rosti ${"and keep them crisp ".repeat(12)}`.trim();

/** A project's bootstrap as the commit row reads it: its repository, project URL, link, and whether the reader may manage it. */
export const commitBootstrap = (
	fields: {
		repository?: string;
		url?: string;
		link?: GitHubLinkStatus;
		manage?: boolean;
	} = {},
): JsonConsoleProject => ({
	project: {
		uuid: "p",
		organization: "org-uuid",
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
	organization: { uuid: "org-uuid", name: "Pompeii LLC", slug: "pompeii-llc" },
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

/** What a commit row shows: each part's text, and where a link leads; muted for words that link nowhere. */
export interface ShownCommit {
	muted?: boolean;
	parts: (readonly [string] | readonly [string, string])[];
}

/** Each row of the commit row's table, for a hash: the project, the title, and what the row shows. */
export const commitRowCases = (
	hash: string,
): [
	string,
	Parameters<typeof commitBootstrap>[0],
	string | undefined,
	ShownCommit,
][] => {
	const commit = `https://github.com/pompeii-llc/rosti/commit/${hash}`;
	const connections = "/console/projects/hashbrown/connections";
	const repository = "pompeii-llc/rosti";
	return [
		[
			"the title of a linked project's commit",
			{ repository, link: GitHubLinkStatus.Connected, manage: false },
			COMMIT_TITLE,
			{ parts: [[COMMIT_TITLE, commit]] },
		],
		[
			"View commit for a known repository with no title",
			{ url: "https://github.com/pompeii-llc/rosti", manage: false },
			undefined,
			{ parts: [["View commit", commit]] },
		],
		[
			"Link GitHub to a Maintainer of an unlinked project",
			{ link: GitHubLinkStatus.Unlinked },
			undefined,
			{ parts: [["Link GitHub", connections]] },
		],
		[
			"the title, then Reconnect GitHub, to a Maintainer of a disconnected project",
			{ repository, link: GitHubLinkStatus.Disconnected },
			COMMIT_TITLE,
			{
				parts: [
					[COMMIT_TITLE, commit],
					["Reconnect GitHub", connections],
				],
			},
		],
		[
			"the muted ask to a reader of an unlinked project with no repository",
			{ link: GitHubLinkStatus.Unlinked, manage: false },
			undefined,
			{ muted: true, parts: [["Ask a Maintainer to link GitHub"]] },
		],
		[
			"the muted ask of an admin on a server without the GitHub App, a title, or a repository",
			{},
			undefined,
			{ muted: true, parts: [["Ask an admin to add the GitHub App"]] },
		],
	];
};

/** The commit row in `root` as a reader meets it, or undefined with none: its logo, whether the logo takes the muted color, and each part's text, with where a link leads. */
export const shownCommit = (root: ParentNode | null | undefined) => {
	const row = root?.querySelector(".cm-row");
	if (!row) {
		return undefined;
	}
	const logo = row.firstElementChild;
	return {
		logo: logo?.getAttribute("data-icon"),
		muted: logo !== null && colorOf(logo) === mutedColor(),
		parts: [...row.querySelectorAll(":scope > .cm-text, :scope > .cm-ask")].map(
			(part) =>
				part.tagName === "A"
					? [part.textContent, part.getAttribute("href")]
					: [part.textContent],
		),
	};
};

/** What `shownCommit` reads for a case of `commitRowCases`, with its links as plain text where `links` is false. */
export const expectedCommit = (shown: ShownCommit | undefined, links = true) =>
	shown && {
		logo: "github",
		muted: shown.muted ?? false,
		parts: shown.parts.map((part) => (links ? [...part] : [part[0]])),
	};

/** Each link of the commit row in `root`, by its text, with the tab it opens in: a commit opens a new tab, as GitHub does, and an ask the same tab. */
export const commitTabs = (root: ParentNode | null | undefined) =>
	[...(root?.querySelectorAll(".cm-row > a") ?? [])].map((link) => [
		link.textContent,
		link.getAttribute("target"),
		link.getAttribute("rel"),
	]);

/** What `commitTabs` reads for a case of `commitRowCases`. */
export const expectedTabs = (shown: ShownCommit) =>
	shown.parts
		.filter((part) => part.length === 2)
		.map(([text]) =>
			text === "Link GitHub" || text === "Reconnect GitHub"
				? [text, null, null]
				: [text, "_blank", "noopener noreferrer"],
		);

/** The number of lines `element`'s text takes. */
export const textLines = (element: Element | null | undefined) => {
	if (!element) {
		return 0;
	}
	const range = document.createRange();
	range.selectNodeContents(element);
	return new Set(
		[...range.getClientRects()].map((rect) => Math.round(rect.top)),
	).size;
};

const colorOf = (element: Element) => getComputedStyle(element).color;

/** The color `value` resolves to in the page's theme. */
const resolvedColor = (value: string) => {
	const probe = document.createElement("span");
	probe.style.color = value;
	document.body.append(probe);
	const color = colorOf(probe);
	probe.remove();
	return color;
};

/** The muted text's color in the page's theme. */
const mutedColor = () => resolvedColor("var(--color-text-muted)");

/** A title row's alert pill as a reader sees it: whether it takes a box, its words, its dot, and its colors. */
export const alertPill = (row: ParentNode | null | undefined) => {
	const pill = row?.querySelector(".rp-alerts");
	if (!pill) {
		return undefined;
	}
	const style = getComputedStyle(pill);
	const box = pill.getBoundingClientRect();
	return {
		shown: box.width > 0 && box.height > 0,
		text: pill.textContent,
		dot: pill.querySelector(".lr-alertdot") !== null,
		color: style.color,
		border: style.borderTopColor,
	};
};

/** The alert pill of `total` alerts: red with a dot once there is one, else muted. */
export const expectedAlertPill = (total: number) =>
	total === 0
		? {
				shown: true,
				text: "0 alerts",
				dot: false,
				color: mutedColor(),
				border: resolvedColor("var(--color-border)"),
			}
		: {
				shown: true,
				text: total === 1 ? "1 alert" : `${total} alerts`,
				dot: true,
				color: resolvedColor("var(--color-text-error)"),
				border: resolvedColor("var(--color-error)"),
			};

/** The page's layout shift from now until the returned stop, which waits 2 frames for the last of it. */
export const watchShift = () => {
	let shift = 0;
	const add = (entries: PerformanceEntryList) => {
		for (const entry of entries) {
			shift += (entry as unknown as { value: number }).value;
		}
	};
	const observer = new PerformanceObserver((list) => add(list.getEntries()));
	observer.observe({ type: "layout-shift", buffered: false });
	return async () => {
		for (let frame = 0; frame < 2; frame += 1) {
			await new Promise<void>((resolve) =>
				requestAnimationFrame(() => setTimeout(resolve, 0)),
			);
		}
		add(observer.takeRecords());
		observer.disconnect();
		return shift;
	};
};

/** A phone's pills line around its alert pill, undefined with no pill. */
export const pillsBand = (row: Element | null | undefined) => {
	const title = row?.querySelector(".rp-titlemain")?.getBoundingClientRect();
	const line = row?.querySelector(".rp-titlepills")?.getBoundingClientRect();
	const pill = row?.querySelector(".rp-alerts")?.getBoundingClientRect();
	const bottom = row?.getBoundingClientRect().bottom;
	if (!title || !line || !pill || bottom === undefined) {
		return undefined;
	}
	return {
		/** How far below the title's line the pills line starts. */
		gap: line.top - title.bottom,
		/** How far the pill sits from the line's middle. */
		offCenter: Math.abs(
			(line.top + line.bottom) / 2 - (pill.top + pill.bottom) / 2,
		),
		/** How much taller the line is than its pill. */
		overPill: line.height - pill.height,
		/** How far the row reaches below the pills line. */
		below: bottom - line.bottom,
	};
};

const middle = (box: DOMRect) => (box.top + box.bottom) / 2;

/** The box of `element`'s first words, cut to its own box, as a reader sees them. */
export const wordsBox = (element: Element) => {
	const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT, {
		acceptNode: (node) =>
			node.textContent?.trim()
				? NodeFilter.FILTER_ACCEPT
				: NodeFilter.FILTER_SKIP,
	});
	const range = document.createRange();
	range.selectNodeContents(walker.nextNode() ?? element);
	const words = range.getBoundingClientRect();
	const box = element.getBoundingClientRect();
	const left = Math.max(words.left, box.left);
	return new DOMRect(
		left,
		words.top,
		Math.min(words.right, box.right) - left,
		words.height,
	);
};

/** Every 2 px from 1 px inside `from`, then 0.5 px inside `to`, so the far edge is pressed too. */
const presses = (from: number, to: number) => {
	const at: number[] = [];
	for (let v = from + 1; v < to - 0.5; v += 2) {
		at.push(v);
	}
	return [...at, to - 0.5];
};

/** Asserts that a press every 2 px inside `area`, and along its far edges, lands on `target`. */
export const expectPressesLandOn = (
	area: DOMRect,
	target: Element | null | undefined,
	label: string,
) => {
	expect(target, label).toBeTruthy();
	expect(area.width * area.height, label).toBeGreaterThan(0);
	for (const x of presses(area.left, area.right)) {
		for (const y of presses(area.top, area.bottom)) {
			const hit = document.elementFromPoint(x, y);
			expect(
				hit !== null && (target as Element).contains(hit),
				`${label} at ${x}, ${y}`,
			).toBe(true);
		}
	}
};

/** A page's commit row as a finger meets it: how much taller it is than its line, and each link's words' distance from the logo's middle, its target's height, and how far the target reaches below the row. */
export const commitReach = (row: Element | null | undefined) => {
	if (!row) {
		return undefined;
	}
	const box = row.getBoundingClientRect();
	const logo = row.querySelector('[data-icon="github"]');
	return {
		overLine: box.height - Number.parseFloat(getComputedStyle(row).lineHeight),
		links: [...row.querySelectorAll(".cm-row > a")].map((link) => {
			const target = link.getBoundingClientRect();
			return {
				text: link.textContent,
				offLogo: logo
					? Math.abs(
							middle(wordsBox(link)) - middle(logo.getBoundingClientRect()),
						)
					: Number.POSITIVE_INFINITY,
				height: target.height,
				below: target.bottom - box.bottom,
			};
		}),
	};
};
