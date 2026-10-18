import { describe, expect, test } from "vitest";
import { organizationPageAt } from "./paths";

describe("organizationPageAt", () => {
	// Kills a page read from the organization's own segment, a deeper, unknown,
	// or inherited path given a page, the account's Organizations taken for an
	// organization's page, and a project's page taken for one.
	test("names the page an organization path draws", () => {
		const organization = "/next/console/organizations/bagge-farm";
		expect(organizationPageAt(`${organization}/settings`)?.page).toBe(
			"OrgGeneral",
		);
		expect(organizationPageAt(`${organization}/members/`)?.page).toBe(
			"OrgMembers",
		);
		expect(organizationPageAt(`${organization}/billing`)?.page).toBe(
			"OrgBilling",
		);
		expect(
			organizationPageAt(
				"/next/console/organizations/0f8c4a2e-5b1d-4c3a-9e7f-2d6b8a1c3e5f/projects/add",
			)?.page,
		).toBe("NewProject");
		expect(organizationPageAt(`${organization}/settings/x`)).toBeUndefined();
		expect(organizationPageAt(`${organization}/projects`)).toBeUndefined();
		expect(organizationPageAt(`${organization}/constructor`)).toBeUndefined();
		expect(
			organizationPageAt("/next/console/organizations/add"),
		).toBeUndefined();
		expect(
			organizationPageAt("/next/console/organizations/settings"),
		).toBeUndefined();
		expect(
			organizationPageAt("/next/console/projects/hashbrown/settings"),
		).toBeUndefined();
	});
});
