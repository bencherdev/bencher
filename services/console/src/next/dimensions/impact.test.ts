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
		expect(archivedNote(1)).toBe("Archived with 1 threshold");
		expect(archivedNote(2)).toBe("Archived with 2 thresholds");
		expect(archivedNote(0)).toBe("Archived");
		expect(archivedNote(undefined)).toBe("Archived");
	});
});

describe("unarchivedNote", () => {
	// Kills a note that claims every threshold back when another dimension
	// still holds some, one that hides those it brought back, and a thresholds
	// count where there are none.
	test("names the thresholds that come back and those that stay archived", () => {
		expect(unarchivedNote(1, 0)).toBe("Unarchived with 1 threshold");
		expect(unarchivedNote(3, 0)).toBe("Unarchived with 3 thresholds");
		expect(unarchivedNote(1, 1)).toBe(
			"Unarchived with 1 threshold · 1 threshold still archived",
		);
		expect(unarchivedNote(2, 3)).toBe(
			"Unarchived with 2 thresholds · 3 thresholds still archived",
		);
		expect(unarchivedNote(0, 2)).toBe(
			"Unarchived · 2 thresholds still archived",
		);
		expect(unarchivedNote(0, 0)).toBe("Unarchived");
		expect(unarchivedNote(undefined, 0)).toBe("Unarchived");
	});
});
