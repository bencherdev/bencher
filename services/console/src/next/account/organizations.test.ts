import { describe, expect, test } from "vitest";
import type { JsonConsoleUser } from "../../types/bencher";
import { nameProblem, organizationRows } from "./organizations";

describe("nameProblem", () => {
	// Kills a name counted in letters, which the API counts in bytes, and a
	// limit applied before the spaces it trims.
	test("refuses a name over 64 bytes", () => {
		expect(nameProblem(` ${"a".repeat(64)} `)).toBeUndefined();
		expect(nameProblem("é".repeat(33))).toBe("66 of 64 bytes");
	});
});

const user = (organizations: string[], projects: string[]): JsonConsoleUser =>
	({
		uuid: "u",
		name: "Muriel Bagge",
		slug: "muriel-bagge",
		organizations: organizations.map((name) => ({
			uuid: name,
			name,
			slug: name.toLowerCase(),
		})),
		projects: projects.map((organization, index) => ({
			uuid: `p${index}`,
			name: `P${index}`,
			slug: `p${index}`,
			organization,
			bmf_version: 1,
			active_alerts: 0,
		})),
	}) as JsonConsoleUser;

describe("organizationRows", () => {
	// Kills rows in the API's order, an organization with no projects left
	// out, and projects counted across organizations.
	test("lists every organization by name with its own projects", () => {
		expect(
			organizationRows(
				user(
					["Tumbleweed", "Bagge Farm", "nowhere"],
					["Tumbleweed", "nowhere", "Tumbleweed"],
				),
			).map(({ name, projects }) => [name, projects]),
		).toEqual([
			["Bagge Farm", 0],
			["nowhere", 1],
			["Tumbleweed", 2],
		]);
	});
});
