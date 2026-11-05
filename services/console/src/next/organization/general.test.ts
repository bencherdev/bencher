import { describe, expect, test } from "vitest";
import { ApiError } from "../api";
import { goneWith, patchOf, problemsOf, saveFailure } from "./general";

const SAVED = { name: "Pompeii LLC", slug: "pompeii-llc" };

describe("patchOf", () => {
	// Kills a patch that sends a field the reader left alone, keeps the spaces
	// around a name, or counts an untouched form as a change.
	test("holds only what changed, the name trimmed", () => {
		expect(patchOf(SAVED, { ...SAVED })).toBeUndefined();
		expect(patchOf(SAVED, { ...SAVED, name: " Pompeii LLC " })).toBeUndefined();
		expect(patchOf(SAVED, { ...SAVED, name: " Pompeii Inc " })).toEqual({
			name: "Pompeii Inc",
		});
		expect(patchOf(SAVED, { ...SAVED, slug: "pompeii" })).toEqual({
			slug: "pompeii",
		});
	});
});

describe("problemsOf", () => {
	// Kills a name that can be saved blank, a slug the API would refuse let
	// through, and a valid form held back.
	test("a name is required and a slug is lowercase words joined by dashes", () => {
		expect(problemsOf({ ...SAVED, name: " " }).name).toBeDefined();
		expect(problemsOf({ ...SAVED, slug: "Pompeii LLC" }).slug).toBeDefined();
		expect(problemsOf({ ...SAVED, slug: "pompeii--llc" }).slug).toBeDefined();
		expect(problemsOf(SAVED)).toEqual({ name: undefined, slug: undefined });
	});
});

describe("goneWith", () => {
	// Kills a list that drops a project, its "and", or its last comma, a verb
	// that does not agree with one project, and a clause for no projects.
	test("names every project that goes with the organization", () => {
		expect(goneWith([])).toBeUndefined();
		expect(goneWith(["Gizmo"])).toBe("Gizmo goes with it.");
		expect(goneWith(["Gizmo", "Windmill"])).toBe(
			"Gizmo and Windmill go with it.",
		);
		expect(goneWith(["Gizmo", "Windmill", "Lighthouse", "Cathode"])).toBe(
			"Gizmo, Windmill, Lighthouse, and Cathode go with it.",
		);
	});

	// Kills a clause that names a hundred projects, or loses count of the rest.
	test("names five projects and counts the rest", () => {
		const projects = Array.from({ length: 1005 }, (_, n) => `p${n}`);
		expect(goneWith(projects.slice(0, 5))).toBe(
			"p0, p1, p2, p3, and p4 go with it.",
		);
		expect(goneWith(projects)).toBe(
			"p0, p1, p2, p3, p4, and 1,000 more go with it.",
		);
	});
});

describe("saveFailure", () => {
	// Kills a taken slug reported in the API's words, a conflict over the name
	// blamed on the slug, and the API's own words lost.
	test("says why the API refused", () => {
		expect(
			saveFailure(new ApiError(409, "client", "{}"), { slug: "taken" }),
		).toBe("Another organization has this slug");
		expect(
			saveFailure(new ApiError(409, "client", '{"message":"Taken"}'), {
				name: "x",
			}),
		).toBe("Bencher did not save the changes: Taken");
	});
});
