import { describe, expect, test } from "vitest";
import type { AlertLine } from "./rows";
import { alertLinesOf } from "./rows";
import {
	type Changes,
	begin,
	changeable,
	confirmedOf,
	inChunks,
	inView,
	rollback,
	settle,
	shownDelta,
	statusOf,
	withPending,
} from "./status";
import { alertId, alertsFixture } from "./testing";

const [active, other, dismissed, silenced] = alertLinesOf(
	alertsFixture([
		{
			report: 0,
			alerts: [
				{ alert: 1 },
				{ alert: 2 },
				{ alert: 3, status: "dismissed" },
				{ alert: 4, status: "silenced" },
			],
		},
	]),
) as AlertLine[];
const rows = (...lines: AlertLine[]) =>
	lines.map((line, index) => ({
		line,
		after: index === 0 ? undefined : lines[index - 1]?.key,
	}));
const none: Changes = new Map();

describe("an optimistic status change", () => {
	// Kills a change that waits for the API before the row shows it.
	test("shows at once and is pending until the API answers", () => {
		const changes = begin(none, 1, "dismissed", rows(active as AlertLine));
		expect(statusOf(active as AlertLine, changes)).toBe("dismissed");
		expect(statusOf(other as AlertLine, changes)).toBe("active");
		expect(changes.get(alertId(1))?.request).toBe(1);

		const settled = settle(changes, 1);
		expect(statusOf(active as AlertLine, settled)).toBe("dismissed");
		expect(settled.get(alertId(1))?.request).toBeUndefined();
	});

	// Kills a refusal that leaves the row changed, or that undoes an earlier
	// change the API already took.
	test("a refusal puts back what the row showed before it", () => {
		const dismissedOnce = settle(
			begin(none, 1, "dismissed", rows(active as AlertLine)),
			1,
		);
		const refused = rollback(
			begin(dismissedOnce, 2, "active", rows(active as AlertLine)),
			2,
		);
		expect(statusOf(active as AlertLine, refused)).toBe("dismissed");
		expect(refused.get(alertId(1))?.request).toBeUndefined();

		expect(
			rollback(begin(none, 3, "dismissed", rows(active as AlertLine)), 3).size,
		).toBe(0);
	});

	// Kills a refusal that rolls back another request's rows too.
	test("a refusal touches only its own request's rows", () => {
		const both = begin(
			begin(none, 1, "dismissed", rows(active as AlertLine)),
			2,
			"dismissed",
			rows(other as AlertLine),
		);
		const refused = rollback(both, 1);
		expect(statusOf(active as AlertLine, refused)).toBe("active");
		expect(statusOf(other as AlertLine, refused)).toBe("dismissed");
		expect(refused.get(alertId(2))?.request).toBe(2);
	});

	// Kills a kept row that forgets the row it followed.
	test("each changed row remembers its row and the one before it", () => {
		const changes = begin(
			none,
			1,
			"dismissed",
			rows(active as AlertLine, other as AlertLine),
		);
		expect(changes.get(alertId(2))).toMatchObject({
			line: other,
			after: alertId(1),
		});
	});
});

describe("a rollback", () => {
	const dismiss = (changes: Changes, request: number) =>
		begin(changes, request, "dismissed", rows(active as AlertLine));
	const reactivate = (changes: Changes, request: number) =>
		begin(changes, request, "active", rows(active as AlertLine));

	// Kills a refused dismiss, made after its undo, that moves the counts a
	// second time or leaves the row dismissed.
	test("a refused dismiss after its undo changes nothing on screen", () => {
		const undone = reactivate(dismiss(none, 1), 2);
		expect(statusOf(active as AlertLine, undone)).toBe("active");
		expect(shownDelta(undone)).toBe(0);

		const refused = rollback(undone, 1);
		expect(statusOf(active as AlertLine, refused)).toBe("active");
		expect(shownDelta(refused)).toBe(0);
		expect(statusOf(active as AlertLine, rollback(refused, 2))).toBe("active");
		expect(statusOf(active as AlertLine, settle(refused, 2))).toBe("active");
	});

	// Kills a refusal that returns to the API's first answer rather than to
	// the last status the API took.
	test("returns a row to its last confirmed status", () => {
		const confirmed = settle(dismiss(none, 1), 1);
		const refused = rollback(reactivate(confirmed, 2), 2);
		expect(statusOf(active as AlertLine, refused)).toBe("dismissed");
		expect(confirmedOf(active as AlertLine, refused)).toBe("dismissed");
	});
});

