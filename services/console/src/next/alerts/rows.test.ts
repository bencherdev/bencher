import { describe, expect, test } from "vitest";
import {
	type AlertLine,
	alertLinesOf,
	alertSlots,
	distinctLines,
	groupState,
	toggleGroup,
	visibleLines,
	withKept,
	withStatuses,
} from "./rows";
import { alertId, alertsFixture, reportId } from "./testing";

const START = Date.parse("2026-09-13T21:16:00Z");

/** Lines by alert number, from groups of one report each. */
const linesOf = (...reports: number[][]) =>
	alertLinesOf(
		alertsFixture(
			reports.map((alerts, report) => ({
				report,
				hours: report,
				alerts: alerts.map((alert) => ({ alert })),
			})),
		),
	);
const keys = (lines: readonly AlertLine[]) =>
	lines.map(({ key }) => Number(key.slice(-11)));

describe("alertLinesOf", () => {
	// Kills a row resolved against the wrong table, an alert dot on an alert no
	// longer active, and a row that forgets its report or when it changed.
	test("each alert is its line, keyed by the alert, with its report", () => {
		const batch = alertsFixture([
			{
				report: 7,
				branch: 1,
				hours: 2,
				total: 5,
				alerts: [
					{ alert: 1, benchmark: 1 },
					{ alert: 2, status: "dismissed", modified: START - 1_000 },
				],
			},
		]);
		const [active, dismissed] = alertLinesOf(batch);
		expect(active).toMatchObject({
			key: alertId(1),
			branch: { name: "feature", head: "feature-head" },
			testbed: { name: "ubuntu-latest" },
			benchmark: { name: "sha256" },
			variant: "v-sha256",
			parameters: { n: 0 },
			measure: { name: "Latency" },
			value: 20.6,
			upper_limit: 19.9,
			alert: { uuid: alertId(1), status: "active" },
			guard: "upper",
			status: "active",
			report: {
				uuid: reportId(7),
				start: START - 2 * 3_600_000,
				created: START - 2 * 3_600_000 + 125_000,
				hash: "9c1f2e4",
				adapter: "json",
				total: 5,
			},
		});
		expect(active?.line).not.toBe(dismissed?.line);
		expect(dismissed).toMatchObject({
			key: alertId(2),
			benchmark: { name: "blake3" },
			alert: undefined,
			status: "dismissed",
			modified: START - 1_000,
		});
		expect(active?.history.alerts).toEqual([2]);
	});
});

describe("distinctLines", () => {
	// Kills an alert shown twice when a later batch repeats one the list moved.
	test("each alert once, where it first came", () => {
		const [first = [], second = []] = [linesOf([1, 2]), linesOf([2, 3])];
		expect(keys(distinctLines([first, second]))).toEqual([1, 2, 3]);
	});
});

describe("withKept", () => {
	const rows = linesOf([1, 2, 3, 4]);
	const at = (key: number) => rows[key - 1] as AlertLine;
	const kept = (key: number, after: number | undefined) => ({
		line: at(key),
		after: after === undefined ? undefined : alertId(after),
	});
	const without = (...gone: number[]) =>
		rows.filter((line) => !gone.includes(Number(line.key.slice(-11))));

	// Kills kept rows left out, appended at the end, or put before their anchor.
	test("a row the list no longer holds comes back after the row it followed", () => {
		expect(keys(withKept(without(3), [kept(3, 2)]))).toEqual([1, 2, 3, 4]);
	});

	// Kills a first row that has no anchor dropped or moved to the end.
	test("a row that led the list leads it again", () => {
		expect(keys(withKept(without(1), [kept(1, undefined)]))).toEqual([
			1, 2, 3, 4,
		]);
	});

	// Kills chains broken when every row was kept, as after Dismiss all.
	test("rows kept one after another keep their order", () => {
		expect(
			keys(
				withKept([], [kept(1, undefined), kept(2, 1), kept(3, 2), kept(4, 3)]),
			),
		).toEqual([1, 2, 3, 4]);
	});

	// Kills a kept row drawn twice when the list still holds it.
	test("a row the list still holds stays where the list has it", () => {
		expect(keys(withKept(rows, [kept(2, 1)]))).toEqual([1, 2, 3, 4]);
	});

	// Kills new rows above the old ones pulling kept rows to the top.
	test("rows new to the list do not move a kept row from its anchor", () => {
		const fresh = linesOf([9, 1, 2, 4]);
		expect(keys(withKept(fresh, [kept(3, 2)]))).toEqual([9, 1, 2, 3, 4]);
	});

	// Kills a kept row lost when its anchor is gone.
	test("a row whose anchor is gone goes last", () => {
		expect(keys(withKept(without(2, 3), [kept(3, 2)]))).toEqual([1, 4, 3]);
	});
});

describe("visibleLines", () => {
	const [active, dismissed, silenced] = alertLinesOf(
		alertsFixture([
			{
				report: 0,
				alerts: [
					{ alert: 1 },
					{ alert: 2, status: "dismissed" },
					{ alert: 3, status: "silenced" },
				],
			},
		]),
	) as AlertLine[];
	const lines = [active, dismissed, silenced] as AlertLine[];
	const as = (status: "active" | "dismissed" | "silenced") => () => status;

	// Kills a status view that shows a row it should not, or hides one it should.
	test.each([
		["active", [1]],
		["dismissed", [2, 3]],
		["all", [1, 2, 3]],
	] as const)("%s shows %o", (status, shown) => {
		expect(
			keys(
				visibleLines(
					lines,
					status,
					(line) => line.status,
					() => false,
				),
			),
		).toEqual(shown);
	});

	// Kills an alert dismissed on the page that leaves Active at once, and one
	// reactivated on the page that stays under Dismissed.
	test("Active keeps what the page dismissed; Dismissed lets go of what it reactivated", () => {
		expect(
			keys(
				visibleLines(
					[active as AlertLine],
					"active",
					as("dismissed"),
					() => true,
				),
			),
		).toEqual([1]);
		expect(
			keys(
				visibleLines(
					[dismissed as AlertLine],
					"dismissed",
					as("active"),
					() => true,
				),
			),
		).toEqual([]);
	});
});

