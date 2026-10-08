import { describe, expect, test } from "vitest";
import { isGroup, offsetsOf, slotRange, slotsOf } from "./slots";

const group = (start: number, lines: number) => ({
	name: `group ${start}`,
	lines,
	variants: lines,
	alerts: 0,
	start,
});
const line = (key: string) => ({ key }) as never;

describe("slotsOf", () => {
	// Kills a header drawn after its first line, or one drawn before its lines have loaded.
	test("puts each group's header before its first loaded line", () => {
		const slots = slotsOf(
			[group(0, 2), group(2, 1), group(3, 4)],
			[line("a"), line("b"), line("c")],
		);
		expect(
			slots.map((slot) =>
				isGroup(slot) ? slot.name : (slot as { key: string }).key,
			),
		).toEqual(["group 0", "a", "b", "group 2", "c"]);
	});

	// Kills new objects on every load, which would draw every row on screen again.
	test("keeps the objects it was given", () => {
		const groups = [group(0, 1)];
		const lines = [line("a")];
		const slots = slotsOf(groups, lines);
		expect(slots[0]).toBe(groups[0]);
		expect(slots[1]).toBe(lines[0]);
	});
});

describe("slotRange", () => {
	// Rows of 44 px, a group header of 38 px, and an expanded row of 44 + 330 px.
	const offsets = offsetsOf([38, 44, 374, 44, 44, 44]);

	// Kills offsets that skip a row's own height.
	test("stacks each slot under the ones before it", () => {
		expect(offsets).toEqual([0, 38, 82, 456, 500, 544, 588]);
	});

	// Kills a uniform row height, which would draw the wrong rows under an expanded one.
	test("draws the slots that cross the screen", () => {
		expect(
			slotRange({ offsets, top: -100, viewport: 380, overscan: 0 }),
		).toEqual({ start: 2, end: 4 });
		expect(
			slotRange({ offsets, top: -460, viewport: 50, overscan: 0 }),
		).toEqual({ start: 3, end: 5 });
	});

	// Kills an overscan that ignores the slots either side of the screen.
	test("draws the overscan either side of the screen", () => {
		expect(
			slotRange({ offsets, top: -460, viewport: 50, overscan: 40 }),
		).toEqual({ start: 2, end: 6 });
	});

	// Kills drawing every slot when the list sits below the screen, or none when it starts on it.
	test("draws only what is on screen when the list starts below or above it", () => {
		expect(
			slotRange({ offsets, top: 900, viewport: 800, overscan: 0 }),
		).toEqual({ start: 0, end: 0 });
		expect(
			slotRange({ offsets, top: 500, viewport: 800, overscan: 0 }),
		).toEqual({ start: 0, end: 3 });
		expect(
			slotRange({ offsets, top: -2000, viewport: 800, overscan: 0 }),
		).toEqual({ start: 6, end: 6 });
	});
});
