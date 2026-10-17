import { describe, expect, test } from "vitest";
import { accountPage } from "./routes";

describe("accountPage", () => {
	// Kills Organizations left outside the account pages, its dialog's path
	// drawn as another page, and an organization's own pages taken for it.
	test("names the account page a path draws", () => {
		expect(accountPage("/next/console/users/muriel-bagge/settings")).toBe(
			"Profile",
		);
		expect(accountPage("/next/console/organizations")).toBe("Organizations");
		expect(accountPage("/next/console/organizations/add")).toBe(
			"Organizations",
		);
		expect(
			accountPage("/next/console/organizations/bagge-farm/settings"),
		).toBeUndefined();
		expect(accountPage("/next/console/projects/hashbrown")).toBeUndefined();
	});
});
