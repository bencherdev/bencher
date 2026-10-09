import { describe, expect, test } from "vitest";
import { lineKey } from "../query/line";
import { decodeQuery } from "../query/query";
import { type AlertLine, alertLinesOf } from "./alerts";
import { exploreSearchOf } from "./explore";
import { alertsFixture } from "./testing";

const DAY = 86_400_000;

describe("alertLinesOf", () => {
	const batch = alertsFixture();
	const [newest, sha256, blake3Again] = alertLinesOf(batch);

	// Kills rows keyed by their line, which collide when one line alerts in two reports.
	test("keys each row by its alert, so one line alerting twice is two rows", () => {
		expect([newest?.key, sha256?.key, blake3Again?.key]).toEqual([
			"alert-1",
			"alert-2",
			"alert-3",
		]);
		expect(newest?.line).toBe(blake3Again?.line);
	});

	// Kills every row drawn over the first group's points, or the tables read at index 0.
	test("draws each row over its own report's history, from the batch's tables", () => {
		expect(sha256?.points.x).toBe(batch.groups[1]?.points.x);
		expect(sha256?.points.reports).toBe(batch.reports);
		expect(sha256?.benchmark.name).toBe("sha256");
		expect(sha256?.parameters).toEqual({ input_bytes: 1024 });
		expect(sha256?.history.alerts).toEqual([1]);
		expect(sha256?.guard).toBe("upper");
	});

	// Kills a row that does not know its report or the alert's status now.
	test("knows its report and the alert's status", () => {
		expect(newest?.report).toEqual({
			uuid: "report-new",
			start: Date.parse("2026-09-13T21:16:00Z"),
			hash: "9c1f2e4",
		});
		expect(sha256?.report).toEqual({
			uuid: "report-old",
			start: Date.parse("2026-09-13T21:16:00Z") - 18 * DAY,
			hash: undefined,
		});
		expect([newest?.status, sha256?.status, blake3Again?.status]).toEqual([
			"active",
			"dismissed",
			"silenced",
		]);
	});

	// Kills a dismissed or silenced alert drawn as alerting, with the active ones' dot and tint.
	test("draws only an active alert's row as alerting", () => {
		expect(newest?.alert?.uuid).toBe("alert-1");
		expect(sha256?.alert).toBeUndefined();
		expect(blake3Again?.alert).toBeUndefined();
	});
});

describe("exploreSearchOf", () => {
	const lines = alertLinesOf(alertsFixture());
	const search = new URLSearchParams(
		exploreSearchOf(lines, { kind: "rolling", days: 7 }),
	);

	// Kills a line drawn once per alert, and a line left out.
	test("draws each line that alerted once", () => {
		expect(search.get("only")?.split(",")).toEqual([
			lineKey({
				branch: "main-uuid",
				testbed: "testbed-uuid",
				benchmark: "blake3-uuid",
				parameters: { input_bytes: 65536 },
				measure: "latency-uuid",
				metric: "value",
			}),
			lineKey({
				branch: "main-uuid",
				testbed: "testbed-uuid",
				benchmark: "sha256-uuid",
				parameters: { input_bytes: 1024 },
				measure: "latency-uuid",
				metric: "value",
			}),
		]);
		expect(search.get("benchmarks")).toBe("blake3-uuid,sha256-uuid");
	});

	// Kills Explore pinned to the head that alerted, rather than following the branch, and the page's window dropped.
	test("follows the branch over the page's window", () => {
		expect(search.get("branches")).toBe("main-uuid");
		expect(search.has("heads")).toBe(false);
		expect(search.get("window")).toBe("1w");
	});

	// Kills a custom range opened in Explore as a rolling window.
	test("opens a custom range from its start to its end", () => {
		const range = { start: Date.UTC(2026, 7, 18), end: Date.UTC(2026, 8, 13) };
		expect(
			decodeQuery(exploreSearchOf(lines, { kind: "custom", ...range })).window,
		).toEqual(range);
	});

	// Kills the focus put on a line whose alert is no longer active.
	test("focuses the line still alerting", () => {
		const [active, dismissed] = lines as [AlertLine, AlertLine];
		const dismissedFirst = new URLSearchParams(
			exploreSearchOf([dismissed, active], { kind: "rolling", days: 7 }),
		);
		expect(dismissedFirst.get("focus")).toBe(active.line);
	});
});
