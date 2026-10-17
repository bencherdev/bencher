import { describe, expect, test } from "vitest";
import type {
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../types/bencher";
import { searchRows, switcherRows } from "./model";

const NOW = Date.parse("2026-09-14T00:00:00Z");
const HOUR = 60 * 60 * 1000;
/** A project with no reports, as the API sends it. */
const NEVER = null as unknown as number;

const POMPEII = { uuid: "o1", name: "Pompeii LLC", slug: "pompeii-llc" };
const DINER = { uuid: "o2", name: "Bagge Diner", slug: "bagge-diner" };

const project = (
	name: string,
	fields: Partial<JsonConsoleUserProject> = {},
): JsonConsoleUserProject => ({
	uuid: name,
	name,
	slug: name.toLowerCase().replace(/ /g, "-"),
	organization: POMPEII.uuid,
	bmf_version: 1,
	last_report: NOW - HOUR,
	active_alerts: 0,
	...fields,
});

const user = (
	projects: JsonConsoleUserProject[],
	organizations = [POMPEII],
): JsonConsoleUser => ({
	uuid: "u",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	organizations,
	projects,
});

const slugs = (rows: { slug: string }[]) => rows.map(({ slug }) => slug);

describe("order", () => {
	// Kills an order by run time alone, memory read oldest first, and slugs the
	// memory knows that the reader no longer has.
	test("projects this browser visited lead, most recent first", () => {
		const rows = switcherRows(
			user([
				project("Hashbrown", { last_report: NOW - HOUR }),
				project("Tater Tot", { last_report: NOW - 9 * HOUR }),
				project("Waffle Fries", { last_report: NOW - 2 * HOUR }),
			]),
			["tater-tot", "gone", "waffle-fries"],
			"",
			NOW,
		);
		expect(slugs(rows)).toEqual(["tater-tot", "waffle-fries", "hashbrown"]);
	});

	// Kills unvisited projects ordered by name or by oldest run, and a project
	// that never ran ahead of one that did.
	test("the rest follow by newest run, those that never ran last", () => {
		const rows = switcherRows(
			user([
				project("Cornbread", { last_report: NEVER }),
				project("Gizmo", { last_report: NOW - 3 * HOUR }),
				project("Anvil", { last_report: NOW - 30 * HOUR }),
				project("Dustdevil", { last_report: NOW - HOUR }),
			]),
			[],
			"",
			NOW,
		);
		expect(slugs(rows)).toEqual(["dustdevil", "gizmo", "anvil", "cornbread"]);
	});
});

describe("row", () => {
	// Kills an organization named for a reader of one, and one left out for a
	// reader of two.
	test("names the organization only for a reader of two or more", () => {
		const projects = [
			project("Hashbrown"),
			project("Curly Fries", { organization: DINER.uuid }),
		];
		expect(
			switcherRows(user(projects, [POMPEII]), [], "", NOW).map(
				({ organization }) => organization,
			),
		).toEqual([undefined, undefined]);
		expect(
			switcherRows(user(projects, [POMPEII, DINER]), [], "", NOW).map(
				({ organization }) => organization,
			),
		).toEqual(["Bagge Diner", "Pompeii LLC"]);
	});

	// Kills a version 0 project sent to the new console, a version 1 project
	// sent to the classic one, and a missing run read as a time.
	test("a version 0 project is classic, and a project that never ran has no age", () => {
		const [classic, never, ran] = switcherRows(
			user([
				project("Crinkle Cut", { bmf_version: 0, last_report: NOW - HOUR }),
				project("Cornbread", { last_report: NEVER }),
				project("Waffle Fries", { last_report: NOW - 2 * HOUR }),
			]),
			["crinkle-cut", "cornbread", "waffle-fries"],
			"",
			NOW,
		);
		expect(classic).toMatchObject({
			classic: true,
			href: "/console/projects/crinkle-cut/plots",
		});
		expect(never).toMatchObject({
			classic: false,
			age: undefined,
			href: "/next/console/projects/cornbread/",
		});
		expect(ran).toMatchObject({ classic: false, age: "2h ago" });
	});

	// Kills alerts taken from another project, and a current project picked by
	// anything but the page's slug.
	test("carries its active alerts and marks the current project", () => {
		const rows = switcherRows(
			user([project("Hashbrown", { active_alerts: 5 }), project("Tater Tot")]),
			[],
			"tater-tot",
			NOW,
		);
		expect(
			rows.map(({ slug, alerts, current }) => ({ slug, alerts, current })),
		).toEqual([
			{ slug: "hashbrown", alerts: 5, current: false },
			{ slug: "tater-tot", alerts: 0, current: true },
		]);
	});
});

describe("search", () => {
	const rows = switcherRows(
		user(
			[
				project("Cactus", { organization: DINER.uuid }),
				project("Hashbrown"),
				project("Cathode"),
				project("Teacake", { organization: DINER.uuid }),
			],
			[POMPEII, DINER],
		),
		[],
		"",
		NOW,
	);

	// Kills a search that is case sensitive, matches only a name's start, or
	// misses the organization.
	test("matches project and organization names anywhere, in any case", () => {
		expect(slugs(searchRows(rows, "CA").map(({ row }) => row))).toEqual([
			"cactus",
			"cathode",
			"teacake",
		]);
		expect(slugs(searchRows(rows, "diner").map(({ row }) => row))).toEqual([
			"cactus",
			"teacake",
		]);
		expect(searchRows(rows, "zeppelin")).toEqual([]);
	});

	// Kills a search that skips the organization of a reader of one, whose
	// rows do not name it.
	test("matches the organization even when the rows do not name it", () => {
		const one = switcherRows(
			user([project("Hashbrown"), project("Tater Tot")]),
			[],
			"",
			NOW,
		);
		const matched = searchRows(one, "pompeii");
		expect(slugs(matched.map(({ row }) => row))).toEqual([
			"hashbrown",
			"tater-tot",
		]);
		expect(matched[0]?.organization).toBeUndefined();
	});

	// Kills a highlight off by the query's length, and one on the organization
	// when the name already matched.
	test("marks where each row matched", () => {
		const [cactus] = searchRows(rows, "act");
		expect(cactus?.name).toEqual(["C", "act", "us"]);
		expect(cactus?.organization).toEqual(["Bagge Diner", "", ""]);
		const [byOrganization] = searchRows(rows, "bagge");
		expect(byOrganization?.name).toEqual(["Cactus", "", ""]);
		expect(byOrganization?.organization).toEqual(["", "Bagge", " Diner"]);
	});

	// Kills a blank or spaced query that hides rows.
	test("an empty query keeps every row in order", () => {
		expect(slugs(searchRows(rows, "  ").map(({ row }) => row))).toEqual(
			slugs(rows),
		);
	});
});
