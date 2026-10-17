import { describe, expect, test } from "vitest";
import { patchOf, problemsOf } from "./profile";

const SAVED = {
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	email: "muriel@nowhere.example",
};

describe("patchOf", () => {
	// Kills a patch that sends a field the reader left alone, keeps the
	// spaces around what they typed, or counts an untouched form as a change.
	test("holds only what changed, trimmed", () => {
		expect(patchOf(SAVED, { ...SAVED })).toBeUndefined();
		expect(
			patchOf(SAVED, {
				...SAVED,
				name: " Muriel Courage ",
				email: " m@c.example ",
			}),
		).toEqual({ name: "Muriel Courage", email: "m@c.example" });
		expect(
			patchOf(SAVED, { ...SAVED, name: " Muriel Bagge " }),
		).toBeUndefined();
	});
});

describe("problemsOf", () => {
	// Kills an email that can be saved empty, and a valid form held back.
	test("an email is required", () => {
		expect(problemsOf({ ...SAVED, email: " " }).email).toBeDefined();
		expect(problemsOf(SAVED)).toEqual({
			name: undefined,
			slug: undefined,
			email: undefined,
		});
	});
});
