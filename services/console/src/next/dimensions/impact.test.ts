import { describe, expect, test } from "vitest";
import {
	VARIANT_IMPACT,
	archiveImpact,
	archivedNote,
	unarchivedNote,
} from "./impact";

describe("archiveImpact", () => {
	// Kills a count that always reads plural, or one the line drops.
	test("names how many thresholds archive with it, one or many", () => {
		expect(archiveImpact("feature-simd", 1)).toBe(
			"Archiving feature-simd archives 1 threshold and hides its lines. A run that reports it brings it back.",
		);
		expect(archiveImpact("main", 4)).toBe(
			"Archiving main archives 4 thresholds and hides its lines. A run that reports it brings it back.",
		);
	});

	// Kills "archives 0 thresholds".
	test("says no threshold checks it when none would archive", () => {
		expect(archiveImpact("devel", 0)).toBe(
			"Archiving devel hides its lines. No threshold checks it. A run that reports it brings it back.",
		);
	});

	// Kills a benchmark said to archive thresholds it does not own.
	test("a benchmark keeps its thresholds", () => {
		expect(archiveImpact("blake3", undefined)).toBe(
			"Archiving blake3 hides its lines. Its thresholds stay. A run that reports it brings it back.",
		);
		expect(VARIANT_IMPACT).toBe(
			"Archiving this variant hides its lines. Its thresholds stay. A run that reports it brings it back.",
		);
	});
});

describe("archivedNote", () => {
	// Kills a note that drops the thresholds that went with it, or counts zero.
	test("says what went with it", () => {
		expect(archivedNote("main", 1)).toBe(
			"Archived main with its 1 threshold. Its lines are hidden.",
		);
		expect(archivedNote("main", 2)).toBe(
			"Archived main with its 2 thresholds. Its lines are hidden.",
		);
		expect(archivedNote("devel", 0)).toBe(
			"Archived devel. Its lines are hidden.",
		);
		expect(archivedNote("blake3", undefined)).toBe(
			"Archived blake3. Its lines are hidden; its thresholds stay.",
		);
	});
});

describe("unarchivedNote", () => {
	// Kills a note that claims every threshold back when another dimension
	// still holds some, and one that hides those it brought back.
	test("names the thresholds that come back and those that stay archived", () => {
		expect(unarchivedNote("feature-simd", 1, 0)).toBe(
			"Unarchived feature-simd and 1 threshold.",
		);
		expect(unarchivedNote("main", 3, 0)).toBe(
			"Unarchived main and 3 thresholds.",
		);
		expect(unarchivedNote("main", 1, 1)).toBe(
			"Unarchived main and 1 threshold. One threshold stays archived: another of its dimensions is archived.",
		);
		expect(unarchivedNote("main", 2, 3)).toBe(
			"Unarchived main and 2 thresholds. 3 thresholds stay archived: another of their dimensions is archived.",
		);
		expect(unarchivedNote("main", 0, 1)).toBe(
			"Unarchived main. Its threshold stays archived: another of its dimensions is archived.",
		);
		expect(unarchivedNote("main", 0, 2)).toBe(
			"Unarchived main. 2 thresholds stay archived: another of their dimensions is archived.",
		);
	});

	// Kills a thresholds sentence where there are none.
	test("with no thresholds, its lines are back", () => {
		expect(unarchivedNote("devel", 0, 0)).toBe(
			"Unarchived devel. Its lines are back.",
		);
		expect(unarchivedNote("blake3", undefined, 0)).toBe(
			"Unarchived blake3. Its lines are back.",
		);
	});
});
