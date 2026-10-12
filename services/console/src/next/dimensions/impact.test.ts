import { describe, expect, test } from "vitest";
import { archiveImpact, archivedNote, unarchivedNote } from "./impact";

describe("archiveImpact", () => {
	// Kills a count that always reads plural, or one the line drops.
	test("names how many thresholds archive with it, one or many", () => {
		expect(archiveImpact(1)).toBe("Hides its lines and archives 1 threshold");
		expect(archiveImpact(4)).toBe("Hides its lines and archives 4 thresholds");
	});

	// Kills "archives 0 thresholds", and a benchmark said to archive thresholds it does not own.
	test("names no thresholds when none would archive", () => {
		expect(archiveImpact(0)).toBe("Hides its lines");
		expect(archiveImpact(undefined)).toBe("Hides its lines");
	});
});

describe("archivedNote", () => {
	// Kills a note that drops the thresholds that went with it, or counts zero.
	test("says what went with it", () => {
		expect(archivedNote("main", 1)).toBe("Archived main and 1 threshold");
		expect(archivedNote("main", 2)).toBe("Archived main and 2 thresholds");
		expect(archivedNote("devel", 0)).toBe("Archived devel");
		expect(archivedNote("blake3", undefined)).toBe("Archived blake3");
	});
});

describe("unarchivedNote", () => {
	// Kills a note that claims every threshold back when another dimension
	// still holds some, one that hides those it brought back, and a thresholds
	// count where there are none.
	test("names the thresholds that come back and those that stay archived", () => {
		expect(unarchivedNote("feature-simd", 1, 0)).toBe(
			"Unarchived feature-simd and 1 threshold",
		);
		expect(unarchivedNote("main", 3, 0)).toBe(
			"Unarchived main and 3 thresholds",
		);
		expect(unarchivedNote("main", 1, 1)).toBe(
			"Unarchived main and 1 threshold · 1 threshold still archived",
		);
		expect(unarchivedNote("main", 2, 3)).toBe(
			"Unarchived main and 2 thresholds · 3 thresholds still archived",
		);
		expect(unarchivedNote("main", 0, 2)).toBe(
			"Unarchived main · 2 thresholds still archived",
		);
		expect(unarchivedNote("devel", 0, 0)).toBe("Unarchived devel");
		expect(unarchivedNote("blake3", undefined, 0)).toBe("Unarchived blake3");
	});
});
