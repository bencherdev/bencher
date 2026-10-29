import { expect, test } from "vitest";
import { allNames, runHead, secondsTime } from "./format";

// Kills a run without a hash drawn blank, and a hash drawn whole.
test("a run reads as its short hash, else its version number", () => {
	expect(runHead({ hash: "9c1f2e4aa0b1c2d3e4f5", number: 248 })).toBe(
		"9c1f2e4",
	);
	expect(runHead({ number: 252 })).toBe("v252");
});

// Kills a list cut at the names the API sent, a count of the rest taken from
// the names rather than the run, and a list of none that starts with a comma.
test("every name, and how many more the run holds", () => {
	expect(allNames(["ubuntu-latest", "macos-latest"], 2)).toBe(
		"ubuntu-latest, macos-latest",
	);
	expect(allNames(["t1", "t2"], 9)).toBe("t1, t2, and 7 more");
	expect(allNames([], 3)).toBe("3 more");
});

// Kills a start without its seconds, which cannot tell apart reports started
// in the same minute, and a twelve hour clock.
test("a start reads to the second on a 24 hour clock", () => {
	expect(secondsTime(Date.parse("2026-10-12T21:06:02Z"), "UTC")).toBe(
		"21:06:02",
	);
});