describe("shownDelta", () => {
	// Kills counts that wait for the API, keep moving once it answered, or
	// move the wrong way.
	test("each row moves the active count by what it shows past what the API took", () => {
		const pending = begin(
			begin(none, 1, "dismissed", rows(active as AlertLine)),
			2,
			"active",
			rows(dismissed as AlertLine),
		);
		expect(
			shownDelta(begin(none, 1, "dismissed", rows(active as AlertLine))),
		).toBe(-1);
		expect(shownDelta(pending)).toBe(0);
		expect(shownDelta(settle(pending, 2))).toBe(-1);
		expect(shownDelta(settle(settle(pending, 2), 1))).toBe(0);
	});
});

describe("inView", () => {
	const loaded = [active, other, dismissed, silenced] as AlertLine[];

	// Kills an offset counted by what the row shows before the API took it,
	// or one that counts rows that left the view.
	test.each([
		["active", 1],
		["dismissed", 3],
		["all", 4],
	] as const)("%s counts %i loaded rows still in it", (view, count) => {
		const changes = begin(
			settle(begin(none, 1, "dismissed", rows(other as AlertLine)), 1),
			2,
			"dismissed",
			rows(active as AlertLine),
		);
		expect(inView(loaded, view, changes)).toBe(count);
	});
});

describe("changeable", () => {
	const lines = [active, dismissed, silenced] as AlertLine[];

	// Kills Dismiss counting an alert that is not active, and Reactivate
	// counting a silenced one, which the API never changes.
	test("dismiss takes the active alerts, reactivate the dismissed ones", () => {
		expect(changeable(lines, "dismissed", none)).toEqual([active]);
		expect(changeable(lines, "active", none)).toEqual([dismissed]);
	});

	// Kills a selection counted by the status the API last sent rather than the one on screen.
	test("the status on screen decides", () => {
		const changes = begin(none, 1, "dismissed", rows(active as AlertLine));
		expect(changeable(lines, "dismissed", changes)).toEqual([]);
		expect(changeable(lines, "active", changes)).toEqual([active, dismissed]);
	});
});

describe("withPending", () => {
	// Kills counts and Dismiss all's number that ignore a change still in
	// flight, or move the wrong way.
	test("pending changes move the counts until the API answers", () => {
		const counts = { active: 5, dismissed: 2, silenced: 1 };
		expect(withPending(counts, [])).toEqual(counts);
		expect(withPending(counts, [-2, 1])).toEqual({
			active: 4,
			dismissed: 3,
			silenced: 1,
		});
	});

	// Kills a count that goes below zero when the API changed more than the
	// page counted.
	test("a count never drops below zero", () => {
		expect(withPending({ active: 3, dismissed: 0, silenced: 1 }, [-4])).toEqual(
			{ active: 0, dismissed: 4, silenced: 1 },
		);
		expect(withPending({ active: 0, dismissed: 3, silenced: 1 }, [4])).toEqual({
			active: 4,
			dismissed: 0,
			silenced: 1,
		});
	});
});

describe("inChunks", () => {
	// Kills a request listing more alerts than the API changes at once, and
	// a chunk that drops or repeats one.
	test("lists at most 255 alerts a request", () => {
		const keys = Array.from({ length: 300 }, (_, index) => index);
		expect(inChunks(keys).map((chunk) => chunk.length)).toEqual([255, 45]);
		expect(inChunks(keys).flat()).toEqual(keys);
		expect(inChunks(keys.slice(0, 255))).toHaveLength(1);
		expect(inChunks([])).toEqual([]);
	});
});
