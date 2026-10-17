import { describe, expect, test } from "vitest";
import { accountPage } from "./paths";

describe("accountPage", () => {
	// Kills every account path drawn as Profile, a page read from another
	// segment, a deeper or unknown path given a page, Organizations left outside
	// the account pages, and an organization's own pages taken for it.
	test("names the page an account path draws", () => {
		const users = "/next/console/users/muriel-bagge";
		expect(accountPage(`${users}/settings`)?.page).toBe("Profile");
		expect(accountPage(`${users}/keys`)?.page).toBe("UserKeys");
		expect(accountPage(`${users}/tokens/`)?.page).toBe("LegacyTokens");
		expect(accountPage("/next/console/organizations")?.page).toBe(
			"Organizations",
		);
		expect(accountPage("/next/console/organizations/add")?.page).toBe(
			"Organizations",
		);
		expect(accountPage(`${users}/keys/everett-laptop`)).toBeUndefined();
		expect(accountPage(`${users}/help`)).toBeUndefined();
		expect(accountPage(`${users}/constructor`)).toBeUndefined();
		expect(accountPage(`${users}/organizations`)).toBeUndefined();
		expect(
			accountPage("/next/console/organizations/bagge-farm/settings"),
		).toBeUndefined();
		expect(
			accountPage("/next/console/projects/hashbrown/keys"),
		).toBeUndefined();
	});
});