describe("alertSlots", () => {
	// Kills a header missing before a report's rows, a report split in two by
	// a batch boundary, and a header that does not say where its rows start.
	test("one header before each run of one report's rows", () => {
		const batch = alertsFixture([
			{ report: 1, alerts: [{ alert: 1 }, { alert: 2 }] },
			{ report: 2, hours: 1, alerts: [{ alert: 3 }] },
		]);
		const next = alertsFixture([
			{ report: 2, hours: 1, alerts: [{ alert: 4 }] },
		]);
		const lines = distinctLines([alertLinesOf(batch), alertLinesOf(next)]);
		const { slots, groups } = alertSlots(lines);
		expect(
			slots.map((slot) =>
				"start" in slot ? `group ${slot.start}` : Number(slot.key.slice(-11)),
			),
		).toEqual(["group 0", 1, 2, "group 2", 3, 4]);
		expect(
			groups.map(({ report, keys }) => [report.uuid, keys.length]),
		).toEqual([
			[reportId(1), 2],
			[reportId(2), 2],
		]);
		expect(groups[0]?.row).toBe(slots[0]);
	});

	// Kills a header drawn anew on every change, which takes the focus from
	// its controls, and one that keeps where its rows started before.
	test("a report keeps its header from one layout to the next", () => {
		const lines = linesOf([1, 2], [3]);
		const before = alertSlots(lines);
		const [older, newer] = before.groups.map(({ row }) => row);
		const after = alertSlots(lines.slice(1), before.groups);
		expect(after.groups[0]?.row).toBe(older);
		expect(after.groups[1]?.row).toBe(newer);
		expect(after.slots[2]).toBe(newer);
		expect([newer?.start, newer?.lines]).toEqual([1, 1]);
		expect(alertSlots(linesOf([], [3]), []).groups[0]?.row).not.toBe(newer);
	});
});

describe("withStatuses", () => {
	// Kills a status the page took that a later visit does not read back, a
	// history marker left at the old status, a batch changed in place, and
	// alerts or groups the change did not touch copied rather than kept.
	test("each changed alert reads its new status and time, and the rest keep their objects", () => {
		const batch = alertsFixture([
			{ report: 1, alerts: [{ alert: 1 }, { alert: 2 }] },
			{ report: 2, hours: 1, alerts: [{ alert: 3, status: "dismissed" }] },
		]);
		const next = withStatuses(
			batch,
			new Map([[alertId(2), "dismissed"]]),
			START + 5,
		);
		const [one, two, three] = alertLinesOf(next);
		expect([one?.status, two?.status, three?.status]).toEqual([
			"active",
			"dismissed",
			"dismissed",
		]);
		expect(two?.modified).toBe(START + 5);
		expect(two?.alert).toBeUndefined();
		expect(
			next.groups[0]?.alerts[1]?.line.history.alerts?.map(
				({ status }) => status,
			),
		).toEqual(["dismissed"]);
		expect(batch.groups[0]?.alerts[1]?.line.alert?.status).toBe("active");
		expect(next.groups[0]?.alerts[0]).toBe(batch.groups[0]?.alerts[0]);
		expect(next.groups[1]).toBe(batch.groups[1]);

		const back = withStatuses(next, new Map([[alertId(3), "active"]]), START);
		expect(alertLinesOf(back)[2]?.alert?.status).toBe("active");
		expect(withStatuses(batch, new Map([[alertId(9), "active"]]), START)).toBe(
			batch,
		);
	});

	// Kills a history that still marks a changed alert at its old status on
	// another row of its line.
	test("another row's history marks the changed alert at its new status", () => {
		const batch = alertsFixture([
			{ report: 1, alerts: [{ alert: 1 }] },
			{ report: 2, hours: 1, alerts: [{ alert: 2 }] },
		]);
		const later = batch.groups[0]?.alerts[0]?.line.history.alerts;
		later?.unshift({
			index: 1,
			uuid: alertId(2),
			limit: "upper",
			status: "active",
		} as (typeof later)[number]);
		const next = withStatuses(
			batch,
			new Map([[alertId(2), "dismissed"]]),
			START,
		);
		expect(
			next.groups[0]?.alerts[0]?.line.history.alerts.map(
				({ status }) => status,
			),
		).toEqual(["dismissed", "active"]);
		expect(next.groups[0]?.alerts[0]?.modified).toBe(
			batch.groups[0]?.alerts[0]?.modified,
		);
	});
});

describe("group selection", () => {
	const group = [alertId(1), alertId(2)];

	// Kills a tri-state that reads all as some, some as none, or counts rows outside the group.
	test.each([
		[[], "none"],
		[[alertId(1)], "some"],
		[[alertId(1), alertId(2)], "all"],
		[[alertId(3)], "none"],
	] as const)("%o selected is %s", (selected, state) => {
		expect(groupState(group, new Set(selected))).toBe(state);
	});

	// Kills a group checkbox that clears a partial group instead of filling it,
	// leaves part of a full group, or touches other groups' rows.
	test("toggling fills a group that is not full and empties one that is", () => {
		const other = alertId(3);
		expect(
			[...toggleGroup(group, new Set([alertId(1), other]))].sort(),
		).toEqual([alertId(1), alertId(2), other].sort());
		expect([...toggleGroup(group, new Set([...group, other]))]).toEqual([
			other,
		]);
	});
});
