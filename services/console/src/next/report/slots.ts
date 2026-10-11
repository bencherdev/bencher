import type { GroupRow, ReportLine } from "./lines";

/** What the list draws in order: a group's header, or one line's row. */
export type Slot = GroupRow | ReportLine;

export const isGroup = (slot: Slot): slot is GroupRow => "start" in slot;

/** The loaded lines with each group's header before its first line, each the object it was given. */
export const slotsOf = (
	groups: readonly GroupRow[],
	lines: readonly ReportLine[],
): Slot[] => {
	const slots: Slot[] = [];
	let next = 0;
	lines.forEach((line, position) => {
		for (; next < groups.length; next++) {
			const group = groups[next] as GroupRow;
			if (group.start > position) {
				break;
			}
			slots.push(group);
		}
		slots.push(line);
	});
	return slots;
};

/** Where each slot starts, from the heights before it, and the list's height last. */
export const offsetsOf = (heights: readonly number[]): number[] => {
	const offsets = [0];
	let top = 0;
	for (const height of heights) {
		top += height;
		offsets.push(top);
	}
	return offsets;
};

/**
 * The slots to draw: those that cross the screen, plus `overscan` pixels either
 * side. `top` is where the list's top edge sits, relative to the top of the screen.
 */
export const slotRange = ({
	offsets,
	top,
	viewport,
	overscan,
}: {
	offsets: readonly number[];
	top: number;
	viewport: number;
	overscan: number;
}) => {
	const from = -top - overscan;
	const to = -top + viewport + overscan;
	const count = offsets.length - 1;
	// The first slot ending after `from`, and the first starting at or after `to`.
	return {
		start: search(count, (slot) => (offsets[slot + 1] ?? 0) > from),
		end: search(count, (slot) => (offsets[slot] ?? 0) >= to),
	};
};

/** The first of `count` slots that passes, or `count`, for a test that once passed keeps passing. */
const search = (count: number, passes: (slot: number) => boolean) => {
	let low = 0;
	let high = count;
	while (low < high) {
		const middle = (low + high) >> 1;
		if (passes(middle)) {
			high = middle;
		} else {
			low = middle + 1;
		}
	}
	return low;
};
