import { describe, expect, test } from "vitest";
import { accountPage, shortcutPage } from "./paths";

describe("accountPage", () => {
	// Kills every account path drawn as Profile, a page read from another
	// segment, a deeper or unknown path given a page, Organizations or the
	// GitHub return page left outside the account pages, and an organization's
	// own pages taken for it.
	test("names the page an account path draws", () => {
		const users = "/console/users/muriel-bagge";
		expect(accountPage(`${users}/settings`)?.page).toBe("Profile");
		expect(accountPage(`${users}/keys`)?.page).toBe("UserKeys");
		expect(accountPage(`${users}/apps`)?.page).toBe("ConnectedApps");
		expect(accountPage(`${users}/tokens/`)?.page).toBe("LegacyTokens");
		expect(accountPage("/console/organizations")?.page).toBe("Organizations");
		expect(accountPage("/console/organizations/add")?.page).toBe(
			"Organizations",
		);
		expect(accountPage("/console/github")?.page).toBe("GitHub");
		expect(accountPage("/console/github/hashbrown")).toBeUndefined();
		expect(accountPage(`${users}/keys/everett-laptop`)).toBeUndefined();
		expect(accountPage(`${users}/help`)).toBeUndefined();
		expect(accountPage(`${users}/constructor`)).toBeUndefined();
		expect(accountPage(`${users}/organizations`)).toBeUndefined();
		expect(
			accountPage("/console/organizations/bagge-farm/settings"),
		).toBeUndefined();
		expect(accountPage("/console/projects/hashbrown/keys")).toBeUndefined();
	});

	// Kills the shortcut read as a user named "apps" with no page, and that
	// user's own pages read as the shortcut.
	test("names Connected Apps for the shortcut that names no user", () => {
		expect(accountPage("/console/users/apps")?.page).toBe("ConnectedApps");
		expect(accountPage("/console/users/apps/apps")?.page).toBe("ConnectedApps");
		expect(accountPage("/console/users/apps/keys")?.page).toBe("UserKeys");
	});
});

describe("shortcutPage", () => {
	// Kills a shortcut sent to another account page, a user's own path or an
	// unknown one taken for a shortcut, and help, which stays classic.
	test("names the reader's page a shortcut opens", () => {
		expect(shortcutPage("/console/users")).toBe("settings");
		expect(shortcutPage("/console/users/settings")).toBe("settings");
		expect(shortcutPage("/console/users/keys")).toBe("keys");
		expect(shortcutPage("/console/users/keys/add")).toBe("keys");
		expect(shortcutPage("/console/users/tokens/")).toBe("tokens");
		expect(shortcutPage("/console/users/tokens/add")).toBe("tokens");
		expect(shortcutPage("/console/users/help")).toBeUndefined();
		expect(shortcutPage("/console/users/settings/add")).toBeUndefined();
		expect(shortcutPage("/console/users/keys/x")).toBeUndefined();
		expect(shortcutPage("/console/users/muriel-bagge/keys")).toBeUndefined();
		expect(shortcutPage("/console/organizations")).toBeUndefined();
	});
});
